# Task specs — phase 6 (desktop shell) — skeleton

Skeleton to be expanded at phase start against the then-current `apps/`
state. Standing constraints as in `phase-1.md`. Architecture
contracts: `../architecture.md` §Frontend integration (TauriCoreClient,
PlaybackEngine trait, WebKitGTK mitigations).

### T6.1 · Tauri shell + TauriCoreClient
`apps/desktop/`: Tauri 2 project wrapping `apps/ui`; `TauriCoreClient`
implementing the same `CoreClient` interface over `invoke`; native
`phx-engine` behind commands; file I/O in Rust commands with the dialog
plugin used only to obtain paths. Gate: the full web Playwright suite (via
tauri-driver/WebdriverIO) passes against the desktop build on Linux.

### T6.2 · native playback engine
`PlaybackEngine` trait in a new `crates/phx-playback`: cpal callback thread,
symphonia decode, atomic sample-counter clock, position events to the
webview at display rate; seek/loop over selections. Gate: cursor drift
< 1 frame over 5 minutes; no underruns on the 10-minute fixture.

### T6.3 · large-file path
Streamed decode + tile computation for recordings that exceed memory
comfort (the LongSound case): disk-backed sample cache, pyramid built
incrementally. Gate: a 1-hour 48 kHz file opens in < 5 s and scrolls
smoothly.

### T6.4 · packaging + platform integration
Bundles for Windows (msi), macOS (dmg, signed-later note), Linux (AppImage +
deb); file associations (`.wav`, `.TextGrid`, project files); WebKitGTK
slow-path detection wired to the Canvas2D fallback with the documented
`WEBKIT_DISABLE_DMABUF_RENDERER=1` advisory. Gate: install + open-with on
each OS.

### T6.5 · cross-OS CI matrix
Desktop build + e2e smoke on ubuntu/macos/windows runners; artifact upload
per platform.

### T6.6 · phase gate review
Same demo script as web on all three OSes; roadmap phase-6 gate.

Sequencing: T6.1 → {T6.2, T6.3, T6.4} parallel; T6.5 after T6.1; playback
(T6.2) owns thread-safety judgment.
