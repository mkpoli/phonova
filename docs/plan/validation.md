# Validation

Two independent lines of evidence per analysis: synthetic signals with
analytic ground truth (exact, no license entanglement, run everywhere), and
Praat as a black-box oracle (parity with the field's reference tool). A
measurement feature merges only with both.

## Synthetic ground truth (in `cargo test`)

Pure Rust, part of each crate's test suite:

- Pitch: pure/AM/noisy tones at known F0; harmonic complexes engineered to
  provoke octave errors; silence and unvoiced noise for voicing decisions.
- Formants: impulse-train excitation through known all-pole filters —
  recovered frequencies/bandwidths vs the filter's construction values.
- Spectrogram: sinusoid line sharpness, sidelobe floor, Parseval energy
  checks against analytic PSD.
- Intensity: known-RMS signals to exact dB; modulation ripple bounds.
- DSP invariants as property tests: FrameGrid viewport-independence, tile
  seam bit-equality, UTF-8-always on TextGrid write.

## Praat oracle (`tools/oracle/`, out of process)

A uv-managed Python project running praat-parselmouth (GPL-3.0) strictly as
an external process: it is a dev tool, never a dependency of any crate.
Licensing reasoning for invoking parselmouth out-of-process is in
`../research/algorithms-and-validation.md` §7.1.

Mechanics:

1. `oracle run --case pitch-defaults --audio tests/fixtures/audio/…` calls
   parselmouth with parameters mirrored from the Rust `*Params` structs and
   dumps frame-level JSON.
2. `oracle diff` compares against the Rust CLI dump of the same case,
   classifying disagreements per the bands below and separating gross from
   fine errors (GPE/MFPE methodology).
3. CI job `oracle` installs parselmouth via uv; when the wheel is
   unavailable for the runner, the job reports "skipped", never silently
   passes.

### Committed references

`tools/oracle/references/` holds one deterministic JSON file per (case,
audio) pair, named `<case>__<audio-stem>.json` (for example
`pitch-defaults__synth_vowel_a.json`). Each file carries the analysis
parameters mirrored from the Rust `*Params` struct, the frame-level
measurements, and the `parselmouth_version`, `praat_version`, and
`praat_version_date` that produced them, so a disagreement can always be
traced to a specific Praat build. `oracle generate` regenerates the full set
from the fixture audio in `tests/fixtures/audio/`; `oracle run --case …
--audio … --out …` regenerates a single file. `oracle diff-all
--measured-dir …` compares every committed reference against a directory of
same-named Rust dumps in one pass, skipping the spectrogram case (validated
separately against scipy, not oracle-diffed — see the tolerance table
below).

Committing these files carries no GPL obligation: they hold Praat's program
output — measured floating-point numbers — not Praat's or parselmouth's
source code, so redistributing them does not inherit the GPL-3.0 that covers
the tool producing them.

## Tolerance bands

From the algorithms report §7.3; tightened empirically once real numbers
exist:

| Measure | Band | Notes |
|---|---|---|
| F0 (both voiced, same octave) | ≤ 1% relative | finer target after tuning: 0.5% |
| F0 gross errors | listed individually | investigate; voicing/octave mismatches are the expected disagreement mode |
| Voicing decision | majority agreement on clean speech | boundary frames reported separately |
| F1–F3, clear vowels | ≤ max(50 Hz, 3%) | Praat's own tracker varies with ceiling |
| Intensity | ≤ 1 dB | window approximation difference documented |
| HNR | ≤ 1 dB on well-voiced spans | larger near voicing boundaries |
| Jitter/shimmer | ≤ 10% relative on sustained vowels | pulse-placement differences dominate |
| Spectrogram | not oracle-diffed | validated against scipy STFT + analytic cases instead |

## Reproducibility invariants (regression suite, forever)

Praat's documented failure modes, encoded as tests that must never pass by
accident:

1. **Zoom independence**: track values and tile contents at (t, f) are
   identical across any viewport, tile size, or request order (pain point
   2.9).
2. **Batch = GUI**: the engine call the UI makes and the same call from a
   script/CLI produce byte-identical numbers (pain point 2.5; demo wow-moment
   4).
3. **Frame-count stability**: durations that divide the window/step exactly
   lose no frames (Praat issue #2011).
4. **Encoding stability**: TextGrid write is UTF-8 regardless of label
   content; round-trip of every fixture variant is lossless (pain point 2.6).
5. **Undo completeness**: any journaled command sequence undoes to a
   state-hash-identical document.

## Ground-truth F0 (stretch)

PTDB-TUG and Keele provide laryngograph-referenced F0 for validating against
truth rather than against Praat. Their licensing (TIMIT-derived sentence
content in PTDB-TUG) is unresolved for redistribution — keep them as local
opt-in datasets (`oracle run --dataset ptdb --local-path …`), never in the
repo (algorithms report open item 6).
