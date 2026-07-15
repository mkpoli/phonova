"""Write Praat-resampled copies of fixture WAVs for formant stage isolation.

Resamples each fixture to a target rate with Praat's own resampler (via
parselmouth, precision 50) and writes 32-bit float WAVs into an output
directory under the same filenames. Feeding these into the Rust formant path
(whose internal resample then becomes a no-op, source rate == target rate)
isolates whether the raw-Burg formant gap against the parselmouth reference
comes from the resampling stage: if the gap collapses on Praat-resampled
input, rubato's windowed-sinc transition band is the cause.

    uv run --extra parselmouth python resample_fixtures.py --target-hz 11000 --out-dir <DIR>

Diagnostic only; not part of the committed oracle reference flow.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import soundfile as sf

from oracle.cases import SPEECH_AND_VOWEL_CORPUS
from oracle.paths import fixtures_audio_dir
from oracle.runner import load_parselmouth


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-hz", type=float, default=11000.0)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--audio", action="append", help="repeatable; default: speech+vowel corpus")
    args = parser.parse_args()

    parselmouth = load_parselmouth()
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    audio_files = args.audio or list(SPEECH_AND_VOWEL_CORPUS)

    for name in audio_files:
        src = fixtures_audio_dir() / name
        sound = parselmouth.Sound(str(src))
        resampled = sound.resample(args.target_hz)  # precision 50 (Praat default)
        samples = np.asarray(resampled.values).reshape(-1)
        out_path = out_dir / name
        sf.write(out_path, samples, int(round(args.target_hz)), subtype="FLOAT")
        print(f"wrote {out_path} ({samples.size} samples @ {args.target_hz:.0f} Hz)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
