"""Frame extraction from parselmouth analysis objects.

Every function here takes a `parselmouth.Sound` and one of the parameter
dataclasses from `oracle.params`, calls the matching parselmouth method with
Praat's own keyword argument names (confirmed against the installed
`praat-parselmouth` package's public method signatures — see
`docs/plan/validation.md`), and returns plain Python data (no numpy types,
no NaN) ready for `oracle.jsonio`.
"""

from __future__ import annotations

import math
from typing import Any

from oracle.params import FormantParams, IntensityParams, PitchParams


def _clean(value: float) -> float | None:
    """NaN/inf from Praat means "no value at this frame" -> JSON null."""
    if value is None:
        return None
    if math.isnan(value) or math.isinf(value):
        return None
    return float(value)


def pitch_frames(sound: Any, params: PitchParams) -> list[dict]:
    """Per-frame F0 (Hz) and candidate strength via `Sound.to_pitch_ac`.

    A frequency of 0.0 in Praat's selected-candidate array means the frame's
    winning candidate is the unvoiced one; that is reported as
    `f0: null, voiced: false` here.
    """
    pitch = sound.to_pitch_ac(
        time_step=params.time_step,
        pitch_floor=params.floor_hz,
        max_number_of_candidates=params.max_candidates,
        very_accurate=params.very_accurate,
        silence_threshold=params.silence_threshold,
        voicing_threshold=params.voicing_threshold,
        octave_cost=params.octave_cost,
        octave_jump_cost=params.octave_jump_cost,
        voiced_unvoiced_cost=params.voiced_unvoiced_cost,
        pitch_ceiling=params.ceiling_hz,
    )
    times = pitch.xs()
    selected = pitch.selected_array
    frames = []
    for t, (freq, strength) in zip(times, selected):
        voiced = freq > 0.0
        frames.append(
            {
                "time": float(t),
                "voiced": bool(voiced),
                "f0_hz": float(freq) if voiced else None,
                "strength": _clean(float(strength)),
            }
        )
    return frames


def formant_frames(sound: Any, params: FormantParams) -> list[dict]:
    """Per-frame formant frequency/bandwidth pairs via `Sound.to_formant_burg`.

    Queries each formant slot at its own frame's center time; linear
    interpolation at an exact frame time returns that frame's own value, so
    this matches an exact (non-interpolated) per-frame read.
    """
    formant = sound.to_formant_burg(
        time_step=params.time_step,
        max_number_of_formants=float(params.max_formants),
        maximum_formant=params.ceiling_hz,
        window_length=params.window_length,
        pre_emphasis_from=params.preemphasis_from_hz,
    )
    times = formant.xs()
    frames = []
    for t in times:
        points = []
        for slot in range(1, params.max_formants + 1):
            freq = _clean(formant.get_value_at_time(slot, t))
            bw = _clean(formant.get_bandwidth_at_time(slot, t))
            if freq is None:
                continue
            points.append({"formant": slot, "frequency_hz": freq, "bandwidth_hz": bw})
        frames.append({"time": float(t), "formants": points})
    return frames


def intensity_frames(sound: Any, params: IntensityParams) -> list[dict]:
    """Per-frame dB SPL via `Sound.to_intensity`."""
    intensity = sound.to_intensity(
        minimum_pitch=params.pitch_floor_hz,
        time_step=params.time_step,
        subtract_mean=params.subtract_mean,
    )
    times = intensity.xs()
    values = intensity.values[0]
    frames = []
    for t, db in zip(times, values):
        frames.append({"time": float(t), "db": _clean(float(db))})
    return frames


# Jitter/shimmer boundary arguments below are Praat's own "Get jitter/
# shimmer (...)..." dialog defaults (manual pages "Voice 2. Jitter",
# "Voice 3. Shimmer"): shortest/longest accepted period 0.1-20 ms, maximum
# period factor 1.3, maximum amplitude factor 1.6. `phx_voice` has no
# equivalent knob at the jitter/shimmer-formula stage -- its period-ratio
# gate (`PulseParams::min_period_factor`/`max_period_factor`, 0.6-1.6) runs
# earlier, during pulse extraction -- so these stay Praat-only constants,
# recorded here for reproducibility rather than mirrored into a Rust struct.
_JITTER_SHORTEST_PERIOD = 0.0001
_JITTER_LONGEST_PERIOD = 0.02
_JITTER_MAX_PERIOD_FACTOR = 1.3
_SHIMMER_MAX_AMPLITUDE_FACTOR = 1.6

# Harmonicity defaults match `phx_voice::HarmonicityParams::default()`
# exactly (silence threshold, periods per window) except `minimum_pitch`,
# which follows the case's own `PitchParams.floor_hz`, and `time_step`,
# which follows `PitchParams.time_step` when set.
_HNR_SILENCE_THRESHOLD = 0.1
_HNR_PERIODS_PER_WINDOW = 4.5
_HNR_DEFAULT_TIME_STEP = 0.01


def voice_report(sound: Any, params: PitchParams, span: tuple[float, float]) -> dict:
    """Scalar voice-report measures over `span`, via Praat's PointProcess/
    Pitch/Harmonicity "Get ..." commands -- documented public API reached
    through `parselmouth.praat.call` -- matching the comparable scalar
    surface of `phx_voice::voice_report`.
    """
    from parselmouth.praat import call

    start, end = span
    point_process = call(
        sound, "To PointProcess (periodic, cc)", params.floor_hz, params.ceiling_hz
    )

    def jitter_get(command: str) -> float | None:
        return _clean(
            call(
                point_process,
                f"Get jitter ({command})",
                start,
                end,
                _JITTER_SHORTEST_PERIOD,
                _JITTER_LONGEST_PERIOD,
                _JITTER_MAX_PERIOD_FACTOR,
            )
        )

    def shimmer_get(command: str) -> float | None:
        return _clean(
            call(
                [sound, point_process],
                f"Get shimmer ({command})",
                start,
                end,
                _JITTER_SHORTEST_PERIOD,
                _JITTER_LONGEST_PERIOD,
                _JITTER_MAX_PERIOD_FACTOR,
                _SHIMMER_MAX_AMPLITUDE_FACTOR,
            )
        )

    jitter = {
        "local": jitter_get("local"),
        "rap": jitter_get("rap"),
        "ppq5": jitter_get("ppq5"),
        "ddp": jitter_get("ddp"),
    }
    shimmer = {
        "local": shimmer_get("local"),
        "apq3": shimmer_get("apq3"),
        "apq5": shimmer_get("apq5"),
        "apq11": shimmer_get("apq11"),
        "dda": shimmer_get("dda"),
    }

    harmonicity_time_step = (
        params.time_step if params.time_step is not None else _HNR_DEFAULT_TIME_STEP
    )
    harmonicity = sound.to_harmonicity_ac(
        time_step=harmonicity_time_step,
        minimum_pitch=params.floor_hz,
        silence_threshold=_HNR_SILENCE_THRESHOLD,
        periods_per_window=_HNR_PERIODS_PER_WINDOW,
    )
    mean_hnr_db = _clean(call(harmonicity, "Get mean", start, end))

    pitch = sound.to_pitch_ac(
        time_step=params.time_step,
        pitch_floor=params.floor_hz,
        max_number_of_candidates=params.max_candidates,
        very_accurate=params.very_accurate,
        silence_threshold=params.silence_threshold,
        voicing_threshold=params.voicing_threshold,
        octave_cost=params.octave_cost,
        octave_jump_cost=params.octave_jump_cost,
        voiced_unvoiced_cost=params.voiced_unvoiced_cost,
        pitch_ceiling=params.ceiling_hz,
    )
    pitch_summary = {
        "mean_hz": _clean(call(pitch, "Get mean", start, end, "Hertz")),
        "median_hz": _clean(call(pitch, "Get quantile", start, end, 0.5, "Hertz")),
        "min_hz": _clean(call(pitch, "Get minimum", start, end, "Hertz", "Parabolic")),
        "max_hz": _clean(call(pitch, "Get maximum", start, end, "Hertz", "Parabolic")),
    }

    return {
        "pitch": pitch_summary,
        "jitter": jitter,
        "shimmer": shimmer,
        "mean_hnr_db": mean_hnr_db,
    }


def spectrogram_slice(sound: Any, at: float) -> dict:
    """One power spectrum slice (Pa^2/Hz) nearest to `at`, via `to_spectrogram`.

    Not oracle-diffed against Praat (validation.md: spectrogram is validated
    against a scipy STFT reference instead); this exists so the harness can
    dump a slice for manual/scipy comparison, not for `oracle diff`.
    """
    spec = sound.to_spectrogram(
        window_length=0.005,
        maximum_frequency=5000.0,
        time_step=0.002,
        frequency_step=20.0,
    )
    times = spec.xs()
    freqs = spec.ys()
    idx = min(range(len(times)), key=lambda i: abs(times[i] - at))
    column = spec.values[:, idx]
    return {
        "time": float(times[idx]),
        "frequencies_hz": [float(f) for f in freqs],
        "power_pa2_per_hz": [float(p) for p in column],
    }
