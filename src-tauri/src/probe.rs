use crate::config::{endpoint_key, TargetConfig};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, net::{lookup_host, TcpStream}};

const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
const DNS_CACHE_MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeResult {
    /// Effective probe latency shown by the HUD/statistics. This is measured
    /// from the beginning of the probe through DNS resolution and any address
    /// fallback attempts until a TCP connection succeeds.
    pub(crate) latency_ms: Option<f64>,
    /// Raw TCP connect RTT for the address that finally succeeded.
    pub(crate) tcp_ms: Option<f64>,
    pub(crate) dns_ms: Option<f64>,
    pub(crate) resolved_address: Option<String>,
    pub(crate) attempted_addresses: Vec<String>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    /// What was actually verified: TCP handshake, SSH banner, or HTTP response.
    pub(crate) probe_mode: String,
    pub(crate) response_detail: Option<String>,
    /// OS route for the observed peer; utun is a warning, not proof of interception.
    pub(crate) route_interface: Option<String>,
}

#[derive(Clone)]
struct DnsCacheEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}

static DNS_CACHE: OnceLock<Mutex<HashMap<String, DnsCacheEntry>>> = OnceLock::new();
const ROUTE_CACHE_TTL: Duration = Duration::from_secs(60);
static ROUTE_CACHE: OnceLock<Mutex<HashMap<IpAddr, (Instant, Option<String>)>>> = OnceLock::new();

fn dns_cache() -> &'static Mutex<HashMap<String, DnsCacheEntry>> {
    DNS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_addresses(key: &str, now: Instant) -> Option<Vec<SocketAddr>> {
    let mut cache = dns_cache().lock().ok()?;
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.addresses.clone())
}

fn cache_addresses(key: String, addresses: &[SocketAddr], now: Instant) {
    if addresses.is_empty() {
        return;
    }
    if let Ok(mut cache) = dns_cache().lock() {
        cache.retain(|_, entry| entry.expires_at > now);
        if cache.len() >= DNS_CACHE_MAX_ENTRIES && !cache.contains_key(&key) {
            if let Some(oldest_key) = cache.keys().next().cloned() {
                cache.remove(&oldest_key);
            }
        }
        cache.insert(
            key,
            DnsCacheEntry {
                addresses: addresses.to_vec(),
                expires_at: now + DNS_CACHE_TTL,
            },
        );
    }
}

fn invalidate_cached_addresses(key: &str) {
    if let Ok(mut cache) = dns_cache().lock() {
        cache.remove(key);
    }
}

fn normalize_addresses(mut addresses: Vec<SocketAddr>, family: &str) -> Vec<SocketAddr> {
    addresses.sort_by_key(|addr| if addr.is_ipv4() { 0 } else { 1 });
    addresses.dedup();
    addresses.retain(|addr| match family {
        "ipv4" => addr.is_ipv4(),
        "ipv6" => addr.is_ipv6(),
        _ => true,
    });
    addresses
}

/// Dispatch by required verification level. A TCP handshake is not application health.
pub(crate) async fn probe_target(target: &TargetConfig) -> ProbeResult {
    match target.probe_mode.as_str() {
        "http" | "https" => http_probe(target).await,
        _ => tcp_probe(target).await,
    }
}

pub(crate) async fn tcp_probe(target: &TargetConfig) -> ProbeResult {
    let total_timeout = Duration::from_millis(target.timeout_ms);
    let total_started = Instant::now();
    let cache_key = endpoint_key(target);

    let (addresses, dns_ms, cache_hit) = if let Some(addresses) = cached_addresses(&cache_key, Instant::now()) {
        (addresses, 0.0, true)
    } else {
        let dns_started = Instant::now();
        let lookup = tokio::time::timeout(
            total_timeout,
            lookup_host((target.host.as_str(), target.port)),
        )
        .await;

        let raw_addresses: Vec<SocketAddr> = match lookup {
            Err(_) => {
                return ProbeResult {
                    probe_mode: target.probe_mode.clone(),
                    response_detail: None,
                    route_interface: None,
                    latency_ms: None,
                    tcp_ms: None,
                    dns_ms: None,
                    resolved_address: None,
                    attempted_addresses: vec![],
                    status: "dns_timeout".into(),
                    error: Some("DNS resolve timeout".into()),
                }
            }
            Ok(Err(err)) => {
                return ProbeResult {
                    probe_mode: target.probe_mode.clone(),
                    response_detail: None,
                    route_interface: None,
                    latency_ms: None,
                    tcp_ms: None,
                    dns_ms: Some(dns_started.elapsed().as_secs_f64() * 1000.0),
                    resolved_address: None,
                    attempted_addresses: vec![],
                    status: "dns_error".into(),
                    error: Some(format!("DNS: {err}")),
                }
            }
            Ok(Ok(iter)) => iter.collect(),
        };

        let dns_ms = dns_started.elapsed().as_secs_f64() * 1000.0;
        let addresses = normalize_addresses(raw_addresses, &target.address_family);
        cache_addresses(cache_key.clone(), &addresses, Instant::now());
        (addresses, dns_ms, false)
    };

    let mut attempted_addresses: Vec<String> = Vec::new();
    if addresses.is_empty() {
        return ProbeResult {
                    probe_mode: target.probe_mode.clone(),
                    response_detail: None,
                    route_interface: None,
            latency_ms: None,
            tcp_ms: None,
            dns_ms: Some(dns_ms),
            resolved_address: None,
            attempted_addresses,
            status: "dns_error".into(),
            error: Some(format!("DNS 未返回 {} 可用地址", target.address_family)),
        };
    }

    let mut last_status = "offline".to_string();
    let mut last_error = Some("TCP connect failed".to_string());
    let mut last_address = None;

    for addr in addresses {
        let elapsed_total = total_started.elapsed();
        if elapsed_total >= total_timeout {
            last_status = "timeout".into();
            last_error = Some("TCP connect timeout".into());
            break;
        }
        attempted_addresses.push(addr.to_string());
        let remaining = total_timeout.saturating_sub(elapsed_total);
        let connect_started = Instant::now();
        match tokio::time::timeout(remaining, TcpStream::connect(addr)).await {
            Err(_) => {
                last_status = "timeout".into();
                last_error = Some(format!("TCP connect timeout: {addr}"));
                last_address = Some(addr.to_string());
            }
            Ok(Ok(mut stream)) => {
                let tcp_ms = connect_started.elapsed().as_secs_f64() * 1000.0;
                let mut response_detail = None;
                if target.probe_mode == "ssh" {
                    let remaining = total_timeout.saturating_sub(total_started.elapsed());
                    let verification = tokio::time::timeout(remaining, verify_ssh_banner(&mut stream)).await;
                    match verification {
                        Ok(Ok(banner)) => response_detail = Some(banner),
                        Ok(Err(reason)) => {
                            return ProbeResult {
                                probe_mode: target.probe_mode.clone(),
                                response_detail: None,
                                route_interface: route_interface_for(addr).await,
                                latency_ms: None,
                                tcp_ms: Some(tcp_ms),
                                dns_ms: Some(dns_ms),
                                resolved_address: Some(addr.to_string()),
                                attempted_addresses,
                                status: "protocol_error".into(),
                                error: Some(reason),
                            };
                        }
                        Err(_) => {
                            return ProbeResult {
                                probe_mode: target.probe_mode.clone(),
                                response_detail: None,
                                route_interface: route_interface_for(addr).await,
                                latency_ms: None,
                                tcp_ms: Some(tcp_ms),
                                dns_ms: Some(dns_ms),
                                resolved_address: Some(addr.to_string()),
                                attempted_addresses,
                                status: "timeout".into(),
                                error: Some("TCP 已建连，但等待 SSH 协议标识超时".into()),
                            };
                        }
                    }
                }
                // Stop timing before collecting optional diagnostics.
                let latency_ms = total_started.elapsed().as_secs_f64() * 1000.0;
                drop(stream);
                let route_interface = route_interface_for(addr).await;
                return ProbeResult {
                    probe_mode: target.probe_mode.clone(),
                    response_detail,
                    route_interface,
                    latency_ms: Some(latency_ms),
                    tcp_ms: Some(tcp_ms),
                    dns_ms: Some(dns_ms),
                    resolved_address: Some(addr.to_string()),
                    attempted_addresses,
                    status: "ok".into(),
                    error: None,
                };
            }
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
                last_status = "refused".into();
                last_error = Some(format!("TCP connection refused: {addr}"));
                last_address = Some(addr.to_string());
            }
            Ok(Err(err)) => {
                last_status = "offline".into();
                last_error = Some(format!("{addr}: {err}"));
                last_address = Some(addr.to_string());
            }
        }
    }

    // A cached route that no longer connects should be re-resolved on the next
    // probe rather than waiting for the full TTL. Connection refused is kept
    // because the address is valid and the service itself is answering.
    if cache_hit && last_status != "refused" {
        invalidate_cached_addresses(&cache_key);
    }

    ProbeResult {
        probe_mode: target.probe_mode.clone(),
        response_detail: None,
        route_interface: None,
        latency_ms: None,
        tcp_ms: None,
        dns_ms: Some(dns_ms),
        resolved_address: last_address,
        attempted_addresses,
        status: last_status,
        error: last_error,
    }
}


async fn verify_ssh_banner(stream: &mut TcpStream) -> Result<String, String> {
    // RFC 4253 allows informational lines before the SSH identification string.
    // Never count a bare TCP SYN/ACK as successful SSH verification.
    let mut collected = Vec::with_capacity(256);
    let mut buf = [0u8; 512];
    loop {
        let size = stream.read(&mut buf).await
            .map_err(|err| format!("读取 SSH 标识失败: {err}"))?;
        if size == 0 {
            return Err("TCP 已连接，但服务未发送 SSH 协议标识".into());
        }
        collected.extend_from_slice(&buf[..size]);
        if collected.len() > 8192 {
            return Err("SSH 服务标识超过安全读取上限".into());
        }
        while let Some(pos) = collected.iter().position(|byte| *byte == b'\n') {
            let line = collected.drain(..=pos).collect::<Vec<_>>();
            let text = String::from_utf8_lossy(&line);
            let banner = text.trim_end_matches(['\r', '\n']);
            if banner.starts_with("SSH-2.0-") || banner.starts_with("SSH-1.99-") {
                if banner.len() > 255 {
                    return Err("SSH 协议标识超过 255 字节".into());
                }
                return Ok(banner.to_string());
            }
        }
    }
}

async fn http_probe(target: &TargetConfig) -> ProbeResult {
    let mode = target.probe_mode.clone();
    let scheme = if mode == "https" { "https" } else { "http" };
    let host = if target.host.contains(':') && !target.host.starts_with('[') {
        format!("[{}]", target.host)
    } else {
        target.host.clone()
    };
    let url = format!("{scheme}://{host}:{}{}", target.port, target.http_path);
    let base = |status: &str, error: Option<String>| ProbeResult {
        latency_ms: None,
        tcp_ms: None,
        // Reqwest performs DNS inside the request; claiming a separate DNS sample
        // would misrepresent its actual internal timing.
        dns_ms: None,
        resolved_address: None,
        attempted_addresses: Vec::new(),
        status: status.into(),
        error,
        probe_mode: mode.clone(),
        response_detail: None,
        route_interface: None,
    };
    let url = match reqwest::Url::parse(&url) {
        Ok(value) => value,
        Err(err) => return base("invalid_target", Some(format!("HTTP 目标地址无效: {err}"))),
    };
    let timeout = Duration::from_millis(target.timeout_ms);
    // Forcing an address family must work in HTTP(S) too. Reqwest's default
    // resolver otherwise ignores the per-target ipv4/ipv6 preference.
    let mut explicit_addresses: Option<Vec<SocketAddr>> = None;
    let mut dns_cost_ms = None;
    if target.address_family != "auto" {
        let key = endpoint_key(target);
        let addresses = if let Some(cached) = cached_addresses(&key, Instant::now()) {
            cached
        } else {
            let dns_started = Instant::now();
            let lookup = tokio::time::timeout(timeout, lookup_host((target.host.as_str(), target.port))).await;
            let resolved: Vec<SocketAddr> = match lookup {
                Err(_) => return base("dns_timeout", Some("DNS 解析超时".into())),
                Ok(Err(err)) => return base("dns_error", Some(format!("DNS 解析失败: {err}"))),
                Ok(Ok(iter)) => iter.collect(),
            };
            dns_cost_ms = Some(dns_started.elapsed().as_secs_f64() * 1000.0);
            let filtered = normalize_addresses(resolved, &target.address_family);
            cache_addresses(key, &filtered, Instant::now());
            filtered
        };
        if addresses.is_empty() {
            return base("dns_error", Some(format!("DNS 未返回 {} 地址", target.address_family)));
        }
        explicit_addresses = Some(addresses);
    }
    let remaining = timeout.saturating_sub(Duration::from_secs_f64(dns_cost_ms.unwrap_or(0.0) / 1000.0));
    if remaining.is_zero() {
        return base("timeout", Some("DNS 已耗尽探测超时预算".into()));
    }
    // A fresh client forces each sample to make a real request. Avoid environment
    // HTTP proxies; OS-level TUN/VPN routing may still transparently intercept.
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .timeout(remaining)
        .connect_timeout(remaining);
    if let Some(addresses) = &explicit_addresses {
        builder = builder.resolve_to_addrs(&target.host, addresses);
    }
    let client = match builder.build()
    {
        Ok(value) => value,
        Err(err) => return base("protocol_error", Some(format!("创建 HTTP 客户端失败: {err}"))),
    };
    // Measure the actual request, not local TLS client construction overhead.
    let start = Instant::now();
    let response = match client.get(url).send().await {
        Ok(value) => value,
        Err(err) => {
            let status = if err.is_timeout() { "timeout" }
                else if err.is_connect() { "offline" }
                else { "protocol_error" };
            if err.is_connect() && explicit_addresses.is_some() {
                invalidate_cached_addresses(&endpoint_key(target));
            }
            return base(status, Some(format!("{} 请求失败: {err}", mode.to_uppercase())));
        }
    };
    // Headers received: this is HTTP response-header latency, NOT full-body time.
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0 + dns_cost_ms.unwrap_or(0.0);
    let status_code = response.status();
    let peer = response.remote_addr();
    let route_interface = match peer {
        Some(addr) => route_interface_for(addr).await,
        None => None,
    };
    let valid = status_code.is_success();
    ProbeResult {
        latency_ms: if valid { Some(elapsed_ms) } else { None },
        tcp_ms: None,
        dns_ms: dns_cost_ms,
        resolved_address: peer.map(|addr| addr.to_string()),
        attempted_addresses: Vec::new(),
        status: if valid { "ok" } else { "http_error" }.into(),
        error: if valid { None } else {
            Some(format!("HTTP {status_code}：已收到服务响应，但不满足 2xx 健康检查标准"))
        },
        probe_mode: mode,
        response_detail: Some(format!("HTTP {status_code} · 响应头耗时 {elapsed_ms:.1} ms")),
        route_interface,
    }
}

async fn route_interface_for(peer: SocketAddr) -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = peer;
        None
    }
    #[cfg(target_os = "macos")]
    {
        let cache = ROUTE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let ip = peer.ip();
        if let Ok(entries) = cache.lock() {
            if let Some((at, interface)) = entries.get(&ip) {
                if at.elapsed() < ROUTE_CACHE_TTL {
                    return interface.clone();
                }
            }
        }
        // Diagnostic only, bounded, does not affect recorded network latency.
        let output = tokio::time::timeout(
            Duration::from_millis(350),
            tokio::process::Command::new("/sbin/route")
                .args(["-n", "get", &ip.to_string()])
                .kill_on_drop(true)
                .output(),
        ).await;
        let interface = match output {
            Ok(Ok(result)) if result.status.success() => {
                String::from_utf8_lossy(&result.stdout)
                    .lines()
                    .find_map(|line| line.trim().strip_prefix("interface:"))
                    .map(|name| name.trim().to_string())
            }
            _ => None,
        };
        if let Ok(mut entries) = cache.lock() {
            if entries.len() >= 64 { entries.clear(); }
            entries.insert(ip, (Instant::now(), interface.clone()));
        }
        interface
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_family_filtering_is_stable() {
        let addresses = vec![
            "127.0.0.1:443".parse().unwrap(),
            "[::1]:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap(),
        ];
        let ipv4 = normalize_addresses(addresses.clone(), "ipv4");
        let ipv6 = normalize_addresses(addresses, "ipv6");
        assert_eq!(ipv4.len(), 1);
        assert!(ipv4[0].is_ipv4());
        assert_eq!(ipv6.len(), 1);
        assert!(ipv6[0].is_ipv6());
    }
    #[tokio::test]
    async fn ssh_probe_rejects_bare_tcp_without_ssh_banner() {
        use tokio::{io::AsyncWriteExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut target = TargetConfig::default();
        target.host = "127.0.0.1".into();
        target.port = listener.local_addr().unwrap().port();
        target.probe_mode = "ssh".into();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        });
        let result = probe_target(&target).await;
        assert_eq!(result.status, "protocol_error");
        assert!(result.latency_ms.is_none());
        assert!(result.tcp_ms.is_some());
    }

    #[tokio::test]
    async fn ssh_probe_accepts_valid_banner() {
        use tokio::{io::AsyncWriteExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut target = TargetConfig::default();
        target.host = "127.0.0.1".into();
        target.port = listener.local_addr().unwrap().port();
        target.probe_mode = "ssh".into();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"SSH-2.0-test_server\r\n").await.unwrap();
        });
        let result = probe_target(&target).await;
        assert_eq!(result.status, "ok");
        assert_eq!(result.response_detail.as_deref(), Some("SSH-2.0-test_server"));
        assert!(result.latency_ms.is_some());
    }


    #[tokio::test]
    async fn http_ipv6_only_rejects_ipv4_address() {
        let mut target = TargetConfig::default();
        target.host = "127.0.0.1".into();
        target.port = 43995;
        target.address_family = "ipv6".into();
        target.probe_mode = "http".into();
        let result = probe_target(&target).await;
        assert_eq!(result.status, "dns_error");
        assert!(result.latency_ms.is_none());
    }

    #[tokio::test]
    async fn http_probe_accepts_204_health_response() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut target = TargetConfig::default();
        target.host = "127.0.0.1".into();
        target.port = listener.local_addr().unwrap().port();
        target.probe_mode = "http".into();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = socket.read(&mut request).await;
            socket.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
        });
        let result = probe_target(&target).await;
        assert_eq!(result.status, "ok");
        assert!(result.latency_ms.is_some());
        assert!(result.response_detail.unwrap().contains("204"));
    }

    #[tokio::test]
    async fn http_probe_rejects_server_error() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut target = TargetConfig::default();
        target.host = "127.0.0.1".into();
        target.port = listener.local_addr().unwrap().port();
        target.probe_mode = "http".into();
        target.http_path = "/health".into();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = socket.read(&mut request).await;
            socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
        });
        let result = probe_target(&target).await;
        assert_eq!(result.status, "http_error");
        assert!(result.latency_ms.is_none());
        assert!(result.response_detail.unwrap().contains("503"));
    }

}
