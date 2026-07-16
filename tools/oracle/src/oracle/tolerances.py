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

# Voicing decision: majority (>50%) frame agreement required on clean speech.
VOICING_MAJORITY_THRESHOLD = 0.5

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
