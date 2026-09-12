# TCP Latency v0.13.0 — 3D desktop cat

## Display

- Settings → 悬浮窗与启动 → 显示模式 → 3D 桌面小猫 enables a real WebGL 3D cat.
- An original articulated orange-and-cream cat has breathing/blinking/tail motion,
  traffic-driven walking, a lowered head for trouble, resting when paused, and a
  single recovery hop. Its head follows the pointer inside the pet window.
- A readable bubble displays the active target RTT and the selected local
  interface's upload/download rates. Target errors retain their precise labels.
- Compact / Standard / Large use 228×250 / 280×304 / 336×356 logical pixels.
- Drag the cat or bubble to move it; double-click or use the gear to open settings.
  The lock button enables whole-window mouse passthrough. Unlock from the menu bar.
- The desktop outside the bubble is transparent. HUD background preferences are
  retained for switching back. Disabling Network Cat returns to the numeric HUD.

## Resource use and resilience

- Three.js r180 is bundled locally and lazily loaded only in 3D mode. No runtime
  network requests or external character assets are used.
- Power saver defaults to a maximum of 15 FPS and a device-pixel ratio cap of 1.5;
  disabling it allows at most 30 FPS and a ratio cap of 2.
- Disabling animation or enabling macOS Reduce Motion renders a static pose.
- Hiding the floating window, leaving 3D mode, or hiding the document cancels the
  rendering loop and disposes geometry, materials, textures, and the GPU context.
- Missing WebGL2 or a lost graphics context falls back to a static illustration
  while preserving the live numbers; a retry button reinitializes the renderer.
- Two distinct TCP samples confirm pose changes. Traffic events never count as
  additional probes. Pause/stale/error labels update immediately. A target or
  endpoint change resets the animation history and cannot trigger false recovery.
- Stale, unavailable, or disabled traffic is excluded from the busy state.

## Configuration and validation

- UI schema 9 adds `floatingDisplayMode` (`hud` / `pet3d`) and
  `pet3dPowerSaver` (default true). Existing installations remain in HUD mode.
- Rust tests cover config migration and persistence; `node --test
  scripts/test-pet-state.mjs` covers state transitions and data validity.
- macOS native dragging, Spaces/fullscreen behavior and hardware energy use need
  manual checks on a real Mac; browser visual QA does not establish those results.
