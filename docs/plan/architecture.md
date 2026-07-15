# Architecture

Cargo workspace monorepo. The analysis core is a set of small crates with no UI
dependencies, each publishable on its own under the `phx-` prefix
(`naming.md` covers the product name). Two frontends share one Svelte UI
package; the same core runs natively under Tauri and as WASM in the browser.

## Workspace layout

```
Cargo.toml              # [workspace] with dependency + metadata inheritance
crates/
  phx-audio             # buffers, decode, resample
  phx-dsp               # windows, FFT, framing, interpolation
  phx-spectrogram       # Gaussian STFT power spectra + tiles
  phx-pitch             # Boersma-1993 autocorrelation F0
  phx-formant           # Burg LPC formants + DP tracking
  phx-intensity         # windowed-RMS intensity
  phx-voice             # pulses, HNR, jitter, shimmer, CPP, voice report
  phx-annot             # annotation model (tiers, hierarchy)
  phx-textgrid          # Praat TextGrid import/export
  phx-render            # colormaps, dB→RGBA tile rendering
  phx-figure            # figure description + export backends
  phx-project           # project file, autosave, media references
  phx-engine            # session engine: commands, undo journal, cache
  phx-wasm              # wasm-bindgen bindings over phx-engine
apps/
  ui/                   # shared Svelte 5 component library + views
  web/                  # SvelteKit app: WASM core in a Worker, OPFS storage
  desktop/              # Tauri 2 shell: native core, cpal playback
tools/
  oracle/               # Python (uv) parselmouth comparison harness — dev only
tests/fixtures/         # small permissively licensed audio + TextGrids
docs/
```

Post-v0.1 additions, reserved in the design: `phx-py` (PyO3 bindings over the
same engine API), `phx-cli` (batch analysis to CSV), `phx-server` (REST surface
on the Gentle model).

## Crate responsibilities and dependency edges

Arrows point at dependencies. Analysis crates depend on `phx-dsp` only;
they take sample slices, so they stay usable outside this project.

```
phx-audio ──────────────┐
phx-dsp ◄── phx-spectrogram, phx-pitch, phx-formant, phx-intensity
phx-voice ──► phx-pitch, phx-dsp
phx-annot ◄── phx-textgrid
phx-render ──► (plain arrays; no core deps)
phx-figure ──► plotters, svg2pdf (feature-gated backends)
phx-project ──► phx-audio, phx-annot
phx-engine ──► everything above
phx-wasm ──► phx-engine
```

| Crate | Responsibility | Key external deps |
|---|---|---|
| phx-audio | `Audio` (planar f32 samples + f64 sample rate), WAV via hound, broad decode via symphonia (opt-in features; MPL-2.0 unmodified), resampling via rubato | hound, symphonia, rubato |
| phx-dsp | Hanning/Gaussian/Kaiser windows, real FFT wrappers, absolute-time frame grid, windowed-sinc interpolation, pre-emphasis | realfft, rustfft, ndarray |
| phx-spectrogram | Gaussian-window STFT power spectral density; viewport-independent tile computation in dB | phx-dsp |
| phx-pitch | Window-corrected autocorrelation candidates + Viterbi path finder (Boersma 1993); full parameter surface with Praat-documented defaults | phx-dsp |
| phx-formant | Pre-emphasis, Burg recursion, polynomial roots → frequency/bandwidth, Xia–Espy-Wilson DP tracking | phx-dsp |
| phx-intensity | Squared signal convolved with Gaussian window (3.2/pitchFloor), dB SPL re 2×10⁻⁵ Pa | phx-dsp |
| phx-voice | Pulse extraction from pitch + waveform, jitter/shimmer families, HNR, CPP/CPPS, spectral moments, aggregate voice report | phx-pitch |
| phx-annot | Tiers (interval/point), typed hierarchy (parent/child spans on the ELAN model), validation of cross-tier integrity | — |
| phx-textgrid | Praat TextGrid read (long/short/binary text variants; UTF-8/UTF-16/Latin-1) and write (UTF-8 always) | phx-annot |
| phx-render | Perceptual colormaps (viridis, magma, grayscale), theme-aware dB→RGBA mapping | — |
| phx-figure | Backend-agnostic figure model; exporters: SVG, PDF (svg2pdf), PGFPlots/TikZ, Typst/CeTZ, Vega JSON, matplotlib/R/Julia code + data, GraphML | plotters, svg2pdf, typst (later) |
| phx-project | Project file (versioned, self-describing), media references, analysis parameter profiles, autosave snapshots | serde |
| phx-engine | The one API both frontends and future bindings consume: commands with explicit arguments, journaled unified undo, content-addressed analysis cache | all core crates |

## Fixed design rules

These come straight from the pain-point catalog and bind every crate:

1. **Explicit arguments everywhere.** No selection state, no implicit current
   object. `pitch_track(&audio, &PitchParams) -> PitchTrack` is the calling
   convention; the GUI builds calls, it never becomes one.
2. **Analysis anchored to the signal.** Frame grids derive from the audio
   object's time domain alone. A value queried at time *t* is identical
   regardless of zoom, scroll, window size, or whether the query came from the
   GUI, a tile, or a batch call. This is a tested invariant (`validation.md`).
3. **f32 storage, f64 computation.** Samples are stored planar f32; analysis
   frames are promoted to f64 before windowing/FFT/recursions. Tolerance bands
   in validation absorb the storage difference.
4. **UTF-8 always on write.** Encoding never depends on content. Legacy
   encodings are read, never produced.
5. **Every mutation is a serializable command** through the engine journal —
   one undo stack for boundary moves, label edits, object lifecycle, project
   changes; unlimited depth. Analyses are pure functions of (audio, params)
   and live in the cache, outside undo.
6. **MIT OR Apache-2.0**, enforced by cargo-deny allowlist (MIT, Apache-2.0,
   BSD, MPL-2.0 unmodified-dependency only). GPL anywhere in the tree fails CI.

## API sketch (phx-engine surface)

```rust
// Analysis (pure, cached)
fn pitch_track(audio: AudioRef, params: &PitchParams) -> PitchTrack;
fn formant_track(audio: AudioRef, params: &FormantParams) -> FormantTrack;
fn intensity_track(audio: AudioRef, params: &IntensityParams) -> IntensityTrack;
fn spectrogram_tile(audio: AudioRef, req: &TileRequest) -> Tile;      // dB values
fn voice_report(audio: AudioRef, span: TimeSpan, params: &VoiceParams) -> VoiceReport;

// Parameter structs: every field explicit, Default = documented Praat default,
// each field doc-commented with provenance and the failure mode of a bad value.
pub struct PitchParams { pub floor_hz: f64, pub ceiling_hz: f64, /* … */ }

// Mutation (journaled)
enum Command { AddTier {..}, MoveBoundary {..}, SetLabel {..}, ImportAudio {..}, /* … */ }
fn apply(&mut self, cmd: Command) -> Result<Applied>;
fn undo(&mut self); fn redo(&mut self);
```

`TileRequest` carries absolute time/frequency bounds and pixel dimensions; the
engine snaps to the object-level frame grid internally so adjacent tiles share
frame columns exactly.

## Frontend integration

One TypeScript interface, two transports:

```ts
interface CoreClient {
  importAudio(src: File | string): Promise<AudioId>;
  waveformSlice(id, t0, t1, px): Promise<MinMaxPyramidSlice>;
  spectrogramTile(id, req): Promise<ImageBitmap /* from RGBA bytes */>;
  pitchTrack(id, params): Promise<PitchTrack>;
  apply(cmd): Promise<Applied>;  undo(): Promise<void>;
  // …
}
```

- **Web**: `WasmCoreClient` — a dedicated Worker owns the WASM engine instance
  and an OPFS sync access handle; imported audio is copied into OPFS; results
  cross as transferable buffers. simd128 enabled, scalar fallback;
  wasm-bindgen-rayon deferred until batch analysis demands it.
- **Desktop**: `TauriCoreClient` — Tauri commands into native `phx-engine`.
  File I/O happens in Rust commands (dialog plugin only picks paths). Playback
  runs on a cpal callback thread behind a `PlaybackEngine` trait
  (play/pause/seek/position), with an atomic sample-counter clock emitted to
  the webview at display rate; the web build implements the same trait shape
  over WebAudio as transport.

Rendering: waveform and spectrogram draw on a WebGL2 canvas fed by
core-computed tiles (RGBA textures), pan/zoom in the shader layer only.
Frame-time is measured at startup; a sustained slow path (WebKitGTK software
rendering) switches to a Canvas2D blit and surfaces a notice. WebGPU is a
runtime-detected extra on the hosted build, never required.

## Version pins (July 2026)

Rust stable, edition 2024, MSRV 1.85. symphonia 0.6, hound 3.5, cpal 0.18,
rubato 4, rustfft 6.4 + realfft 3.5, ndarray 0.16, Tauri 2.11, wasm-bindgen
0.2.126 / wasm-pack 0.15. Frontend: Svelte 5, SvelteKit 2, Vite, Tailwind 4,
bun as package manager. Release automation: release-plz, independent per-crate
semver, cargo-semver-checks in CI, crates.io Trusted Publishing.

## Starting frontend

The web app ships first: a URL is the lowest-friction professional demo, the
WASM build forces the core to stay portable from day one, and the shared UI
package means the Tauri desktop app (phase 6) reuses nearly everything. The
desktop app is the long-term daily driver (native speed, large files, system
integration); it enters the roadmap as soon as the editor stabilizes.
