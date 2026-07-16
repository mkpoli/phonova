#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "soundfile",
# ]
# ///
"""Synthesize a sustained vowel with an exactly known, non-random jitter and
shimmer injection, for oracle ground-truth comparison (docs/plan/tasks/
phase-4.md T4.2, T4.5).

Same source-filter construction as `synth_vowel.py` (glottal impulse train
through the Peterson & Barney (1952) /a/ formant cascade, Fant (1960)
bandwidths), but the glottal source has no random component: pulse period
and pulse amplitude alternate by a fixed fraction every other pulse. Over an
alternating +-x/2 sequence, Praat's local-jitter/local-shimmer formula (mean
absolute difference between consecutive periods or amplitudes, divided by
the mean) reduces to x, so INJECTED_JITTER and INJECTED_SHIMMER below are
the closed-form local jitter and local shimmer of the glottal source itself.
The resonator cascade's ringing overlaps adjacent periods and measurably
reshapes both quantities in the radiated waveform (confirmed against
parselmouth: local jitter on the synthesized WAV comes out close to
INJECTED_JITTER, local shimmer measurably lower than INJECTED_SHIMMER) --
the constants below document the source recipe, not an assertion about the
post-filter measurement, which is exactly what the oracle case measures
empirically.

Output: 16-bit PCM WAV, 16 kHz, mono, 2 seconds.
"""

import numpy as np
import soundfile as sf

SAMPLE_RATE = 16_000
DURATION_S = 2.0
F0_HZ = 110.0
INJECTED_JITTER = 0.03  # alternating period +-1.5% -> local jitter = 3%
INJECTED_SHIMMER = 0.06  # alternating amplitude +-3% -> local shimmer = 6%

# Average /a/ formants and bandwidths (Peterson & Barney 1952; Fant 1960),
# identical to synth_vowel.py.
FORMANTS_HZ = [730.0, 1090.0, 2440.0, 3400.0]
BANDWIDTHS_HZ = [f / 10.0 + 50.0 for f in FORMANTS_HZ]


def glottal_pulse_train(n_samples: int, f0: float, fs: float) -> np.ndarray:
    """Impulse train with an exact alternating period/amplitude perturbation.

    No randomness: pulse `i`'s amplitude and the period leading to pulse
    `i + 1` both alternate sign every other pulse, giving a closed-form
    injected jitter/shimmer rather than a randomly sampled one.
    """
    x = np.zeros(n_samples)
    period = fs / f0
    pos = 0.0
    i = 0
    while pos < n_samples:
        idx = int(round(pos))
        sign = -1.0 if i % 2 == 0 else 1.0
        amplitude = 1.0 + sign * INJECTED_SHIMMER * 0.5
        if idx < n_samples:
            x[idx] = amplitude
        pos += period * (1.0 + sign * INJECTED_JITTER * 0.5)
        i += 1
    return x


def resonator(x: np.ndarray, freq: float, bandwidth: float, fs: float) -> np.ndarray:
    """Second-order all-pole resonator, unity gain at its center frequency."""
    r = np.exp(-np.pi * bandwidth / fs)
    theta = 2.0 * np.pi * freq / fs
    a1 = 2.0 * r * np.cos(theta)
    a2 = -(r ** 2)
    gain = 1.0 - a1 - a2  # DC-normalized; close enough near resonance for this fixture
    y = np.zeros_like(x)
    y_1 = 0.0
    y_2 = 0.0
    for n in range(len(x)):
        yn = gain * x[n] + a1 * y_1 + a2 * y_2
        y[n] = yn
        y_2 = y_1
        y_1 = yn
    return y


def main() -> None:
    n_samples = int(SAMPLE_RATE * DURATION_S)

    source = glottal_pulse_train(n_samples, F0_HZ, SAMPLE_RATE)

    signal = source
    for freq, bw in zip(FORMANTS_HZ, BANDWIDTHS_HZ):
        signal = resonator(signal, freq, bw, SAMPLE_RATE)

    # Short raised-cosine on/off ramp to avoid onset/offset clicks.
    ramp_len = int(0.01 * SAMPLE_RATE)
    ramp = 0.5 * (1 - np.cos(np.linspace(0, np.pi, ramp_len)))
    signal[:ramp_len] *= ramp
    signal[-ramp_len:] *= ramp[::-1]

    peak = np.max(np.abs(signal))
    if peak > 0:
        signal = signal / peak * 0.8

    out_path = __file__.replace("scripts/synth_vowel_perturbed.py", "audio/synth_vowel_perturbed.wav")
    sf.write(out_path, signal.astype(np.float32), SAMPLE_RATE, subtype="PCM_16")
    print(
        f"wrote {out_path} ({len(signal) / SAMPLE_RATE:.2f}s, "
        f"injected jitter={INJECTED_JITTER:.1%}, injected shimmer={INJECTED_SHIMMER:.1%})"
    )


if __name__ == "__main__":
    main()
