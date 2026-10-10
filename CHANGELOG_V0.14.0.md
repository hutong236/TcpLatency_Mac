# TCP Latency v0.14.0 — Verified Network Probes

2026-10-10

## Changes

- Introduces opt-in TCP, SSH, HTTP and HTTPS verification modes per target. Existing targets remain in TCP mode for backward compatibility.
- Differentiates TCP handshake reachability from application availability. SSH verifies the protocol identification banner; HTTP(S) requires a 2xx response, and HTTPS verifies TLS certificates.
- HTTP(S) reports response-header completion time, not a fabricated TCP RTT; supports per-target IPv4/IPv6 resolution filtering.
- Displays verification mode and evidence in settings, HUD and menu bar. macOS route-interface detection flags `utun` as a possible TUN/VPN proxy path, without claiming interception is proven.
- Includes protocol mode and HTTP request path in endpoint identity, so switching verification methods resets historical aggregates.
- Classifies HTTP status errors, protocol failures and timeouts separately; alerts identify the failing verification mode.
- Repairs address-attempt diagnostics and adds regression tests for wrong SSH banners, SSH success, HTTP 204/503 and address-family filtering.

## Interpretation

- TCP Connected means only that the TCP handshake succeeded; it does not establish end-to-end service health.
- SSH Verified means the peer emitted an SSH identification string, not that identity/authentication succeeded.
- HTTP(S) Verified requires HTTP 2xx response headers (no redirects). Other status codes are health-check failures by design.
- HTTPS includes validated TLS. HTTP(S) uses fresh connections without honoring environment proxy variables; operating-system TUN routing may still intercept traffic.
- The current version does not implement ICMP ping. Reported RTT-like times are not interchangeable with ICMP echo RTT.

## Compatibility and validation

- Existing targets continue using TCP, default HTTP path is `/`; saves are backwards-compatible via serde defaults.
- GitHub macOS CI runs static checks and Rust tests. Real-network TUN/VPN behavior should still be verified on a Mac.
- Universal macOS app releases are built through GitHub Actions and ad-hoc signed, not notarized.
