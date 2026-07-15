# Design lessons for a Praat replacement

Research notes for Phonova's v0.1 direction. Sources are linked inline; claims
not backed by a source are marked as such.

## 1. Tantacrul's design method (MuseScore 4, Audacity 4)

Martin Keary ("Tantacrul") built a public track record of software-design
critique before joining Muse Group: hour-long video reviews of
[Sibelius](https://yahnd.com/theater/r/youtube/dKx1wnXClcI/) (2018),
[MuseScore 3](https://musescore.org/en/handbook/developers-handbook/ux-design/design-reviews-and-responses/tantacrul-music-software)
(2019), and [Dorico](https://forums.steinberg.net/t/tantacruls-take-on-dorico/146905)
(2020). The MuseScore video led to his hire as head of design at Muse Group,
where he later became VP of Product and ran the redesigns of MuseScore 4 and
Audacity 4.

Principles extracted from his own account of the MuseScore 4 process
([martinkeary.com/musescore-4](https://www.martinkeary.com/musescore-4),
["How We Made MuseScore 4"](https://www.youtube.com/watch?v=Qct6LKbneKQ)) and
the Audacity 4 preview coverage
([CDM](https://cdm.link/audacity-4-in-ui-preview/)):

- **Collapse the funnel, don't just prettify it.** MuseScore's new-score
  workflow went from five separate steps/pages down to two, by merging pages
  that asked overlapping questions (instrument, template, title) into one
  screen.
- **Set the default to what people actually do, not what looks
  comprehensive.** User behavior showed people building scores by adding
  instruments one at a time far more than by picking a template, so manual
  instrument selection became the default path instead of the template
  gallery. Titles are auto-filled and left editable rather than presented as
  a blank required field.
- **Surface state instead of burying it in menus.** The instruments list
  moved from a File-menu dialog to a persistent, one-click panel, because
  changing instrumentation is a frequent action, not a one-time setup step.
- **Merge tools that do overlapping jobs.** Two separate playback panels and
  two separate text-entry/editing panels were each collapsed into one, on the
  reasoning that having near-duplicate controls in different places is a
  navigation tax, not added flexibility.
- **Use pop-up/contextual panels to cut baseline visual noise.** Rather than
  keeping every control visible at all times, MuseScore 4 shows tool options
  only when the relevant object is selected, keeping the default screen
  closer to just the score.
- **Treat community backlash as signal, but not as veto.** Community
  feedback (forum and social-media threads) fed into the redesign, but
  Keary's own retrospective records pushback he disagreed with (e.g.
  complaints about the concert-pitch toggle's new position) without reversing
  the decision — the team distinguished "this broke my workflow" from "this
  is objectively worse."
- **Audacity 4: fix refusal states before fixing looks.** The stated
  motivating problem was "too many cases where Audacity refuses to do
  something" (sync-lock conflicts, rigid track/channel handling) — friction
  from the app blocking valid actions, not just outdated visuals. The
  decluttering goal list published for Audacity 4 was explicit:
  producer-focused features, elimination of technical debt, migration to Qt,
  visual/usability improvements, and a foundation for future musical
  features, in that order.
- **Telemetry needs opt-in transparency, decided before backlash, not after.**
  A 2021 proposal to add telemetry to Audacity was read by the community as
  spyware and was dropped; Keary's later position was that any future
  telemetry would ship with explicit, visible consent rather than being
  bundled quietly
  ([Hackaday](https://hackaday.com/2021/07/13/muse-group-continues-tone-deaf-handling-of-audacity/),
  [GitHub discussion](https://github.com/audacity/audacity/discussions/1225)).
  The lesson generalizes beyond telemetry: opt-in, disclosed defaults for
  anything privacy-adjacent, decided before shipping, not retrofitted after
  a controversy.

Caveat: the Dorico critique video itself was not universally accepted by that
program's users — Steinberg forum members and others pointed out factual
errors and cases where Keary missed features that existed but were
undiscoverable to him as a first-time user
([Steinberg forums](https://forums.steinberg.net/t/tantacruls-take-on-dorico/146905)).
That is itself a design lesson: his critiques are consistently a first-run,
no-manual perspective — useful for finding onboarding failures, unreliable
for judging power-user depth.

## 2. Tool landscape — what each does better than Praat

| Tool | Strength Praat lacks | Relevance to Phonova |
|---|---|---|
| [ELAN](https://archive.mpi.nl/tla/elan) ([manual](https://www.mpi.nl/corpus/manuals/manual-elan_ug.pdf), [Wikipedia](https://en.wikipedia.org/wiki/ELAN_software)) | Tiered annotation model: independent tiers time-aligned to media, dependent tiers hierarchically linked to parent annotations (word → gloss → translation), cross-tier and cross-document search with regex | Praat's TextGrid tiers are flat and only loosely coupled by time; ELAN's parent/child tier linkage is closer to what phonological/morphological annotation actually needs |
| [EMU-SDMS](https://ips-lmu.github.io/The-EMU-SDMS-Manual/chap-overview.html) ([paper](https://www.phonetik.uni-muenchen.de/~jmh/papers/emucsl.pdf), [ICPhS 2019](https://www.internationalphoneticassociation.org/icphs-proceedings/ICPhS2019/papers/ICPhS_1366.pdf)) | Whole corpora treated as queryable databases rather than loose folders of files; derived signals (spectrograms, pitch tracks) computed on demand from R, with a documented import path from existing Praat TextGrids | Praat has no project/corpus concept at all — every file is opened and analyzed in isolation; a database layer is the single biggest structural gap |
| [WaveSurfer](https://www.isca-archive.org/icslp_2000/sjolander00_icslp.pdf) ([Wikipedia](https://en.wikipedia.org/wiki/WaveSurfer)) | Multiple synchronized, stacked panes (waveform/spectrogram/pitch/transcription) in one scrolling view with a shared cursor, plus a plugin architecture | The synchronized multi-pane layout is closer to a DAW timeline than Praat's separate floating windows |
| [VoiceSauce](https://www.internationalphoneticassociation.org/icphs-proceedings/ICPhS2011/OnlineProceedings/RegularSession/Shue/Shue.pdf) | Batch voice-quality measurement (jitter, shimmer, H1*-H2*, HNR, CPP) over whole directories, restricted to TextGrid-labeled intervals | Batch/headless analysis over a corpus, not a single file at a time |
| [Prosogram](https://sites.google.com/site/prosogram/home) ([paper](https://www.researchgate.net/publication/228979942_The_prosogram_Semi-automatic_transcription_of_prosody_based_on_a_tonal_perception_model)) | Perceptually motivated pitch stylization — reduces raw f0 to the tonal movements a listener actually perceives, syllable by syllable, with interactive resynthesis playback | A model for prosody visualization that is closer to auditory reality than a raw, jittery f0 trace |
| [praatIO / Parselmouth](https://parselmouth.readthedocs.io/) ([paper](https://pure.mpg.de/rest/items/item_2627915_2/component/file_2627914/content), [repo](https://github.com/YannickJadoul/Parselmouth)) | A typed, Pythonic API that calls Praat's actual C/C++ internals rather than reimplementing algorithms — exact numerical parity with Praat plus normal Python tooling (types, package ecosystem, error messages) | Praat's own scripting language has a small user base and thin tooling/documentation compared to Python; scripting ergonomics should target a general-purpose language with a native binding, not a bespoke DSL |
| [Gentle](https://github.com/strob/gentle) forced aligner | A local web server with both a browser GUI (drop file, get results in-browser) and a REST API, so the same alignment engine serves manual and scripted workflows | Local-first, dual GUI/API access to the same engine is a pattern worth reusing for alignment/ASR features |
| [Descript](https://help.descript.com/hc/en-us/articles/10255582172173-Keyboard-shortcuts) | Full-keyboard transcript editing (mode switches, word-level edits, comment threads) with text-as-the-primary-editing-surface | Editing audio by editing its transcript text is a proven interaction model for annotation-heavy workflows |
| Reaper / Logic (DAWs) | Deep zoom-level customization (mouse-wheel zoom, modifier-key vertical zoom, fully remappable actions) tuned for scanning long recordings fast | Praat's zoom/scroll interaction is comparatively rigid; DAW-grade timeline navigation is a bar to clear |
| [iZotope RX](https://www.izotope.com/en/products/rx/features/spectral-editor) ([docs](https://docs.izotope.com/rx11/en/spectrogram-waveform-display.html)) | A resizable, high-resolution spectrogram/waveform overlay with selectable time/frequency resolution trade-offs, direct click-drag spectral selection and repair, and a searchable module/action list | Widely regarded as the best spectrogram interaction in professional audio; the "select directly on the spectrogram, act on the selection" model is the target UX for spectral work in Phonova |

## 3. UX patterns to adopt for v0.1

- **Command palette** for the full action surface (Ctrl/Cmd-K style, as in
  VS Code, Sublime Text, Linear, GitHub). Concrete effect: every menu action
  and every less-common analysis function stays reachable without owning
  permanent screen space
  ([Mobbin](https://mobbin.com/glossary/command-palette),
  [Rob Dodson](https://robdodson.me/posts/command-palettes/)).
- **Non-modal inspector panels** instead of Praat's stacked modal dialogs
  for each analysis type — settings for pitch/formant/intensity display live
  in a docked panel that updates the view live, never blocks interaction with
  the signal underneath.
- **DAW-grade timeline zoom/pan**: scroll-wheel zoom centered on cursor,
  modifier-key vertical (amplitude) zoom independent of horizontal (time)
  zoom, remappable zoom/scroll bindings — matching Reaper's customizable
  navigation rather than Praat's fixed zoom buttons.
- **Spectral selection and direct manipulation** modeled on iZotope RX:
  click-drag a time-frequency region directly on the spectrogram and act on
  that selection (isolate, measure, export), instead of Praat's separate
  selection-then-menu-command flow.
- **Perceptually uniform, colorblind-safe spectrogram palette** — viridis
  (or a similar perceptually uniform, monotonic-luminance map) instead of
  Praat's default grayscale or ad hoc heat palettes, following the same
  reasoning that made viridis the Matplotlib 2.0 default
  ([viridis intro](https://cran.r-project.org/web/packages/viridis/vignettes/intro-to-viridis.html),
  [Roseus for Audacity](https://github.com/dofuuz/roseus)). Offer at least
  one alternate palette (e.g. grayscale) for print/publication figures, since
  phoneticians often need spectrograms for papers.
- **Tiered, hierarchical annotation** on the ELAN model rather than Praat's
  flat TextGrid tiers: dependent tiers that reference a parent annotation's
  span rather than only its own independent time alignment, so segment →
  syllable → word → gloss hierarchies are structural, not conventional.
- **Corpus-as-database, not file-as-universe** — an EMU-SDMS-style project
  layer that indexes many recordings/TextGrids together and supports
  cross-file query, rather than every file being its own disconnected
  session (Praat's model today).
- **Undo-everything, non-destructive by default** — every edit (label change,
  boundary move, spectral repair) goes through a single undo/redo stack, on
  the Photoshop non-destructive-editing model
  ([Adobe](https://helpx.adobe.com/photoshop/using/nondestructive-editing.html)),
  not Praat's per-window, often absent undo.
- **Autosave plus explicit project files** — continuous background autosave
  (à la Mail drafts or GitHub comment boxes) alongside a real save action, so
  users get both the safety net and the confidence signal of manually
  saving
  ([autosave pattern](https://ui-patterns.com/patterns/autosave)).
- **Keyboard-first annotation loop**: play/pause, nudge boundary, split
  segment, next/previous segment, and label-entry all bound to keys that
  don't require leaving the home row, on the Descript model of editing
  audio primarily through keyboard-driven transcript/segment actions.
- **Local-first with an optional server surface** — take the Gentle pattern
  of one engine exposed through both a local GUI and a REST/CLI API, so
  Phonova's analyses remain scriptable outside the GUI.
- **Dark mode and light mode as first-class, not inverted CSS** — spectrogram
  palettes and waveform colors need independent tuning per theme since a
  palette that reads correctly on white can wash out or clip on black.
- **Drag-and-drop import** of audio/TextGrid/annotation files onto the
  canvas or corpus panel, replacing Open-dialog-only file access.

## 4. Demo wow-moments for 2026 phoneticians

Five moments that would make a working phonetician stop and pay attention in
a live demo, based on the gaps above:

1. **Drop a folder of recordings in, get a queryable corpus in seconds** —
   no per-file setup, immediate cross-file search across annotations
   (the EMU-SDMS gap Praat never closed).
2. **Click-drag directly on the spectrogram to select and act** — isolate a
   frequency band, measure it, export it, with no separate selection object
   or modal dialog (the iZotope RX interaction Praat has never had).
3. **Undo a boundary move, a label edit, and a spectral repair from the same
   stack, instantly** — demonstrating that nothing in the app is
   destructive, unlike Praat where undo is inconsistent or absent per
   window.
4. **Script the same analysis from a command palette and from a Python cell,
   getting numerically identical results** — proving the GUI and the API are
   the same engine, not two implementations that can drift (the Parselmouth
   lesson).
5. **Switch a spectrogram from viridis to grayscale for a publication figure
   in one click, in both light and dark mode, and watch it stay legible** —
   a small moment, but one that signals the tool was actually designed for
   how phoneticians publish, not just how they explore data.

## Sources

- [MuseScore 4 — Tantacrul](https://www.martinkeary.com/musescore-4)
- [How We Made MuseScore 4 (YouTube)](https://www.youtube.com/watch?v=Qct6LKbneKQ)
- [Tantacrul — Music Software & Interface Design: MuseScore (MuseScore handbook)](https://musescore.org/en/handbook/developers-handbook/ux-design/design-reviews-and-responses/tantacrul-music-software)
- [Tantacrul's Take on Dorico — Steinberg Forums](https://forums.steinberg.net/t/tantacruls-take-on-dorico/146905)
- [HN Theater — Sibelius critique video discussion](https://yahnd.com/theater/r/youtube/dKx1wnXClcI/)
- [Audacity 4 UI preview — CDM](https://cdm.link/audacity-4-in-ui-preview/)
- [Muse Group Continues Tone Deaf Handling Of Audacity — Hackaday](https://hackaday.com/2021/07/13/muse-group-continues-tone-deaf-handling-of-audacity/)
- [Clarification of Privacy Policy — audacity/audacity GitHub Discussion #1225](https://github.com/audacity/audacity/discussions/1225)
- [ELAN — The Language Archive](https://archive.mpi.nl/tla/elan)
- [ELAN User Guide (PDF)](https://www.mpi.nl/corpus/manuals/manual-elan_ug.pdf)
- [ELAN — Wikipedia](https://en.wikipedia.org/wiki/ELAN_software)
- [EMU-SDMS Manual — overview](https://ips-lmu.github.io/The-EMU-SDMS-Manual/chap-overview.html)
- [EMU-SDMS: Advanced speech database management and analysis in R (PDF)](https://www.phonetik.uni-muenchen.de/~jmh/papers/emucsl.pdf)
- [EMU-SDMS — ICPhS 2019 (PDF)](https://www.internationalphoneticassociation.org/icphs-proceedings/ICPhS2019/papers/ICPhS_1366.pdf)
- [WaveSurfer — ICSLP 2000 paper (PDF)](https://www.isca-archive.org/icslp_2000/sjolander00_icslp.pdf)
- [WaveSurfer — Wikipedia](https://en.wikipedia.org/wiki/WaveSurfer)
- [VoiceSauce — ICPhS 2011 paper (PDF)](https://www.internationalphoneticassociation.org/icphs-proceedings/ICPhS2011/OnlineProceedings/RegularSession/Shue/Shue.pdf)
- [Prosogram — project site](https://sites.google.com/site/prosogram/home)
- [Prosogram — tonal perception model paper](https://www.researchgate.net/publication/228979942_The_prosogram_Semi-automatic_transcription_of_prosody_based_on_a_tonal_perception_model)
- [Parselmouth documentation](https://parselmouth.readthedocs.io/)
- [Introducing Parselmouth: A Python Interface to Praat (PDF)](https://pure.mpg.de/rest/items/item_2627915_2/component/file_2627914/content)
- [Parselmouth GitHub repo](https://github.com/YannickJadoul/Parselmouth)
- [Gentle forced aligner GitHub repo](https://github.com/strob/gentle)
- [Descript keyboard shortcuts — help docs](https://help.descript.com/hc/en-us/articles/10255582172173-Keyboard-shortcuts)
- [iZotope RX Spectral Editor](https://www.izotope.com/en/products/rx/features/spectral-editor)
- [RX Spectrogram/Waveform Display docs](https://docs.izotope.com/rx11/en/spectrogram-waveform-display.html)
- [Introduction to the viridis color maps (CRAN)](https://cran.r-project.org/web/packages/viridis/vignettes/intro-to-viridis.html)
- [Roseus — perceptually uniform colormap for Audacity](https://github.com/dofuuz/roseus)
- [Command Palette — Mobbin glossary](https://mobbin.com/glossary/command-palette)
- [Command palettes for the web — Rob Dodson](https://robdodson.me/posts/command-palettes/)
- [Nondestructive editing in Photoshop — Adobe](https://helpx.adobe.com/photoshop/using/nondestructive-editing.html)
- [Autosave design pattern — UI Patterns](https://ui-patterns.com/patterns/autosave)
- [Praat GitHub Issue #591 — "Terrible user interface"](https://github.com/praat/praat/issues/591)
