"""Compare a measured JSON dump against a reference JSON, per validation.md.

Both files share one schema (produced by `oracle.measures` on the Python
side; the Rust side dumps the same shape from its own analysis run):

    {
      "case": "pitch-defaults",
      "measure": "pitch" | "formant" | "intensity",
      "audio": "tests/fixtures/audio/....wav",
      "params": {...},
      "frames": [ ... ]
    }

Frames are matched positionally: both sides must come from the same
FrameGrid (same duration/window/step), so frame `i` in one file corresponds
to frame `i` in the other. A length mismatch is reported as a hard failure
rather than a best-effort alignment, since it usually means the FrameGrid
itself disagrees, which is the more important bug.

The `voice` measure (case `voice-report-defaults`) carries a `report`
scalar object instead of `frames` -- it is a span-level aggregate, not a
per-frame track -- compared field-by-field instead of positionally:

    {
      "case": "voice-report-defaults",
      "measure": "voice",
      "audio": "tests/fixtures/audio/....wav",
      "params": {...},
      "span": {"start": ..., "end": ...},
      "report": {
        "pitch": {"mean_hz": ..., "median_hz": ..., "min_hz": ..., "max_hz": ...},
        "jitter": {"local": ..., "rap": ..., "ppq5": ..., "ddp": ...},
        "shimmer": {"local": ..., "apq3": ..., "apq5": ..., "apq11": ..., "dda": ...},
        "mean_hnr_db": ...
      }
    }

Gross errors (octave/voicing-class disagreement) are separated from fine
errors (small numeric deviation where both sides already agree on
voicing/octave), per the GPE/MFPE methodology
(`docs/research/algorithms-and-validation.md` §7.3). Both are part of the
pass/fail decision: `oracle.tolerances` holds the specific per-track budgets
`docs/plan/gates.md`'s phase gate reviews accepted with documentation (a
handful of boundary-frame fine violations, zero gross errors, a bounded
number of out-of-band intensity frames, a widened jitter/shimmer band on one
named running-speech fixture, ...); anything past those recorded, cited
bands fails the run rather than being routed to a notes/counter field the
decision ignores.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from oracle import tolerances as tol


@dataclass
class DiffReport:
    measure: str
    passed: bool
    summary: dict[str, Any]
    violations: list[dict[str, Any]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "measure": self.measure,
            "passed": self.passed,
            "summary": self.summary,
            "violations": self.violations,
            "notes": self.notes,
        }


class DiffError(Exception):
    """Structural problem (measure mismatch, frame-count mismatch, ...)."""


def _require_same_measure(reference: dict, measured: dict) -> str:
    r_measure = reference.get("measure")
    m_measure = measured.get("measure")
    if r_measure != m_measure:
        raise DiffError(
            f"measure mismatch: reference is {r_measure!r}, measured is {m_measure!r}"
        )
    return r_measure


def _require_same_length(reference_frames: list, measured_frames: list) -> None:
    if len(reference_frames) != len(measured_frames):
        raise DiffError(
            "frame count mismatch: reference has "
            f"{len(reference_frames)} frames, measured has {len(measured_frames)} "
            "-- check that both sides used the same FrameGrid (duration/window/step)"
        )


def diff_pitch(reference: dict, measured: dict) -> DiffReport:
    ref_frames = reference["frames"]
    meas_frames = measured["frames"]
    _require_same_length(ref_frames, meas_frames)

    both_unvoiced = 0
    voicing_mismatches: list[dict] = []
    gross_errors: list[dict] = []
    fine_violations: list[dict] = []
    fine_checked = 0
    voiced_total = 0
    voicing_agree = 0

    for i, (r, m) in enumerate(zip(ref_frames, meas_frames)):
        r_voiced, m_voiced = r["voiced"], m["voiced"]
        voiced_total += 1
        if r_voiced == m_voiced:
            voicing_agree += 1
        else:
            voicing_mismatches.append(
                {"frame": i, "time": r["time"], "reference_voiced": r_voiced, "measured_voiced": m_voiced}
            )
            continue
        if not r_voiced and not m_voiced:
            both_unvoiced += 1
            continue

        r_f0, m_f0 = r["f0_hz"], m["f0_hz"]
        rel = abs(m_f0 - r_f0) / r_f0
        if rel > tol.F0_GROSS_RELATIVE:
            gross_errors.append(
                {"frame": i, "time": r["time"], "reference_f0_hz": r_f0, "measured_f0_hz": m_f0, "relative_diff": rel}
            )
            continue

        fine_checked += 1
        if rel > tol.F0_FINE_RELATIVE:
            fine_violations.append(
                {
                    "frame": i,
                    "time": r["time"],
                    "reference_f0_hz": r_f0,
                    "measured_f0_hz": m_f0,
                    "relative_diff": rel,
                    "tolerance": tol.F0_FINE_RELATIVE,
                }
            )

    voicing_agreement_rate = voicing_agree / voiced_total if voiced_total else 1.0
    fine_violation_rate = len(fine_violations) / fine_checked if fine_checked else 0.0
    max_fine_violation_relative = max(
        (v["relative_diff"] for v in fine_violations), default=0.0
    )
    # Per docs/plan/gates.md T2.6: gross errors, fine-violation rate, and
    # fine-violation magnitude are all part of the pass/fail decision, not
    # just voicing agreement -- see oracle.tolerances for the accepted bands.
    passed = (
        len(gross_errors) <= tol.F0_GROSS_ERROR_MAX
        and voicing_agreement_rate >= tol.VOICING_MAJORITY_THRESHOLD
        and fine_violation_rate <= tol.F0_FINE_VIOLATION_RATE_MAX
        and max_fine_violation_relative <= tol.F0_FINE_VIOLATION_MAX_RELATIVE
    )

    return DiffReport(
        measure="pitch",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "both_unvoiced": both_unvoiced,
            "voicing_mismatches": len(voicing_mismatches),
            "voicing_agreement_rate": voicing_agreement_rate,
            "voicing_majority_threshold": tol.VOICING_MAJORITY_THRESHOLD,
            "gross_errors": len(gross_errors),
            "gross_errors_max": tol.F0_GROSS_ERROR_MAX,
            "fine_checked": fine_checked,
            "fine_violations": len(fine_violations),
            "fine_violation_rate": fine_violation_rate,
            "fine_violation_rate_max": tol.F0_FINE_VIOLATION_RATE_MAX,
            "max_fine_violation_relative": max_fine_violation_relative,
            "fine_violation_max_relative": tol.F0_FINE_VIOLATION_MAX_RELATIVE,
            "tolerance_relative": tol.F0_FINE_RELATIVE,
        },
        violations=fine_violations,
        notes=[f"voicing mismatch at frame {v['frame']} (t={v['time']})" for v in voicing_mismatches]
        + [f"gross F0 error (octave/voicing class) at frame {g['frame']} (t={g['time']})" for g in gross_errors],
    )


def diff_formant(reference: dict, measured: dict) -> DiffReport:
    ref_frames = reference["frames"]
    meas_frames = measured["frames"]
    _require_same_length(ref_frames, meas_frames)

    checked = 0
    missing: list[dict] = []
    violations: list[dict] = []

    for i, (r, m) in enumerate(zip(ref_frames, meas_frames)):
        r_by_slot = {p["formant"]: p for p in r["formants"]}
        m_by_slot = {p["formant"]: p for p in m["formants"]}
        for slot in tol.FORMANT_TRACKED_SLOTS:
            r_p, m_p = r_by_slot.get(slot), m_by_slot.get(slot)
            if r_p is None or m_p is None:
                if r_p is not None or m_p is not None:
                    missing.append({"frame": i, "time": r["time"], "formant": slot})
                continue
            checked += 1
            diff_hz = abs(m_p["frequency_hz"] - r_p["frequency_hz"])
            band = max(tol.FORMANT_ABSOLUTE_HZ, tol.FORMANT_RELATIVE * r_p["frequency_hz"])
            if diff_hz > band:
                violations.append(
                    {
                        "frame": i,
                        "time": r["time"],
                        "formant": slot,
                        "reference_hz": r_p["frequency_hz"],
                        "measured_hz": m_p["frequency_hz"],
                        "diff_hz": diff_hz,
                        "tolerance_hz": band,
                    }
                )

    violation_rate = len(violations) / checked if checked else 0.0
    # Per docs/plan/gates.md T2.6: missing tracked slots fail outright (never
    # part of the accepted residual); the violation rate is checked against
    # the corpus-aggregate band recorded there. This per-file check alone is
    # stricter than that aggregate on some fixtures -- `oracle diff-all`
    # additionally aggregates checked/violations across the whole formant
    # corpus and gates on that number, matching how the record was measured.
    passed = (
        len(missing) <= tol.FORMANT_MISSING_MAX
        and violation_rate <= tol.FORMANT_CORPUS_VIOLATION_RATE_MAX
    )
    return DiffReport(
        measure="formant",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "checked_points": checked,
            "missing_points": len(missing),
            "missing_points_max": tol.FORMANT_MISSING_MAX,
            "violations": len(violations),
            "violation_rate": violation_rate,
            "violation_rate_max": tol.FORMANT_CORPUS_VIOLATION_RATE_MAX,
            "tolerance": f"max({tol.FORMANT_ABSOLUTE_HZ} Hz, {tol.FORMANT_RELATIVE:.0%})",
        },
        violations=violations,
        notes=[
            f"formant slot present on only one side at frame {v['frame']} (t={v['time']})"
            for v in missing
        ],
    )


def diff_intensity(reference: dict, measured: dict) -> DiffReport:
    ref_frames = reference["frames"]
    meas_frames = measured["frames"]
    _require_same_length(ref_frames, meas_frames)

    checked = 0
    null_mismatches = 0
    violations: list[dict] = []

    for i, (r, m) in enumerate(zip(ref_frames, meas_frames)):
        r_db, m_db = r["db"], m["db"]
        if r_db is None or m_db is None:
            if r_db != m_db:
                null_mismatches += 1
            continue
        checked += 1
        diff_db = abs(m_db - r_db)
        if diff_db > tol.INTENSITY_ABSOLUTE_DB:
            violations.append(
                {
                    "frame": i,
                    "time": r["time"],
                    "reference_db": r_db,
                    "measured_db": m_db,
                    "diff_db": diff_db,
                    "tolerance_db": tol.INTENSITY_ABSOLUTE_DB,
                }
            )

    max_violation_db = max((v["diff_db"] for v in violations), default=0.0)
    # Per docs/plan/gates.md T2.6: a null-vs-value mismatch fails outright
    # (never part of the accepted residual); out-of-band frames are capped
    # both by count and by how far any one of them may drift, matching the
    # recorded "7 frames on one fixture (max 3.5 dB)".
    passed = (
        null_mismatches <= tol.INTENSITY_NULL_MISMATCH_MAX
        and len(violations) <= tol.INTENSITY_MAX_VIOLATION_FRAMES
        and max_violation_db <= tol.INTENSITY_MAX_VIOLATION_DB
    )
    return DiffReport(
        measure="intensity",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "checked": checked,
            "null_mismatches": null_mismatches,
            "null_mismatches_max": tol.INTENSITY_NULL_MISMATCH_MAX,
            "violations": len(violations),
            "violations_max": tol.INTENSITY_MAX_VIOLATION_FRAMES,
            "max_violation_db": max_violation_db,
            "max_violation_db_max": tol.INTENSITY_MAX_VIOLATION_DB,
            "tolerance_db": tol.INTENSITY_ABSOLUTE_DB,
        },
        violations=violations,
        notes=[
            f"null-vs-value mismatch at frame {i} (t={r['time']})"
            for i, (r, m) in enumerate(zip(ref_frames, meas_frames))
            if (r["db"] is None) != (m["db"] is None)
        ],
    )


_VOICE_F0_FIELDS = ("mean_hz", "median_hz", "min_hz", "max_hz")
_VOICE_JITTER_FIELDS = ("local", "rap", "ppq5", "ddp")
_VOICE_SHIMMER_FIELDS = ("local", "apq3", "apq5", "apq11", "dda")


def _compare_voice_scalar(
    label: str,
    reference_value: float | None,
    measured_value: float | None,
    tolerance: float,
    violations: list[dict],
    notes: list[str],
) -> None:
    """Relative-tolerance comparison for one voice-report scalar field.

    A null on one side with a value on the other is a structural
    disagreement, not a numeric one -- per docs/plan/gates.md, it was never
    part of the accepted residual on any fixture, so it is recorded as a
    violation (not just a note) and fails the run.
    """
    if reference_value is None or measured_value is None:
        if reference_value != measured_value:
            notes.append(
                f"{label}: one side is null (reference={reference_value}, "
                f"measured={measured_value})"
            )
            violations.append(
                {
                    "field": label,
                    "reference": reference_value,
                    "measured": measured_value,
                    "issue": "null-mismatch",
                }
            )
        return
    denominator = abs(reference_value) if reference_value != 0 else 1.0
    relative_diff = abs(measured_value - reference_value) / denominator
    if relative_diff > tolerance:
        violations.append(
            {
                "field": label,
                "reference": reference_value,
                "measured": measured_value,
                "relative_diff": relative_diff,
                "tolerance": tolerance,
            }
        )


def diff_voice(reference: dict, measured: dict) -> DiffReport:
    ref_report = reference["report"]
    meas_report = measured["report"]

    # T4.6: the jitter/shimmer band widens only for the documented
    # running-speech fixture; every other fixture (both sustained-vowel
    # cases) keeps the tight 10% band, matching "both sustained-vowel cases
    # pass 0/14".
    audio_name = reference.get("audio", "")
    is_running_speech = audio_name.endswith(tol.VOICE_RUNNING_SPEECH_AUDIO)
    jitter_shimmer_tolerance = (
        tol.VOICE_RUNNING_SPEECH_JITTER_SHIMMER_RELATIVE
        if is_running_speech
        else tol.JITTER_SHIMMER_RELATIVE
    )

    violations: list[dict] = []
    notes: list[str] = []
    checked = 0

    for field in _VOICE_F0_FIELDS:
        checked += 1
        _compare_voice_scalar(
            f"pitch.{field}",
            ref_report["pitch"].get(field),
            meas_report["pitch"].get(field),
            tol.VOICE_F0_RELATIVE,
            violations,
            notes,
        )

    for field in _VOICE_JITTER_FIELDS:
        checked += 1
        _compare_voice_scalar(
            f"jitter.{field}",
            ref_report["jitter"].get(field),
            meas_report["jitter"].get(field),
            jitter_shimmer_tolerance,
            violations,
            notes,
        )

    for field in _VOICE_SHIMMER_FIELDS:
        checked += 1
        _compare_voice_scalar(
            f"shimmer.{field}",
            ref_report["shimmer"].get(field),
            meas_report["shimmer"].get(field),
            jitter_shimmer_tolerance,
            violations,
            notes,
        )

    checked += 1
    r_hnr = ref_report.get("mean_hnr_db")
    m_hnr = meas_report.get("mean_hnr_db")
    if r_hnr is None or m_hnr is None:
        if r_hnr != m_hnr:
            notes.append(
                f"mean_hnr_db: one side is null (reference={r_hnr}, measured={m_hnr})"
            )
            violations.append(
                {
                    "field": "mean_hnr_db",
                    "reference": r_hnr,
                    "measured": m_hnr,
                    "issue": "null-mismatch",
                }
            )
    else:
        diff_db = abs(m_hnr - r_hnr)
        if diff_db > tol.VOICE_HNR_ABSOLUTE_DB:
            violations.append(
                {
                    "field": "mean_hnr_db",
                    "reference": r_hnr,
                    "measured": m_hnr,
                    "diff_db": diff_db,
                    "tolerance_db": tol.VOICE_HNR_ABSOLUTE_DB,
                }
            )

    null_mismatches = sum(1 for v in violations if v.get("issue") == "null-mismatch")
    # Per docs/plan/gates.md T4.6: a null-vs-value mismatch fails outright on
    # every fixture; numeric violations already fail via `not violations`
    # since a violation only exists past its (fixture-aware) tolerance band.
    passed = not violations and null_mismatches <= tol.VOICE_NULL_MISMATCH_MAX
    return DiffReport(
        measure="voice",
        passed=passed,
        summary={
            "checked_scalars": checked,
            "violations": len(violations),
            "running_speech_fixture": is_running_speech,
            "jitter_shimmer_tolerance_relative": jitter_shimmer_tolerance,
            "f0_tolerance_relative": tol.VOICE_F0_RELATIVE,
            "hnr_tolerance_db": tol.VOICE_HNR_ABSOLUTE_DB,
        },
        violations=violations,
        notes=notes,
    )


_DIFFERS = {
    "pitch": diff_pitch,
    "formant": diff_formant,
    "intensity": diff_intensity,
    "voice": diff_voice,
}


def diff(reference: dict, measured: dict) -> DiffReport:
    measure = _require_same_measure(reference, measured)
    if measure == "spectrogram":
        raise DiffError(
            "spectrogram is not oracle-diffed (validation.md: validated against "
            "a scipy STFT reference instead, not Praat parity)"
        )
    differ = _DIFFERS.get(measure)
    if differ is None:
        raise DiffError(f"no comparator registered for measure {measure!r}")
    return differ(reference, measured)
