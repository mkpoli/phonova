"""Generate binary-format TextGrid fixtures via parselmouth (Praat, GPLv3),
out-of-process only (`pyproject.toml`'s `parselmouth` extra;
`docs/research/algorithms-and-validation.md` §7.1 on the GPL boundary).

Run via the tools/oracle uv project:

    uv run --project tools/oracle --extra parselmouth \
        tools/oracle/generate_textgrid_binary_fixtures.py

Praat's manual page "TextGrid file formats" documents the long and short
text formats but gives no grammar for the binary variant ("we can add it
here on request" — it never was). This script is therefore the only
permitted source for `phx-textgrid`'s binary reader to derive that format
from (`docs/plan/tasks/phase-3.md` T3.2's clean-room constraint: oracle
black-box output, not Praat/parselmouth source). It writes two kinds of
fixture into `tests/fixtures/textgrids/`:

- binary re-saves of existing long-format text fixtures. `parselmouth.Data.read`
  uses Praat's own object reader to parse the source text file; only the
  resulting in-memory TextGrid object crosses into this script, never the
  reader's source code. This gives the same annotation content in both text
  and binary form, letting a from-scratch binary reader be checked against
  content already known correct.
- one TextGrid assembled directly through parselmouth's public "Create
  TextGrid...", "Insert boundary...", "Insert point..." commands and saved
  as binary only. It covers structure the text fixtures do not: four tiers
  (two interval, two point) and non-round fractional boundary times, so the
  binary sample corpus is not just a re-encoding of already-seen numbers.

Every output is round-tripped in-process: read back with `parselmouth.Data.read`
and compared against the source object by diffing each object's long-text
serialization (`save_as_text_file`) byte-for-byte — the same Praat build
producing both dumps makes this an exact structural/content check, not just
a byte-count check on the binary file.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "src"))

from oracle.paths import fixtures_textgrids_dir, repo_root  # noqa: E402
from oracle.runner import OracleUnavailable, load_parselmouth  # noqa: E402

# (text fixture filename, output binary filename)
TEXT_TO_BINARY = [
    ("ipa_diacritics_long_utf8.TextGrid", "ipa_diacritics_binary.TextGrid"),
    ("adjacent_empty_intervals_long_utf8.TextGrid", "adjacent_empty_intervals_binary.TextGrid"),
]

SYNTHETIC_BINARY_NAME = "synthetic_binary_complex.TextGrid"


def _dump_text(parselmouth_module, tg, tmp_path: Path) -> str:
    """Praat's text writer picks UTF-8 for pure-ASCII content and UTF-16
    (with BOM) otherwise (manual page "Unicode"); sniff the BOM rather than
    assuming one encoding."""
    tg.save_as_text_file(str(tmp_path))
    raw = tmp_path.read_bytes()
    if raw.startswith(b"\xff\xfe") or raw.startswith(b"\xfe\xff"):
        return raw.decode("utf-16")
    return raw.decode("utf-8")


def _roundtrip_check(parselmouth_module, source_tg, binary_path: Path, tmp_dir: Path) -> None:
    reread = parselmouth_module.Data.read(str(binary_path))
    a = _dump_text(parselmouth_module, source_tg, tmp_dir / "a.TextGrid")
    b = _dump_text(parselmouth_module, reread, tmp_dir / "b.TextGrid")
    if a != b:
        raise AssertionError(
            f"round-trip mismatch for {binary_path.name}: text dump of the "
            "binary-saved object differs from the source object's dump"
        )


def _build_from_text(parselmouth_module, source_name: str, out_name: str, out_dir: Path, tmp_dir: Path) -> Path:
    source_path = out_dir / source_name
    tg = parselmouth_module.Data.read(str(source_path))
    out_path = out_dir / out_name
    tg.save_as_binary_file(str(out_path))
    _roundtrip_check(parselmouth_module, tg, out_path, tmp_dir)
    return out_path


def _build_synthetic(parselmouth_module, out_dir: Path, tmp_dir: Path) -> Path:
    call = parselmouth_module.praat.call
    tg = call(
        "Create TextGrid",
        0,
        3.14159265,
        "segments annotations cues markers",
        "cues markers",
    )
    # non-round fractional boundaries, deliberately not aligned to any
    # audio fixture's frame grid
    for t in (0.333333333, 1.618033989, 2.71828):
        call(tg, "Insert boundary", 1, t)
    call(tg, "Set interval text", 1, 2, "ə")
    call(tg, "Set interval text", 1, 3, "ˈɡʊd")
    call(tg, "Set interval text", 1, 4, "")

    for t in (0.5, 1.0, 1.5, 2.0, 2.5, 3.0):
        call(tg, "Insert boundary", 2, t)
    call(tg, "Set interval text", 2, 4, "note")

    for t, mark in ((0.1, "c1"), (1.41421356, "c2"), (2.9, "c3")):
        call(tg, "Insert point", 3, t, mark)
    # tier 4 ("markers") left with zero points on purpose

    out_path = out_dir / SYNTHETIC_BINARY_NAME
    tg.save_as_binary_file(str(out_path))
    _roundtrip_check(parselmouth_module, tg, out_path, tmp_dir)
    return out_path


def main() -> int:
    try:
        parselmouth_module = load_parselmouth()
    except OracleUnavailable as exc:
        print(f"SKIP: {exc}")
        return 0

    out_dir = fixtures_textgrids_dir()
    tmp_dir = repo_root() / "tools" / "oracle" / ".tmp-textgrid-roundtrip"
    tmp_dir.mkdir(parents=True, exist_ok=True)

    written = []
    for source_name, out_name in TEXT_TO_BINARY:
        path = _build_from_text(parselmouth_module, source_name, out_name, out_dir, tmp_dir)
        written.append(path)
        print(f"wrote {path.relative_to(repo_root())} (round-trip OK)")

    synth_path = _build_synthetic(parselmouth_module, out_dir, tmp_dir)
    written.append(synth_path)
    print(f"wrote {synth_path.relative_to(repo_root())} (round-trip OK)")

    for f in tmp_dir.iterdir():
        f.unlink()
    tmp_dir.rmdir()

    print(f"generated {len(written)} binary TextGrid fixture(s) in {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
