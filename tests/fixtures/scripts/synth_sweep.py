#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "scipy",
#   "soundfile",
# ]
# ///
"""Synthesize a logarithmic tone sweep for spectrogram/FFT test fixtures.

Sweeps 50 Hz to 7500 Hz (below the 8 kHz Nyquist of a 16 kHz signal) over
5 seconds using a logarithmic chirp, so equal time spans cover equal
octaves — useful for checking log-frequency display and tile boundaries.

Output: 16-bit PCM WAV, 16 kHz, mono, 5 seconds.
"""

import numpy as np
import soundfile as sf
from scipy.signal import chirp

SAMPLE_RATE = 16_000
DURATION_S = 5.0
F_START_HZ = 50.0
F_END_HZ = 7500.0


def main() -> None:
    t = np.linspace(0, DURATION_S, int(SAMPLE_RATE * DURATION_S), endpoint=False)
    signal = chirp(t, f0=F_START_HZ, f1=F_END_HZ, t1=DURATION_S, method="logarithmic")

    ramp_len = int(0.01 * SAMPLE_RATE)
    ramp = 0.5 * (1 - np.cos(np.linspace(0, np.pi, ramp_len)))
    signal[:ramp_len] *= ramp
    signal[-ramp_len:] *= ramp[::-1]

    signal = signal.astype(np.float64) * 0.8

    out_path = __file__.replace("scripts/synth_sweep.py", "audio/synth_tone_sweep.wav")
    sf.write(out_path, signal.astype(np.float32), SAMPLE_RATE, subtype="PCM_16")
    print(f"wrote {out_path} ({DURATION_S:.2f}s, {F_START_HZ:.0f}-{F_END_HZ:.0f} Hz log sweep)")


if __name__ == "__main__":
    main()
