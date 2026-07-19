# Sample corpus manifest

Curated, redistributable speech material for Phonix's built-in samples.
Total size: 1.3 MB (`du -sh samples`, excluding `scripts/`). Every file
below was verified against its corpus's own primary license source, not an
aggregator; see `docs/research/sample-corpus.md` for the full survey,
including rejected candidates and the reasoning against each.

No NC-, ND-, or LDC-licensed material is present. Every annotation tier is
either a real forced alignment carried over from the source corpus with its
boundary times preserved exactly, a single whole-utterance orthographic
span with no invented internal boundaries, or absent.

## CMU ARCTIC — `arctic_*_a0001.{wav,TextGrid}`

Source: CMU_ARCTIC databases, Language Technologies Institute, Carnegie
Mellon University. <http://festvox.org/cmu_arctic/>

License: CMU's own permissive grant, identical text per speaker (only the
copyright year differs): "This voice is free for use for any purpose
(commercial or otherwise)," subject to retaining the copyright notice,
marking modifications, and keeping the original authors' names — an
MIT-equivalent license. Full text at
`http://festvox.org/cmu_arctic/cmu_arctic/cmu_us_<spk>_arctic/COPYING` for
each speaker. Required attribution: "CMU_ARCTIC databases, Carnegie Mellon
University, Copyright (c) 2003 (bdl, slt) / 2005 (awb, ksp)."

All four files are utterance `arctic_a0001`, "Author of the danger trail,
Philip Steels, etc." — the same sentence across speakers, useful for
directly comparing vowel and consonant realization across accents.

Annotation: real phone-level forced alignment, published by CMU alongside
the audio (HTK label format, `<end_time_s> <state> <ARPABET phone>`), one
`phones` interval tier per file. Boundary times are exactly the published
end times; the only change is extending the final interval to the audio's
true duration (a forced aligner's last frame commonly falls a few
milliseconds short of the file's actual sample count, i.e. untranscribed
trailing silence). No boundary is invented, merged, or moved otherwise, and
no label is anything but the corpus's own ARPABET phone symbol.

### `arctic_bdl_a0001.wav` / `.TextGrid`

- Speaker: `bdl`, US English, male ("experienced voice talent" per the
  corpus page; no age published).
- 16 kHz, 16-bit PCM, mono, 3.235 s. 35 phone intervals.
- Illustrates: citation-form connected speech, modal male voice, a full
  phone-level segmentation usable for VOT and vowel-formant exercises.

### `arctic_slt_a0001.wav` / `.TextGrid`

- Speaker: `slt`, US English, female ("experienced voice talent"; no age
  published).
- 16 kHz, 16-bit PCM, mono, 3.355 s. 38 phone intervals.
- Illustrates: modal female voice on the same sentence as `bdl`, for direct
  cross-speaker formant comparison.

### `arctic_awb_a0001.wav` / `.TextGrid`

- Speaker: `awb`, Scottish English, male.
- 16 kHz, 16-bit PCM, mono, 4.000 s (the slower, more deliberate reading
  pace stretches the same sentence to nearly a full second longer than
  `bdl`'s). 37 phone intervals.
- Illustrates: rhotic, non-General-American vowel space on the same
  sentence — a ready-made accent-comparison exercise against `bdl`/`slt`.

### `arctic_ksp_a0001.wav` / `.TextGrid`

- Speaker: `ksp`, Indian English, male.
- 16 kHz, 16-bit PCM, mono, 3.235 s. 37 phone intervals.
- Illustrates: a second non-American English variety on the same sentence;
  the forced alignment records a genuine 10 ms micro-pause between "trail"
  and "Philip" that neither `bdl` nor `slt` shows, a real per-speaker
  timing difference rather than an artifact.

## LibriSpeech — `librispeech_2277-149896-0005.{wav,TextGrid}`

- Source: LibriSpeech corpus, `dev-clean` subset, speaker 2277 (female, per
  the corpus's own `SPEAKERS.TXT`), chapter 149896, utterance
  `2277-149896-0005`. LibriSpeech audio derives from public-domain LibriVox
  audiobook recordings.
- URL: <https://www.openslr.org/12/> (OpenSLR resource 12), archive member
  `LibriSpeech/dev-clean/2277/149896/2277-149896-0005.flac`.
- License: CC BY 4.0, stated directly on the OpenSLR resource page.
  Attribution: "LibriSpeech (c) 2014 by Vassil Panayotov, licensed under a
  Creative Commons Attribution 4.0 International License."
- 16 kHz, 16-bit PCM, mono, 5.600 s.
- Annotation: **orthographic-only**. No forced alignment is bundled — the
  only openly-licensed LibriSpeech alignment release found (Zenodo record
  2619474, Lugosch et al., CC BY 4.0) ships as a single 623 MB archive with
  no per-utterance download, which this task's fetch-budget explicitly
  rules out. The `orthographic` tier is one interval spanning the whole
  file, text "many little wrinkles gathered between his eyes as he
  contemplated this and his brow moistened" (the utterance's published
  transcript) — a single span, not per-word timing, so no boundary is
  invented.
- Illustrates: continuous audiobook narration against ARCTIC's citation-style
  isolated sentences — the connected-speech side of that contrast.

## Mandarin tones — `zh_tones_ma.wav`

- Source: Wikimedia Commons, "Zh-pinyin tones with ma," own work by Peter
  Isotalo (Wikimedia username Karmosin), 2005.
- URL: <https://commons.wikimedia.org/wiki/File:Zh-pinyin_tones_with_ma.ogg>
- License: CC BY 3.0. Attribution: "Zh-pinyin tones with ma by Peter Isotalo
  (Karmosin), CC BY 3.0, via Wikimedia Commons."
- Original format Ogg Vorbis, 44100 Hz, mono; decoded to 16-bit PCM WAV with
  `ffmpeg` (`-c:a pcm_s16le`), no resampling, no trimming.
- 44100 Hz, 16-bit PCM, mono, 4.457 s.
- Annotation: none shipped. The four tones ("mā," "má," "mǎ," "mà," the
  textbook minimal set on one syllable) are not separately time-boundaried
  in the source, and no independent alignment for this clip exists, so no
  tier is included — see `docs/research/sample-corpus.md` for the pitch
  contours measured in verifying this file actually contains four distinct
  tone shapes.
- Illustrates: Mandarin lexical tone (a tone language), a non-Latin script
  (Hanzi) language, and the corpus's only 44.1 kHz file, in one recording.

## Original synthetic vowels — `synth_vowel_*.wav`

Source-filter formant synthesis, original work for this repository
(project license, MIT OR Apache-2.0), no external rights involved. All four
use the same /a/ formant cascade (Peterson & Barney 1952 average adult-male
formants: F1 730 Hz, F2 1090 Hz, F3 2440 Hz, F4 3400 Hz; Fant 1960
bandwidths BW = 50 + F/10 Hz), so the four are directly comparable — same
vowel, same vocal-tract filter, phonation type as the only variable. No
annotation tier: a synthesized sustained vowel has no phonemic content to
align. 16 kHz, 16-bit PCM, mono, 2.000 s each.

- `synth_vowel_modal.wav` — modal phonation baseline, 110 Hz glottal
  impulse train (source: `tests/fixtures/audio/synth_vowel_a.wav`,
  identical bytes).
- `synth_vowel_breathy.wav` — breathy phonation: raised-cosine (rather than
  impulsive) glottal pulses plus an aspiration-noise floor, fixed
  noise-to-harmonic ratio. Verified against the modal baseline by
  autocorrelation: periodicity at the 110 Hz fundamental drops from 0.86
  (modal) to 0.45, and 3-7 kHz energy rises about eightfold.
- `synth_vowel_creaky.wav` — creaky phonation (vocal fry): an exact
  strong/weak glottal pulse-pairing at a 70 Hz pulse rate (35 Hz perceived
  pitch), the diplophonic pattern reported for creaky voice. Verified by
  direct peak-picking on the waveform: a repeating strong-pulse-then-two-weak-
  pulses pattern at a stable ~28.6 ms period.
- `synth_vowel_perturbed.wav` — closed-form jitter/shimmer perturbation
  (existing fixture, identical bytes to
  `tests/fixtures/audio/synth_vowel_perturbed.wav`; see that file's own
  script for the exact formula).

Regeneration: `uv run samples/scripts/synth_vowel_breathy.py` and
`uv run samples/scripts/synth_vowel_creaky.py`, both deterministic.

## Diversity coverage

| Dimension | Coverage |
|---|---|
| Language / family | English (Indo-European); Mandarin (Sino-Tibetan) |
| Tone language | Mandarin (`zh_tones_ma.wav`) |
| Non-Latin script | Mandarin (Hanzi) |
| Speaker sex | male (`bdl`, `awb`, `ksp`), female (`slt`, LibriSpeech 2277) |
| Speaker accent | US (`bdl`, `slt`), Scottish (`awb`), Indian (`ksp`) |
| Voice quality | modal, breathy, creaky (synthetic, directly comparable) |
| VOT / stop contrasts | any ARCTIC `phones` tier (e.g. `t`, `d`, `p` intervals) |
| Fricative spectra | ARCTIC `phones` tier `f`/`s` intervals |
| Tone contours | `zh_tones_ma.wav` |
| Nasality | ARCTIC `phones` tier `n`/`m`-bearing words (e.g. "danger," "not") |
| Connected speech vs. citation form | ARCTIC (citation) vs. LibriSpeech (audiobook narration) |
| Sustained vowels for voice-quality measures | `synth_vowel_{modal,breathy,creaky,perturbed}.wav` |
| Sample rate 16 kHz | ARCTIC, LibriSpeech, synthetic vowels |
| Sample rate 44.1/48 kHz | `zh_tones_ma.wav` (44.1 kHz) |

Not covered in this pass, and why: speaker age is not published for any
accepted corpus here (CMU ARCTIC and Wikimedia Commons both omit it); a
48 kHz *natural-speech* sample (as opposed to the 44.1 kHz Mandarin clip)
would need VCTK, whose only primary-source download is an 11 GB archive —
see the survey's VCTK entry for the license verification and why it is
not bundled here.
