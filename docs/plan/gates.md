# Phase gate record

Outcomes of the architect gate reviews defined in `roadmap.md`. Each entry
states the evidence the decision rests on; the referenced numbers come from
test runs and `oracle diff-all` reports reproducible from the repository.

## T1.7 — phase 1 gate (walking skeleton): CLOSED

- Workspace: 32 test binaries green; clippy `-D warnings` clean.
- STFT magnitudes agree with a scipy `ShortTimeFFT` reference to 5.3×10⁻⁸
  relative (threshold 1×10⁻⁶); overlapping tiles share columns bit-for-bit
  (zoom independence).
- Web walking skeleton: Playwright end-to-end passes (drop WAV → waveform +
  spectrogram render → zoom → 2 s playback with cursor advance → theme
  toggle). Frame times on the 10-minute fixture: p50 16.7 ms, p95 16.9 ms
  against the 32 ms budget.
- Screenshots in light and dark reviewed: waveform envelope, Gaussian-STFT
  spectrogram with visible formant structure, playback cursor, both themes
  legible (`apps/web/e2e/screenshots/`).
- Outstanding, outside the code: the 3-OS CI run requires the first push to
  GitHub, which awaits authorization.

## T2.6 — phase 2 gate (analysis tracks): CLOSED, accept-with-documentation

Oracle: parselmouth 0.4.7 wrapping Praat 6.1.38; committed references in
`tools/oracle/references/`; comparison via `tools/oracle-bridge` +
`oracle diff-all`. Frame grids match Praat's frame counts exactly on all
12 diffed cases after the documented fixes (pitch automatic time step
0.75/floor; physical window duration in frame-grid margins; discrete
duration as n·(1/fs)).

- **Pitch** — 2/4 fixtures fully pass; the failures are 1 and 3 fine
  violations (≤5.7%) at voicing-boundary frames. Zero gross/octave errors;
  voicing agreement 94.9–100%. Within `validation.md`'s framing of boundary
  frames as the expected disagreement mode.
- **Intensity** — Kaiser-20 window per the Praat manual; 3/4 fixtures pass
  the 1 dB band. Residual: 7 frames on one fixture (max 3.5 dB), all on
  sharp onsets, reproduced identically by an ideal reference Kaiser-20 —
  intrinsic to the documented window match, mean absolute error 0.068 dB.
- **Formants** — raw Burg vs raw Burg. With Praat-resampled input the
  pipeline agrees at 8/6717 checked points (0.1%), which isolates every
  divergence to the resampling stage. After pinning the anti-alias cutoff
  to the destination Nyquist (`ResampleQuality::Best`), violations are
  487/6717 (7.3%; F1/F2/F3 median residuals 116/180/191 Hz on violating
  frames). The remainder is attributable to Praat's unpublished
  precision-50 sinc window and cannot be closed clean-room. Accepted with
  this record; the framing, Gaussian window, pre-emphasis, Burg recursion,
  root-solving, and gating stages are verified exact.
- **Spectrogram** — exempt from the Praat oracle per `validation.md`;
  validated against scipy as under T1.7.
- Formant DP-tracking weights remain provisional (no numeric values in the
  cited literature); raw Burg is the display default and the smoothed track
  is labeled provisional in the inspector.

## T3.6 — phase 3 gate (annotation): CLOSED

- TextGrid round-trip: every text-format fixture (long/short × UTF-8/UTF-16/
  Latin-1, IPA labels, points, empty intervals) imports and re-exports with
  structural equality and byte-stable canonical output; malformed inputs
  return typed errors under fuzzing. The undocumented binary variant is
  detected and rejected with a typed error; read support stays on the backlog
  with oracle-generated samples available for format derivation.
- Keyboard-only annotation: Playwright covers the full loop (tier creation,
  splits at the cursor, label entry including IPA, merge, boundary nudge by
  pixel and by sample) with no pointer use.
- Undo: the journal's 50-mixed-operation random test holds hash stability
  through full undo/redo cycles; point commands, tier relations, and reorder
  are journaled with id-stable inverses.
- Screenshots in both themes reviewed; tier panes align with the spectrogram
  and labels stay legible.

## T4.6 — phase 4 gate (project & voice): CLOSED, accept-with-documentation

- Project: save/load round-trip, kill-and-recover via the autosave sidecar
  (page reload restores unsaved edits behind a recovery prompt), folder-drop
  corpus import with matching-stem TextGrid attachment, hash-based media
  re-linking. Container format documented in `docs/formats/project.md`.
- Selection tooling: box selection readout values equal direct engine queries
  bit-for-bit (asserted at engine, WASM, and end-to-end levels).
- Voice report vs the Praat oracle: both sustained-vowel cases pass 0/14
  (including a closed-form perturbed vowel at 3% jitter / 6% shimmer).
  On running speech, pulse placement follows the manual's documented
  cross-correlation method (parabolic refinement, 0.3/0.7 thresholds), which
  cut jitter-local disagreement from 191% to 12% relative; the remaining
  12–33% on perturbation quotients traces to sub-sample placement detail the
  public documentation does not specify. Voice reports are defined on
  sustained phonation; the running-speech residual is recorded here and in
  the crate documentation rather than tuned.

## T2.4 UI review

Overlay screenshots in light and dark reviewed
(`apps/web/e2e/screenshots/overlays-*.png`): pitch line on its own Hz scale
over voiced runs, formant speckles sized by bandwidth, intensity contour,
per-track toggles, non-modal inspector with Praat-provenance defaults and a
clipping badge. Visible-span re-render after a parameter change measured at
384 ms against the 500 ms budget. Pane labels and fallback notices sit on
contrast chips; the overview strip follows the theme.
