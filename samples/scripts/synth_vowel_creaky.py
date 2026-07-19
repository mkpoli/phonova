#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "soundfile",
# ]
# ///
"""Synthesize a sustained /a/ with a creaky (vocal fry) phonation type, for
a voice-quality teaching contrast against the modal `synth_vowel_a.wav` and
breathy `synth_vowel_breathy.wav` fixtures.

Same source-filter construction as `synth_vowel_a.wav` (glottal impulse
train through a cascade of four second-order resonators tuned to the
Peterson & Barney (1952) /a/ formants, Fant (1960) bandwidths), but the
glottal source pulses in strong/weak alternating pairs at a low fundamental
-- the pulse-doubling ("diplophonic") pattern and low rate widely reported
as creaky voice's acoustic signature (e.g. Hollien & Michel 1968;
Blomgren et al. 1998). The pairing is an exact, non-random alternation
(every second pulse is attenuated to PAIR_RATIO of the first, and pushed
PAIR_SHIFT_FRACTION of a period closer to it) rather than a randomly
sampled irregularity, so the output is deterministic and reproducible.

Output: 16-bit PCM WAV, 16 kHz, mono, 2 seconds.
"""

import numpy as np
import soundfile as sf

SAMPLE_RATE = 16_000
DURATION_S = 2.0
PAIR_F0_HZ = 70.0  # fundamental of the strong/weak pulse pair, in the creaky range
PAIR_RATIO = 0.35  # weak pulse amplitude relative to the strong pulse
PAIR_SHIFT_FRACTION = 0.3  # weak pulse pulled this fraction of a period toward the strong one

# Average /a/ formants and bandwidths (Peterson & Barney 1952; Fant 1960),
# identical to synth_vowel_a.wav.
FORMANTS_HZ = [730.0, 1090.0, 2440.0, 3400.0]
BANDWIDTHS_HZ = [f / 10.0 + 50.0 for f in FORMANTS_HZ]


def creaky_glottal_source(n_samples: int, pair_f0: float, fs: float) -> np.ndarray:
    """Impulse train with an exact strong/weak pulse-pair pattern (vocal fry)."""
    x = np.zeros(n_samples)
    pair_period = fs / pair_f0
    pos = 0.0
    i = 0
    while pos < n_samples:
        if i % 2 == 0:
            idx = int(round(pos))
            amplitude = 1.0
        else:
            idx = int(round(pos - pair_period * PAIR_SHIFT_FRACTION))
            amplitude = PAIR_RATIO
        if 0 <= idx < n_samples:
            x[idx] = amplitude
        pos += pair_period
        i += 1
    return x


def resonator(x: np.ndarray, freq: float, bandwidth: float, fs: float) -> np.ndarray:
    """Second-order all-pole resonator, unity gain at its center frequency."""
    r = np.exp(-np.pi * bandwidth / fs)
    theta = 2.0 * np.pi * freq / fs
    a1 = 2.0 * r * np.cos(theta)
    a2 = -(r ** 2)
    gain = 1.0 - a1 - a2
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

    source = creaky_glottal_source(n_samples, PAIR_F0_HZ, SAMPLE_RATE)

    signal = source
    for freq, bw in zip(FORMANTS_HZ, BANDWIDTHS_HZ):
        signal = resonator(signal, freq, bw, SAMPLE_RATE)

    ramp_len = int(0.01 * SAMPLE_RATE)
    ramp = 0.5 * (1 - np.cos(np.linspace(0, np.pi, ramp_len)))
    signal[:ramp_len] *= ramp
    signal[-ramp_len:] *= ramp[::-1]

    peak = np.max(np.abs(signal))
    if peak > 0:
        signal = signal / peak * 0.8

    out_path = __file__.replace("scripts/synth_vowel_creaky.py", "synth_vowel_creaky.wav")
    sf.write(out_path, signal.astype(np.float32), SAMPLE_RATE, subtype="PCM_16")
    print(f"wrote {out_path} ({len(signal) / SAMPLE_RATE:.2f}s, pair_f0={PAIR_F0_HZ})")


if __name__ == "__main__":
    main()
