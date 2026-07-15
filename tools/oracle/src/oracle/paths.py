"""Repository path resolution, independent of the current working directory."""

from __future__ import annotations

from pathlib import Path


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for candidate in here.parents:
        if (candidate / "tests" / "fixtures" / "audio").is_dir():
            return candidate
    raise RuntimeError("could not locate repository root from " + str(here))


def fixtures_audio_dir() -> Path:
    return repo_root() / "tests" / "fixtures" / "audio"


def oracle_dir() -> Path:
    return repo_root() / "tools" / "oracle"


def references_dir() -> Path:
    return oracle_dir() / "references"
