#!/usr/bin/env python3
"""Generate a Gaussian STFT PSD reference fixture on scipy.signal.ShortTimeFFT.

Run via the tools/oracle uv project (numpy, scipy, soundfile are project
dependencies, not a standalone script environment):

    uv run --project tools/oracle tools/oracle/spectrogram_reference.py

With no WAV argument, the script analyzes a deterministic 80 ms synthetic
signal. A WAV path can be supplied to analyze external audio with the same
parameters. Output is CSV so the Rust test can include it as text
(`crates/phx-spectrogram/src/lib.rs::scipy_oracle_fixture_matches_relative_tolerance`).

Independence from the Rust implementation: only two things are shared with
`crates/phx-dsp` and `crates/phx-spectrogram` by design, and both are grid
*selection* formulas, not measurement code —

- `gaussian_window` is the window shape itself, defined in Praat's manual
  ("Sound: To Spectrogram...") and restated in
  `docs/research/algorithms-and-validation.md` §3.2 (−3 dB bandwidth
  `1.2982804 / windowLength`) and in `crates/phx-dsp/src/window.rs`. The
  window is the spec being validated against, not an implementation detail;
  reusing its formula here is the same thing an independent Praat-manual
  reader would do.
- `frame_centers` / `frequency_grid` reproduce `phx_dsp::FrameGrid` and
  `phx-spectrogram`'s frequency-bin selection *only* to know which (t, f)
  coordinates to report — the coordinates a caller of `compute_tile` would
  also land on. They never touch how a PSD value is computed at those
  coordinates.

Everything else — extracting the windowed frame from the signal, running the
FFT, and scaling the result to a one-sided power spectral density — is done
by `scipy.signal.ShortTimeFFT` (`scale_to="psd"`, `fft_mode="onesided2X"`),
independently of the hand-rolled arithmetic in `phx-spectrogram`. A shared
conceptual bug in that arithmetic (rounding, normalisation constant, one-sided
factor) would no longer be invisible to this comparison.

Frame alignment. `ShortTimeFFT` places its p-th frame at continuous time
`t[p] = p * hop / fs`, window centred at `t[p]` using its own centering
convention `m_num_mid = m_num // 2`. `phx-spectrogram::Analysis::frame_db`
places a window sample `i` at `round(center * fs + i - (window_len - 1) / 2)`,
i.e. the same centred convention when `window_len` is odd (`m_num // 2 ==
(m_num - 1) / 2` for odd `m_num`); it differs by half a sample when
`window_len` is even. This script sets `hop = 1` sample, the finest possible
native scipy grid, and for each target `FrameGrid` center `c` selects the
single nearest native frame, `p = round(c * fs)` — by construction the
closest scipy can get on a 1-sample grid, so no cross-frame interpolation is
needed. For the committed fixture below, `window_len` is 321 (odd; see
`stft_db`), so nearest-frame selection lands exactly on the same sample
positions `phx-spectrogram` would extract and the two computations agree to
floating-point noise (see the module-level comment in the Rust test for
measured numbers). Callers who pass parameters that make `window_len` even
should expect up to half a sample of extra alignment error, undocumented
further here because the committed fixture never exercises that case.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import ShortTimeFFT

DEFAULT_SAMPLE_RATE = 16_000.0
DEFAULT_DURATION = 0.08
PSD_FLOOR = 1.0e-300


def rust_round(x: float) -> int:
    """`f64::round()` (half away from zero), not Python/NumPy's round-half-to-even."""
    if x >= 0.0:
        return int(np.floor(x + 0.5))
    return int(np.ceil(x - 0.5))


def next_pow2(n: int) -> int:
    return 1 << max(0, n - 1).bit_length()


def frame_centers(duration: float, window: float, step: float) -> list[float]:
    """Reproduces `phx_dsp::FrameGrid` — the reporting grid, not the measurement."""
    if duration < window:
        return []
    count = int(np.floor((duration - window) / step)) + 1
    first = (duration - (count - 1) * step) / 2.0
    return [first + i * step for i in range(count)]


def gaussian_window(n: int, effective_len_factor: float = 2.0) -> np.ndarray:
    """Praat's Gaussian analysis window (Praat manual "Sound: To Spectrogram...";
    docs/research/algorithms-and-validation.md §3.2; crates/phx-dsp/src/window.rs)."""
    if n == 0:
        return np.array([], dtype=np.float64)
    if n == 1:
        return np.array([1.0], dtype=np.float64)
    last = float(n - 1)
    factor_sq = effective_len_factor * effective_len_factor
    edge = np.exp(-3.0 * factor_sq)
    denom = 1.0 - edge
    u = np.arange(n, dtype=np.float64) / last - 0.5
    return (np.exp(-12.0 * factor_sq * u * u) - edge) / denom


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
    """Reproduces phx-spectrogram's bin selection — again a reporting grid."""
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
    """PSD in dB at the FrameGrid centers and frequency-grid bins, computed by
    scipy.signal.ShortTimeFFT (see module docstring for the alignment strategy
    and what is/isn't shared with the Rust implementation)."""
    time_step = max(time_step, window_length / (8.0 * np.sqrt(np.pi)))
    frequency_step = max(frequency_step, np.sqrt(np.pi) / (8.0 * window_length))
    physical_window = 2.0 * window_length
    window_len = max(rust_round(physical_window * sample_rate) + 1, 1)
    min_fft_len = int(np.ceil(sample_rate / frequency_step)) + 1
    fft_len = next_pow2(max(window_len, min_fft_len))
    window = gaussian_window(window_len)
    bins, frequencies = frequency_grid(sample_rate, fft_len, max_frequency, frequency_step)
    duration = len(samples) / sample_rate
    centers = frame_centers(duration, window_length, time_step)

    db = np.empty((len(frequencies), len(centers)), dtype=np.float64)
    if not centers or not frequencies:
        return centers, frequencies, db

    sft = ShortTimeFFT(
        win=window,
        hop=1,
        fs=sample_rate,
        mfft=fft_len,
        scale_to="psd",
        fft_mode="onesided2X",
    )
    p_min = sft.p_min
    p_max = sft.p_max(len(samples))

    for t_index, center in enumerate(centers):
        p = rust_round(center * sample_rate)
        if not (p_min <= p < p_max):
            raise ValueError(
                f"frame center {center}s (native index {p}) falls outside "
                f"scipy's valid native frame range [{p_min}, {p_max}); "
                "the requested parameters push the analysis window entirely "
                "off the signal"
            )
        spectrum = sft.stft(samples, p0=p, p1=p + 1)[:, 0]
        psd = np.abs(spectrum) ** 2
        for f_index, bin_index in enumerate(bins):
            db[f_index, t_index] = 10.0 * np.log10(max(psd[bin_index], PSD_FLOOR))
    return centers, frequencies, db


def write_fixture(path: Path, centers: list[float], frequencies: list[float], db: np.ndarray) -> None:
    selected_times = range(0, min(len(centers), 16))
    selected_freqs = range(0, min(len(frequencies), 27))
    with path.open("w", encoding="utf-8") as out:
        out.write("# generated_by=tools/oracle/spectrogram_reference.py\n")
        out.write("# method=scipy.signal.ShortTimeFFT(scale_to=psd, fft_mode=onesided2X)\n")
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
