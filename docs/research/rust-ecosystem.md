# Rust ecosystem survey for a cross-platform phonetics toolkit

Survey of crates, tooling, and prior art for a phonetics analysis toolkit with a Rust core, a Tauri 2 desktop shell (Windows/Linux/macOS), and a WASM web build. Licensing constraint throughout: all recommended dependencies must be MIT and/or Apache-2.0 compatible; copyleft findings are flagged explicitly. Survey date: July 2026.

## 1. Audio I/O and decoding

### symphonia — decoding

Pure-Rust demuxing and decoding: WAV, FLAC, Vorbis/OGG, MP3, AAC-LC, ALAC, MP1/MP2, ADPCM; containers AIFF, CAF, ISO-MP4, MKV. v0.6.0 (May 2026), used by ~600 crates, ~1M downloads/month, MSRV 1.85 ([crates.io](https://crates.io/crates/symphonia), [GitHub](https://github.com/pdeljanov/Symphonia)).

- **License: MPL-2.0** for the whole project, including the MP3 and AAC decoder crates ([symphonia-bundle-mp3 Cargo.toml](https://github.com/pdeljanov/Symphonia/blob/master/symphonia-bundle-mp3/Cargo.toml)). MPL-2.0 is file-level weak copyleft: depending on an unmodified copy imposes no obligations on MIT/Apache code; only modifying symphonia's own source files triggers share-alike on those files ([Mozilla MPL 2.0 FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)). Safe as an unmodified dependency.
- MP3/AAC/MPA/ALAC decoders are behind non-default feature flags ("only royalty-free open standard codecs and formats are enabled by default"). MP3 patents expired worldwide in 2017; AAC patent pools remain active in some jurisdictions — enable `aac` only after independent confirmation.
- **WASM: unverified by upstream.** No declared wasm32 target support; a minimal pure-Rust build should compile, but confirm empirically before relying on it.

### hound — WAV read/write

Apache-2.0, v3.5.1, used by ~1,000 crates, ~1.3M downloads/month ([crates.io](https://crates.io/crates/hound), [GitHub](https://github.com/ruuda/hound)). No native or OS dependencies, so it compiles to wasm32 without friction. Stable and unchanging since 2023 — for a WAV-only format this is a finished library, not abandonment.

### cpal — playback/recording

Apache-2.0, v0.18.1 (June 2026), used by ~1,400 crates ([GitHub](https://github.com/RustAudio/cpal)). The standard low-level cross-platform audio I/O crate (WASAPI, CoreAudio, ALSA/JACK).

- **WASM:** two wasm32 backends. The Web Audio backend (behind the `wasm-bindgen` feature) works on stable Rust. The `audioworklet` backend has lower latency but requires nightly Rust with `-Zbuild-std` (atomics) plus COOP/COEP headers for `SharedArrayBuffer`, and is still maturing ([cpal WASM wiki](https://github.com/RustAudio/cpal/wiki/Setting-up-a-new-CPAL-WASM-project)).

### rodio — high-level playback

Dual MIT/Apache-2.0, actively maintained, decodes via symphonia by default ([GitHub](https://github.com/RustAudio/rodio)). Its value is mixing, queueing, and effects — mostly unneeded for an analysis tool. No documented WASM support beyond what cpal provides.

### rubato — resampling

Dual MIT/Apache-2.0, v4.0.0 (July 2026), used by ~400 crates, ~1M downloads/month ([crates.io](https://crates.io/crates/rubato)). Pure Rust, no OS dependencies; should compile to wasm32 (no explicit upstream claim — verify with a `cargo build --target wasm32-unknown-unknown` smoke test).

### Recommendation

- `hound` for WAV I/O on all targets.
- `symphonia` for broad-format decoding on desktop; keep `mp3`/`aac` features opt-in and note the MPL-2.0 dependency (acceptable unmodified). Verify its wasm build before including it in the web bundle; a WAV/FLAC-only web build via `hound` + `symphonia-bundle-flac` is a reasonable fallback.
- `cpal` for native playback/recording. For the web build, prefer driving the browser's Web Audio API directly from the JS/WASM boundary; avoid cpal's nightly-only `audioworklet` backend for now.
- `rubato` for resampling everywhere.
- Skip `rodio`; its playback abstractions add nothing for this use case.

## 2. DSP and pitch/formant analysis

### Foundations

- **rustfft** — dual MIT/Apache-2.0, v6.4.1, ~2M downloads/month; the dominant pure-Rust FFT ([lib.rs](https://lib.rs/crates/rustfft)). WASM SIMD via the opt-in `wasm_simd` feature; WASM has no runtime feature detection, so SIMD is a compile-time decision per deployment target.
- **realfft** — MIT, v3.5.0, wraps rustfft for real-input FFTs (~2x speedup on real audio) ([lib.rs](https://lib.rs/crates/realfft)). Use it for all spectral work on real signals.
- **ndarray** vs **nalgebra** — both permissive (MIT/Apache-2.0 and Apache-2.0 respectively), both maintained. ndarray (numpy-style n-dim arrays, slicing, windowed views) fits framed/multi-channel signal processing; nalgebra fits fixed-size linear algebra and decompositions ([forum discussion](https://users.rust-lang.org/t/ndarray-vs-nalgebra-which-is-best/88699)). Existing speech crates (e.g. pyin-rs) build on ndarray. Choose **ndarray** as the array foundation; pull nalgebra only if a specific decomposition needs it.

### Existing pitch/LPC/formant crates

| Crate | Scope | Status | License |
|---|---|---|---|
| [pitch-detection](https://crates.io/crates/pitch-detection) | Autocorrelation, McLeod (NSDF), YIN; WASM-oriented | Stale (v0.3.0, June 2022) | MIT/Apache-2.0 |
| [pitch-detector](https://lib.rs/crates/pitch-detector) | FFT-based, cepstrum | Stale (v0.3.1, June 2022) | MIT |
| [pyin-rs](https://github.com/Sytronik/pyin-rs) | pYIN (librosa port), per-frame F0 + voicing probability | Small, working; v1.2.0 (July 2024) | MIT |
| [loqa-voice-dsp](https://lib.rs/crates/loqa-voice-dsp) | YIN/pYIN, LPC formants, HNR, H1-H2 | Active (Dec 2025) but built for one product; API generality unclear | MIT |
| [linear-predictive-coding](https://lib.rs/crates/linear-predictive-coding) | LPC coefficients (Burg and autocorrelation) | Maintained (v0.4.0, Feb 2025); no formant picking | MIT/Apache-2.0 |
| [vox_box](https://lib.rs/crates/vox_box) | LPC, MFCC, Boersma pitch, McCandless formant tracking | Dead (Dec 2017, Rust 2015 edition) | MIT |

### aubio — flagged, avoid

aubio (C) is **GPL-3.0-or-later** ([aubio COPYING](https://github.com/aubio/aubio)), last released 2019. The `aubio-rs` binding is itself licensed **GPL-3.0** ([lib.rs](https://lib.rs/crates/aubio-rs)) because it links (or vendors and compiles) the GPL C source; any binary or `.wasm` bundle linking it becomes a GPL-3.0 combined work when distributed. Shipping WASM to a browser is distribution — copyleft applies identically to the web build. Incompatible with this project's licensing constraint.

### Recommendation

No maintained, permissively licensed crate covers pitch + LPC + formants at Praat quality. Write small in-house crates on top of `realfft`/`rustfft` + `ndarray`:

- Pitch: YIN/pYIN (de Cheveigné & Kawahara) and McLeod NSDF — each is a few hundred lines from published papers. `pyin-rs` and `loqa-voice-dsp` are useful algorithm references.
- LPC + formants: Burg LPC (optionally via `linear-predictive-coding`, or an in-house Levinson-Durbin/Burg), then polynomial root-finding and frequency/bandwidth filtering for formant candidates, with McCandless-style continuity tracking (`vox_box` is prior art for the picking step).

This keeps the license chain clean, avoids abandoned dependencies, and produces atomic crates reusable outside the app. See also `praatfan-core-rs` in §3: it validates against Praat bit-for-bit, but its GPL-3.0 license means it is reference reading only (algorithms and test methodology, never code copying).

## 3. Prior art and naming

### Existing Rust phonetics projects

| Project | Scope | Activity | License |
|---|---|---|---|
| [sadda-speech/sadda](https://github.com/sadda-speech/sadda) | Phonetics/speech-science toolkit: DSP engine, clinical measures (jitter, shimmer, HNR, CPP, AVQI), PyO3 bindings, egui/wgpu GUI; "Praat-validated" | Started May 2026, pushed July 2026, 2 stars | Apache-2.0 |
| [ucpresearch/praatfan-core-rs](https://github.com/ucpresearch/praatfan-core-rs) | Reimplementation of Praat's pitch/LPC-formant/intensity/harmonicity algorithms, bit-accuracy claims vs parselmouth; native + PyO3 + WASM | Active, v0.1.9 (June 2026), 2 stars | **GPL-3.0** (derived from Praat's GPL algorithms) |
| [mosaic-rs/MOSAIC](https://github.com/mosaic-rs/MOSAIC) | Articulatory/motor-speech analysis | Active, 3 stars | **GPL-3.0** |
| [dasp-rs/dasp-rs](https://github.com/dasp-rs/dasp-rs) | General DSP/speech/music analysis | Active, 8 stars | MIT |

Component-level building blocks: spectrogram crates ([Spectrograms](https://github.com/jmg049/Spectrograms), [spectrs](https://github.com/giacomopiccinini/spectrs), [mel-spec](https://github.com/wavey-ai/mel-spec), [resonators](https://github.com/jhartquist/resonators)), TextGrid I/O crates ([textgrid](https://github.com/amirhosseinghanipour/textgrid), [textgridde-rs](https://github.com/ccodaze/textgridde-rs), [gridio](https://github.com/fncokg/gridio)). Nothing has meaningful adoption (all under ~100 stars); the space is open, but `sadda` and `praatfan-core-rs` overlap enough in intent to warrant a feature comparison before finalizing scope.

A caution on the `praatfan-core-rs` precedent: it treats reimplementing Praat's algorithms as GPL-derivative work. Implementing pitch/LPC/formant estimation from the published literature (papers, textbooks) rather than from Praat's source keeps an MIT/Apache implementation defensible; do not port Praat source code.

### Name availability (crates.io, checked 2026-07-16)

- `phonix` — **taken** (June 2026): an unrelated AGPL-3.0 wake-word detection crate ([crates.io](https://crates.io/crates/phonix)).
- `phonics` — **taken** (2020): dormant phonetic-spelling-algorithms crate, MIT ([crates.io](https://crates.io/crates/phonics)).
- `phonix-dsp`, `phonix-core`, `phx-core`, `phx-dsp` — all **available**.

crates.io namespace ownership ([RFC 3243](https://rust-lang.github.io/rfcs/3243-packages-as-optional-namespaces.html)) is still unimplemented as of July 2026 ([project goals](https://rust-lang.github.io/rust-project-goals/2026/open-namespaces.html)); nothing registry-side protects a prefix, so publish placeholder crates for core names once chosen. Trusted Publishing (OIDC from GitHub Actions, no long-lived tokens) is available and recommended for the release pipeline ([crates.io update](https://blog.rust-lang.org/2026/01/21/crates-io-development-update/)).

### Recommendation

Keep "Phonix" as the product name; publish crates under a prefix that avoids the occupied bare name. Two workable schemes:

1. `phonix-*` sub-crates (`phonix-dsp`, `phonix-pitch`, `phonix-formant`, …) — recognizable, but the bare `phonix` crate belongs to someone else, which will confuse crates.io search.
2. `phx-*` (`phx-core`, `phx-dsp`, …) — short, collision-free, verified available.

Prefer option 2 for published crates; register `phx-core` early.

## 4. Tauri 2

### Stability

Tauri 2.0 went stable in October 2024 ([announcement](https://v2.tauri.app/blog/tauri-20/)); current stable is v2.11.5 (July 2026), with point releases limited to dependency and bugfix work ([releases](https://github.com/tauri-apps/tauri/releases), [release page](https://v2.tauri.app/release/)). MSRV is 1.85 (edition 2024).

### Plugins: fs and dialog

`tauri-plugin-fs` 2.5.1 and `tauri-plugin-dialog` 2.7.1 (both May 2026, ~1.6M downloads/month each) are mature and widely used ([plugins-workspace](https://github.com/tauri-apps/plugins-workspace)). Known friction:

- The capability/permission system trips users: `fs:allow-*` without an explicit path scope yields "forbidden path" ([tauri-docs#3536](https://github.com/tauri-apps/tauri-docs/issues/3536), [discussion #11792](https://github.com/orgs/tauri-apps/discussions/11792)).
- A reported bug where a `[` in a file path (reachable via drag-and-drop or the file picker) corrupts the fs plugin's persisted scope ([tauri#11708](https://github.com/tauri-apps/tauri/issues/11708); current status unverified).

Rust backend commands bypass the frontend permission scopes entirely. For an app that reads/writes arbitrary audio files, do file I/O in own Rust commands; use the dialog plugin only to obtain paths.

### Webview rendering on Linux — the main risk

Tauri's own docs dedicate a page to [Linux graphics issues](https://v2.tauri.app/develop/debug/linux-graphics/): WebKitGTK (especially with NVIDIA drivers) produces blank windows, DMABUF failures, and canvas/WebGL content silently landing on a slow software path with no error thrown. Field evidence: canvas at ~5 FPS on Ubuntu vs 17-30 FPS in Chromium for identical code, closed as upstream ([tauri#5761](https://github.com/tauri-apps/tauri/issues/5761)); the airi project hit blank WebGL/canvas on Linux/NVIDIA, could not fix it, and migrated its desktop app to Electron in October 2025 ([issue](https://github.com/moeru-ai/airi/issues/263), [devlog](https://airi.moeru.ai/docs/en/blog/DevLog-2025.10.20/)). The Tauri team's long-term answer is alternative Linux webviews (CEF bundling, Servo/Verso — [security/future](https://v2.tauri.app/security/future/), [Verso integration](https://v2.tauri.app/blog/tauri-verso-integration/)), neither stable yet. Windows (WebView2) and macOS (WKWebView) show no comparable complaints.

Mitigations to build in: detect the slow path by measuring achieved frame time (no error is raised), ship a Canvas2D fallback for the tile blit, and document `WEBKIT_DISABLE_DMABUF_RENDERER=1` for affected Linux users.

### Audio playback strategy

Surveyed Tauri audio apps split by precision needs. Backend-cpal camp: a 2026 DAW build series runs a dedicated cpal callback thread, commands over channels from IPC handlers, playback position as an `AtomicU64` sample counter incremented in the audio callback and pushed to the frontend at display rate ([writeup](https://whoisryosuke.com/blog/2026/creating-a-daw-in-rust/)); Claket (soundboard) uses rodio+cpal in the backend with the frontend sending only triggers ([repo](https://github.com/aera128/claket-tauri)). Frontend-WebAudio camp: Musicat plays via `<audio>`/WebAudio in the webview and its author reports WebAudio "particularly buggy on Safari" — pops, clicks, wrong-speed playback, which applies to WKWebView ([writeup](https://slavbasharov.com/blog/building-music-player-tauri-svelte)). airi's WebAudio-in-webview pain contributed to its Electron migration.

### Recommendation

Play back from the Rust backend via cpal on desktop: symphonia decode → cpal output, atomic sample-counter clock in the audio callback, position events emitted to the webview at display rate. This gives a sample-accurate cursor independent of three divergent webview audio stacks and sidesteps the WKWebView WebAudio bugs. On the web build, WebAudio is the only output; use it purely as transport with `AudioContext.currentTime`-derived position. Isolate both behind a small `PlaybackEngine` trait (play/pause/seek/position) with two implementations. (This hybrid is inferred from the observed tradeoffs; no surveyed project runs exactly this split.)

## 5. WASM for DSP

### Toolchain

wasm-bindgen and wasm-pack were transferred from the sunset rustwasm org to a dedicated `wasm-bindgen` org in July 2025 with new maintainers ([Rust blog](https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org/)); both show active 2026 releases (wasm-bindgen 0.2.126, June 2026; wasm-pack 0.15.0, May 2026). They remain the standard toolchain.

### SIMD

`simd128` works on stable Rust via `RUSTFLAGS="-C target-feature=+simd128"`, with `std::arch::wasm32` intrinsics stable; guard code with `#[cfg(target_feature = "simd128")]` and keep scalar fallbacks ([rustc platform docs](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html)). Browser support is universal at baseline (Chrome 91+, Firefox 89+, Safari 16.4+). Note WASM has no runtime feature detection — SIMD is a per-artifact compile-time decision.

### Threads

`wasm-bindgen-rayon` ([repo](https://github.com/RReverser/wasm-bindgen-rayon), v1.3.0) works but still requires nightly Rust (`-Z build-std` with `+atomics,+bulk-memory,+mutable-globals`) and cross-origin isolation (COOP/COEP headers) for `SharedArrayBuffer` ([web.dev](https://web.dev/articles/coop-coep)) — a toolchain plus deployment commitment, with every cross-origin subresource then needing CORP headers. Production-usable in 2026 but not a stable flag.

### Performance vs native

No clean FFT-specific benchmark exists; synthesized band: well-optimized WASM+SIMD lands roughly 1.5-2.5x slower than native. Data points: KISS FFT in WASM ~7-8x faster than fft.js ([benchmark](https://toughengineer.github.io/demo/dsp/fft-perf/)); Wasmtime 2.41x slower than native on a crypto workload, ~1.3-1.5x with wide-arithmetic ([2026 runtime benchmarks](https://00f.net/2026/06/23/webassembly-runtimes-2026/)); WASM SIMD is capped at 128-bit lanes vs AVX2's 256-bit natively. For phonetics workloads (spectrograms and pitch/formant tracks over utterance-length 16-48 kHz audio), that budget is interactive; the desktop build runs the same core at full native speed anyway.

### AudioWorklet integration

The `AudioWorkletProcessor` global scope lacks globals that wasm-bindgen's `--target web` glue expects ([wasm-bindgen#2367](https://github.com/wasm-bindgen/wasm-bindgen/issues/2367)); established workarounds are (a) postMessage the `.wasm` bytes into the worklet and instantiate there ([pattern writeup](https://whoisryosuke.com/blog/2025/processing-web-audio-with-rust-and-wasm)) or (b) keep the processor shell in plain JS and call raw `extern "C"` exports without bindgen glue. Most phonetics analysis is not hard-real-time — run it in an ordinary Worker; the worklet is only needed for playback-time processing.

### Large files: OPFS

OPFS is Baseline (Chrome/Edge 86+, Firefox 111+, Safari 15.2+; [MDN](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system)). `FileSystemSyncAccessHandle` — dedicated Workers only — gives synchronous random byte-range reads (`read(buffer, {at: offset})`) without loading the file into memory ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle)); measured ~90 ms for a 100 MB write vs ~850 ms in IndexedDB ([writeup](https://renderlog.in/blog/origin-private-file-system-opfs/)). Firefox and Safari lack `showOpenFilePicker`, so file import needs an `<input type=file>` fallback there.

### Recommendation

1. Build the DSP core on stable Rust with `simd128` enabled and scalar fallbacks.
2. Defer `wasm-bindgen-rayon`: single-threaded WASM+SIMD is sufficient for utterance-scale interactivity; add a threaded opt-in artifact later only if whole-file batch analysis of long recordings proves too slow.
3. Run analysis in a dedicated Worker that owns both the WASM instance and an OPFS sync access handle, streaming fixed-size chunks; store imported audio in OPFS.
4. For playback-time analysis, use the raw-exports-in-JS-shell AudioWorklet pattern rather than bindgen glue inside the worklet.

## 6. Waveform/spectrogram rendering in a webview

### GPU API baseline

Tauri renders through the OS webview (WebView2 / WKWebView / WebKitGTK via wry); GPU API availability is inherited, not configurable ([wry](https://github.com/tauri-apps/wry), [webview versions](https://v2.tauri.app/reference/webview-versions/), [tauri#6381](https://github.com/tauri-apps/tauri/issues/6381)).

- WebView2: WebGPU shipped (Chromium, since Chrome 113) ([caniuse](https://caniuse.com/webgpu)).
- WKWebView: WebGPU arrived with Safari 26 (macOS Tahoe) but embedded WKWebView does not automatically match Safari; a fallback is still required ([WebKit blog](https://webkit.org/blog/16993/news-from-wwdc25-web-technology-coming-this-fall-in-safari-26-beta/)).
- WebKitGTK: **no WebGPU, no active work on it** ([webkit-gtk list](https://www.mail-archive.com/webkit-gtk@lists.webkit.org/msg03883.html)). This is the binding constraint.

WebGL2 is present on all three backends. Conclusion: **WebGL2 is the baseline renderer; WebGPU only as a runtime-detected fast path** for the browser-hosted build.

### wavesurfer.js assessment

Actively maintained (v7.12.10, July 2026) but renders with Canvas2D, decodes whole files in-browser, and its spectrogram plugin renders the entire spectrogram up front ([repo](https://github.com/katspaugh/wavesurfer.js), [discussion #4033](https://github.com/katspaugh/wavesurfer.js/discussions/4033)). Disqualifying issues for a Praat-class analysis view:

- `fftSamples` changes bin count but the image is stretched to a fixed height — no genuine narrowband/broadband resolution control ([issue #3663](https://github.com/katspaugh/wavesurfer.js/issues/3663)).
- Color mapping limited to a 256-entry LUT; window functions available but dynamic-range handling is basic.
- Multi-hour/multi-GB files fail to decode or lag; workarounds are chunking with stutter at boundaries.

Acceptable, if desired, for a lightweight overview/minimap waveform only.

### Custom tile pipeline

No mature open-source precedent computes spectrogram tiles in Rust/WASM and blits them to a canvas — closest analogues are EMU-webApp (JS/Canvas, phonetics-grade but no WASM; [repo](https://github.com/IPS-LMU/EMU-webApp/)) and spectro (worker FFT → WebGL shader colorization; [making-of](https://github.com/calebj0seph/spectro/blob/master/docs/making-of.md)). The pattern itself (WASM number-crunching + GPU blit) is standard in map/scientific rendering.

### Recommendation

Build a Rust-computed tile pipeline: STFT tiles (chosen window, zero-padding, dB scaling, proper colormaps) computed in the shared Rust core (native in Tauri, WASM on the web), uploaded as textures to a thin WebGL2 canvas layer that handles pan/zoom only. Compute and cache tiles for the visible viewport plus margin — this structurally solves long recordings. Treat WebGPU as an optional runtime-detected path; never require it.

## 7. Plot and figure generation

### Options surveyed

- **plotters** — MIT, actively maintained; SVG (`plotters-svg`), bitmap, and WASM `CanvasBackend` with an identical API ([repo](https://github.com/plotters-rs/plotters), [wasm demo](https://plotters-rs.github.io/wasm32/plotters/index.html)). General chart library; heatmaps/spectrograms are hand-built on `DrawingArea`.
- **SVG → PDF chain** — `svg2pdf` (Typst team, builds on `usvg`/`resvg`/`pdf-writer`) converts SVG to vector PDF ([repo](https://github.com/typst/svg2pdf)); `pdf-writer` is dual MIT/Apache-2.0 ([crates.io](https://crates.io/crates/pdf-writer)). `vl-convert-pdf` adds real font embedding (selectable text) if needed ([lib.rs](https://lib.rs/crates/vl-convert-pdf)).
- **Typst as a library** — the `typst` crate (Apache-2.0) is explicitly supported for embedding; implement the `World` trait (helper: `typst-kit`), compile in-process, export with `typst_pdf::pdf` ([crates.io](https://crates.io/crates/typst), [Typst blog on automated generation](https://typst.app/blog/2025/automated-generation/)). `typst-as-lib` wraps the boilerplate but self-describes as early/unstable ([repo](https://github.com/Relacibo/typst-as-lib)).
- **PGF/TikZ** — the `pgfplots` crate emits PGFPlots LaTeX source from a Rust API, optionally compiling via `tectonic` ([docs.rs](https://docs.rs/pgfplots/latest/pgfplots/)). Small crate; check maintenance before depending on it — plain text emission is also trivial to hand-roll.
- **Vega-Lite** — typed spec-building crates exist ([vega_lite_5](https://github.com/procyon-rs/vega_lite_5.rs)), but static rendering from Rust requires `vl-convert`, which embeds a full V8 runtime ([repo](https://github.com/vega/vl-convert)). Heavy and non-Rust-native.

### Recommendation

A multi-backend figure-export crate is feasible because the SVG/PDF/Typst backends share one foundation (`usvg`/`resvg`/`pdf-writer`). Define an internal backend-agnostic plot description, then build in this order:

1. `plotters` for on-screen rendering (native + WASM canvas, one API).
2. SVG export via `plotters-svg`, chained to `svg2pdf` for publication PDF — nearly free once (1) exists.
3. PGF/TikZ text emission (via `pgfplots` or in-house templating) for LaTeX users.
4. `typst`-as-library for full multi-figure reports, once there is enough content to report on.

Skip Vega-Lite unless "export an editable chart spec" becomes a requirement; the V8 dependency is not justified otherwise.

## 8. Cargo workspace and release tooling

### Structure

- Use `[workspace.package]` / `[workspace.dependencies]` inheritance so shared metadata and internal version requirements live in one place (the pattern Bevy adopted: [bevy#16652](https://github.com/bevyengine/bevy/issues/16652)).
- Cargo 1.90+ (Sept 2025) supports `cargo publish --workspace`, which verifies interdependent crates against each other before uploading — this removes the manual dependency-order publish dance ([Tweag writeup](https://www.tweag.io/blog/2025-07-10-cargo-package-workspace/)).
- Split crates by functional domain (core DSP, pitch, formants, I/O, figure export), each usable standalone.

### Release tooling

- **release-plz** ([repo](https://github.com/release-plz/release-plz)) — compares workspace crates against the registry, generates changelogs via git-cliff, opens a continuously updated Release PR, detects breaking changes with `cargo-semver-checks`, and propagates dependency version bumps across dependents. First-class GitHub Action. Actively maintained.
- **cargo-release** ([repo](https://github.com/crate-ci/cargo-release)) — actively maintained CLI, but the GitHub Action ecosystem around it (`cargo-bins/release-pr`) is soft-deprecated with an explicit pointer to release-plz ([release-pr README](https://github.com/cargo-bins/release-pr)).

### Versioning

Independent per-crate semver (the tokio-ecosystem model: [tokio-util](https://crates.io/crates/tokio-util)) beats lockstep for many small crates — lockstep forces meaningless version bumps and changelog noise on unchanged crates. The classic failure mode of independent versioning (dependents pinned to incompatible core versions) is exactly what release-plz's automatic dependent-bumping addresses.

### Recommendation

release-plz with Conventional Commits, `cargo-semver-checks` in CI, independent per-crate versions, workspace dependency inheritance, and crates.io Trusted Publishing (OIDC) for the publish step.

## Summary of recommended crates

| Area | Crate | License | Rationale |
|---|---|---|---|
| WAV I/O | hound | Apache-2.0 | Stable, dependency-free, WASM-clean |
| Decoding (desktop) | symphonia | MPL-2.0 (ok unmodified) | Pure Rust, broad formats; keep mp3/aac opt-in |
| Playback/recording | cpal | Apache-2.0 | Standard cross-platform audio I/O; Web Audio for web build |
| Resampling | rubato | MIT/Apache-2.0 | Pure Rust, active, no OS deps |
| FFT | rustfft + realfft | MIT/Apache-2.0, MIT | Dominant FFT; realfft halves real-signal cost |
| Arrays | ndarray | MIT/Apache-2.0 | Best fit for framed/windowed signal processing |
| LPC | linear-predictive-coding (or in-house) | MIT/Apache-2.0 | Maintained Burg/autocorrelation LPC coefficients |
| Pitch/formants | in-house atomic crates | MIT/Apache-2.0 | No maintained permissive alternative exists |
| On-screen plots | plotters (+ plotters-svg, plotters-canvas) | MIT | One API for native, SVG, and WASM canvas |
| SVG→PDF | svg2pdf + pdf-writer | MIT/Apache-2.0 | Vector-preserving publication PDF |
| LaTeX figures | pgfplots (or templating) | MIT/Apache-2.0 | Direct PGFPlots emission |
| Reports | typst (library) | Apache-2.0 | In-process compilation to PDF |
| Releases | release-plz | MIT/Apache-2.0 | Workspace-native Release-PR automation |

Flagged and excluded: `aubio`/`aubio-rs` (GPL-3.0), `praatfan-core-rs` (GPL-3.0, reference only), Vega-Lite static rendering (`vl-convert`, embeds V8).
