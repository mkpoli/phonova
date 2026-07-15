"""Parameter sets mirrored from the Rust `*Params` structs.

Field names and defaults match `crates/phx-pitch`, `crates/phx-formant`, and
`crates/phx-intensity` as specified in `docs/plan/tasks/phase-2.md` (T2.1,
T2.2, T2.3), so a case run here uses the same nominal analysis settings as
the Rust side it is compared against.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass


@dataclass(frozen=True)
class PitchParams:
    """Mirrors `phx_pitch::PitchParams`."""

    time_step: float | None = None
    floor_hz: float = 75.0
    ceiling_hz: float = 600.0
    max_candidates: int = 15
    very_accurate: bool = False
    silence_threshold: float = 0.03
    voicing_threshold: float = 0.45
    octave_cost: float = 0.01
    octave_jump_cost: float = 0.35
    voiced_unvoiced_cost: float = 0.14

    def as_dict(self) -> dict:
        return asdict(self)


@dataclass(frozen=True)
class FormantParams:
    """Mirrors `phx_formant::FormantParams`."""

    ceiling_hz: float = 5500.0
    max_formants: int = 5
    window_length: float = 0.025
    time_step: float | None = None
    preemphasis_from_hz: float = 50.0

    def as_dict(self) -> dict:
        return asdict(self)


@dataclass(frozen=True)
class IntensityParams:
    """Mirrors `phx_intensity::IntensityParams`."""

    pitch_floor_hz: float = 100.0
    time_step: float | None = None
    subtract_mean: bool = True

    def as_dict(self) -> dict:
        return asdict(self)
