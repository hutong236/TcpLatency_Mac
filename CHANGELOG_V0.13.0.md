# TCP Latency v0.13.0 — Observatory

2026-10-08

## What's new

- New Observatory design system for the macOS settings window, with improved spacing, visual hierarchy, contrast and native light/dark support.
- A dedicated live health dashboard featuring improved metric cards and 60-second time-series visualization.
- Sticky section navigation, responsive settings layouts, and a persistent Save action.
- Stable settings markup for the traffic sampler and Network Cat; removes runtime injection of configuration controls.
- Keyboard-accessible target switching, accessible announcement of errors and save outcomes, and clearer unsaved changes feedback.
- More reliable asynchronous history updates when switching monitored targets.
- Smaller recurring HUD animation overhead while the pointer is not hovering.
- Automated UI contract validation as part of CI.

## Compatibility

- Retains existing monitoring configuration schema (uiVersion 8).
- Does not change TCP/DNS sampling semantics, alert thresholds, DNS fallback, or TCP RTT reporting.
- macOS 12+ with a universal Apple Silicon / Intel package from the existing release workflow.

## Validation

- GitHub CI: static checks and Rust tests.
- Native accessibility, motion, rendering and manual macOS regression still require hands-on verification.
- Released binaries are built using the repository's automated macOS workflow; they are ad-hoc signed and not notarized.
