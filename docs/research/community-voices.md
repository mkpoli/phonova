# Community voices: Praat frustrations and wishes

A citation-backed catalog of real-user Praat complaints, organized by theme,
anchored in the project owner's own tweets and the tweets they follow,
broadened with a general web search, and closed with an analysis of
PraatPlusPlus, a comparable multimodal-annotation project. Each theme lists
its evidence, then one line on what Phonia currently ships toward it or a gap
marker. Feature-state lines are checked against `README.md` and
`docs/plan/roadmap.md` as of this writing; they describe planned/implemented
scope, not aspirations.

Quoted-tweet sourcing note: three tweet URLs were supplied directly and
resolved via the fxtwitter API, including their reply/quote chain. The
remaining ~19 quotes were supplied by the project owner as handle+date+text
without an ID; a search pass (WebSearch across Japanese-language distinctive
phrases, nitter mirrors, GitHub/forum threads) found zero canonical URLs for
any of them — X's search index does not surface individual post bodies
reliably, particularly for Japanese text, and no nitter mirror returned live
results. Those are cited here as quoted material (handle + date, as supplied),
not as broken or fabricated links.

## 0. The owner's own multi-year thread

The three directly-supplied URLs form one dated arc, not three unrelated
posts:

- [@mkpoli, 2024-12-21 15:16 UTC](https://x.com/mkpoli/status/1870488395279180051):
  "Praatを使うのにあまりにもストレスフル / 現代的なソフトウェアで手間がかからない機能はないものだろうか😇"
  (Using Praat is far too stressful — isn't there some feature from modern
  software that doesn't take this much effort?)
- [@mkpoli, 2024-12-21 15:37 UTC](https://x.com/mkpoli/status/1870493745122247062):
  cites Martin "Tantacrul" Keary's MuseScore 4 redesign
  ([youtu.be/Qct6LKbneKQ](https://youtu.be/Qct6LKbneKQ)) as the model to apply
  to Praat: "Praatを零から再設計したい" (want to redesign Praat from zero).
- [@mkpoli, 2024-12-21 15:41 UTC](https://x.com/mkpoli/status/1870494841249153452):
  states the reason nobody has built a Praat replacement — acoustic phonetics
  is a niche within a niche field, with no realistic revenue base, no
  corporate backing, and a build that needs combined expertise in phonetics,
  physics, signal processing, and software engineering, so it's a losing
  proposition for anyone to attempt.
- [@mkpoli, 2024-12-21 15:45 UTC](https://x.com/mkpoli/status/1870495813992120543):
  "作りたいとは思うが到底他の事業で忙しくて作れる余裕はどうも少なくとも暫くは見込めないので、アイデアというかブループリントだけでもイメージして、私が考えた最強のPraatだけでも作って放流するか"
  (Want to build it, but too tied up with other work to find the time for the
  foreseeable future — so at least sketch the idea/blueprint and put out the
  strongest Praat I can imagine).
- [@mkpoli, 2026-05-09 12:31 UTC](https://x.com/mkpoli/status/2053090496512397427):
  "Can someone vibe code a new Praat? 😅" — quoted by @BlackCatAnalogy
  (2026-05-10, unresolved URL) linking Praat's own
  [v6.4.65 release](https://github.com/praat/praat.github.io/releases/tag/v6.4.65)
  and noting "まぁでもあれか、インターフェースとか新しくしてほしいというなら別科"
  (well, but if what you mean is wanting a new interface, that's a separate
  matter); @mkpoli's reply (2026-05-10, unresolved URL): "2026年のUI/UX基準ではありえないです😇
  ずっと作ろうとはしておりますがなかなか手が回らず" (by 2026 UI/UX standards
  it's unacceptable; I've kept trying to build one but haven't had the
  bandwidth).
- [@mkpoli, 2026-07-17 01:32 UTC](https://x.com/mkpoli/status/2077929436339675256)
  and its [companion repost](https://x.com/mkpoli/status/2077929291720147412):
  the owner reposted the December 2024 thread on the day this catalog was
  requested, re-surfacing the same complaint 19 months later.

The directly-evidenced span is December 2024 to July 2026 (about 19 months);
the owner's own framing of "about ten years" of tweeting this complaint is
their characterization and was not independently verified — no tweets prior
to December 2024 were supplied or located.

**Phonia status.** This document and the roadmap are that blueprint; Phase 0
of `docs/plan/roadmap.md` is the "reset and scaffold" step following directly
from the December 2024 thread's stated intent.

## 1. Mobile access

- [@awesomenewways, 2020-07-03](https://x.com/awesomenewways) (unresolved
  URL): "Praat が iPhone で使えたらなという" (wish Praat worked on iPhone) —
  @payao880PY replies "めっちゃ思います" (strongly agree). Owner's own
  annotation on the raw input: "Mobile access is our priority, too," alongside
  a [linked video](https://www.youtube.com/watch?v=MIzHwH9WmiI&t=254s).
- Web search corroboration: an open GitHub feature request, "Use Praat on iOS
  and Android"
  ([praat/praat.github.io#629](https://github.com/praat/praat/issues/629)),
  and a 2014 LINGUIST List query asking whether any linguist had found a
  usable tablet phonetics workflow
  ([LINGUIST List 25.2127](https://linguistlist.org/issues/25/25-2127)) — both
  show the request is long-standing and, as of the LINGUIST List post, still
  unanswered a decade ago.

**Phonia status.** `phx-wasm` compiles the analysis core to WebAssembly for a
browser app (`README.md`), which runs in a mobile browser without a native
port, but no touch-first mobile UI or native mobile app is in v0.1 scope —
gap beyond browser accessibility.

## 2. Format support, including video containers

- [@awesomenewways, 2020-06-07](https://x.com/awesomenewways) (unresolved
  URL): describes recording on an iPhone as video (the easiest way to attach
  audio to a tweet), which Praat then cannot read natively, forcing a
  re-recording inside Praat itself to keep audio and image in sync; and
  separately, that audio recorded in Praat needs conversion before it can be
  posted, so a single tweet showing both waveform and audio often ends up
  mismatched. Owner's own gloss on the raw input: "Praat is supporting too
  few formats, we should be infinite, even videos."
- No additional Praat-specific video/format complaint threads surfaced in a
  general web search; this theme rests on the tweet evidence.

**Phonia status.** Phase 1 of the roadmap scopes `phx-audio` to WAV decode
only; broader container/codec support (including video containers) is not in
v0.1 — open gap matching the complaint directly.

## 3. Playback reliability

- [@awesomenewways, 2020-07-23](https://x.com/awesomenewways) (unresolved
  URL): reports that Praat's first playback of a sound is delayed and cuts
  off partway through, for unknown reasons; the workaround is to start
  playback, then trigger it again before the first playback finishes, so the
  second attempt plays cleanly through.
- Web search corroboration, beyond the one tweet: an open GitHub issue,
  "cannot play / view & edit"
  ([praat/praat.github.io#1932](https://github.com/praat/praat/issues/1932));
  a Praat-Users-List thread, "praat failing to play audio"
  ([groups.io](https://groups.io/g/Praat-Users-List/topic/praat_failing_to_play_audio/90538272));
  a Linux-specific GitHub issue, "Audio playback problems in Linux"
  ([praat/praat.github.io#87](https://github.com/praat/praat/issues/87)); and
  a third-party troubleshooting page listing OS-specific workarounds —
  switching audio backend to "Alsa via PulseAudio" on Linux, raising the
  "Silence before" setting to at least 0.7 seconds for short clips, and
  changing sample bit-depth in the OS sound settings
  ([swphonetics.com](https://swphonetics.com/praat/technical-stuff/troubleshooting-playback)).
  A workaround catalog this broad (OS-level backend switching, manual buffer
  padding) indicates the fault sits below Praat's own playback logic, in how
  it talks to each platform's audio stack.

**Phonia status.** `crates/phx-playback/src/clock.rs` implements a
sample-counter clock explicitly designed so playback position is locked to
the audio stream by construction rather than tracked against wall-clock time,
which is the structural fix for drift/delay-class bugs like the one reported
— ships toward this theme by design, not yet validated against the specific
first-playback failure mode on all three target OSes (Phase 6 gate).

## 4. Discoverability and the objects-model trap

- [@awesomenewways, 2019-07-28](https://x.com/awesomenewways) (unresolved
  URL): tries to get "Draw with Pitch" from a submenu after multi-selecting a
  Sound and TextGrid; the submenu doesn't appear for the TextGrid alone, and
  selecting Sound and TextGrid together only produces an error message
  telling them to select Sound and Pitch together — with no pitch curve drawn
  either way. The tweet ends with the realization: "ピッチオブジェクトを作ると
  いうステップのことを全く知らなかった" (I had no idea at all that you needed
  a step to create a Pitch object first).
- [@awesomenewways, 2020-07-16](https://x.com/awesomenewways) (unresolved
  URL): knows pitch can be manipulated in Praat but has never done it,
  because the UI is difficult enough in various ways that they don't feel
  motivated to try; replies from @hedalu244 ("わかる（できない）" — I get it,
  can't do it either) and @vericava ("えっできそう，今度やってみる" — oh, that
  looks doable, I'll try it sometime) show the same hesitation and its
  opposite in the same thread.
- This corroborates, rather than duplicates, the "Select-then-act object
  model" and "Objects-window workflow" sections already in
  `docs/research/praat-features-and-pain-points.md` — the addition here is a
  concrete, named failure instance (Draw with Pitch specifically) rather than
  the general pattern.

**Phonia status.** Phase 2 of the roadmap builds pitch/formant/intensity as
always-available overlay tracks in the editor with a non-modal, live-updating
inspector, rather than separate Objects-window entities the user must create,
name, and co-select before drawing anything — this removes the missing-step
class of failure by design (no analog to "create a Pitch object" exists as a
prerequisite action).

## 5. Figure-making friction

- [@awesomenewways, 2019-11-01](https://x.com/awesomenewways) (unresolved
  URL): "Praat で画像作るの面倒くさいから嫌い。" (I dislike Praat because
  making images in it is a hassle.)
- Corroborates the "Picture-window painter canvas" pain point already
  documented in `praat-features-and-pain-points.md`; no new mechanism found,
  just a direct user statement of the cost.

**Phonia status.** Phase 5 of the roadmap ships a backend-agnostic figure
model with SVG, PDF, PNG, PGFPlots/TikZ, Typst/CeTZ, Vega JSON, and
matplotlib/R/Julia code-plus-data exporters, with a live theme-aware export
preview — directly targets this complaint; gate criterion requires one
reference figure to export correctly through every backend.

## 6. IPA input

- [@awesomenewways, 2020-03-21](https://x.com/awesomenewways) (unresolved
  URL): "Praat最近になって明らかに機能増えましたよね？ なんかアノテーションす
  る画面にIPAパッドついてたし、再起動してもデータ消えなくなったし。" (Praat's
  clearly gained features recently, right? There's now an IPA pad on the
  annotation screen, and data no longer disappears after a restart.)
- Praat's own changelog confirms the IPA click-to-insert palette in the
  TextGrid editor was added in v6.0.24 (2017-01-23), with later refinements —
  v6.1.36 (2020-12-06) swapped a laminality symbol for a breathiness symbol,
  and v6.1.43 (2021-05-13) made the chart glyphs scale with window size
  ([What's new? — fon.hum.uva.nl](https://www.fon.hum.uva.nl/praat/manual/What_s_new_.html)).
  The restart/data-persistence half of the same tweet's observation was not
  found in the changelog; current Praat documentation and user-list threads
  describe the opposite (a crash loses unsaved Objects-window state, with no
  autosave) — that half of the tweet does not match a locatable source and is
  reported as unconfirmed rather than cited as fact.
- Before the pad existed, Praat's documented input method was to either type
  directly via an OS input method or use backslash-trigraph mnemonic codes
  (e.g. `s\.v` for a dotted-below retroflex diacritic), with separate
  vowel/consonant/diacritic reference pages to consult
  ([Phonetic symbols — fon.hum.uva.nl](https://www.fon.hum.uva.nl/praat/manual/Phonetic_symbols.html)).
  An archived Praat-Users-List thread, "Need help in using phonetic symbols
  in Praat textgrid," shows a user asking how the trigraph system works at
  all, evidence of the friction the pad was later added to fix.

**Phonia status.** Phase 3's TextGrid round-trip gate requires IPA and
combining-character labels to survive import/export losslessly, and its
keyboard-first annotation loop includes inline IPA label entry
(`docs/plan/tasks/phase-3.md`), but no IPA input palette or auto-IPA
transcription assist (the owner's stated "extremely easy/intuitive; auto-IPA"
direction) is scoped for v0.1 — partial coverage, entry works, ease-of-input
tooling is a gap.

## 7. Channel operations

- [@akikomuni, 2024-03-25](https://x.com/akikomuni) (unresolved URL): "Praat
  で、音声のinputが複数ある場合に、特定のchannelを消すのってどうやるのでしょ
  う・・・？🙏（この、channel 1をまるっと消したいみたいな）" (When a recording
  has multiple input channels, how do you delete one specific channel? I mean
  like wanting to wholesale delete channel 1.)
- Praat's manual splits mono/stereo handling across two separate commands:
  "Extract one channel..." (keeps one channel, discarding the other, and
  appends `_ch1`/`_ch2` to the resulting object's name) and "Convert to
  mono" (averages all channels together, discarding channel identity
  entirely) — [Extract one channel — fon.hum.uva.nl](https://www.fon.hum.uva.nl/praat/manual/Extract_one_channel___.html),
  [Convert to mono — swphonetics.com](https://swphonetics.com/praat/objects-window/convert-to-mono).
  There is no single "delete/mute this channel in place" action; the two
  available commands only produce a new object (a copy of one channel, or an
  averaged-down copy), which explains why the tweet's question has no
  one-step documented answer.

**Phonia status.** `phx-audio` stores audio as planar per-channel buffers
internally (`crates/phx-audio/src/lib.rs`), which is the right internal shape
for channel-level operations, but no extract/delete/mute-channel command
exists yet in any crate or the desktop app — gap.

## 8. Fieldwork and ELAN integration

- [@shimojizemi, 2024-08-23](https://x.com/shimojizemi) (unresolved URL,
  account confirmed as Michinori Shimoji, Kyushu University linguistics):
  describes using ELAN exclusively for fieldwork discourse transcription,
  interview data, and lexical-survey data management — building a "topic"
  annotation tier so that typing a questionnaire ID lets them pull just the
  matching example-sentence spans straight out of raw recordings — and notes
  that the ELAN-to-Praat handoff is smooth enough to check acoustic detail
  the moment something looks interesting. Owner's own gloss on the raw input:
  the ELAN×Praat integration is cool, and Phonia should aim for more advanced
  field-worker features on top of it.
- Web search corroboration: a field-linguistics blog post, "Time-saving magic
  for linguist fieldworkers: automatic segmenting with PRAAT and ELAN"
  ([humans-who-read-grammars.blogspot.com](https://humans-who-read-grammars.blogspot.com/2017/05/time-saving-magic-for-linguist.html)),
  and its follow-up on the same author's ELAN workflow, describing a concrete
  round trip — ELAN for time-aligned transcription, Praat scripts for
  automatic segmentation, results reimported into ELAN — with each hop
  requiring manual file export/import rather than a live link. An MPI
  technical paper, "ELAN — aspects of interoperability and functionality"
  ([pure.mpg.de](https://pure.mpg.de/rest/items/item_1321925_2)), documents
  that surviving repeated export/import cycles without alignment drift is a
  named engineering concern for ELAN's own authors, not only a user
  complaint.

**Phonia status.** `phx-annot` adopts ELAN's hierarchical parent/child tier
model rather than Praat's flat TextGrid tiers (`docs/plan/architecture.md`),
but no direct ELAN file (`.eaf`) import/export exists in the roadmap — the
tiering *model* is borrowed, file-level interoperability with ELAN itself is
a gap.

## 9. Community-size dynamics

- [@awesomenewways, 2020-07-16](https://x.com/awesomenewways) (unresolved
  URL): "ソフトの使いやすさってやっぱユーザー層の厚さと相関あるよな。Praat使
  ってる層が少なすぎるからPraatの類似ソフトが無数にあるという状況にならな
  い。" (Software usability really does correlate with the size of its user
  base. Praat's user base is too thin for there to ever be countless
  Praat-alternative programs the way there are for bigger categories.)
- [@awesomenewways, 2020-07-16](https://x.com/awesomenewways) (unresolved
  URL, same day): reports that a well-known Praat alternative failed to
  launch at all on a first machine; on a second try on Windows it launched
  but the microphone captured no input at all, and the attempt was abandoned
  there.
- This is a structural observation about the field rather than a single
  fixable bug: a small user base means fewer competing tools, less pressure
  on any one tool to improve, and less tolerance for a broken alternative
  before a user gives up and returns to the incumbent.

**Phonia status.** Not a feature gap — a market/adoption risk the project
should track (single-alternative attempts failing outright, as reported,
raise the bar for a new entrant's out-of-box reliability). No code marker
applies.

## 10. Icon and brand perception

- [@awesomenewways, 2019-03-15](https://x.com/awesomenewways) (unresolved
  URL): "Praat のアイコン、キモくない？" (Isn't Praat's icon kind of
  gross-looking?)
- [@awesomenewways, 2021-01-16](https://x.com/awesomenewways) (unresolved
  URL): "Praatきらい" (I dislike Praat).
- [@awesomenewways, 2019-06-29](https://x.com/awesomenewways) (unresolved
  URL): "Praatとても使いにくいので好きになれない / しかし代わりがない" (Praat
  is so hard to use I can't bring myself to like it — but there's no
  alternative.)
- [@awesomenewways, 2021-01-07](https://x.com/awesomenewways) (unresolved
  URL): "Praat一生慣れない気がする" (I don't think I'll ever get used to
  Praat, as long as I live.)
- Read together with the community-size theme above, these four map the same
  complaint from the brand/first-impression angle: dislike that persists for
  years, held alongside an acknowledgment that no alternative exists.

**Phonia status.** No icon or brand asset has been finalized in the
repository as of this writing; not a code gap, a design-pass item to carry
into Phase 7 polish.

## 11. Vowel-space and pedagogy dashboards

The owner's addendum shows a screenshot of a vowel-space pedagogy dashboard:
an F1×F2 scatter with IPA vowel targets plotted, a waveform pane, a
formant-track panel, and a "Delta (threshold = 5.6 Hz)" panel driving
automatic per-vowel colored-span segmentation on the timeline, endorsing Ian
Howell's work as the reference.

- Ian Howell, DMA, is a classical countertenor (Chanticleer) who was voice
  faculty at New England Conservatory 2013–2022, where he directed the
  graduate voice pedagogy program and founded the NEC Voice and Sound
  Analysis Laboratory
  ([ianhowellcountertenor.com](https://www.ianhowellcountertenor.com/),
  [academia.edu profile](https://newenglandconservatory.academia.edu/IanHowell)).
  He now runs Embodied Music Lab, focused on voice acoustics, perception, and
  functional vocal training
  ([embodiedmusiclab.com/about-ian-howell](https://www.embodiedmusiclab.com/about-ian-howell)),
  published *Hearing Singing: Functional Listening & Voice Perception*
  ([embodiedmusiclab.com/hearing-singing-book](https://www.embodiedmusiclab.com/hearing-singing-book)),
  and runs an annual Acoustic Vocal Pedagogy Workshop that teaches VoceVista
  Video Pro alongside Praat
  ([embodiedmusiclab.com/acoustic-vocal-pedagogy-workshop-2026](https://www.embodiedmusiclab.com/acoustic-vocal-pedagogy-workshop-2026)).
- The findable, citable precedent for the F1×F2 IPA-target scatter itself is
  Howell's associated tool VoceVista (successor to his earlier Overtone
  Analyzer): its Vowel Chart feature plots F1 against F2 with IPA symbols
  positioned at their approximate formant coordinates, drawn from selectable
  per-language reference datasets, with click-to-navigate
  ([VoceVista Vowel Chart documentation](https://www.vocevista.com/en/documentation/program-reference/ui-contents/rulers-and-vowelchart/),
  [VoceVista product page](https://www.vocevista.com/en/products/)).
- The delta-threshold auto-segmentation panel in the screenshot (an
  automatic vowel-boundary detector driven by a formant-velocity/derivative
  threshold) is not attributable to Howell or to VoceVista's documentation on
  the evidence found — VoceVista's Vowel Chart is documented as a manual
  reference/navigation display, not an automatic segmenter, and no search
  turned up a Howell-authored source describing threshold-based automatic
  vowel segmentation. That panel should be understood as the owner's own
  proposed extension of a real, citable pedagogy tool, not as a feature
  Howell has published.

**Phonia status.** `phx-figure`'s planned exporters (Phase 5) do not include
a vowel-space (F1×F2 scatter with IPA targets) chart type, and no
formant-delta auto-segmentation exists in `phx-formant` or `phx-annot` — open
gap on both the charting and the auto-segmentation half of this request.

## 12. Tracking Praat's own upstream development

The owner's framing treats Praat's authors as a moving target to watch, not a
frozen baseline to replace once.

- Praat's releases are published at
  [github.com/praat/praat/releases](https://github.com/praat/praat/releases)
  (the `praat.github.io` release-tag links the tweets use point at the same
  tags). The cadence is rapid: ten releases were published between
  2026-02-05 and 2026-06-30 alone (v6.4.59 through v6.6.30), roughly one
  every two to three weeks. Paul Boersma is the releaser of record on all
  recent tags; the v6.4.67 release notes credit Anastasia Shchupak as Praat's
  third author alongside Boersma and David Weenink.
- The two features the 2020-03-21 tweet (theme 6, above) attributes to a
  recent Praat update were checked individually against Praat's own
  changelog: the IPA click-to-insert palette is a real, dated addition
  (v6.0.24, 2017-01-23, with later refinements in v6.1.36 and v6.1.43); the
  restart/data-persistence claim in the same tweet does not match any located
  changelog entry and should be treated as unconfirmed.
- Recommendation: treat `github.com/praat/praat/releases` as a standing
  source to check periodically for feature parity and regression tracking,
  the same way `docs/research/praat-features-and-pain-points.md` treats the
  current feature set — Praat is actively maintained, not abandoned, so a
  replacement needs an ongoing diff against it rather than a one-time
  snapshot.

**Phonia status.** No standing process for this exists yet in the repository
— gap; this document is the first record of the practice being proposed.

## 13. Audacity technology, beyond `design-lessons.md`

`docs/research/design-lessons.md` already covers Tantacrul's MuseScore
4/Audacity 4 redesign method, its general UX principles, and a tool-landscape
table. The following is the audio-processing technology underneath Audacity
that document does not cover — relevant to the owner's stated denoising/
near-RX-pipeline direction.

- **Noise Reduction** is a two-step spectral-subtraction effect: select a
  noise-only region of at least 2048 samples, capture its frequency profile
  with "Get Noise Profile," then apply reduction (dB), sensitivity, and
  frequency-smoothing controls to the full selection
  ([Audacity Manual](https://manual.audacityteam.org/man/noise_reduction.html)).
  The manual documents it working well on constant noise (hiss, hum) and
  poorly on background speech or music, with the well-known "musical" or
  "robotic" artifact appearing when the Sensitivity/Reduction settings are
  pushed too far in pursuit of a cleaner result.
- **Repair** and **Click Removal** are two distinct tools for the same class
  of defect: Click Removal is an automated broadband declicker, while Repair
  is a manual, surgical tool that reconstructs a selection of up to 128
  samples (about 3ms at 44.1kHz) by interpolating from the audio immediately
  outside the selection, and refuses to run if there isn't enough
  surrounding context to interpolate from
  ([Repair — Audacity Support](https://support.audacityteam.org/repairing-audio/removing-clicks-pops),
  [Click Removal — Audacity Manual](https://manual.audacityteam.org/man/click_removal.html)).
- **Vamp** is a cross-platform C/C++ plugin format for audio feature
  extraction (pitch, beat, onset, note tracking) that Audacity hosts as an
  external, pluggable analysis layer rather than building every analysis
  algorithm into its own codebase; results render as label tracks. Notable
  plugins include pYIN (a YIN-derived F0 estimator) and the aubio bundle
  (onset/beat/pitch/MFCC extraction)
  ([vamp-plugins.org](https://www.vamp-plugins.org/plugin-doc/qm-vamp-plugins.html),
  [Audacity Analyze menu](https://manual.audacityteam.org/man/analyze_menu.html),
  [aubio Vamp plugins](https://aubio.org/vamp-aubio-plugins/)) — a precedent
  for an extensible, host-independent analysis-plugin surface, distinct from
  Audacity's own maintainers.
- **Spectral Selection / Spectral Edit Multi Tool** predates and differs from
  the iZotope RX interaction `design-lessons.md` already targets as the model
  for Phonia's spectral work. Audacity restricts spectral drag-selection to
  filter-topology actions only: a selection with a center frequency and upper
  and lower bounds becomes a notch filter; a selection touching 0 Hz becomes
  a high-pass filter; a selection touching Nyquist becomes a low-pass filter
  ([Spectral Edit Multi Tool](https://manual.audacityteam.org/man/spectral_edit_multi_tool.html),
  [Spectral Selection](https://manual.audacityteam.org/man/spectral_selection.html)).
  RX's model allows arbitrary-shape region selection with direct spectral
  repair and repaint; Audacity's is filter-shape-only. The distinction
  matters for Phonia's design: RX's arbitrary-region model is the harder and
  more valuable target, not something Audacity's own implementation already
  achieves.
- No 2024–2026 Muse Group or Tantacrul-authored material on Audacity's
  spectral tools, noise reduction, or a stem-separation feature surfaced in
  search beyond what `design-lessons.md` already cites (the Audacity 4 UI
  preview and the 2021 telemetry controversy) — reported as not found rather
  than assumed absent.

**Phonia status.** Denoising and a near-RX processing pipeline are stated
owner directions but are not scoped in v0.1 (`docs/plan/roadmap.md` phases 0
through 7 do not include a noise-reduction or spectral-repair crate) — gap,
explicitly deferred past the current roadmap horizon.

## 14. PraatPlusPlus

[UTA-ACL2/PraatPlusPlus](https://github.com/UTA-ACL2/PraatPlusPlus), flagged
by the owner as a candidate roadmap input.

**What it is.** A Flask (Python) and browser-JS web application for
multimodal speech/vocalization annotation — a standalone companion tool
inspired by Praat and by "Praat on the Web" (Domínguez et al. 2016), not a
Praat fork or patch. It is an accepted ACL 2026 System Demonstrations paper:
Zhang & Zhu, "Praat++: Multimedia Annotation System for Speech and
Vocalization"
([aclanthology.org/2026.acl-demo.80](https://aclanthology.org/2026.acl-demo.80/)).
Over stock Praat, it adds: video playback synchronized alongside
waveform/spectrogram/pitch/intensity views; drag-to-create/resize/move
region-based annotation with per-region confidence scores; multi-user,
role-based file-pool management with heartbeat-based file locking to prevent
simultaneous-edit conflicts; TextGrid import/export compatible with Praat;
and PANNs-based (a pretrained Cnn14 audio-tagging model) AI pre-annotation
with human-in-the-loop review
([README](https://raw.githubusercontent.com/UTA-ACL2/PraatPlusPlus/main/README.md)).

**License.** MIT
([github.com/UTA-ACL2/PraatPlusPlus](https://api.github.com/repos/UTA-ACL2/PraatPlusPlus)).

**Maturity.** Created 2025-04-30, last pushed 2026-07-08 — active and
recent. Five stars, one fork, no open issues, one contributor (eight commits)
— a single-author academic research prototype rather than a maintained,
multi-contributor tool. Its language breakdown (SCSS and CSS together over
1.1MB, JS 334KB, HTML 190KB, Python 79KB, Praat script only 2.2KB) shows a
heavily frontend-weighted single-server web app with a thin Flask backend and
a negligible amount of actual Praat-script code
([GitHub API — languages](https://api.github.com/repos/UTA-ACL2/PraatPlusPlus/languages),
[GitHub API — contributors](https://api.github.com/repos/UTA-ACL2/PraatPlusPlus/contributors)).

**What Phonia should take from it, and what it shouldn't.** There is no
code-reuse path — Flask/JS versus a Rust core with Tauri/WASM shells share no
runtime — so this is a feature/architecture reference only. Worth borrowing
as design ideas: heartbeat-based file locking for collaborative annotation, a
concrete pattern Phonia's `phx-project` (Phase 4) does not currently address
since Phonia's model is single-user, local-first; synchronized video-timeline
playback alongside acoustic tracks, directly relevant to both the format-
support gap (theme 2) and the ELAN/fieldwork theme (theme 8); and AI
pre-annotation gated through mandatory human review, a template for how the
owner's stated auto-segmentation/auto-IPA direction should be built (assist,
never auto-commit). Not applicable: the multi-user file-pool/role/login
system is server-hosted-collaboration infrastructure orthogonal to a
local-first single-binary design, and the PANNs dependency (pretrained
weights fetched separately by the user, not bundled) is a heavy Python ML
dependency inconsistent with a Rust-native core.

**Verdict.** A legitimate, recent, MIT-licensed academic prototype, useful as
evidence that video-synchronized, AI-assisted annotation is an active
research direction other teams are pursuing independently — a UX/feature-set
reference, not a source of reusable code.
