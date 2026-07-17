# Besra — a Python fieldwork tool, and what it means for Phonia

Besra is a Python audio/text annotation and model-training tool for
field linguists, presented in a recorded talk by its author, Dr. Kellen
Parker van Dam (University of Passau), on 2026-06-23
([talk recording](https://www.youtube.com/watch?v=uaNj-dnbj4g), title card:
"Kellen Parker van Dam (Passau): Besra — a tool & workflow for rapid
audio/text processing"). This note summarizes the talk, checks for any
independent trace of the project (repo, package, paper, site), and maps its
ideas against `../plan/horizon.md`.

Sourcing note: the primary source is the YouTube auto-caption transcript,
pulled with `yt-dlp --write-auto-sub` and de-duplicated locally. Auto-captions
mis-hear domain terms consistently — "Elon" for **ELAN**, "prot"/"prod"/"prop"
for **Praat**, "protcript" for **Praat script**, "Andra"/"Bessra" for
**Besra**, "wave to vec" for **wav2vec**, "Xaralda" for a program the speaker
names as a German language-documentation tool (unverified spelling; no
independent match found — see Open questions). Quotes below are corrected by
ear against context; timestamps refer to the source video.

## What Besra is

### Motivation and positioning

The author frames Besra as roughly ten years in gestation, starting from a
conversation with a colleague, Andre Godrich (unverified spelling), about
wanting to script phonetic analysis "in Python or even JavaScript" instead of
Praat's own scripting language while both were master's students
([00:07:06](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=426)). The stated
reason to build it now, in 2026, is that "the technology is good enough" —
both general tooling and machine-learning options — to make a rewrite
practical (opening minutes of the talk).

The problem statement is a fieldwork data bottleneck: recordings pile up
faster than they can be transcribed. The author cites an estimate of 4–10
hours of transcription work per hour of recorded speech, then asks an
audience member ("David") how long his first fieldwork recording took to
transcribe — "a long, long time" for an eight-minute recording, "a few
weeks" — as a live illustration. Because of this cost, fieldwork audio is
"recorded, but it might as well not exist."

The author surveys existing tools and states specific complaints against
each (all early in the talk, roughly 00:03–00:10):

- **SIL tools (e.g. FLEx)** — Windows-only in large part, Bible-translation
  focused, and prone to abandonment ("it's become kind of a joke that you
  don't know how quickly they're just going to give up").
- **ELAN** — fine for basic transcription, but the moment dependencies
  between a source-language tier and a translation tier are involved, "this
  gets complicated very quickly," to the point that ELAN's own maintainers
  run advanced-ELAN workshops the speaker says he still doesn't fully
  understand despite a decade of use ([00:04:53](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=293)).
- **Praat** — scriptable, but Praat script is "idiosyncratic," and writing one
  is called "probably the least favorite part of any phonology student's
  phonology class."
- **A German program the transcript renders "Xaralda"** — praised for
  responsive developers but "not widely used," and criticized for using XML
  as its core data structure, which the author says is a poor fit for version
  control (diffing/merging transcripts in Git).

### Architecture, as stated

- **Pure Python.** "Bessra is made entirely in Python, which means you can
  script any feature you want in Python" — no Praat-script equivalent to
  learn ([00:10:09](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=610)). The
  author preemptively addresses the "Python is slow, not ideal for UI"
  objection by saying this can be true but isn't necessarily, citing testers
  ("David and Luca," "Patrick") who worked on responsiveness.
- **ffmpeg for audio I/O**, instead of the WAV-only behavior of ELAN and
  Praat — the author calls ffmpeg "the foundation of all digital audio in the
  universe" and notes this matters practically because consultants send
  recordings over WhatsApp in whatever format WhatsApp produces
  ([00:10:30](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=630)).
- **Backward file compatibility** with existing ELAN and Praat project files
  ("you don't have to lose all of your existing files that you've been
  working on ... for 10 years").
- **Interface localization**: English, German, Chinese (simplified), Irish,
  Maltese, and Kamyungan (~6,000 speakers, northeast India) are named as
  interface languages already added, framed as a way to let community
  members work in the tool without needing English computer-literacy concepts
  like "copy and paste" ([00:11:22](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=682)–[00:11:40](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=700)).
- **Theming**, including a skin that mimics ELAN's look, to ease the
  transition for existing ELAN/Praat users.
- **Plugin system in plain Python** with full access to "all of the raw
  data, everything in the user interface" — demoed live as a plugin that
  connects to the author's own Phonemica corpus website via its API to pull
  and re-upload recordings ([00:23:51](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1431)).
  Because plugins are Python, the author says any PyPI package is available
  inside a plugin — Hugging Face models, Allosaurus, LingPy are named
  examples.
- **"User scripts"** as a layer above plugins — named sequences that chain
  plugins together (e.g. "do this plugin, then do this plugin, then save the
  file") to batch-process a backlog of recordings.
- **Export targets**: CLDF (Cross-Linguistic Data Format) datasets,
  "storybooks," and by-design version-control-friendly output — plugin
  settings are saved as TSV rather than the XML the author criticized ELAN
  and "Xaralda" for.

### UI and interaction, as shown in the recording

Described concretely from the screen-share portion of the talk
(~00:15:00–00:33:00), in viewing order:

- A conventional multi-pane editor: waveform, spectrogram, and an optional
  amplitude track, with formant and pitch overlays toggled on/off — visually
  close to Praat/ELAN's layout by the author's own comparison ("this is just
  Elon. This is just prot.").
- Tiers are created by name and typed into directly, same interaction model
  as ELAN/Praat tiers.
- **Hands-off transcription**: the author opens a Maltese political speech
  recording, triggers transcription, and lets it run in the background while
  talking, then returns to a filled-in IPA tier
  ([00:17:00](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1020) area). The
  demo model was trained "in about an hour" on roughly 2 hours of Mozilla
  Common Voice Maltese audio, after the author manually filtered out
  low-user-score and code-mixed clips.
- **Model chaining per recording**: after IPA transcription, a second trained
  model refines word boundaries, and a third handles orthography conversion
  — explicitly framed as more than a lookup table, because Maltese has
  homophones that map to different spellings depending on context, so the
  third model is context-aware.
- **A second language demo (Kamyungan / "Kamyungan")**: one click runs
  silence-based-but-model-assisted sentence segmentation, then word-boundary
  detection, then forced alignment of segments, automatic tone-contour
  detection (the author notes wav2vec-style models don't do this natively),
  syllabification, and orthographic rendering — chained as one pipeline
  triggered from a single action.
- **Plugin/model switcher**: a dropdown to choose which trained model
  ("plugin") is active per task, demoed switching from the Maltese model to
  the Kamyungan model and re-running transcription live.
- **Private Use Area (PUA) glyph encoding for training targets.** Described
  around [00:20:46](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1246): ASR
  frameworks like wav2vec are described as unable to natively handle a
  transcription unit made of multiple Unicode codepoints (e.g. a base
  consonant plus an aspiration diacritic) as a single training target, so
  Besra remaps such multi-codepoint IPA sequences to single PUA codepoints
  before training, then remaps back for display. The author names diphthongs,
  vowel length, and Semitic hiatus (e.g. Arabic /dasa/ vs. /daːsa/, spelling
  per the speaker's own example) as cases this measurably improves.
- **Auto-segmentation with an adaptive silence model**: not a fixed
  amplitude threshold — the author demos a "floating window" that adapts if
  ambient noise changes mid-recording (example given: a thunderstorm starting
  partway through a field recording).
- **Split-at-cursor**: cut a long annotated interval at the playhead; both
  halves inherit the correct half of the original text automatically
  ([00:22:20](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1340)).
- **Audio editing** is present but the author explicitly discourages using it
  ("you shouldn't do that. Always leave your audio alone if you're doing
  documentation, but it's in there because people will do it").
- **Vowel plotting**: F1×F2 plots generated directly from automatically
  determined (not manually transcribed) formant values, with a choice of
  plot styles including a density-contour vowel plot, described as scriptable
  in the same Python environment as everything else.
- **Speaker binning**: a per-recording speaker-diarization pass using
  acoustic cues (harmonics, F0) that the author says performs with "really
  high accuracy" on tested recordings, once told how many speakers are
  present ([00:31:23](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1883)).
- **Quality-check bars**: three or more colored bars per recording/segment
  scoring things on a 0–100 scale — one for general audio-training
  suitability (signal-to-noise ratio, lossy-encoding detection via spectral
  cutoff, noise-floor characterization, amplitude bands), and others tied to
  ongoing work by "Abishek Stephen" on phonotactic surprisal
  ([00:27:42](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1662),
  [00:29:39](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1779)).
- **A "phonological complexity score"** (work in progress, credited to
  "Carlo Maloney" and others): the stated goal is to tell a fieldworker, in
  advance, how many minutes of audio they need to reach a target
  transcription accuracy, as a function of a language's phonotactic
  complexity and the recording's measured audio quality — the author's
  example: "if it's Hawaiian ... you need 10 minutes"
  ([00:30:32](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=1832)).
- **Automatic interlinear glossing** by cross-referencing Lexibank word
  lists from related documented languages/dialects, demoed on Chunki (a
  Tibeto-Burman language of northeast India not itself in Lexibank, glossed
  via related dialects that are).
- **Confidence intervals on formant estimates**, drawn on the formant
  overlay, based on unspecified "research that has been done on this
  particular feature" (no citation given in the talk).

### Status, distribution, and paper

- **No public code repository found.** Searches of GitHub (user/org
  `patkaiist`, `phonemica`, and a general `besra` name search via the GitHub
  API) turn up nothing under this name; the pinned and visible repos on
  `github.com/patkaiist` are Glottolog, Concepticon, EDICTOR, and a Phonemica
  data repo — none is Besra
  ([github.com/patkaiist](https://github.com/patkaiist)).
- **`besra.net` is unrelated** — it is the site of "BESRA AcadEx U.G.," a
  German conference/journal-hosting business, coincidentally same-named
  ([besra.net](https://besra.net/)).
- **No PyPI package**: `pypi.org/pypi/besra/json` returns 404 as of this
  check (2026-07-17).
- **No paper.** The author's Google Scholar profile lists 2024–2026
  publications on EDICTOR, historical-linguistics typology, and a 2026 paper
  on phonotactic-inconsistency detection in wordlists, but nothing titled or
  evidently about Besra
  ([scholar.google.com/citations?user=uYZH1joAAAAJ](https://scholar.google.com/citations?user=uYZH1joAAAAJ&hl=en)).
  arXiv/LingBuzz-style searches for "Besra" plus audio/ASR/fieldwork terms
  return no hits.
- **The talk itself states a release target**: "my plan is to have this
  available for those who are interested July 1st"
  ([00:35:25](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=2125)), with an
  initial console-only Python distribution "because I get better debugging
  information," followed by Windows/Linux/macOS binaries with heavier
  model-training components stripped out for a smaller download. The author
  also says at least one phonetics course in Taiwan plans to use Besra
  instead of Praat next semester, because students "are revolting about
  having to learn Praat script"
  ([00:35:44](https://www.youtube.com/watch?v=uaNj-dnbj4g&t=2144)).
- Today's date is 2026-07-17, past the stated July 1st target, but no public
  release, mailing list, or landing page was found in this research pass.
  **Verdict: unreleased as of this writing** — status is most plausibly
  "distributed informally to people who contacted the author," not yet
  indexed anywhere public. This is unverified either way; it could also mean
  the release slipped, or exists behind a non-indexed link (institutional
  page, direct email). No license was stated in the talk or found anywhere
  else, so "open-source" cannot be confirmed or denied — treat as unverified.

### Open questions (unverified)

- The exact spelling of "Xaralda" — the German tool the author name-checks
  as ELAN's closest relative and criticizes for XML-as-data-structure. No
  matching project was found under several spelling guesses; the sound is
  consistent with an auto-caption mangling of a real name.
- Whether "Besra" has any relation to the Tangsa/Nocte/Patkaian naming
  convention the author uses elsewhere (his own site is `patkai.ist`, and his
  research area is literally called "Patkaian" — see
  [patkai.ist](https://patkai.ist)). No etymology for the tool's name was
  stated in the talk or found on his site.
- Full author-team roster: "David," "Luca," and "Patrick" are thanked by
  first name only as testers; "Andre Godrich" (spelling unverified) as an
  early collaborator; "Abishek Stephen" and "Carlo Maloney" (spelling
  unverified) are credited with the phonotactic-surprisal and
  phonological-complexity-score subsystems respectively. None of their
  surnames or affiliations beyond first names are confirmed here.

## Relevance to Phonia

### Direct overlap with items already in `../plan/horizon.md`

- **ELAN/Praat interop (H1 §3)**: Besra's pitch is explicitly "we keep your
  existing ELAN and Praat files working." Phonia's horizon item 3 is
  scoped narrower — read/write `.eaf` round-trips — but the same
  fieldworker pain point (losing a decade of existing annotation files to a
  format switch) is the evidence base for both. Nothing here changes the
  approach already planned (`phx-eaf`, fixtures against ELAN's published
  schema); it is independent confirmation the gap is real and current.
- **Broad decode: formats and containers (H1 §4)**: Besra solved this by
  wrapping ffmpeg rather than a Rust decoder crate. Phonia's plan (symphonia,
  wasm-compatible, in-binary) is architecturally different by necessity — a
  local-first WASM/Tauri app can't shell out to a system ffmpeg binary the
  way a Python desktop tool can — but the underlying user complaint (ELAN
  and Praat are WAV-only, fieldworkers get MP4/WhatsApp-format audio) is the
  same one already cited via CV §2.
- **Forced alignment and auto-IPA (H2 §7, investigation-first)**: Besra
  ships silence-adaptive segmentation, wav2vec-based transcription, word
  boundary detection, and forced alignment as a live one-click pipeline.
  This is squarely the landscape horizon item 7 says to survey (Montreal
  Forced Aligner / Charsiu / WebMAUS) before deciding build-vs-integrate —
  Besra is now a fourth data point, notable because it's Python/wav2vec-based
  rather than the Kaldi/torch tools already named, and because it demos
  training a usable model from ~2 hours of audio in about an hour on
  commodity hardware. Worth adding to that investigation's landscape survey
  when it runs.
- **Analysis plugin surface (H3 §18, investigation-first)**: Besra's plugin
  system — plain Python, full data access, no sandboxing — is the same
  shape of feature horizon item 18 is scoping, but the opposite security
  posture from what item 18 proposes (WASM component plugins, sandboxed).
  That's a real trade-off worth naming explicitly when that investigation
  runs: Besra's "any PyPI package, no ceremony" plugin model is exactly what
  gives it Hugging Face/Allosaurus/LingPy access for free, and exactly what
  a Rust/WASM sandboxed plugin ABI would have to give up or work much harder
  to preserve.
- **Labeling workflows (H2 §14)**: adjacent, not equivalent. Besra's
  colored quality bars (0–100 audio-suitability score, phonotactic-surprisal
  bars) answer "is this segment good training data," not horizon item 14's
  "which spans need human review" — different question, similar visual
  language (a confidence/quality bar next to a span). No action needed here
  beyond noting the adjacent idea exists.
- **Video-synced annotation (H2 §12)**: not mentioned anywhere in the talk.
  Besra appears to be audio/text-only; this is not a point of overlap.
- **Python bindings (H3 §15)**: overlap in ecosystem, not architecture — see
  below.

### Ideas with no current horizon item — proposals, clearly marked as such

These are not commitments; they are candidate additions surfaced by this
research, for the same kind of investigation-first treatment horizon.md
already uses elsewhere.

- **PROPOSAL — training-target atomicity for multi-codepoint IPA units.**
  Besra's PUA remapping trick (collapse a diacritic+base sequence into one
  private-use codepoint before feeding a model, expand back for display) is
  a concrete, testable idea for whichever forced-alignment/auto-IPA
  investigation (H2 §7) eventually happens, if Phonia trains or fine-tunes
  any model on IPA sequences. It's a data-representation trick, not a UI
  feature — low cost to evaluate, worth a citation in that investigation's
  writeup rather than a horizon item of its own.
- **PROPOSAL — pre-flight audio/data-sufficiency estimate.** Besra's
  "how many minutes of audio do you need" score (phonotactic complexity ×
  measured audio quality) is a genuinely new idea with no Phonia analog.
  It's speculative even in Besra itself (the author calls it "work that's
  currently being done," not shipped and validated), so any Phonia version
  should be investigation-first and should not be scheduled ahead of the
  forced-alignment work it would depend on.
- **PROPOSAL — interface localization for community fieldwork use.** Besra
  treats non-English, non-technical-vocabulary interface language as a
  first-class fieldwork requirement, not an afterthought i18n pass — framed
  around getting the tool into the hands of community members with no prior
  computer literacy. Phonia has no horizon item addressing this; it's a
  legitimate fieldwork-adoption blocker independent of any acoustic/DSP
  work, and cheap to scope once the UI text is reasonably stable.
- **PROPOSAL — a Praat/ELAN-mimicking visual theme**, purely as a migration
  aid for muscle-memory transfer, distinct from Phonia's own design
  language. Minor, low-priority, but a real onboarding-friction reducer
  Besra called out by name.

### Where the two projects diverge, and could complement rather than compete

The load-bearing architectural difference is Python-all-the-way-down versus
Rust-core-with-bindings. Besra is written in Python end to end — UI included
— explicitly so that "if they do know any coding, they'll know maybe Python
or R," and so that any PyPI package (Hugging Face, Allosaurus, LingPy) is a
plugin away with no FFI. Phonia's plan is the inverse: a Rust core
(`phx-*` crates) for correctness and performance, with horizon item 15
proposing a `phonia` Python package as bindings over that core, parselmouth-
shaped — Python as a client of a compiled engine, not the engine itself.

That difference is a real trade-off, not a bug on either side: Besra can
absorb any Python ML ecosystem release the same day it lands on PyPI, at the
cost of Python's UI-responsiveness ceiling (which the author spent
real effort mitigating, by his own account, rather than getting for free).
Phonia's Rust core buys a single local-first binary with no Python runtime
dependency and (per horizon item 11) an actual mobile PWA story, at the cost
of every ML integration needing either a native Rust reimplementation or a
Python-bindings round-trip that Besra doesn't need.

Given that, the more useful framing than "competitor" is where a fieldworker
would reach for one over the other:

- A fieldworker who already scripts in Python/R, wants one-click wav2vec
  model training, and is comfortable with a Python install/console workflow
  is Besra's target user today, by the author's own framing ("Linguists are
  not programmers, but if they do know any coding, they'll know maybe
  Python or R").
- A fieldworker who wants a single downloadable binary, offline-first,
  with no Python environment to manage, and who is doing manual
  acoustic-phonetic work (formant/pitch/intensity measurement, TextGrid-style
  annotation) rather than ASR-model training, is closer to what Phonia
  ships and is aiming at per `../plan/BRIEF.md`.
- If Phonia's horizon item 15 (Python bindings) ships, a
  Besra-style plugin could in principle call into Phonia's Rust engine for
  the acoustic-analysis primitives (formant/pitch confidence, spectral
  measures) that Besra's own quality-score subsystem is still hand-rolling —
  a plausible complementary integration point, not proposed as a commitment
  here, just noted as architecturally possible once both sides of that
  bridge exist.

## Sources

- [Besra talk recording](https://www.youtube.com/watch?v=uaNj-dnbj4g) — "Kellen
  Parker van Dam (Passau): Besra — a tool & workflow for rapid audio/text
  processing," uploaded by Nathan Hill, recorded 2026-06-23, 36:01 duration.
  Auto-captions pulled via `yt-dlp --write-auto-sub --sub-langs en`.
- [patkai.ist](https://patkai.ist) — author's site; no mention of Besra found.
- [github.com/patkaiist](https://github.com/patkaiist) — no Besra repository.
- [besra.net](https://besra.net/) — unrelated same-named organization.
- [scholar.google.com/citations?user=uYZH1joAAAAJ](https://scholar.google.com/citations?user=uYZH1joAAAAJ&hl=en) —
  publication list, 2024–2026, no Besra paper.
- [geku.uni-passau.de — Kellen Parker van Dam](https://www.geku.uni-passau.de/en/mcl/team/kellen-parker-van-dam) —
  institutional bio, University of Passau, Chair of Multilingual Computational
  Linguistics.
- [sciences.social/@patkaiist](https://sciences.social/@patkaiist) — Mastodon
  profile; no Besra-related posts surfaced in this pass.
- `pypi.org/pypi/besra/json` — checked 2026-07-17, returns 404 (no package).
- `../plan/horizon.md` — Phonia's own long-range roadmap, cited throughout
  above by item number.
