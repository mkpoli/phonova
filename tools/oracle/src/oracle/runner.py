"""Runs a case against one audio file via parselmouth and builds the payload.

Import of `parselmouth` is deferred to `load_parselmouth()` so every other
module here (params, cases, tolerances, diff, jsonio) stays usable --
including `oracle diff`, which never needs Praat installed at all -- when
the GPL dev dependency is absent.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from oracle import measures
from oracle.cases import CASES, Case
from oracle.paths import fixtures_audio_dir


class OracleUnavailable(Exception):
    """`praat-parselmouth` is not installed in this environment."""


def load_parselmouth():
    try:
        import parselmouth
    except ImportError as exc:
        raise OracleUnavailable(
            "praat-parselmouth is not installed -- install the 'parselmouth' "
            "extra (`uv sync --extra parselmouth`) or run with "
            "`uv run --extra parselmouth`. Skipping oracle generation/run: "
            "this is expected on platforms without a praat-parselmouth wheel."
        ) from exc
    return parselmouth


def versions(parselmouth_module) -> dict[str, str]:
    return {
        "parselmouth_version": parselmouth_module.__version__,
        "praat_version": parselmouth_module.PRAAT_VERSION,
        "praat_version_date": parselmouth_module.PRAAT_VERSION_DATE,
    }


_MEASURE_FN = {
    "pitch": measures.pitch_frames,
    "formant": measures.formant_frames,
    "intensity": measures.intensity_frames,
}


def run_case(case: Case, audio_filename: str, parselmouth_module=None) -> dict[str, Any]:
    """Run one case against one fixture audio file; returns the JSON payload."""
    if parselmouth_module is None:
        parselmouth_module = load_parselmouth()
    audio_path = fixtures_audio_dir() / audio_filename
    if not audio_path.is_file():
        raise FileNotFoundError(f"fixture audio not found: {audio_path}")

    sound = parselmouth_module.Sound(str(audio_path))

    payload: dict[str, Any] = {
        "case": case.name,
        "measure": case.measure,
        "audio": f"tests/fixtures/audio/{audio_filename}",
        **versions(parselmouth_module),
        "params": case.params.as_dict() if case.params is not None else None,
    }

    if case.measure == "spectrogram":
        payload["slice"] = measures.spectrogram_slice(sound, at=1.0)
        return payload

    frame_fn = _MEASURE_FN[case.measure]
    payload["frames"] = frame_fn(sound, case.params)
    return payload


def reference_filename(case_name: str, audio_filename: str) -> str:
    stem = Path(audio_filename).stem
    return f"{case_name}__{stem}.json"


def resolve_case(case_name: str) -> Case:
    try:
        return CASES[case_name]
    except KeyError as exc:
        known = ", ".join(sorted(CASES))
        raise KeyError(f"unknown case {case_name!r}; known cases: {known}") from exc
