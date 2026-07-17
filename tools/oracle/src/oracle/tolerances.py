"""Tolerance bands from `docs/plan/validation.md` § Tolerance bands.

Kept as named constants (not a free-form dict) so a band change shows up as
a one-line diff against the source table.
"""

from __future__ import annotations

# F0, both frames voiced at the same octave: relative tolerance.
F0_FINE_RELATIVE = 0.01  # <=1% relative (validation.md target after tuning: 0.5%)

# F0 gross-error threshold (GPE definition, algorithms-and-validation.md §7.3):
# fraction deviation beyond this is an octave/voicing-class disagreement, not
# a numeric one, and is reported separately rather than tolerance-checked.
F0_GROSS_RELATIVE = 0.20

# F1-F3 on clear vowels: max(absolute, relative) tolerance.
FORMANT_ABSOLUTE_HZ = 50.0
FORMANT_RELATIVE = 0.03
FORMANT_TRACKED_SLOTS = (1, 2, 3)  # F1-F3 only; validation.md gives no band for F4/F5

# Intensity: absolute dB tolerance.
INTENSITY_ABSOLUTE_DB = 1.0

# Voice-report scalars (validation.md § Tolerance bands, "Jitter/shimmer"
# and "HNR" rows -- those rows already name these exact measures; the
# voice-report case aggregates them into span-level scalars instead of
# per-frame/per-period values, so the same bands apply directly).
JITTER_SHIMMER_RELATIVE = 0.10  # <=10% relative on sustained vowels
VOICE_HNR_ABSOLUTE_DB = 1.0  # <=1 dB on well-voiced spans

# F0 summary scalars (mean/median/min/max) in a voice report reuse the
# frame-level F0 band above: same measure, aggregated rather than
# per-frame, over a span both sides already agree is voiced.
VOICE_F0_RELATIVE = F0_FINE_RELATIVE

# --- T2.6 / T4.6 accept-with-documentation bands (docs/plan/gates.md) ---
#
# These encode the specific residuals the phase-2 and phase-4 gate reviews
# accepted with documentation, so `oracle diff-all` can tell an accepted,
# already-recorded residual apart from a real regression instead of
# routing whole classes of disagreement around the pass/fail decision.
# Every band below has a citation to the gates.md number it is derived
# from, with a small safety margin so the accepted fixtures keep passing.

# T2.6: "Zero gross/octave errors" on the pitch corpus. A gross error
# (GPE-style octave/voicing-class disagreement, not a numeric one) was
# never part of the accepted residual, so any occurrence fails.
F0_GROSS_ERROR_MAX = 0

# T2.6: "the failures are 1 and 3 fine violations (<=5.7%) at
# voicing-boundary frames" -- a fine violation's relative deviation may
# reach this far before it stops being the documented boundary-frame
# residual and becomes a real regression. Margin over the recorded 5.7%.
F0_FINE_VIOLATION_MAX_RELATIVE = 0.06

# T2.6: those same two fixtures had 1/173 (0.6%) and 3/206 (1.5%) fine
# violations; the other two fixtures had none. Margin over the recorded
# rate, well under the old "any violation fails" behavior.
F0_FINE_VIOLATION_RATE_MAX = 0.05

# T2.6: "voicing agreement 94.9-100%" across the pitch corpus. Floor with
# headroom below the recorded minimum -- well above the previous
# majority-only (0.5) floor, which could not have caught a real collapse.
VOICING_MAJORITY_THRESHOLD = 0.90

# Formant: a tracked slot (F1-F3) present on only one side was not part of
# the T2.6 accepted residual (0 missing on all 4 fixtures); any occurrence
# fails.
FORMANT_MISSING_MAX = 0

# T2.6: "After pinning the anti-alias cutoff to the destination Nyquist
# (ResampleQuality::Best), violations are 487/6717 (7.3%)" -- aggregated
# over the whole formant corpus (all of SPEECH_AND_VOWEL_CORPUS), not
# per-fixture (individual fixtures range from ~0.4% to ~14%). Margin over
# the recorded corpus-wide rate.
FORMANT_CORPUS_VIOLATION_RATE_MAX = 0.08

# Intensity: a null on one side with a value on the other was not part of
# the accepted residual; any occurrence fails.
INTENSITY_NULL_MISMATCH_MAX = 0

# T2.6: "Residual: 7 frames on one fixture (max 3.5 dB)... the other
# fixtures passed the 1 dB band clean." Cap both how many frames may sit
# outside the 1 dB band on a fixture and how far any one of them may
# drift. Margin over the recorded 7 frames / 3.5 dB.
INTENSITY_MAX_VIOLATION_FRAMES = 8
INTENSITY_MAX_VIOLATION_DB = 4.0

# Voice: a null on one side with a value on the other was not part of the
# accepted residual on any fixture; any occurrence fails.
VOICE_NULL_MISMATCH_MAX = 0

# T4.6: "both sustained-vowel cases pass 0/14" -- jitter/shimmer scalars on
# a sustained-phonation fixture use the tight band above (10%) with no
# widening. The one fixture named below is the documented exception.
#
# T4.6: "On running speech... the remaining 12-33% on perturbation
# quotients traces to sub-sample placement detail the public documentation
# does not specify... the running-speech residual is recorded here and in
# the crate documentation rather than tuned." Widen the jitter/shimmer
# band for this fixture only; F0 and HNR scalars are not part of the
# documented residual and keep their normal bands. Margin over the
# recorded 33.4% maximum.
VOICE_RUNNING_SPEECH_AUDIO = "arctic_bdl_a0001.wav"
VOICE_RUNNING_SPEECH_JITTER_SHIMMER_RELATIVE = 0.35
