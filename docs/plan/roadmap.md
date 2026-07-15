# Roadmap — empty repo to v0.1 demo

Phases are gates, not calendar units. A phase closes only when its
verification gate passes; DSP gates mean numeric agreement with the Praat
oracle (`validation.md`), UI gates mean screenshots in light and dark themes
reviewed against `ux.md`. Per-phase task specs live in `tasks/` and are
written at phase start from this document.

## Phase 0 — reset and scaffold

**Goal.** Clean workspace ready for parallel implementation.

**Deliverables.** Old Tauri/SvelteKit scaffold deleted (git history kept);
Cargo workspace with all crate skeletons compiling; CI (fmt, clippy
`-D warnings`, tests, wasm32 build check, cargo-deny license allowlist);
`tests/fixtures/` populated with permissively licensed audio clips and
TextGrid samples with attribution manifest; `phx-core` name registered on
crates.io as a placeholder.

**Gate.** `cargo test --workspace` and `cargo build --target
wasm32-unknown-unknown -p phx-wasm` green in CI on Linux/macOS/Windows
runners.

## Phase 1 — walking skeleton: see and hear a sound

**Goal.** Open audio in the browser, see waveform + spectrogram, scroll,
zoom, play. Demoable end-to-end, however plain.

**Deliverables.** `phx-audio` (WAV decode, buffer model), `phx-dsp` (windows,
real FFT, absolute-time frame grid, pre-emphasis), `phx-spectrogram`
(Gaussian STFT, dB tiles), `phx-render` (viridis/magma/grayscale),
minimal `phx-engine` + `phx-wasm` (import, waveform pyramid, tiles), web app
with Worker + OPFS, WebGL2 tile canvas with pan/zoom (Canvas2D fallback),
WebAudio playback with engine-clocked cursor.

**Gate.** A 10-minute WAV loads and scrolls at interactive frame rates;
spectrogram values at a given (t, f) are bit-identical across tile grids
(zoom-independence test); STFT magnitudes validated against scipy/numpy
reference within 1e-6 relative; screenshots light+dark.

**Parallelism.** Core crates (audio/dsp/spectrogram/render) proceed in
parallel with the web-app shell; they meet at the engine.

## Phase 2 — analysis tracks: pitch, formants, intensity

**Goal.** The Praat editor's analysis surface, with honest defaults.

**Deliverables.** `phx-pitch` (Boersma 1993, full parameter set, Viterbi),
`phx-formant` (Burg, root-solving, DP tracking), `phx-intensity`; overlay
rendering in the editor; non-modal inspector with live re-analysis and
clipping warnings; oracle harness (`tools/oracle/`) running in CI.

**Gate.** Tolerance bands from `validation.md` met on the fixture corpus for
pitch (voiced-frame agreement, GPE separated), formants (F1–F3 on clear
vowels), intensity; inspector parameter changes re-render the visible region
without blocking; zoom-independence holds for every track.

## Phase 3 — annotation

**Goal.** Tier-based annotation a working phonetician can live in.

**Deliverables.** `phx-annot` (interval/point tiers, hierarchical tier
relations, integrity validation), `phx-textgrid` (read all three variants ×
all encodings; write UTF-8), tier UI with the keyboard-first loop, label
search across the project, journaled undo across all annotation edits.

**Gate.** Round-trip corpus test: a body of real-world TextGrids (including
UTF-16, short format, IPA labels) imports and re-exports losslessly;
keyboard-only annotation of a sentence recorded on video; undo stack survives
50 mixed operations.

## Phase 4 — project, voice report, selection tooling

**Goal.** Sessions persist; measurements aggregate.

**Deliverables.** `phx-project` (project file format, autosave, crash
recovery, parameter profiles), home screen / project manager with
drop-a-folder import, `phx-voice` (pulses, jitter/shimmer, HNR, CPP,
spectral moments, voice report card), spectrogram box selection with
readout.

**Gate.** Kill the app mid-edit → relaunch recovers the session; voice
report matches the oracle within bands on sustained vowels; folder-drop
import (wow-moment 1) demonstrated.

## Phase 5 — figures

**Goal.** Publication output better than anything Praat ships.

**Deliverables.** `phx-figure` model + exporters: SVG, PDF, PNG, PGFPlots/
TikZ, Typst/CeTZ, Vega JSON, data + matplotlib/R/Julia code, GraphML (for
annotation-graph data); export dialog with live theme-aware preview.

**Gate.** One reference figure (waveform + spectrogram + pitch + one tier)
exported through every backend; TikZ compiles under latexmk, Typst under
typst compile, generated Python/R/Julia scripts run and reproduce the figure;
visual diff of SVG/PDF against the on-screen preview.

## Phase 6 — desktop shell

**Goal.** The same product, offline, native speed, real files.

**Deliverables.** Tauri app wrapping `apps/ui`; native engine via commands;
cpal playback with sample-counter clock; file associations and OS open-with;
WebKitGTK slow-path detection + fallback; packaged builds for Windows,
macOS, Linux.

**Gate.** Same demo script as the web app passes on all three OSes; a
1-hour recording scrolls smoothly on desktop; playback cursor drift < 1 frame
over 5 minutes.

## Phase 7 — polish and the demo

**Goal.** The five wow-moments run flawlessly; the product feels finished at
its size.

**Deliverables.** Command palette covering the full action surface; keyboard
map documentation; onboarding empty-states; the five-moment demo script; a
public read-only demo deployment with a bundled sample project.

**Gate.** All five acceptance criteria in `ux.md` pass live; a phonetician
outside the project runs the demo script unassisted.

## Sequencing notes

- Phases 0→1→2 are strictly ordered; 3 and 4 can overlap once 2's engine
  surface is stable; 5 needs 2 (tracks to draw) and profits from 3 (tiers in
  figures); 6 starts any time after 2 at the cost of double-testing, best
  after 4; 7 is last.
- The oracle harness (validation.md) lands with phase 2 and stays in CI
  forever; every later analysis feature adds its oracle case in the same PR.
- Praat-script compatibility, Python bindings, alignment/ASR, recording, and
  the pure-native app are explicitly out of v0.1; the engine API is designed
  so none of them require breaking changes.
