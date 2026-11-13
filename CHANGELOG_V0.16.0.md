# TCP Latency v0.16.0 — Focused Settings Redesign

2026-11-13

## Highlights

- Introduces three top-level settings views: **实时监测**, **目标管理**, and **偏好设置**, replacing the long single-page stack of configuration cards.
- Prioritizes current latency, 60-second average, P95 and failure percentage in the live overview, with the remaining low-frequency metrics available through **更多指标**.
- Reduces the target overview table to target, address/protocol, current measurement and status, while preserving one-click active-target selection.
- Uses progressive disclosure for diagnostics, per-target advanced probe parameters, network-cat artwork, background display options, battery policies, notifications, thresholds, hourly history and incident events.
- Adds arrow-key accessible tab navigation, reorganized responsive layout and clearer visual hierarchy. Existing configuration keys, diagnostic controls and CSV exports remain available.

## Compatibility and probe semantics

- This release changes the settings presentation, not the network probe engine or configuration format.
- TCP connect time remains distinct from ICMP echo RTT. SSH verifies protocol identification; HTTP(S) verifies response headers and a 2xx result; HTTPS validates TLS certificates.
- The macOS Universal release is ad-hoc signed; it is not notarized by Apple.

## Validation

- GitHub CI checks static UI contracts and Rust tests on macOS.
- Real-world network paths (including VPN/TUN interception), native window behavior and visual accessibility remain recommended manual checks.
