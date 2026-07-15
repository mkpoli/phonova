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

Gross errors (octave/voicing-class disagreement) are separated from fine
errors (small numeric deviation where both sides already agree on
voicing/octave), per the GPE/MFPE methodology
(`docs/research/algorithms-and-validation.md` §7.3). Only fine-error
tolerance violations and sub-majority voicing agreement fail the run; gross
errors are listed for investigation, matching validation.md's framing of
them as "the expected disagreement mode".
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
    passed = not fine_violations and voicing_agreement_rate >= tol.VOICING_MAJORITY_THRESHOLD

    return DiffReport(
        measure="pitch",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "both_unvoiced": both_unvoiced,
            "voicing_mismatches": len(voicing_mismatches),
            "voicing_agreement_rate": voicing_agreement_rate,
            "gross_errors": len(gross_errors),
            "fine_checked": fine_checked,
            "fine_violations": len(fine_violations),
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

    passed = not violations
    return DiffReport(
        measure="formant",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "checked_points": checked,
            "missing_points": len(missing),
            "violations": len(violations),
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
    missing = 0
    violations: list[dict] = []

    for i, (r, m) in enumerate(zip(ref_frames, meas_frames)):
        r_db, m_db = r["db"], m["db"]
        if r_db is None or m_db is None:
            if r_db != m_db:
                missing += 1
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

    passed = not violations
    return DiffReport(
        measure="intensity",
        passed=passed,
        summary={
            "total_frames": len(ref_frames),
            "checked": checked,
            "missing": missing,
            "violations": len(violations),
            "tolerance_db": tol.INTENSITY_ABSOLUTE_DB,
        },
        violations=violations,
    )


_DIFFERS = {
    "pitch": diff_pitch,
    "formant": diff_formant,
    "intensity": diff_intensity,
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
