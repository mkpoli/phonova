# UX direction — v0.1

Grounded in `../research/design-lessons.md` (Tantacrul's MuseScore 4 /
Audacity account, ELAN, EMU-SDMS, iZotope RX, DAW navigation) and the
pain-point catalog. This document makes the concrete calls for v0.1.

## Application shape

Two screens plus overlays. No Objects window, no Picture window, no modal
forms anywhere.

### 1. Home / project manager

- Grid of recent projects with waveform thumbnails; open, rename, delete,
  duplicate in place.
- A drop target covering the whole screen: dropping audio files or a folder
  creates a project and imports everything in one step (wow-moment 1:
  folder in, browsable corpus out). Dropping a `.TextGrid` next to its audio
  attaches it automatically by name match.
- New-project flow is one screen: name (pre-filled from the dropped
  folder/file, editable) and nothing else. Speaker profiles, tier templates,
  and analysis settings are adjusted later in context, never asked up front
  (the MuseScore-4 funnel lesson).

### 2. Editor

Single-window, synchronized-pane timeline (the WaveSurfer/DAW model):

```
┌────────────────────────────────────────────────────────┐
│ toolbar: transport · selection readout · palette hint  │
├────────────────────────────────────────────────────────┤
│ overview strip (whole file, viewport indicator)        │
├────────────────────────────────────────────────────────┤
│ waveform pane                                          │
│ spectrogram pane (pitch/formant/intensity overlays)    │
│ annotation tiers (ELAN-style, collapsible, reorderable)│
├──────────────────────────────────────────┬─────────────┤
│ status bar: cursor t/f · values at cursor│ inspector ▸ │
└──────────────────────────────────────────┴─────────────┘
```

- **Inspector** (right dock, non-modal): parameters of whatever is focused —
  spectrogram window length, pitch floor/ceiling, formant ceiling, palette.
  Every change re-renders the visible region live; there is no OK button.
  Each parameter shows its default and a one-line explanation at the point of
  use; values that clip against the data (pitch crowding the floor, formants
  crowding the ceiling) get a visible warning badge. Parameter profiles
  (e.g. per speaker) are named, saved in the project, and switchable from the
  toolbar.
- **Selection**: click-drag on the waveform selects a time range; click-drag
  on the spectrogram selects a time–frequency box (the RX model, wow-moment
  2). The readout shows duration, F0 mean, and band energy for the current
  selection; actions on a selection (play, zoom-to, voice report, extract,
  export figure) come from the palette, the context menu, and keys.
- **Navigation**: wheel = horizontal zoom centered on pointer; Shift+wheel =
  scroll; Ctrl/Cmd+wheel = vertical (amplitude / frequency-range) zoom;
  pinch on trackpads; `F` fits selection, `0` fits file. All bindings
  remappable later; defaults follow DAW conventions.
- **Playback**: space plays selection or from cursor; the cursor is
  sample-accurate from the engine clock, never animated by the frontend.
- **Annotation loop** (keyboard-first): `Tab`/`Shift-Tab` next/previous
  interval, `Enter` opens label editing, `S` splits at cursor, `M` merges,
  arrow keys nudge the active boundary by one pixel (`Alt` = one frame),
  digits 1–9 focus tiers. Boundaries shared across aligned tiers move
  together by default. Every one of these actions goes through the engine
  journal — undo works across label edits, boundary moves, and imports alike
  (wow-moment 3).

### Overlays

- **Command palette** (Ctrl/Cmd-K): every action in the application, without
  exception, is a palette entry with the same name the API uses — the palette
  is the discoverability layer and doubles as living API documentation
  (wow-moment 4 depends on this naming parity). Fuzzy search, recent-first.
- **Figure export dialog**: shows a live preview of the figure built from the
  current view or selection (waveform / spectrogram / tracks / tiers as
  toggleable layers), with size in cm/in, font, palette (color or grayscale
  print), and format: SVG, PDF, PNG, TikZ (PGFPlots), Typst/CeTZ, Vega JSON,
  or data + matplotlib/R/Julia code. The preview re-renders per theme so a
  figure that works in dark mode is confirmed against white before export
  (wow-moment 5).
- **Voice report**: rendered as a readable card (values, parameter
  provenance, warnings), copyable as text/CSV, exportable through the same
  figure pipeline.

## Visual language

- Spectrogram palettes: viridis default, magma, grayscale; independent tuning
  per theme — palettes are defined against both backgrounds, never inverted.
- Light and dark are both first-class; every UI change is screenshot-verified
  in both before merging.
- Typography and spacing follow a plain, quiet lab-instrument register; no
  decorative chrome competing with the data. The signal area gets the pixels
  (the MuseScore-4 baseline-noise lesson: contextual panels over permanent
  toolbars).

## Autosave and trust

- Continuous background autosave with crash recovery; an explicit save action
  writes the project file and clears the dirty marker.
- Nothing is destructive: imports never modify source media; edits produce
  journal entries; "revert to saved" and full history are always available.

## Acceptance criteria — the five demo wow-moments

v0.1 is demo-ready when all five run live without rehearsal tricks:

1. Drop a folder of recordings → browsable project with per-file thumbnails
   in seconds; open any file and search labels across the project.
2. Click-drag a time–frequency box directly on the spectrogram → readout
   updates; play, measure, and export act on that box.
3. Move a boundary, edit a label, import a file — then undo all three from
   one stack, instantly.
4. Run the same measurement from the command palette and from a script
   against the engine API → numerically identical output, shown side by side.
5. Switch the spectrogram to grayscale for a publication figure in one click,
   in light and dark mode, and export a PDF that looks identical to the
   preview.
