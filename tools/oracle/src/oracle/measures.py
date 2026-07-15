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
