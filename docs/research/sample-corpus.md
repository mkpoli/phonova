# Sample corpus survey: built-in speech material

Survey of open speech corpora for Phonix's built-in samples, replacing the
single CMU ARCTIC `bdl` recording with a small, license-verified, phonetically
diverse set. Licensing constraint throughout: Phonix is MIT, so every
bundled file must carry a license that permits redistribution inside an MIT
codebase — CC0, CC BY (any version), Apache-2.0, MIT-equivalent grants, and
genuinely-original work all qualify; CC BY-NC/ND, "research and teaching
only," and LDC-membership licenses do not. Every verdict below cites the
corpus's own primary source (its own LICENSE/COPYING file or official page),
fetched directly, not an aggregator's summary. Survey date: July 2026.

The accepted files and their per-file provenance live in `samples/` —
`samples/MANIFEST.md` has the full per-file breakdown (speaker metadata,
exact license text, sample rate/depth/duration, phenomenon illustrated,
annotation provenance). This document is the survey behind that selection:
what was checked, what passed, and — just as importantly — what was
rejected and why, so the rejection isn't re-litigated later.

## Priority audit: the existing shipped samples were fabricated

Before surveying new material, the three samples the app already ships
(`arctic_bdl_a0001`, `arctic_slt_a0001`, `synth_vowel_perturbed`, all under
`apps/web/static/sample/`) were audited, because their annotations were
suspect. They were fabricated. This section documents what was wrong, how
it was found, and how it was corrected — a source-fidelity failure, not a
quiet fix.

**Where the annotations came from.** `tests/fixtures/scripts/build_sample_project.ts`
(commit `b848618`) copied `tests/fixtures/textgrids/arctic_bdl_a0001_long_utf8.TextGrid`
and `arctic_slt_a0001_short_utf8.TextGrid` into the app's bundled sample and
presented them as the recordings' annotation. Those two fixtures are
documented, honestly, in `tests/fixtures/MANIFEST.md`: "Interval and point
boundaries are placed by hand (or synthesized) for format-testing purposes;
they are not the output of acoustic alignment and should not be treated as
phonetic ground truth." They exist to exercise `phx-textgrid`'s parser
(long vs. short format, UTF-8/UTF-16/Latin-1, IPA diacritics) and are
depended on verbatim by `crates/phx-textgrid/tests/fixtures.rs` and
`crates/phx-figure/src/builder.rs` (`include_bytes!`), so they were not
touched. The bug was solely that the sample-assembly script repurposed a
format-testing fixture as if it were verified phonetic content, and the
disclaimer did not travel with it into the shipped file.

**What was actually wrong, checked against acoustics and against CMU
ARCTIC's own published alignment:**

- `arctic_bdl_a0001.TextGrid`'s `phones` tier had the *same* boundaries as
  its `words` tier — nine intervals, one per word, each labeled with a
  whole-word IPA transcription (e.g. `ˈdeɪndʒɚ` as a single "phone" spanning
  0.65-1.1 s). That is not a phone tier under any definition.
- The `words` tier itself claimed a silent pause from 1.55-1.75 s, between
  "trail" and "Philip." Measuring that exact span
  (`tests/fixtures/audio/arctic_bdl_a0001.wav`, RMS and zero-crossing rate)
  gives RMS 0.034 and a fricative-like zero-crossing rate of 0.49 — roughly
  100x louder than the file's genuine silence (RMS 0.0002-0.0008 in the
  real leading/trailing pau spans) and spectrally a fricative, not silence.
  CMU ARCTIC's own forced alignment for this utterance
  (`http://festvox.org/cmu_arctic/cmu_arctic/cmu_us_bdl_arctic/lab/arctic_a0001.lab`)
  confirms why: there is no pause there at all — that span is the onset of
  the /f/ in "Philip." The fixture's pause interval was never checked
  against the audio; had it been, the mislabeling would have been audible
  immediately.
- A fabricated `events` point tier claimed a "breath" at 0.05 s and a
  "click" at 3.2 s — invented events with no stated basis, consistent with
  the fixture's own disclaimer that it was hand-authored, not
  listened-and-annotated.
- `arctic_slt_a0001.TextGrid`'s `words` tier carried the text "Not at this
  particular case, Tom, apologized Whittemore." — the transcript of a
  *different* utterance, `arctic_a0002`. The shipped audio is byte-identical
  (MD5 `16ffc9002c8723b25797d1c2eb9dc397`) to CMU ARCTIC's own
  `cmu_us_slt_arctic/wav/arctic_a0001.wav`, whose real prompt (per the
  corpus's own `etc/txt.done.data`) is "Author of the danger trail, Philip
  Steels, etc." — the same sentence as `bdl`. The annotation carried the
  transcript of a different recording.

**How this was checked:** by fetching CMU ARCTIC's own `.lab` forced
alignment and `etc/txt.done.data` prompt list directly from
`festvox.org` (the corpus's own host, not a mirror or aggregator);
by MD5-comparing the shipped `.wav` bytes against freshly-fetched upstream
copies (both `bdl` and `slt` matched exactly, confirming the audio itself
was never altered — only the annotation was fabricated); and by measuring
RMS, zero-crossing rate, and spectral centroid on 20-170 ms windows at the
disputed boundaries with a small NumPy script, comparing claimed-silent
spans against claimed-voiced spans and against the file's genuine silence.

**The fix.** The shipped `apps/web/static/sample/arctic_bdl_a0001.TextGrid`
and `arctic_slt_a0001.TextGrid` now carry a single `phones` interval tier
converted directly from CMU ARCTIC's own `.lab` alignment via
`phx-textgrid`'s canonical writer (`crates/phx-textgrid/examples/lab_to_textgrid.rs`),
boundary times preserved exactly as published except for extending the
final interval to the audio's true duration (documented in
`tests/fixtures/MANIFEST.md`'s new "Alignments" section). The invented
`events` tier and the wrong-utterance `words` tier are gone; no replacement
word tier was invented in their place, since CMU ARCTIC does not publish
word-level boundaries. `synth_vowel_perturbed.wav` ships with no TextGrid,
which was already correct — it is a synthesized sustained vowel with no
forced alignment to carry, and no annotation was fabricated for it.

The corrected `phones` tiers were spot-checked the same way: the `pau`
spans at both ends of `bdl` and `slt` measure RMS 0.0002-0.005 (near-silent
floor); the `ao` vowel spans measure RMS 0.08-0.19 with a low, vowel-like
spectral centroid (~1.3 kHz); the `f`/`s` fricative spans in `bdl` measure
an elevated zero-crossing rate (0.56-0.57) and spectral centroid (~4.9 kHz)
distinct from both silence and vowels. A third file, `ksp` (see below), was
checked the same way and additionally shows a genuine 10 ms micro-pause the
forced aligner placed between "trail" and "Philip" — a real per-speaker
timing difference, not an artifact, and further evidence the corrected
tiers reflect actual acoustic alignment rather than another round of
hand-placement.

## Accepted

| Corpus | License (verified at) | Offers | Bundled as |
|---|---|---|---|
| CMU ARCTIC | CMU's own permissive grant (MIT-equivalent); `festvox.org/cmu_arctic/.../COPYING` | Real phone-level forced alignment, multiple English accents, 16 kHz | `bdl`, `slt`, `awb`, `ksp` — same sentence, four accents |
| LibriSpeech | CC BY 4.0; `openslr.org/12/` | Public-domain-sourced audiobook narration, connected speech | One `dev-clean` utterance, genuine word+phone MFA alignment |
| Wikimedia Commons ("Zh-pinyin tones with ma") | CC BY 3.0; Commons API `extmetadata` for the file | Mandarin tone minimal set, 44.1 kHz, non-Latin script | `zh_tones_ma.wav`, no tier |
| Original synthesis (this repo) | Project license (MIT OR Apache-2.0) | Modal/breathy/creaky sustained vowel contrast | `synth_vowel_{modal,breathy,creaky,perturbed}.wav` |

### CMU ARCTIC

<http://festvox.org/cmu_arctic/> — phonetically-balanced single-speaker
databases built at CMU's Language Technologies Institute for unit-selection
TTS research, ~1150 utterances per speaker from out-of-copyright Project
Gutenberg text, 16 kHz/16-bit/mono, phone-level forced alignment (HTK
`.lab` format) shipped alongside every speaker's audio.

License verified by fetching each speaker's own `COPYING` file directly
(`bdl`, `slt`, `awb`, `ksp`, plus `jmk`, `clb`, `rms` checked for
consistency): identical text across all seven, an MIT-equivalent grant —
"free for use for any purpose (commercial or otherwise)," conditioned on
retaining the copyright notice, marking modifications, and keeping the
original authors' names. Only the copyright year differs (2003 for the
original `bdl`/`slt`/`clb`/`rms`, 2005 for the added-accent `jmk`/`awb`/`ksp`).

Speaker roster published on the corpus page: `bdl` (US male), `slt` (US
female), `clb` (US female), `rms` (US male), `jmk` (Canadian male), `awb`
(Scottish male), `ksp` (Indian male). No age is published for any speaker.
**Accepted:** `bdl`, `slt` (already shipped, now fixed — see the audit
above), plus `awb` and `ksp` for accent diversity, all on the same
utterance (`arctic_a0001`) for direct cross-accent comparison. `jmk`/`clb`/`rms`
were not added, to keep the bundle small; they carry the identical license
and could be added the same way later.

### LibriSpeech

<https://www.openslr.org/12/> (OpenSLR resource 12) — read audiobook
speech derived from public-domain LibriVox recordings. License stated
directly on the resource page: CC BY 4.0. Attribution requested: "LibriSpeech
(c) 2014 by Vassil Panayotov, licensed under a Creative Commons Attribution
4.0 International License."

**Accepted** as a connected-speech counterpart to ARCTIC's citation-form
sentences, using the utterance already cached in this repo
(`tests/fixtures/audio/librispeech_2277-149896-0005.wav`, dev-clean,
speaker 2277, chapter 149896). **Phone-aligned**: the only openly licensed
LibriSpeech forced-alignment release found is Zenodo record `2619474`
(Lugosch et al., Mila, built with the Montreal Forced Aligner's pretrained
LibriSpeech acoustic model), explicitly CC BY 4.0. It ships as one 623 MB
archive covering all 980 hours with no per-utterance download link — but
the archive is a standard zip, so its central directory (a small index at
the end of the file) can be read with a couple of HTTP range requests to
locate one entry, then only that entry's compressed bytes need fetching.
Doing exactly that pulled `dev-clean/2277/149896/2277-149896-0005.TextGrid`
(1117 compressed bytes) without downloading anything close to the full
archive; its CRC32 was checked against the central directory's own
checksum to confirm a byte-perfect extraction. The file was reformatted
through `phx-textgrid`'s own reader and writer
(`crates/phx-textgrid/examples/reformat_textgrid.rs`) to normalize it to
this repo's canonical output — boundaries and labels unchanged — and now
carries genuine `words` (18 intervals) and `phones` (70 intervals, ARPABET
with MFA's stress digits) tiers, spot-checked against the waveform and
spectrogram in the running app. A widely-cited alternative,
`CorentinJ/librispeech-alignments` on GitHub, carries **no license file and
no license statement at all** — under default copyright that is
unredistributable regardless of the underlying audio's license, so it is
rejected outright rather than treated as a licensing gray area.

### Wikimedia Commons — Mandarin tone demonstration

<https://commons.wikimedia.org/wiki/File:Zh-pinyin_tones_with_ma.ogg> — "The
four main tones of Standard Mandarin pronounced with the syllable 'ma'," own
work by Peter Isotalo (Wikimedia username Karmosin), 2005. License verified
via the Commons API's `imageinfo`/`extmetadata` for this exact file (the
API response *is* the primary source for a Commons file's license, not a
secondary claim about it): CC BY 3.0, attribution required.

This single 35 KB Ogg Vorbis file (44.1 kHz mono, 4.45 s) covers three
diversity gaps in one fetch: a tone language, a non-Latin-script language,
and a 44.1 kHz sample. Verified by measuring an autocorrelation-based pitch
contour across the clip: a sustained rise from ~125 Hz to ~195 Hz in one
segment, a dip from ~118 Hz down to ~94 Hz and back up to ~158 Hz in
another, and a fall from ~135 Hz to ~90 Hz in a third — rising, dipping,
and falling contours, consistent with the description (a crude pitch
tracker on a narration-and-example recording won't cleanly isolate all
four tones from the between-example narration, but the three distinct
contour shapes it does find match what the file's own description claims).

Two other files in the same Commons category ("Mandarin tones") were noted
but not pulled: `File:FourMandarinTones.ogg` (an alternative recording of
the same four tones) and the SVG/PNG tone-contour diagrams (not audio). Both
remain available if a second Mandarin sample is wanted later.

### Original synthetic vowels

`synth_vowel_modal.wav` (identical to the existing `synth_vowel_a.wav`
fixture), `synth_vowel_breathy.wav`, and `synth_vowel_creaky.wav` — all a
from-scratch source-filter synthesizer (glottal source through a four-resonator
cascade tuned to Peterson & Barney 1952's average /a/ formants, Fant 1960
bandwidths), original work under the project's own MIT OR Apache-2.0
license, no external rights question. Breathy and creaky are new for this
survey; `synth_vowel_perturbed.wav` (jitter/shimmer) already existed.
Verified against the modal baseline: breathy shows autocorrelation
periodicity dropping from 0.86 to 0.45 at the fundamental and an eightfold
rise in 3-7 kHz energy; creaky shows a genuine strong/weak glottal
pulse-pairing at a stable ~28.6 ms period, confirmed by direct peak-picking
on the waveform. See `samples/scripts/synth_vowel_breathy.py` and
`synth_vowel_creaky.py` for the exact, deterministic synthesis formula.

Real recorded creaky/breathy speech with a clear redistribution license was
searched for and not found (see UCLA Phonetics Lab Archive and Speech
Accent Archive below, both rejected on license grounds); synthesis was
the only license-clean option for this dimension.

## Rejected, or linkable but not bundleable

| Candidate | License (verified at) | Verdict |
|---|---|---|
| TIMIT | LDC User Agreement (membership/fee); `catalog.ldc.upenn.edu/LDC93S1` | Reject — not redistributable |
| Buckeye Corpus | "FREE for noncommercial uses," registration required; `buckeyecorpus.osu.edu` | Reject — NC, and registration-gated |
| Mozilla Common Voice | CC0 1.0 for audio, but distribution now gated via Mozilla Data Collective | Uncertain / not fetchable as a small file right now |
| UCLA Phonetics Lab Archive | No stated license; `archive.phonetics.ucla.edu` | Reject — linkable, not bundleable |
| Speech Accent Archive | CC BY-NC-SA 2.0; `accent.gmu.edu/about.php` | Reject — NC term |
| Praat demo/tutorial audio | No redistributable asset found | Not applicable |
| VCTK | CC BY 4.0 (verified); `datashare.ed.ac.uk/handle/10283/3443` | Accept-in-principle, not fetched — only an 11 GB archive at the primary source |
| THCHS-30 | OpenSLR tags Apache 2.0; the corpus's own about page says "free to academic users" | Uncertain / rejected — two primary sources from the same rights holder disagree |
| THCHS-30 forced alignment (Zenodo 7528596) | MIT (verified on the record page) | Moot — paired with audio whose own license is unresolved |
| AISHELL-1 | Same pattern as THCHS-30; `openslr.org/33/` | Uncertain / rejected, same reasoning |
| AISHELL-3 | OpenSLR tags Apache 2.0, no caveat found there | Uncertain / rejected — rights holder's own site unreachable to confirm |
| `CorentinJ/librispeech-alignments` | None stated | Reject — unlicensed, a clean rejection rather than a gray area |

### TIMIT

<https://catalog.ldc.upenn.edu/LDC93S1> — the catalog page's own License(s)
field states "LDC User Agreement for Non-Members," LDC's membership/fee
license. **Reject**: not redistributable in an MIT product regardless of
how the audio is used internally.

### Buckeye Corpus

<https://buckeyecorpus.osu.edu/> — the corpus's own homepage: "The corpus
is FREE for noncommercial uses," gated behind a registration form.
**Reject**: the NC term alone rules it out for an MIT project, and
registration-gating means there is no direct-fetch path regardless.

### Mozilla Common Voice

CC0 1.0 for the audio itself, confirmed via Mozilla's Community Playbook
(contributors sign a CC0 waiver before inclusion). But as of the current
platform state, Mozilla states Common Voice datasets are "now exclusively
available through Mozilla Data Collective," a gated dataset marketplace;
no current-terms text or single-clip HTTP access route could be confirmed
from a primary source. **Verdict: uncertain, not accepted** — the
underlying audio license is fine in principle, but the access path cannot
currently be verified as unencumbered or as fetchable within a modest
single-file budget. Worth re-checking if Mozilla's distribution terms
settle.

### UCLA Phonetics Lab Archive

<http://archive.phonetics.ucla.edu/> — the site states its collections are
"open to everyone" and "primarily intended to be used by the linguistics
community," with no copyright notice or explicit redistribution grant
anywhere on the fetched pages. **Reject** — "open to everyone" describes
browsing access, not a redistribution license; linkable from documentation,
not bundleable in the repo.

### Speech Accent Archive

<https://accent.gmu.edu/about.php> — states directly: "This work is
licensed under a Creative Commons License," linking to
`creativecommons.org/licenses/by-nc-sa/2.0/`, i.e. CC BY-NC-SA 2.0.
**Reject** — the NonCommercial term is incompatible with an MIT product by
this task's own hard constraint. Linkable but not bundleable, and a
clear-cut case rather than a judgment call.

### Praat demo audio

Praat itself (Boersma & Weenink) is GPLv3+; its manual pages are CC BY-SA
4.0. No bundled, statically-distributed demo/tutorial `.wav`/`.aiff` files
were found in the Praat GitHub repository or manual — the manual's "Sound"
tutorial examples appear to be synthesized on the fly by manual scripts,
not shipped as static assets. **Not applicable**: there is no asset here to
accept or reject.

### VCTK

<https://datashare.ed.ac.uk/handle/10283/3443> (CSTR VCTK Corpus v0.92,
DOI `10.7488/ds/2645`) — 110 English speakers, various accents, captured at
96 kHz/24-bit and distributed at 48 kHz/16-bit, no forced alignment (audio +
per-sentence text prompt only). License confirmed by fetching the actual
`license_text.txt` bitstream from the DataShare record: CC BY 4.0, full
legal code, standard attribution requirement. Required citation: "Yamagishi,
Junichi; Veaux, Christophe; MacDonald, Kirsten. (2019). CSTR VCTK Corpus:
English Multi-speaker Corpus for CSTR Voice Cloning Toolkit (version 0.92),
University of Edinburgh, CSTR. <https://doi.org/10.7488/ds/2645>."

**Accept-in-principle, not fetched**: the only primary-source download is
the full `VCTK-Corpus-0.92.zip` (10.94 GB); no per-file bitstream link
exists on DataShare. A Hugging Face mirror (`CSTR-Edinburgh/vctk`) carries
the same CC BY 4.0 license but is a loading script that pulls the same full
archive at run time, not a browsable file tree — checked directly via the
HF API, which returned only `.gitattributes`/`README.md`/`vctk.py` as
actual repository contents. This task's constraint against multi-gigabyte
fetches rules VCTK out for now; the license work is done, so a future
one-time 11 GB pull (with the user's explicit sign-off, per the resource-heavy-operation
rule) could extract `p225_001.wav` ("Please call Stella," 23F, Southern
England accent, per `speaker-info.txt`) or similar without re-verifying
licensing.

### THCHS-30 and AISHELL (Mandarin, OpenSLR)

<https://www.openslr.org/18/> and <https://www.openslr.org/33/> — both
tagged Apache License 2.0 on their OpenSLR resource pages. That tag
conflicts with a second primary source, though: THCHS-30's own "about"
text, hosted by the same rights holder (CSLT, Tsinghua University, fetched
directly at `openslr.org/resources/18/about.html`), says plainly "the
database is totally free to **academic** users" — language that reads as
narrower than Apache-2.0's unrestricted grant, including commercial use.
AISHELL-1's own about text carries the same "free for academic use"
wording. Neither about page states a LICENSE file resolving the
discrepancy, and neither corpus's downloaded resource bundle (a small
lexicon/metadata tarball, not the full audio, checked for exactly this
question) contains one either. Two primary sources from the same rights
holder disagreeing is exactly the case this survey's brief says to treat
as **uncertain, not accepted** — downgraded here from an earlier pass's
"confirmed" wording once the about-page text was checked directly rather
than trusting the OpenSLR tag alone. AISHELL-3 (OpenSLR-93) tags
Apache-2.0 too, with no academic-use caveat visible on OpenSLR's own page,
but its rights holder's own site (aishelltech.com) could not be reached to
confirm independently (rate-limited); also left uncertain rather than
assumed clean by absence of a caveat. THCHS-30 (Tsinghua CSLT) ships
word-level Chinese transcripts with its audio; a separate,
independently-hosted forced alignment exists at Zenodo record `7528596`
("THCHS-30 – Aligned IPA transcriptions," Stefan Taubert), **MIT
licensed** (confirmed on the record page), word + tone-marked-IPA phoneme
tiers, built with the Montreal Forced Aligner's Mandarin acoustic model —
of no use while the underlying audio's own license is unresolved. AISHELL
has no phone alignment published anywhere and is otherwise a strictly
worse fit (15 GB vs. 6 GB, word/character transcript only).

**Not fetched, license unresolved rather than merely impractical**:
independent of the license question, THCHS-30's audio is distributed only
as a single 6.0 GB tarball (confirmed via the Apache directory listing;
`Accept-Ranges: bytes` is present but does not make an individual tar
member separately addressable over HTTP). No HTTP-browsable mirror with
individual files and a confirmed license was found — one HF mirror
(`sijunhe/thchs30`) is a loading script with no stored audio, another
(`urarik/thchs30`) stores real parquet data but states no license, and a
third (`anyspeech/THCHS-30-alignments`) has alignment data only, no audio,
and an unfilled README with no license statement. The audio+alignment
license pairing is correct and would be excellent (tone-marked IPA phone
tiers on real Mandarin speech), but obtaining even one utterance's audio
cleanly would mean downloading the full 6 GB archive, which this task rules
out. The Mandarin gap in this pass is filled instead by the Wikimedia
Commons tone clip above; THCHS-30 is a strong candidate for a dedicated
follow-up fetch.

### `CorentinJ/librispeech-alignments`

The most commonly cited LibriSpeech forced-alignment release, on GitHub.
No `LICENSE` file, no license badge, no license statement anywhere in the
README. **Reject** — under default copyright this is all-rights-reserved
regardless of the underlying CC BY 4.0 audio; a derivative alignment needs
its own grant, and none is given. Use Zenodo `2619474` (Lugosch et al.,
explicit CC BY 4.0) instead if a LibriSpeech alignment is pursued later.

## Where these should live, and folding in the existing sample

**Proposal, not implemented here** (per this task's scope: research and
assemble the files, do not restructure the app).

`samples/` at the repository root holds the curated, accepted set — audio,
TextGrid, the generating scripts for the synthetic vowels, and
`samples/MANIFEST.md`. It is deliberately outside both `apps/web/static/`
(SvelteKit-static, web-only, not reachable by the Tauri desktop build) and
`tests/fixtures/` (CI/test-oriented, with its own `include_bytes!` and
test-suite dependencies that make it unsafe to casually add to or
reorganize). A neutral, top-level location lets both the web and desktop
builds copy from the same place without one build's static-asset
convention leaking into the other's.

For the web build, the existing pattern
(`tests/fixtures/scripts/build_sample_project.ts` copying into
`apps/web/static/sample/` plus a `manifest.json` the app fetches) is
already the right shape — it would just need to point at `samples/`
instead of `tests/fixtures/`, once `samples/` is treated as the canonical
source. For desktop, Tauri's bundled-resources mechanism
(`tauri.conf.json`'s `bundle.resources`) can ship the same `samples/`
directory verbatim and resolve it at runtime via the resource-directory
API, so the same files back both a `fetch()` in the web build and a
filesystem read in the desktop build without duplication.

Folding in the existing single sample: `arctic_bdl_a0001` and
`arctic_slt_a0001` (now fixed, see the audit above) are already members of
`samples/` in this deliverable — copied byte-identical from the
now-corrected `apps/web/static/sample/`, not regenerated a second time. The
one-off `apps/web/static/sample/manifest.json` and
`tests/fixtures/scripts/build_sample_project.ts` become redundant with
`samples/MANIFEST.md` once the web build is repointed; at that point the
former can be retired rather than kept as a second, competing manifest —
consistent with this project's standing preference for one coherent
design over parallel "compatible" copies.
