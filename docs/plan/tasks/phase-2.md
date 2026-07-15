# Task specs — phase 2 (analysis tracks)

Standing constraints and lane key as in `phase-1.md`. Algorithm details and
citations: `../../research/algorithms-and-validation.md` (§1 pitch, §2
formants, §4 intensity, §7 validation) — copy the relevant section into each
delegation, since lanes cannot follow references.

### T2.1 · codex · phx-pitch
**Objective.** Boersma-1993 autocorrelation pitch with the Viterbi path
finder.
**Files.** `crates/phx-pitch/src/*`.
**Interfaces.**
```rust
pub struct PitchParams { pub time_step: Option<f64>, pub floor_hz: f64 /* 75 */,
  pub ceiling_hz: f64 /* 600 */, pub max_candidates: usize /* 15 */,
  pub very_accurate: bool /* false */, pub silence_threshold: f64 /* 0.03 */,
  pub voicing_threshold: f64 /* 0.45 */, pub octave_cost: f64 /* 0.01 */,
  pub octave_jump_cost: f64 /* 0.35 */, pub voiced_unvoiced_cost: f64 /* 0.14 */ }
pub struct PitchFrame { pub time: f64, pub f0: Option<f64>, pub strength: f64,
  pub candidates: Vec<PitchCandidate> }
pub struct PitchTrack { /* frames on the FrameGrid; stats queries: mean/median/min/max
  over a TimeSpan, in Hz/semitones */ }
pub fn pitch_track(audio: AudioView, params: &PitchParams) -> PitchTrack;
```
**Requirements.** Window-corrected ACF (r_a/r_w, analytic r_w formula),
Hanning at 3/floor (Gaussian at 6/floor when very_accurate), soft near-Nyquist
lowpass, zero-pad 1.5×→pow2 FFT ACF, candidate lags in
[1/ceiling, 1/floor] via sinc peak interpolation (phx-dsp), unvoiced
candidate strength eq. 23, voiced strength eq. 24, transition costs eq. 27,
DP over all frames. All equations are reproduced in the algorithms report
§1.2–1.4 with the Praat-documented defaults table.
**Verification.** Synthetic ground truth first: pure tones, AM tones,
tone+noise at known F0 (error < 0.1%); octave-error stress case (strong even
harmonics) resolved by the path finder; then oracle comparison on fixture
speech — voiced-frame F0 within 1% where both voiced at the same octave,
voicing decisions majority-consistent, GPE cases listed individually in the
report.

### T2.2 · codex · phx-formant
**Objective.** Burg LPC formant estimation with DP tracking.
**Files.** `crates/phx-formant/src/*`.
**Interfaces.**
```rust
pub struct FormantParams { pub ceiling_hz: f64 /* 5500 */, pub max_formants: usize /* 5 */,
  pub window_length: f64 /* 0.025 */, pub time_step: Option<f64>,
  pub preemphasis_from_hz: f64 /* 50 */ }
pub struct FormantFrame { pub time: f64, pub formants: Vec<FormantPoint> /* freq, bandwidth */ }
pub fn formant_track(audio: AudioView, params: &FormantParams) -> FormantTrack;
pub fn track_smoothed(raw: &FormantTrack, refs: &TrackingRefs) -> FormantTrack; // Xia–Espy-Wilson DP
```
**Requirements.** Resample to 2×ceiling; Gaussian window (effective length
2× nominal); Burg recursion per Numerical Recipes §13.6 spec (reproduced in
algorithms report §2.2); polynomial roots (aberth or in-house Laguerre with
polishing), reflect outside-unit-circle roots, keep positive-imaginary; F =
θ·fs/2π, B = −(fs/π)·ln r; discard F < 50 Hz or > ceiling − 50 Hz; DP
tracking with the report's §2.6 cost functions (neutral refs 500/1500/2500/
3500 Hz).
**Verification.** Synthetic vowels from known all-pole filters (three
formants at set frequencies/bandwidths, impulse-train source): recovered
F1–F3 within 2%, bandwidths within 20%; oracle comparison on fixture vowels
within validation.md bands; a male-speech fixture analyzed at 5000 vs 5500 Hz
ceiling shows the documented shift (sanity of the ceiling semantics).

### T2.3 · grok · phx-intensity
**Objective.** Praat-equivalent intensity contour.
**Files.** `crates/phx-intensity/src/*`.
**Interfaces.** `IntensityParams { pitch_floor_hz /* 100 */, time_step:
Option<f64>, subtract_mean: bool /* true */ }`,
`intensity_track(audio, &params) -> IntensityTrack` (dB SPL re 2×10⁻⁵ Pa).
**Requirements.** Square samples, convolve with analytic Gaussian of
effective duration 3.2/pitchFloor (documented substitute for Praat's
Kaiser-20 — algorithms report §4.1/open item 2), default step 0.8/pitchFloor,
local DC removal, FrameGrid placement.
**Verification.** Constant-amplitude sine → flat contour (ripple < 1e-4 dB);
known-RMS signals map to exact dB; oracle within 1 dB on fixtures.

### T2.4 · grok · editor overlays + inspector
**Objective.** Pitch/formant/intensity rendered over the spectrogram; live
non-modal inspector.
**Files.** `apps/ui/` (TrackOverlay, InspectorPanel), `apps/web/` wiring,
`phx-engine`/`phx-wasm` additions (`pitch_track`, `formant_track`,
`intensity_track` calls + params plumbing).
**Requirements.** Pitch as a line (voiced runs) on its own right-hand scale;
formants as speckles sized by bandwidth; intensity as a thin line; per-track
show/hide; inspector edits params → recompute visible span first, then
background-fill; floor/ceiling clipping warnings per ux.md.
**Verification.** Playwright: toggle each overlay, change pitch ceiling and
assert re-render < 500 ms on the visible span, warning badge appears when
ceiling < max tracked value; screenshots light+dark.

### T2.5 · sonnet · oracle harness bring-up
**Objective.** `tools/oracle/` per validation.md: uv project, parselmouth
runner, JSON diff reports, CI job (skips gracefully when oracle deps are
absent).
**Verification.** CI run showing pass on phases' cases; a deliberately
perturbed value fails.

### T2.6 · architect · phase gate review
Diff review, oracle report judgment (gross vs fine errors separated),
zoom-independence re-check with overlays on, screenshot review, close phase.
