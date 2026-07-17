# Horizon — direction after v0.1 and phase 8

Long-range roadmap synthesizing `../research/community-voices.md` (cited below
as CV §n) and `../research/owner-ideas.md` (OI §name) against what the
codebase already ships. Items are grouped into three horizons: H1 builds on
primitives that exist today, H2 opens new subsystems, H3 grows the ecosystem.
Each item states what it is, its evidence, what it builds on, and a
concrete first step. Items marked *investigation-first* start with a
research phase and a written verdict before any implementation is scheduled.

DESIGN.md binds all UI work here. Clean-room rules from
`tasks/phase-1.md` bind all algorithm work: published literature and public
documentation only, oracles as black boxes, no GPL source consulted.

## Standing practices

- **Upstream Praat tracking** (CV §12). Praat releases every two to three
  weeks; a replacement needs a running diff, and a one-time snapshot goes
  stale. Practice: each month, review
  `github.com/praat/praat/releases` since the last check and append findings
  to `../research/upstream-praat.md` (new capabilities, changed defaults,
  anything touching oracle parity). First step: create the log with the
  current state (v6.6.30, three-author credit as of v6.4.67).
- **Playback reliability as a stated guarantee** (CV §3). Praat's
  first-play truncation and its OS-level workaround folklore are a warning:
  playback must work the first time on every shell. The sample-counter clock
  (`phx-playback`) is the structural half; the missing half is a test.
  First step: add a first-play integration test (cold engine → play → assert
  full-length delivery) to the web e2e and the phase-6 desktop gate.

## H1 — near: builds on shipped primitives

1. **Channel operations** — extract, delete, mute, and mix channels as
   journaled commands with a channel strip in the corpus row and editor.
   Evidence: CV §7 (the unanswerable "delete channel 1" question; Praat
   offers only copy-out commands). Builds on: planar per-channel buffers in
   `phx-audio`, the engine journal, the streamed store. First step:
   `Command::{ExtractChannel, RemoveChannel, MixToMono}` with id-stable
   inverses, wasm bindings, channel badge UI.
2. **Vowel-space analytics** — an F1×F2 chart (IPA targets, per-language
   reference sets, points from labeled intervals) as both an app panel and a
   `phx-figure` layer. Evidence: CV §11 (Ian Howell / VoceVista precedent;
   the delta-segmentation panel in the owner's screenshot is the owner's own
   extension, unpublished elsewhere). Builds on: `phx-formant` means over
   spans (T8.2 `formant_span_means`), the figure model, dataviz-conformant
   palettes. First step: `Layer::VowelSpace` in `phx-figure` with
   a reference-dataset format (license-checked), then the app panel.
3. **ELAN interop** — read and write `.eaf` so fieldwork round-trips stop
   degrading. Evidence: CV §8 (ELAN-first fieldworkers; export/import cycles
   are a named engineering concern for ELAN's own authors). Builds on:
   `phx-annot`'s hierarchical tiers (modeled on ELAN already),
   `phx-textgrid`'s reader discipline (fixtures, fuzzing, typed errors).
   First step: `phx-eaf` crate, EAF XML schema from ELAN's
   published documentation, round-trip fixtures authored + oracle-generated
   via ELAN itself if redistributable.
4. **Broad decode: formats and video containers as audio sources** — open
   FLAC/OGG/MP3/AIFF and the audio track of MP4/MOV/WebM directly, so a
   phone-video recording opens like any WAV. Evidence: CV §2 (the
   iPhone-video workflow), OI §Mobile. Builds on: symphonia already pinned
   behind `decode-extended`, the streamed-source path (T8.3), format-probe at
   import. First step: enable symphonia demux/decode features
   with a wasm binary-size audit (report the cost per codec), fixtures per
   format, streamed equality tests.
5. **IPA input, stage 1** — a click-to-insert IPA pad on the label editor
   plus a search-by-feature picker in the command-palette register, and a
   user-selectable IPA display font. Evidence: CV §6 (Praat added its pad in
   2017; before that, trigraph folklore), OI §UX (input tool; font choice).
   Builds on: TierPane label editor, the palette's fuzzy-search pattern,
   `--font-ipa` tokens. First step: a pad popover fed by a
   feature-tagged symbol table (source and license verified, e.g. the IPA
   chart data used by established open keyboards), font preference in
   settings.
6. **Praat-parity I/O completeness check** — a standing checklist mapping
   Praat's Open/Save menu (LongSound, stereo handling, Kay/NSP and other
   niche formats) against Phonia's importers, so "at least what Praat can"
   (the owner's floor) becomes a tracked table rather than an impression.
   Evidence: CV §2; owner directive. Builds on: format probe above. First
   step: the table in `../research/format-parity.md` with
   citations to Praat's manual pages.

## H2 — mid: new subsystems

7. **Forced alignment and auto-IPA** (*investigation-first*) — transcript →
   time-aligned phone tiers, and audio → suggested IPA, always as
   suggestions a human confirms (the PraatPlusPlus template: assist, never
   auto-commit — CV §14). Evidence: owner directive (auto-segmentation,
   auto-IPA); CV §8 (fieldwork segmentation pipelines). Landscape to judge:
   Montreal Forced Aligner (Kaldi lineage), Charsiu (torch), WebMAUS
   (service API) — licensing, model weights, offline viability, and whether
   Phonia integrates, reimplements, or shells out. First step: an
   investigation phase with a written verdict; no implementation before it.
8. **VOT auto-detection** (*investigation-first*) — burst and voicing-onset
   candidates on stop tokens with confidence display. Evidence: owner
   directive. Landscape: AutoVOT and the published VOT-measurement
   literature; ground truth exists in open corpora. First step: a
   literature survey + oracle-dataset selection; then a DSP implementation
   phase if the verdict is build.
9. **Repair pipeline, stage 1** — noise-profile spectral subtraction
   (capture a noise span, subtract its profile) and a bounded-interpolation
   declick/repair action, as journaled, previewable operations. Evidence:
   owner directive (denoise, near-RX pipeline); CV §13 (Audacity's
   two-step noise reduction and 128-sample Repair as the documented
   baseline; RX's arbitrary-region model as the harder target). Builds on:
   the T8.2 spectral filter primitive, the tile cache for preview, the
   journal. First step: `phx-repair` crate with
   literature-cited spectral subtraction (Boll 1979 lineage; artifacts
   documented honestly), synthetic gates (SNR improvement, artifact bounds).
10. **Arbitrary-region spectral edit** — RX-register box/lasso attenuation
    on the spectrogram, beyond Audacity's filter-topology-only selections
    (CV §13 draws that distinction). Builds on: stage-1 repair, box
    selection, band-filtered render. First step: rectangular
    region attenuate/boost with preview, after item 9 lands.
11. **Mobile PWA** — installable, touch-first operation of the existing web
    app. Evidence: CV §1 (a decade-old unmet ask), OI §Mobile. Builds on:
    the client-side WASM architecture (already runs in mobile browsers),
    pinch handling (task #11's ctrl-wheel path is what mobile pinch emits
    via visual-viewport events — verify), OPFS persistence. First step:
    manifest + service worker + touch-target/responsive
    audit under DESIGN.md, tested via Playwright device emulation.
12. **Video-synced annotation** — show the video track beside the acoustic
    panes, synced to the engine clock. Evidence: CV §2 and §14
    (PraatPlusPlus ships this; fieldwork gesture/mouth context). Builds on:
    item 4 (container demux), the sample-counter clock, the shared viewport.
    First step: a `<video>` element slaved to the playback
    clock on web, frame-step controls, DESIGN.md pane treatment.
13. **Voice-training instruments** — a real-time practice view: live pitch
    and formant readouts against user-set target ranges with in/out-of-range
    feedback, session history, and export. A first-class use case serving
    transgender voice training among others; copy follows DESIGN.md's plain
    register and makes no clinical claims. Evidence: owner directive; OI
    §Voice (IPA-driven TTS interest adjacent). Builds on: the recording
    worklet's chunk stream, `pitch_track_span` speed (task #12 fix),
    formant span means. First step: streaming pitch estimate on
    the live mic path with a target-band meter; then the
    practice-view UI.
14. **Labeling workflows, stage 1** — named label schemes (closed
    vocabularies with colors) stored in project v2 metadata, digit-hotkey
    label application, and a review-queue view (unlabeled/low-confidence
    spans next). Evidence: owner directive citing CVAT/labelImg/EchoML et
    al.; CV §14 (confidence-scored regions precedent). Builds on: the tier
    keyboard loop, project v2 tags (T8.1), the palette. First step:
    scheme editor + hotkey application; review queue
    after.

## H3 — far: ecosystem and collaboration

15. **Python bindings** — a parselmouth-shaped `phonia` Python package over
    the engine API, because scripting users live there (the architecture
    was library-first for exactly this). First step: pyo3 crate
    exposing pitch/formant/intensity/voice over `phx-*`, with the oracle
    harness converted to consume it as its first real user.
16. **Praat-script compatibility** (*investigation-first*) — run existing
    `.praat` scripts against the engine. Carried from `roadmap.md`'s
    out-of-scope list; the palette's API-parity naming was groundwork.
    First step: corpus study of real scripts (what fraction uses which
    commands) before any interpreter design.
17. **Collaboration, stage 1** — advisory locking for shared project files
    (the PraatPlusPlus heartbeat pattern, CV §14) for lab NAS/drive
    workflows; CRDT-grade merging stays an open question until real demand.
    First step: design note only; no implementation scheduled.
18. **Analysis plugin surface** (*investigation-first*) — third-party
    analyses rendering as tracks/tiers without forking Phonia (the Vamp
    precedent, CV §13); candidate shape: WASM component plugins. First
    step: an investigation phase on plugin ABI options and sandboxing.
19. **Corpus-scale auto-labeling** — batch runs of items 7/8 with the
    review queue as the gate, per the assist-never-autocommit rule. Builds
    on: items 7, 8, 14. Scheduled only after those land.
20. **IPA handwriting input and IPA-driven TTS** — draw-a-symbol input (OI
    §UX) and read-aloud from transcription (OI §Voice); both
    *investigation-first* and behind the pad (item 5) in priority.

## Sequencing notes

- H1 items 1–5 are independent of each other and of phase 8's wave B; they
  can interleave into the schedule as capacity opens.
- Item 9 must precede 10; item 4 precedes 12; items 7/8/14 precede 19.
- Mobile (11) benefits from phase 8's T8.6 view work landing first so the
  touch audit hits final layouts once.
- Nothing here amends v0.1 or phase 8 scope; this document is the queue
  after them.
