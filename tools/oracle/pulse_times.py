"""Emit Praat's cross-correlation glottal-pulse times for a fixture span.

Runs Praat's "Sound & Pitch: To PointProcess (cc)..." (via parselmouth) over
a fixture WAV and prints the pulse times inside a chosen span as JSON. This is
the pulse-placement counterpart to the aggregate `voice-report-defaults` case:
the voice report diffs span-level jitter/shimmer scalars, while this dumps the
individual pulse times so a placement gap can be localized (onset drift,
mid-span phase noise, missed or extra pulses) against `phx_voice::pulses`.

    uv run --extra parselmouth python pulse_times.py \
        --audio tests/fixtures/audio/arctic_bdl_a0001.wav \
        --start 0.723 --end 0.943

Pitch floor/ceiling default to Praat's documented pitch defaults (75 / 600 Hz),
the same contour `phx_voice::voice_report` seeds its pulses from. Output is
program text for ad-hoc comparison, not a committed fixture (the clean-room
caution in `docs/research/algorithms-and-validation.md` §7.1 applies to
Praat numeric fixtures, not to a script that regenerates them on demand).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audio", required=True, help="path to a fixture WAV")
    parser.add_argument("--start", type=float, required=True, help="span start in seconds")
    parser.add_argument("--end", type=float, required=True, help="span end in seconds")
    parser.add_argument("--floor-hz", type=float, default=75.0)
    parser.add_argument("--ceiling-hz", type=float, default=600.0)
    args = parser.parse_args(argv)

    try:
        import parselmouth
        from parselmouth.praat import call
    except ImportError:
        print(
            "SKIP: praat-parselmouth is not installed (run with `uv run --extra "
            "parselmouth`); skipping pulse-time dump."
        )
        return 0

    audio_path = Path(args.audio)
    if not audio_path.is_file():
        print(f"error: audio not found: {audio_path}", file=sys.stderr)
        return 2

    sound = parselmouth.Sound(str(audio_path))
    pitch = sound.to_pitch_ac(pitch_floor=args.floor_hz, pitch_ceiling=args.ceiling_hz)
    point_process = call([sound, pitch], "To PointProcess (cc)")
    count = int(call(point_process, "Get number of points"))
    times = [call(point_process, "Get time from index", i) for i in range(1, count + 1)]
    in_span = [t for t in times if args.start <= t <= args.end]

    print(
        json.dumps(
            {
                "audio": str(audio_path),
                "span": {"start": args.start, "end": args.end},
                "floor_hz": args.floor_hz,
                "ceiling_hz": args.ceiling_hz,
                "n_total": count,
                "n_in_span": len(in_span),
                "times": in_span,
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
