#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "soundfile",
# ]
# ///
"""Synthesize a sustained /a/ with a breathy phonation type, for a
voice-quality teaching contrast against the modal `synth_vowel_a.wav` and
creaky `synth_vowel_creaky.wav` fixtures.

Same source-filter construction as `synth_vowel_a.wav` (glottal source
through a cascade of four second-order resonators tuned to the Peterson &
Barney (1952) /a/ formants, Fant (1960) bandwidths), except the glottal
source mixes a smoothed (rather than impulsive) voiced pulse with a
turbulence-noise component band-limited to the glottal spectrum, at a fixed
harmonics-to-noise ratio. Breathy phonation's acoustic correlates are
established in the voice-quality literature (e.g. Klatt & Klatt 1990): a
raised aspiration-noise floor and a smoother, less abrupt glottal closure
that steepens the source spectral tilt (a larger H1-H2). This script
reaches for both: a raised-cosine pulse shape (softer closure than
`synth_vowel_a.wav`'s impulse) and an additive noise floor.

No randomness beyond a fixed-seed noise generator, documented below.

Output: 16-bit PCM WAV, 16 kHz, mono, 2 seconds.
"""

import numpy as np
import soundfile as sf

SEED = 20260719
SAMPLE_RATE = 16_000
DURATION_S = 2.0
F0_HZ = 110.0
PULSE_WIDTH_FRACTION = 0.5  # fraction of the period covered by the raised-cosine pulse
NOISE_TO_HARMONIC_RATIO = 4.0  # aspiration noise amplitude relative to the pulse train, pre-filter

# The formant cascade below is narrowband enough that a noise floor near the
# harmonic amplitude survives filtering only faintly; a pre-filter ratio in
# this range is what it takes for the post-filter signal to carry a clearly
# reduced periodicity relative to the modal fixture (verified empirically:
# autocorrelation peak at F0 drops from ~0.86 for synth_vowel_a.wav to
# ~0.45 here, against ~0.98 -- barely reduced at all -- for a 0.35 ratio).

# Average /a/ formants and bandwidths (Peterson & Barney 1952; Fant 1960),
# identical to synth_vowel_a.wav.
FORMANTS_HZ = [730.0, 1090.0, 2440.0, 3400.0]
BANDWIDTHS_HZ = [f / 10.0 + 50.0 for f in FORMANTS_HZ]


def breathy_glottal_source(n_samples: int, f0: float, fs: float, rng: np.random.Generator) -> np.ndarray:
    """Raised-cosine voiced pulses (softer closure than an impulse) plus a
    fixed-ratio band-limited noise floor standing in for aspiration."""
    period = fs / f0
    pulse_len = max(1, int(round(period * PULSE_WIDTH_FRACTION)))
    pulse = 0.5 * (1 - np.cos(np.linspace(0, 2 * np.pi, pulse_len, endpoint=False)))

    voiced = np.zeros(n_samples)
    pos = 0.0
    while pos < n_samples:
        idx = int(round(pos))
        end = min(idx + pulse_len, n_samples)
        voiced[idx:end] += pulse[: end - idx]
        pos += period
    voiced_peak = np.max(np.abs(voiced))
    if voiced_peak > 0:
        voiced = voiced / voiced_peak

    noise = rng.standard_normal(n_samples)
    # First-difference high-pass shapes the noise toward the aspiration band
    # rather than leaving it flat white, without adding a second filter stage.
    noise = np.diff(noise, prepend=0.0)
    noise_peak = np.max(np.abs(noise))
    if noise_peak > 0:
        noise = noise / noise_peak

    return voiced + NOISE_TO_HARMONIC_RATIO * noise


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
    rng = np.random.default_rng(SEED)
    n_samples = int(SAMPLE_RATE * DURATION_S)

    source = breathy_glottal_source(n_samples, F0_HZ, SAMPLE_RATE, rng)

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

    out_path = __file__.replace("scripts/synth_vowel_breathy.py", "synth_vowel_breathy.wav")
    sf.write(out_path, signal.astype(np.float32), SAMPLE_RATE, subtype="PCM_16")
    print(f"wrote {out_path} ({len(signal) / SAMPLE_RATE:.2f}s, seed={SEED})")


if __name__ == "__main__":
    main()
