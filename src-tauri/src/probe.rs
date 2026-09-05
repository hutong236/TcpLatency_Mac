use crate::config::{endpoint_key, TargetConfig};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::net::{lookup_host, TcpStream};

const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
const DNS_CACHE_MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeResult {
    pub(crate) latency_ms: Option<f64>,
    pub(crate) dns_ms: Option<f64>,
    pub(crate) resolved_address: Option<String>,
    pub(crate) attempted_addresses: Vec<String>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
}

#[derive(Clone)]
struct DnsCacheEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}

static DNS_CACHE: OnceLock<Mutex<HashMap<String, DnsCacheEntry>>> = OnceLock::new();

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
                    latency_ms: None,
                    dns_ms: None,
                    resolved_address: None,
                    attempted_addresses: vec![],
                    status: "dns_timeout".into(),
                    error: Some("DNS resolve timeout".into()),
                }
            }
            Ok(Err(err)) => {
                return ProbeResult {
                    latency_ms: None,
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

    let attempted_addresses: Vec<String> = addresses.iter().map(ToString::to_string).collect();
    if addresses.is_empty() {
        return ProbeResult {
            latency_ms: None,
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
        let remaining = total_timeout.saturating_sub(elapsed_total);
        let connect_started = Instant::now();
        match tokio::time::timeout(remaining, TcpStream::connect(addr)).await {
            Err(_) => {
                last_status = "timeout".into();
                last_error = Some(format!("TCP connect timeout: {addr}"));
                last_address = Some(addr.to_string());
            }
            Ok(Ok(stream)) => {
                let latency_ms = connect_started.elapsed().as_secs_f64() * 1000.0;
                drop(stream);
                return ProbeResult {
                    latency_ms: Some(latency_ms),
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
        latency_ms: None,
        dns_ms: Some(dns_ms),
        resolved_address: last_address,
        attempted_addresses,
        status: last_status,
        error: last_error,
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
}
