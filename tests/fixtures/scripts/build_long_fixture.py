#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "numpy",
#   "soundfile",
# ]
# ///
"""Build the ~10-minute scroll-test fixture by looping the short clips.

Concatenates the CMU ARCTIC, LibriSpeech, synthetic vowel, and tone-sweep
clips already present in tests/fixtures/audio/, separated by silence,
repeating the sequence until the total reaches 600 seconds, then trims to
exactly 600 seconds with a short fade-out. All source clips are 16 kHz
mono 16-bit PCM, so no resampling is needed.

Run synth_vowel.py and synth_sweep.py first (or after; this script skips
files it cannot find on the first pass and only requires the CMU ARCTIC
and LibriSpeech clips plus its own prior output to already exist... in
practice: run in the order vowel, sweep, then this script).

Output: 16-bit PCM WAV, 16 kHz, mono, 600 seconds.
"""

from pathlib import Path

import numpy as np
import soundfile as sf

TARGET_S = 600.0
GAP_S = 0.5
LOOP_GAP_S = 1.0
SAMPLE_RATE = 16_000

AUDIO_DIR = Path(__file__).resolve().parent.parent / "audio"

CLIP_ORDER = [
    "arctic_bdl_a0001.wav",
    "arctic_slt_a0001.wav",
    "librispeech_2277-149896-0005.wav",
    "synth_vowel_a.wav",
    "synth_tone_sweep.wav",
]


def load_clip(name: str) -> np.ndarray:
    data, sr = sf.read(AUDIO_DIR / name, dtype="float32")
    if sr != SAMPLE_RATE:
        raise ValueError(f"{name}: expected {SAMPLE_RATE} Hz, got {sr}")
    if data.ndim > 1:
        data = data.mean(axis=1)
    return data


def main() -> None:
    clips = [load_clip(name) for name in CLIP_ORDER]
    gap = np.zeros(int(GAP_S * SAMPLE_RATE), dtype=np.float32)
    loop_gap = np.zeros(int(LOOP_GAP_S * SAMPLE_RATE), dtype=np.float32)

    sequence_parts = []
    for clip in clips:
        sequence_parts.append(clip)
        sequence_parts.append(gap)
    sequence = np.concatenate(sequence_parts)
    sequence_with_loop_gap = np.concatenate([sequence, loop_gap])

    target_samples = int(TARGET_S * SAMPLE_RATE)
    chunks = []
    total = 0
    while total < target_samples:
        chunks.append(sequence_with_loop_gap)
        total += len(sequence_with_loop_gap)
    full = np.concatenate(chunks)[:target_samples]

    fade_len = int(0.5 * SAMPLE_RATE)
    fade = np.linspace(1.0, 0.0, fade_len, dtype=np.float32)
    full[-fade_len:] *= fade

    out_path = AUDIO_DIR / "long_scroll_test.wav"
    sf.write(out_path, full, SAMPLE_RATE, subtype="PCM_16")
    size_mb = out_path.stat().st_size / 1e6
    print(f"wrote {out_path} ({len(full) / SAMPLE_RATE:.1f}s, {size_mb:.2f} MB)")


if __name__ == "__main__":
    main()
