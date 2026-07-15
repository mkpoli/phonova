#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "scipy",
#   "soundfile",
# ]
# ///
"""Generate a small Gaussian STFT PSD reference fixture.

With no WAV argument, the script analyzes a deterministic 80 ms synthetic
signal. A WAV path can be supplied to analyze external audio with the same
parameters. Output is CSV so the Rust test can include it as text.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.fft import rfft


DEFAULT_SAMPLE_RATE = 16_000.0
DEFAULT_DURATION = 0.08
PSD_FLOOR = 1.0e-300


def rust_round(x: float) -> int:
    if x >= 0.0:
        return int(np.floor(x + 0.5))
    return int(np.ceil(x - 0.5))


def next_pow2(n: int) -> int:
    return 1 << max(0, n - 1).bit_length()


def frame_centers(duration: float, window: float, step: float) -> list[float]:
    if duration < window:
        return []
    count = int(np.floor((duration - window) / step)) + 1
    first = (duration - (count - 1) * step) / 2.0
    return [first + i * step for i in range(count)]


def gaussian_window(n: int, effective_len_factor: float = 2.0) -> np.ndarray:
    if n == 0:
        return np.array([], dtype=np.float64)
    if n == 1:
        return np.array([1.0], dtype=np.float64)
    last = float(n - 1)
    factor_sq = effective_len_factor * effective_len_factor
    edge = np.exp(-3.0 * factor_sq)
    denom = 1.0 - edge
    values = []
    for i in range(n):
        u = i / last - 0.5
        values.append((np.exp(-12.0 * factor_sq * u * u) - edge) / denom)
    return np.array(values, dtype=np.float64)


def synthetic_signal(sample_rate: float, duration: float) -> np.ndarray:
    frames = rust_round(sample_rate * duration)
    t = np.arange(frames, dtype=np.float64) / sample_rate
    envelope = 0.7 + 0.2 * np.cos(2.0 * np.pi * 7.0 * t)
    signal = (
        envelope * np.sin(2.0 * np.pi * 440.0 * t)
        + 0.3 * np.cos(2.0 * np.pi * 1230.0 * t + 0.2)
        + 0.05 * np.sin(2.0 * np.pi * 2750.0 * t)
    )
    return signal.astype(np.float32).astype(np.float64)


def mono_wav(path: Path) -> tuple[np.ndarray, float]:
    samples, sample_rate = sf.read(path, always_2d=True, dtype="float32")
    mono = samples.mean(axis=1).astype(np.float32).astype(np.float64)
    return mono, float(sample_rate)


def frequency_grid(
    sample_rate: float, fft_len: int, max_frequency: float, frequency_step: float
) -> tuple[list[int], list[float]]:
    nyquist = sample_rate / 2.0
    max_frequency = min(max_frequency, nyquist)
    fft_bin_hz = sample_rate / fft_len
    max_bin = fft_len // 2
    bins: list[int] = []
    frequencies: list[float] = []
    previous: int | None = None
    target = 0.0
    while target <= max_frequency + frequency_step * 1.0e-9:
        bin_index = min(rust_round(target / fft_bin_hz), max_bin)
        if bin_index != previous:
            frequency = bin_index * fft_bin_hz
            if frequency <= max_frequency + fft_bin_hz * 0.5:
                bins.append(bin_index)
                frequencies.append(frequency)
                previous = bin_index
        target += frequency_step
    return bins, frequencies


def stft_db(
    samples: np.ndarray,
    sample_rate: float,
    window_length: float,
    max_frequency: float,
    time_step: float,
    frequency_step: float,
) -> tuple[list[float], list[float], np.ndarray]:
    time_step = max(time_step, window_length / (8.0 * np.sqrt(np.pi)))
    frequency_step = max(frequency_step, np.sqrt(np.pi) / (8.0 * window_length))
    physical_window = 2.0 * window_length
    window_len = max(rust_round(physical_window * sample_rate) + 1, 1)
    min_fft_len = int(np.ceil(sample_rate / frequency_step)) + 1
    fft_len = next_pow2(max(window_len, min_fft_len))
    window = gaussian_window(window_len)
    window_energy = float(np.sum(window * window))
    bins, frequencies = frequency_grid(sample_rate, fft_len, max_frequency, frequency_step)
    duration = len(samples) / sample_rate
    centers = frame_centers(duration, window_length, time_step)

    db = np.empty((len(frequencies), len(centers)), dtype=np.float64)
    midpoint = (window_len - 1) / 2.0
    for t_index, center in enumerate(centers):
        frame = np.zeros(fft_len, dtype=np.float64)
        center_sample = center * sample_rate
        for i, weight in enumerate(window):
            sample_index = rust_round(center_sample + i - midpoint)
            if 0 <= sample_index < len(samples):
                frame[i] = samples[sample_index] * weight
        spectrum = rfft(frame)
        for f_index, bin_index in enumerate(bins):
            one_sided = 1.0 if bin_index == 0 or bin_index == fft_len // 2 else 2.0
            psd = one_sided * abs(spectrum[bin_index]) ** 2 / (sample_rate * window_energy)
            db[f_index, t_index] = 10.0 * np.log10(max(psd, PSD_FLOOR))
    return centers, frequencies, db


def write_fixture(path: Path, centers: list[float], frequencies: list[float], db: np.ndarray) -> None:
    selected_times = range(0, min(len(centers), 16))
    selected_freqs = range(0, min(len(frequencies), 27))
    with path.open("w", encoding="utf-8") as out:
        out.write("# generated_by=tools/oracle/spectrogram_reference.py\n")
        out.write("# signal=synthetic\n")
        out.write("# sample_rate=16000\n")
        out.write("# duration=0.08\n")
        out.write("# window_length=0.01\n")
        out.write("# max_frequency=3200\n")
        out.write("# time_step=0.004\n")
        out.write("# frequency_step=125\n")
        out.write("time_s,frequency_hz,db\n")
        for f_index in selected_freqs:
            for t_index in selected_times:
                out.write(
                    f"{centers[t_index]:.17g},"
                    f"{frequencies[f_index]:.17g},"
                    f"{db[f_index, t_index]:.17g}\n"
                )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("wav", nargs="?", type=Path)
    parser.add_argument("--window-length", type=float, default=0.01)
    parser.add_argument("--max-frequency", type=float, default=3200.0)
    parser.add_argument("--time-step", type=float, default=0.004)
    parser.add_argument("--frequency-step", type=float, default=125.0)
    parser.add_argument("--out", type=Path, default=Path(__file__).with_suffix(".csv"))
    args = parser.parse_args()

    if args.wav is None:
        samples = synthetic_signal(DEFAULT_SAMPLE_RATE, DEFAULT_DURATION)
        sample_rate = DEFAULT_SAMPLE_RATE
    else:
        samples, sample_rate = mono_wav(args.wav)

    centers, frequencies, db = stft_db(
        samples,
        sample_rate,
        args.window_length,
        args.max_frequency,
        args.time_step,
        args.frequency_step,
    )
    write_fixture(args.out, centers, frequencies, db)
    print(f"wrote {args.out} ({len(centers)} frames, {len(frequencies)} frequencies)")


if __name__ == "__main__":
    main()
