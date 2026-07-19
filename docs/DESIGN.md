# Phonia design

This document records the design system behind every Phonia surface — the
web app, the desktop app, the landing page, and exported figures. It exists
so that new work converges on one look without redesigning from scratch each
time. Token values live in code (`apps/web/src/app.css`, `apps/desktop`
mirrors them); this document explains what they mean and how to extend them.

## Character

Phonia is a lab instrument. People sit inside it for hours measuring voices,
so the interface behaves like good laboratory equipment: quiet chrome, dense
and legible data, controls that respond instantly and never surprise. Warmth
keeps long sessions comfortable; restraint keeps attention on the signal.
Decoration appears in exactly one place — the serif wordmark — and nowhere
else.

## Identity

The mark is a rise–fall–rise pitch contour held in a circle that opens at the
top right; the contour's final rise passes through the gap and an amber dot
sits just outside it. Strokes are round-capped teal. In-app the mark inherits
`--accent` and `--warn` so it follows the theme; standalone renderings
(favicon, exported icons) pin teal-600 with amber-600 on light and teal-300
with amber-300 on dark. The wordmark sets "Phonia" in the serif stack beside
the mark, and the two only appear together in chrome — the mark alone carries
identity at icon sizes.

## Principles

Each principle carries a test. A change that fails its test does not ship.

1. **Beauty through material honesty.** The spectrogram is the most beautiful
   thing on screen; the chrome frames it and steps back. Test: in a
   screenshot, the eye lands on data, never on a control.
2. **Productive by keyboard.** Every action is reachable through the command
   palette and, for the frequent ones, a direct shortcut; annotation of a
   full sentence never needs the pointer. Test: perform the workflow with the
   mouse unplugged.
3. **Pragmatic defaults, honest parameters.** Every analysis control shows
   its default and where that default comes from; values that mislead for a
   voice type carry a warning at the moment of use. Test: a new user can
   state which settings produced any number on screen.
4. **Accessible as a floor.** Text and essential UI meet WCAG 2.2 AA contrast
   in both themes; spectrogram palettes are colorblind-safe with a grayscale
   print option; every interactive element has a visible focus state; motion
   respects `prefers-reduced-motion`. Test: the axe audit passes and the
   keyboard walk reaches everything in a sensible order.
5. **Simple means fewer concepts.** One selection model, one undo stack, one
   viewport transform shared by every pane, one place to search for actions.
   New features reuse these concepts. Test: the feature adds no new mode and
   no new selection type.

## Relationship to other systems

Phonia borrows judgment where established systems solved a problem well, and
keeps its own visual identity:

- **Apple HIG** — direct manipulation and restraint: the selection is made on
  the spectrogram itself, and controls appear where the work is.
- **IBM Carbon** — data-table discipline: dense rows, uppercase muted
  headers, tabular numerals, right-aligned quantities.
- **Radix / WAI-ARIA authoring practices** — behavioral contracts for
  composite widgets (dialogs, listboxes, the palette) rather than invented
  keyboard semantics.
- **Linear** — the command palette as the app's spine and shortcut hints as
  passive teaching.
- **Ableton Live and iZotope RX** — the calm of professional audio tools:
  dark-capable, low-chrome, meters and readouts as first-class citizens.
- **MuseScore 4** — defaults over templates, merged panels over parallel
  ones, and contextual controls over global modes.

## Foundations

### Color

One accent family over warm neutrals:

| Role | Light | Dark | Use |
|---|---|---|---|
| Paper | `#f7f6f3` | `#1d1d1a` | app background |
| Ink ramp | warm grays | warm grays | text, borders, surfaces |
| Identity (teal) | anchored `teal-600` | anchored `teal-300` | wordmark, selection, active states, chrome accents, primary form actions |
| Semantic | green / amber / red triads | same, dark-tuned | success, warning, destructive |

The neutrals are warm (red ≥ green ≥ blue) in both themes; the dark theme is
charcoal, never blue-black. The dark ground holds red and green equal at
`#1d1d1a` with blue a shade lower, so it reads neutral rather than
orange-cast. Depth in the dark theme recedes toward the signal: chrome is
the brightest structural layer, panels sink a touch below it to `#191917`,
and wells drop further to `#121210` for the waveform and spectrogram floor,
keeping the eye pulled toward the data per the material-honesty principle.
Hairlines run `#37372e`, brighter than the surfaces they separate so a
border always reads before the fill on either side of it. Elevation itself
is a shadow property, not a fill property (see Space, radius, depth) — the
recede-toward-data ordering here is about focus, not stacking height. Teal
is the single accent across chrome and primary form actions: Create,
Save, Download, and Recover. Semantic colors retain their separate roles for
success, warning, and destructive states; teal never substitutes for them.
`--on-accent` supplies each theme's readable label color against a solid teal
fill. The light pairing has a 5.47:1 contrast ratio and the dark pairing has
an 11.39:1 ratio; both clear WCAG AA for normal text.

Data colors are a separate, frozen vocabulary (see Data display).

### Typography

- **UI text**: the platform sans stack. No webfont for chrome.
- **Phonetic notation**: Voces (`--font-ipa`, SIL OFL) for tier labels, the
  label editor, and transcriptions — IPA glyphs render consistently and
  visibly differently from UI text.
- **Numerals**: every time, frequency, and level readout sets
  `font-variant-numeric: tabular-nums` so values align and don't jitter as
  they update.
- **Serif** (`--font-serif`): the wordmark only.
- Scale: the Tailwind default scale, `text-xs` for meta, `text-sm` for body
  and labels, semibold for headings. No thin weights on data.

### Space, radius, depth

- Spacing on the Tailwind quarter-rem scale; control heights inside a
  toolbar are uniform (2.1 rem in the transport).
- Radii are generous and size-graded (`--radius-sm` … `--radius-xl`):
  chips < buttons and inputs < cards and panels < overlays.
- Elevation uses layered soft shadows (`--shadow-sm/md/lg`), deeper in dark
  mode. Shadow depth states elevation tier: resting surfaces, raised cards,
  floating overlays. Borders separate; shadows elevate; the two are never
  interchangeable.

### Motion

Transitions run 0.15–0.2 s `ease-in-out` and animate opacity, transform, and
color only. Data panes never animate through easing — viewport changes apply
transforms immediately (the transform-first rendering contract), because a
measurement surface that glides is a measurement surface that lies. Under
`prefers-reduced-motion`, decorative transitions collapse to instant.

### Iconography

Lucide, exclusively, via `unplugin-icons`. Icons accompany labels in chrome
(icon + text); icon-only buttons carry `aria-label` and a tooltip. No second
icon family, ever — mixed stroke registers read as clutter.

### Navigation

A vertical rail on the far left holds the brand mark and the app's modes:
icon over an uppercase label, teal fill and a left border on the active one,
`aria-current="page"` marking it for assistive tech. Library covers the
project manager and its corpus view; Analyze is the annotation editor.
Studio, Plot, and Script are planned modes — reserved in the rail's data
order, not rendered until each one has a surface behind it. Every mode is a
native button, reachable by Tab and disabled rather than hidden when its
target has nothing to show yet.

## Data display

- **Spectrogram palettes**: Phonia (default), Golden, Viridis, Magma,
  Inferno, Plasma, Cividis, and Grayscale tuned per theme for print. Palette
  changes recolor instantly; they never recompute analysis.
- **Phonia** is the default because the spectrogram should read as the same
  material as the rest of the interface: it runs from a warm charcoal floor,
  just below the dark-theme paper, up through the teal identity ramp into a
  warm soft-yellow highlight. Its 256 control points are placed in Oklab for
  even perceptual spacing, and the whole table is verified to increase
  monotonically in WCAG relative luminance, so a louder region never renders
  darker than a quieter one. The floor stops short of pure black and the
  highlight short of pure white, keeping headroom at both ends.
- **Golden** is a warm sibling of Phonia: the same charcoal floor through a
  burnt-umber and amber midtone into a golden-cream highlight, more
  saturated than Phonia's paper cream. Built the same way — Oklab control
  points, verified monotonically increasing in WCAG relative luminance.
- **Viridis** is the colorblind-validated alternative, kept for any figure a
  reader must trust under color-vision deficiency; the other perceptual ramps
  and the grayscale print ramp stay selectable alongside it.
- **Custom ramps**: the gradient editor builds a named ramp from draggable
  color stops and stores it in `localStorage` (`phonia:custom-ramps`), so a
  ramp is available to every project on the machine without entering any one
  project file. Saved ramps list in the palette picker beside the built-ins
  and recolor through the same cached-dB path. Each carries a live
  luminance-monotonicity badge; a ramp is not checked for color-vision
  deficiency, so the built-ins remain the choice when that guarantee matters.
- **Overlay conventions are frozen**: pitch `#9cc4ff` (blue line, right-hand
  Hz scale), formants `#ff5a52`, intensity `#ffcc33` (thin line). These match
  the colors a Praat user already reads fluently; changing them would break
  twenty years of trained eyes. The formant color is frozen; its mark is not
  — speckles sized by bandwidth (default, the Praat-familiar view) or
  connected per-formant tracks, user-selectable in the inspector. Tracks
  need the Viterbi-smoothed candidates, disabled otherwise: only that pass
  assigns a candidate to a specific formant, so only it has a track to draw.
  A track breaks wherever a frame has no candidate for that formant and
  wherever consecutive frames jump further than a formant plausibly moves in
  one frame, rather than joining across either gap, so it never draws a
  measurement the analysis did not produce.
- Overlay strokes carry a dark halo so they stay legible over any palette in
  either theme.
- Readouts state units (`Hz`, `dB`, `s`) and never round below measurement
  precision.

## Interaction

- **One viewport.** Waveform, spectrogram, overlays, and tiers share a single
  time transform; nothing scrolls independently on the time axis.
- **Selection** lives in signal coordinates (time, or time × frequency box)
  and survives zoom, pan, and theme changes.
- **Undo is universal**: every mutation goes through the engine journal;
  Ctrl/Cmd-Z works on annotation edits, imports, and tier operations alike,
  without per-editor stacks or depth caps.
- **The palette (Ctrl/Cmd-K)** lists every action with its shortcut and its
  engine-API name, so the GUI teaches the scripting surface.
- **Non-modal by default.** Inspectors, readouts, the export dialog, and the
  recording strip are panels beside the work. A blocking modal is reserved
  for data-loss decisions (recovery, deletion).
- **Focus** is always visible: one `:focus-visible` ring style everywhere.
- Empty states name the one action that fills them, in one or two sentences,
  with keycap chips for the relevant keys.

## Voice and copy

Interface text states facts in plain register: what a thing is, what an
action does, which key does it. No exclamation marks, no marketing
adjectives, no rhetorical questions, no anthropomorphism. Parameter help
cites provenance ("Default 600 Hz — Praat") in a sentence or less. Errors
say what happened and what to do next.

## Per-surface register

- **Editor**: chrome shrinks to labels-on-chips; the data area owns the
  viewport. Panels (inspector, readout) dock to edges.
- **Home and corpus**: card grid and data table, Carbon-style discipline,
  identity accents on hover and active states only.
- **Dialogs and overlays**: `--radius-xl`, `--shadow-lg`, backdrop blur,
  one primary action in the identity accent.
- **Landing and documentation**: may use the serif more freely and show the
  product at work — real screenshots over illustrations; effects may be
  richer than in-app, and every effect respects reduced motion. The palette
  and type foundations are the same; the landing page is the same material,
  lit differently.
- **Exported figures**: publication register — no chrome, axis-first, theme
  aware, deterministic output.

## Change discipline

A visual change is finished when screenshots in both themes have been looked
at and judged against this document, behavior tests stay green, and any new
color or type decision is recorded here in the same commit. Additions to the
token set require a use across at least two surfaces; single-use tokens are
inlined instead.
