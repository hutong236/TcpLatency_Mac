# TCP Latency v0.11.0 — Network Cat 2.0

## New features

- Added a dedicated Network Cat enable/disable switch in the settings page.
- Added a separate animation switch. Built-in themes become static when disabled; custom animated images are frozen to a snapshot.
- Added three built-in themes:
  - Default Cat — minimal silhouette style.
  - Pixel Cat — crisp 8-bit style.
  - Cyber Cat — line-art data-flow style.
- Added Custom theme support for SVG, PNG/APNG, WebP and GIF assets up to 8 MB.
- Added a file picker in settings. Selected assets are copied into the application config directory so they remain available after restart.
- Added custom asset validation and a Test Asset action.

## Network state visualization

Network Cat now fuses multiple live signals instead of reacting only to latency:

- TCP latency controls running cadence and slow/critical motion.
- Download + upload traffic controls packet density and Busy state.
- Jitter and failure percentage can trigger Unstable/Critical states.
- Timeout/offline/refused/DNS failures switch the cat into an offline/rest state.
- A compact health badge shows: Fast / Stable / Busy / Jitter / Slow / Critical / Offline / Idle using the short HUD labels 快 / 稳 / 忙 / 抖 / 慢 / 危 / 断 / 待.

## Compatibility and safety

- Existing TCP probe scheduling and traffic sampling architecture are unchanged.
- Existing Compact / Standard / Large HUD sizes remain supported.
- Respects macOS Reduce Motion.
- Custom assets are limited to supported image formats and 8 MB to prevent accidental oversized HUD resources.
- Custom files are copied using a sanitized file name into `TcpLatency/cat-assets`.

## Configuration migration

Configuration schema is upgraded to UI version 8 with these defaults:

- `networkCatEnabled: true`
- `networkCatAnimationEnabled: true`
- `networkCatTheme: "default"`
- `networkCatCustomAsset: ""`

Existing user configurations are migrated automatically.
