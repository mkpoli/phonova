#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "soundfile",
# ]
# ///
"""Synthesize a sustained vowel-like signal via source-filter formant synthesis.

Source: a glottal pulse train at a fixed f0. Filter: a cascade of four
second-order resonators set to the average adult-male formant frequencies
Peterson & Barney (1952) report for /a/ (F1 730 Hz, F2 1090 Hz, F3 2440 Hz,
F4 3400 Hz; bandwidths per the Fant (1960) source-filter convention
BW = 50 + F / 10 Hz). No external audio or proprietary code is used — this
is a from-scratch textbook resonator cascade.

Output: 16-bit PCM WAV, 16 kHz, mono, 2 seconds.
"""

import numpy as np
import soundfile as sf

SEED = 20260716
SAMPLE_RATE = 16_000
DURATION_S = 2.0
F0_HZ = 110.0  # sustained pitch, typical adult male

# Average /a/ formants and bandwidths (Peterson & Barney 1952; Fant 1960).
FORMANTS_HZ = [730.0, 1090.0, 2440.0, 3400.0]
BANDWIDTHS_HZ = [f / 10.0 + 50.0 for f in FORMANTS_HZ]


def glottal_pulse_train(n_samples: int, f0: float, fs: float, rng: np.random.Generator) -> np.ndarray:
    """Impulse train at f0 with small period jitter (shimmer/jitter) for realism."""
    x = np.zeros(n_samples)
    period = fs / f0
    pos = 0.0
    while pos < n_samples:
        idx = int(round(pos))
        if idx < n_samples:
            x[idx] = 1.0 + 0.02 * rng.standard_normal()
        pos += period * (1.0 + 0.01 * rng.standard_normal())
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
    rng = np.random.default_rng(SEED)
    n_samples = int(SAMPLE_RATE * DURATION_S)

    source = glottal_pulse_train(n_samples, F0_HZ, SAMPLE_RATE, rng)

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

    out_path = __file__.replace("scripts/synth_vowel.py", "audio/synth_vowel_a.wav")
    sf.write(out_path, signal.astype(np.float32), SAMPLE_RATE, subtype="PCM_16")
    print(f"wrote {out_path} ({len(signal) / SAMPLE_RATE:.2f}s, seed={SEED})")


if __name__ == "__main__":
    main()
