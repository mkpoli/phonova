# Phonix — project brief

Phonix is an open-source toolkit for phonetic research: a Rust analysis core
surrounded by several user interfaces. It replaces Praat for the everyday
research workflow — analyze voice, manage audio, show spectrograms, annotate,
draw publication figures — and is planned to grow into a superset of Praat's
research-critical features.

"Phonix" is a working name. The crate name `phonix` is registered to an
unrelated project on crates.io, so published crates use the `phx-` prefix
until the product name is settled.

## Why a rebuild

Praat is the de-facto standard and its design predates most of what users now
expect from software. The pain-point catalog
(`../research/praat-features-and-pain-points.md`) documents the recurring
failures: the select-then-act Objects model, a scripting language whose
commands are GUI button labels, silent measurement traps in default
parameters, zoom level affecting measured values, TextGrid files that switch
encoding when a label contains non-ASCII characters, no project or session
concept, per-editor undo capped at ten steps, an append-only Picture window,
and export limited to a handful of formats. Each of these maps to a design
requirement below.

## Architecture

A Cargo workspace monorepo. The analysis core is a set of small,
single-purpose crates with no UI dependencies, usable by anyone as ordinary
Rust libraries and compiled to WASM for the web. User interfaces are separate
projects consuming the core:

1. **Web app** — the core compiled to WASM, running fully client-side. The
   first interface to ship: a URL is the lowest-friction demo and requires no
   packaging or install.
2. **Tauri desktop app** — the same UI with native core, file-system
   integration, and offline use. Ships close behind the web app.
3. **Native app** — a later, minimal, highest-performance interface for
   professional and low-spec use.

The core is library-first: everything the GUI can do is a plain function
call with explicit arguments (this is the structural fix for Praat's
selection-state model and its scripting language at once). Python bindings
over the same API are a planned early deliverable because parselmouth's
popularity shows where scripting users already are.

Ecosystem choices, evaluated in `../research/rust-ecosystem.md`: symphonia
and hound for decoding, cpal for native playback behind a `PlaybackEngine`
trait (WebAudio as transport on the web), rubato for resampling, rustfft /
realfft / ndarray for DSP. Spectrogram rendering uses Rust-computed STFT
tiles blitted through WebGL2; wavesurfer.js was evaluated and rejected.
Figure export builds on plotters (SVG → PDF via svg2pdf), with text backends
for PGF/TikZ (pgfplots), Vega JSON, Typst/CeTZ, GraphML, and generated
R / Python / Julia plotting code alongside the data.

## v0.1 scope — the demo slice

A phonetician can, in one sitting:

- open or drag in audio (and a folder of audio as a project),
- scroll and zoom a long recording with waveform and spectrogram,
- see pitch, formant, and intensity tracks overlaid, with honest defaults
  (documented parameter provenance, warnings where Praat's defaults mislead),
- select a region, play it, run a voice report (F0 statistics, HNR, jitter,
  shimmer, CPP),
- annotate on tiers and read/write Praat `.TextGrid` files losslessly
  (all three on-disk variants, both encodings),
- save a project file that restores the full session, and
- export a publication figure of any view to SVG, PDF, TikZ, Typst, Vega,
  or as data plus plotting code.

Measured values are independent of zoom, window position, and any other view
state. Undo covers every operation. Autosave is on by default.

## Algorithms and validation

Algorithm specifications with literature citations are in
`../research/algorithms-and-validation.md`: Boersma-1993 autocorrelation
pitch with the Viterbi path finder, Burg LPC formants with dynamic-programming
tracking, Gaussian-window STFT spectrograms, windowed-RMS intensity, HNR,
jitter/shimmer, spectral moments, CPP/CPPS, windowed-sinc resampling.

License: MIT (or MIT OR Apache-2.0). Clean-room protocol: implementations
derive from published papers and Praat's public manual. Praat's GPL source
may inform ideas; no code is ported from it. Praat itself, driven
out-of-process (e.g. through parselmouth), serves as a black-box numeric
oracle in the test suite, with tolerance bands based on GPE/MFPE and
ground-truth F0 corpora (PTDB-TUG, Keele) plus openly licensed audio
(LibriSpeech, VCTK, CMU ARCTIC, Common Voice).

## Design direction

Design lessons from MuseScore 4, Audacity, ELAN, EMU-SDMS, iZotope RX, and
DAW navigation conventions are collected in
`../research/design-lessons.md`. For v0.1: DAW-grade zoom/pan, direct
selection on the spectrogram itself, a command palette, non-modal
inspectors, keyboard-first annotation, colorblind-safe spectrogram palettes
with a grayscale print option, and light/dark themes verified by screenshot
in both modes before any visual change is called done.

## Sources

- `../research/praat-features-and-pain-points.md`
- `../research/algorithms-and-validation.md`
- `../research/rust-ecosystem.md`
- `../research/design-lessons.md`
