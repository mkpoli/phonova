"""Named oracle cases: a measure, a parameter set, and a default corpus.

Case names and the `oracle run --case <name> --audio <path>` shape follow
`docs/plan/validation.md`. `tests/fixtures/audio/long_scroll_test.wav` (10
minutes, built by concatenating the other fixtures) is excluded from every
default corpus — it adds no case coverage beyond its five source clips and
is expensive to re-run.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from oracle.params import FormantParams, IntensityParams, PitchParams

Measure = Literal["pitch", "formant", "intensity", "spectrogram"]

SPEECH_AND_VOWEL_CORPUS = (
    "arctic_bdl_a0001.wav",
    "arctic_slt_a0001.wav",
    "librispeech_2277-149896-0005.wav",
    "synth_vowel_a.wav",
)


@dataclass(frozen=True)
class Case:
    name: str
    measure: Measure
    params: PitchParams | IntensityParams | FormantParams | None
    default_audio: tuple[str, ...]
    description: str


CASES: dict[str, Case] = {
    "pitch-defaults": Case(
        name="pitch-defaults",
        measure="pitch",
        params=PitchParams(),
        default_audio=SPEECH_AND_VOWEL_CORPUS,
        description="Boersma raw-autocorrelation pitch, Praat's documented defaults.",
    ),
    "formant-defaults": Case(
        name="formant-defaults",
        measure="formant",
        params=FormantParams(),
        default_audio=SPEECH_AND_VOWEL_CORPUS,
        description="Burg LPC formants, Praat's documented defaults.",
    ),
    "intensity-defaults": Case(
        name="intensity-defaults",
        measure="intensity",
        params=IntensityParams(),
        default_audio=SPEECH_AND_VOWEL_CORPUS,
        description="Intensity contour, Praat's documented defaults.",
    ),
    "spectrogram-slice-defaults": Case(
        name="spectrogram-slice-defaults",
        measure="spectrogram",
        params=None,
        default_audio=("synth_vowel_a.wav",),
        description=(
            "Single power-spectrum slice at t=1.0s, Praat's documented "
            "spectrogram defaults. Not oracle-diffed (validation.md: "
            "spectrogram parity is checked against scipy STFT instead); "
            "`oracle diff` refuses this measure."
        ),
    ),
}
