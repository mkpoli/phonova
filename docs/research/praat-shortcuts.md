# Praat keyboard-shortcut behavior: Sound Editor and TextGridEditor

Ground-truth inventory of Praat's actual keyboard bindings in the Sound
editor and TextGridEditor, for building a "Praat-compatible" keyboard mode
in Phonia. The specific trigger for this research is the Tab key, which
plays a selection and is described below in full mechanical detail.

## Method and license note

Phonia is MIT-licensed and clean-room; Praat is GPL. The primary source is
Praat's manual at
[fon.hum.uva.nl/praat/manual](https://www.fon.hum.uva.nl/praat/manual/) —
specifically `SoundEditor.html`, `TextGridEditor.html`,
`Keyboard_shortcuts.html`, `Intro_2_2__Viewing_and_editing_a_sound.html`,
and `Play.html`, fetched directly and cited inline.

The project owner authorized reading Praat's source
(`github.com/praat/praat`) for behavior the manual leaves ambiguous. That
authorization turned out to be necessary here in a way it wasn't for the
two sibling documents in this directory: several manual pages disagree with
each other, and at least four manual-documented shortcuts are stale relative
to the code currently on Praat's `master` branch. Every finding sourced from
code rather than the manual is marked **[source-observed]**, describes
*behavior only* in this document's own words, and never reproduces Praat
source text, comments, or identifiers verbatim beyond the bare minimum
needed to name a function or a constant for traceability (e.g. citing that a
constant is called `GuiMenu_COMMAND_EXTRA` is a fact about the binding, not
copied prose). Where neither the manual nor a source read settled a
question, that is recorded as "not confirmed."

## Praat version/commit inspected

- Manual: the live site at `fon.hum.uva.nl/praat/manual`, fetched
  2026-07-19/20. Individual pages carry their own last-edit stamp, noted per
  page below where it matters (`TextGridEditor.html`: `© ppgb 20210228`;
  `Keyboard_shortcuts.html`: `© ppgb 20220701`; `SoundEditor.html`:
  `© Paul Boersma 20220814,2023-06-08`).
- Source: `github.com/praat/praat`, `master` branch, commit
  `626bc1f2686d42a60c8f5472780d6fec9bac5906` (repository push timestamp
  2026-07-19). This is the same commit `docs/formats/praat-formats.md`
  inspected, where it is noted the codebase self-identifies as pre-release
  `7.0beta`. The latest tagged stable release is `v6.6.30`
  (`gh api repos/praat/praat/tags`), which predates this commit — several of
  the source/manual mismatches below are plausibly changes made after the
  manual pages were last edited but before (or without) a new stable tag.
- Files read: `foned/FunctionEditor.cpp`/`.h`, `foned/SoundEditor.cpp`/`.h`,
  `foned/TextGridEditor.cpp`/`.h`, `foned/SoundArea.cpp`,
  `foned/TextGridArea.cpp`, `foned/SoundAnalysisArea.cpp`,
  `foned/FunctionArea.cpp`, `sys/Gui.h`, `sys/GuiMenuItem.cpp`,
  `sys/Editor.cpp`.

## Architecture note: one shared implementation, not two

**[source-observed]** The Sound editor (`SoundEditor`) and the TextGrid
editor (`TextGridEditor`) are both direct subclasses of a common
`FunctionEditor` base class (`Thing_implement (SoundEditor, FunctionEditor,
0)`, `Thing_implement (TextGridEditor, FunctionEditor, 0)`). Every playback
key binding — Tab, Shift-Tab, Escape, and the click-to-play rectangles — is
registered exactly once, in `FunctionEditor.cpp`'s
`v_createMenuItems_play`. Neither `SoundArea.cpp`, `TextGridArea.cpp`,
`SoundEditor.cpp`, nor `TextGridEditor.cpp` overrides the key binding
itself. This means the answer to "does Tab differ between the Sound editor
and the TextGrid editor?" is no, at the level of *which key does what* — the
code path is identical. What differs is only whether an editor happens to
have a playable sound attached at all (see "What Tab actually does" below).

## Sound Editor bindings

### Shared `FunctionEditor` bindings (identical in TextGridEditor)

| Key | Action | Source |
|---|---|---|
| Tab | Play or stop (see dedicated section below) | [source-observed, `FunctionEditor.cpp` `v_createMenuItems_play`]; [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Shift-Tab | Play window — stops any current playback, then plays the entire visible window, regardless of selection | [source-observed]; [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Escape | Interrupt playing (stop) | [source-observed]; [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Command-A | Show all / zoom to full duration | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); confirmed [source-observed] (`'A' \| GuiMenu_DEPTH_1`) |
| Command-I | Zoom in | same, `'I'` |
| Command-O | Zoom out | same, `'O'` |
| Command-N | Zoom to selection | same, `'N'` |
| Command-B | Zoom back (undo last zoom) | [source-observed] (`'B'`); **not listed** in `Keyboard_shortcuts.html`'s own shortcut table |
| Page Up / Page Down | Scroll page back / forward | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); confirmed [source-observed] |
| Arrow Up / Arrow Down | Select earlier / select later (move both selection edges together, stepwise) | same |
| Shift-Arrow Up / Down | Move start of selection left / right | same |
| Command-Arrow Up / Down | Move end of selection left / right | same |
| F6 | Get cursor (report cursor time) | same |
| Command-Z | Undo | [source-observed, `sys/Editor.cpp`, generic to every Praat editor, not Sound/TextGrid-specific]; [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Command-Y | Redo | same |
| *(none)* | "Move cursor to start of selection" / "Move cursor to end of selection" exist as menu commands but carry **no key binding** | [source-observed] — confirmed by the absence of any accelerator flag on those two `EditorMenu_addCommand` calls |
| *(none)* | Home and End are **not bound to anything** anywhere in `FunctionEditor.cpp`, `SoundArea.cpp`, or `TextGridArea.cpp` | [source-observed] — `grep`-confirmed absence of `GuiMenu_HOME`/`GuiMenu_END` in all editor source under `foned/` |

### `SoundArea`-specific bindings (editing — Sound Editor only)

These require `editable()` to be true. **[source-observed]**: in a
TextGridEditor, the embedded copy of the sound is created with
`editable = false`, and its own draw-legend text literally labels it
"non-modifiable copy of sound" (vs. "modifiable sound" in the standalone
Sound editor) — so none of the rows in this sub-table exist in
TextGridEditor at all, confirmed both by the `if (our editable())` guards
around their registration and by that legend string.

| Key | Action | Source |
|---|---|---|
| Command-X | Cut | [SoundEditor manual](https://www.fon.hum.uva.nl/praat/manual/SoundEditor.html); [source-observed] (`'X'`) |
| Command-C | Copy selection to Sound clipboard | same, `'C'` |
| Command-V | Paste after selection | same, `'V'` |
| Extra-Command-V | Paste over selection | same, `GuiMenu_COMMAND_EXTRA \| 'V'` |
| Command-R | Reverse selection | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); [source-observed] (`'R'`) |
| *(none)* | "Set selection to zero" — menu-only, no key binding | [source-observed] |
| Command-click on the channel mute icon | Mute/unmute one channel of a multichannel sound (mouse, not keyboard) | [SoundEditor manual](https://www.fon.hum.uva.nl/praat/manual/SoundEditor.html) |

### Zero-crossing navigation (Sound Editor; also present in TextGridEditor when a plain `Sound`, not `LongSound`, is attached)

| Key | Action | Source |
|---|---|---|
| Command-, | Move start of selection to nearest zero crossing | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); [source-observed] (`','`) |
| Command-0 | Move cursor to nearest zero crossing | same, `'0'` |
| Command-. | Move end of selection to nearest zero crossing | same, `'.'` |

These three are **not** gated by `editable()` — confirmed [source-observed]
— so unlike cut/copy/paste/reverse they are available in TextGridEditor
whenever a `Sound` (not a disk-streamed `LongSound`) is attached.

### Analysis query keys (`SoundAnalysisArea`; shared by both editors when the corresponding analysis is shown)

| Key | Action | Source |
|---|---|---|
| F1 | Get first formant | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); [source-observed] |
| Command-F1 | Formant listing | same |
| F2 / F3 / F4 | Get second / third / fourth formant | same |
| F5 (Windows/Linux) or F10 (Mac) | Get pitch | same — platform split confirmed [source-observed] (`#ifdef macintosh` picks `GuiMenu_F10`, else `GuiMenu_F5`) |
| +Command | Pitch listing | same |
| +Option | Get minimum pitch | same |
| +Shift | Get maximum pitch | same |
| Extra-Command-L | Move cursor to minimum pitch | same |
| Extra-Command-H | Move cursor to maximum pitch | same |
| F7 | Get spectral power at cursor cross | same |
| F8 | Get intensity | same |
| Command-F8 | Intensity listing | same |
| Option-F8 / Shift-F8 | Get minimum / maximum intensity | same |
| Command-L | View spectral slice | [source-observed] (`'L' \| GuiMenu_DEPTH_1` in `SoundAnalysisArea.cpp`); matches [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| F12 | Log 1 | [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html); [source-observed] |
| Shift-F12 | Log 2 | same |
| Option-F12 | Log script 3 | same |
| Command-F12 | Log script 4 | same |

**Manual/source mismatch — F9/F11 [source-observed]:** `Keyboard_shortcuts.html`
states "F11: Voice report" and "Command-F9: Pulse listing." Current source
binds **Voice report to plain F9** and **Pulse listing to Command-F9**;
there is no `GuiMenu_F11` binding anywhere in `SoundAnalysisArea.cpp`. F11
does not currently do anything in either editor. Treat the manual's F11
claim as stale.

## TextGridEditor bindings

### Boundary / interval / point creation

| Key | Action | Source |
|---|---|---|
| Enter | "Add on selected tier" — insert a boundary or point at the cursor time (or at the current selection's two edges) on the currently selected tier | [TextGridEditor manual](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html); [source-observed] (`GuiMenu_ENTER`) |
| Command-1 … Command-8 | "Add interval on tier N" — insert boundary/boundaries on tier N; if the cursor (not a selection) sits inside an interval and another tier already has a boundary nearby, this variant also inserts a matching boundary to preserve cross-tier alignment | [source-observed] (`GuiMenu_COMMAND \| '1'…'8'`), matches [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Extra-Command-1 … Extra-Command-8 | "Add on tier N" — plain boundary/point insertion on tier N, no cross-tier lookup | [source-observed] (`GuiMenu_COMMAND_EXTRA \| '1'…'8'`), matches [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |
| Extra-Command-9 | Add a boundary on all tiers | same, `GuiMenu_COMMAND_EXTRA \| '9'` |

**Manual/source mismatch [source-observed]:** `TextGridEditor.html` itself
says boundary insertion on any tier uses "shortcuts: Command-F1 through
Command-F9." No such binding exists anywhere in current
`TextGridArea.cpp`; the real mechanism is the Command-1…8 /
Extra-Command-1…8 / Extra-Command-9 scheme above, which matches the
separately-maintained `Keyboard_shortcuts.html` reference page instead.
`TextGridEditor.html` was last edited 2021-02-28, a year and a half before
`Keyboard_shortcuts.html`'s last edit (2022-07-01) — this looks like one
page falling out of sync with a later shortcut redesign.

### Boundary/point removal and movement

| Key | Action | Source |
|---|---|---|
| Option-Backspace (Alt-Backspace on Windows/Linux) | Remove the selected boundary or point | [source-observed] (`GuiMenu_OPTION \| GuiMenu_BACKSPACE`) — **not documented in either manual page fetched** |
| Shift-drag | Move all boundaries/points sharing a time (across tiers) together | [TextGridEditor manual](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) (mouse gesture, not a keyboard binding) |
| *(none, menu-only)* | "Move to nearest zero crossing" for a selected boundary/point | [source-observed], no accelerator |

### Interval-level commands

| Key | Action | Source |
|---|---|---|
| Command-D | Diarize interval (pyannote-based speaker diarization on the selected interval, a recent addition — see the "speaker diarization with adapted pyannote.audio" page linked from `TextGridEditor.html`) | [source-observed] (`'D'`) |
| Command-E | Align interval | [source-observed] (`'E'`) |
| Command-T | Transcribe interval | [source-observed] (`'T'`), matches [Keyboard shortcuts](https://www.fon.hum.uva.nl/praat/manual/Keyboard_shortcuts.html) |

**Manual/source mismatch [source-observed]:** `Keyboard_shortcuts.html`
lists "Command-D (in TextGrid window): Align interval." In current source,
Command-D is **Diarize interval**, and Align interval has moved to
**Command-E** — not documented on either manual page checked. The most
plausible explanation, cross-referenced against the diarization feature
link visible on `TextGridEditor.html`'s own "Links to this page" list, is
that diarization is a newer feature that claimed the Command-D slot and
displaced the older Align-interval binding.

### Text search and spelling

| Key | Action | Source |
|---|---|---|
| Command-F | Find — search text in the currently selected tier | [TextGridEditor manual](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html); [source-observed] (`'F'`) |
| Command-G | Find again | same, `'G'` |
| Extra-Command-L | Check spelling in tier (requires a `SpellingChecker` object attached at editor-launch time) | [source-observed] (`GuiMenu_COMMAND_EXTRA \| 'L'`) |
| *(none, menu-only)* | Check spelling in interval | [source-observed], no accelerator |

**Manual/source mismatch [source-observed]:** Both `TextGridEditor.html`
("Check spelling in tier (Command-N)") and the general shortcut summary
agree the spelling checker used to sit on Command-N. Current source binds
it to **Extra-Command-L** instead. Command-N in current source is "Zoom to
selection" (the generic `FunctionEditor` binding, see the Sound Editor
table above) — a real collision that would misfire under the manual's
stated binding.

### Text editing

Direct typing into the interval/point label field; arrow keys navigate
within that text field using ordinary text-field conventions (not a special
Praat binding). [TextGridEditor manual](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html)
Cut/Copy/Paste in this text field use the generic Command-X/C/V bindings
described above for text, not the Sound-specific clipboard variants.

## What Tab actually does

**[source-observed]**, from `FunctionEditor.cpp`'s `PLAY_DATA__playOrStop`,
bound to plain Tab (`GuiMenu_TAB`) with no modifier, in the shared
`FunctionEditor` base class used by both editors:

1. If a sound is currently playing, Tab **stops it** (an explicit stop, via
   `MelderAudio_stopPlaying (MelderAudio_EXPLICIT)`), and the cursor is left
   at the time playback reached. This is confirmed independently by the
   [TextGridEditor manual](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html):
   "If you press it while a sound is playing, the Tab key will halt the
   playing sound, and the cursor will move to the time at which the sound
   stopped playing." Source and manual agree exactly here.
2. Otherwise, if there is a nonempty time selection (`startSelection <
   endSelection`), Tab plays exactly that selection.
3. Otherwise — cursor only, no selection — if the cursor sits strictly
   inside the visible window (not sitting at either edge), Tab plays from
   the cursor to the end of the visible window.
4. Otherwise (cursor at a window edge, or no meaningful cursor position),
   Tab plays the entire visible window.

So the menu label Praat itself uses — "Play or stop" — is accurate: Tab is
a genuine play/pause toggle, not a one-shot "always play" key. What varies
run to run is *what* it plays when starting fresh: selection first, then
cursor-to-window-end, then whole window, in that priority order.

Shift-Tab ("Play window", `GuiMenu_SHIFT | GuiMenu_TAB`) is a different,
simpler command: it always stops any current playback first
(`MelderAudio_stopPlaying (MelderAudio_IMPLICIT)`) and then plays the
entire visible window, ignoring the selection state entirely. Pressing
Shift-Tab again while it is already playing does not toggle to a stop — it
restarts window playback from the beginning.

Escape (`GuiMenu_ESCAPE`) is a third, separate binding: "Interrupt
playing," an unconditional stop with no play behavior at all.

Clicking one of the click-to-play rectangles around the waveform (documented
in [SoundEditor manual](https://www.fon.hum.uva.nl/praat/manual/SoundEditor.html)
and [Intro 2.2](https://www.fon.hum.uva.nl/praat/manual/Intro_2_2__Viewing_and_editing_a_sound.html))
is **[source-observed]** a fourth, independent code path
(`gui_drawingarea_cb_mouse`'s rectangle-index `switch`): it always calls
`v_play` directly for that rectangle's fixed time range (whole file,
visible window, left-of-window, right-of-window, one of up to three marker
segments, or the current selection) with no stop-first call and no
toggle-to-stop behavior. Whether clicking a rectangle while something is
already playing overlaps, restarts, or is silently dropped by the
lower-level audio engine was not traced further — **not confirmed**.

**TextGridEditor-specific consequence [source-observed]:** `TextGridEditor`
overrides `v_play` as `if (our soundArea()) SoundArea_play (...)`, guarding
on whether a `Sound`/`LongSound` was attached when the editor was opened.
If a bare `TextGrid` is opened with no attached sound, Tab, Shift-Tab, and
the click rectangles are all silent no-ops — there is nothing to play. The
manual's TextGridEditor walkthrough always assumes a sound is present and
does not call this out explicitly.

## Surprising vs. expected

The task's framing — modern DAWs (Reaper, Audacity, Adobe Audition, Pro
Tools) conventionally use **Space** for play/pause and reserve **Tab** for
panel/field navigation or marker-jumping — is the right baseline for
judging each binding below.

- **Genuinely surprising: the key, not the verb.** Praat's Tab *is* a
  play/pause toggle in the way any DAW user would recognize the concept —
  press once to play, press again to stop where you are. That part matches
  expectations exactly. What collides is the *key*: this behavior sits on
  Tab, and **[source-observed]** the literal space character is bound to
  nothing anywhere in `FunctionEditor.cpp`, `SoundArea.cpp`,
  `TextGridArea.cpp`, or `SoundAnalysisArea.cpp` — confirmed by grepping
  all four files for a bare `' '` accelerator and finding zero matches. A
  user arriving from Audacity or Pro Tools, where Space toggles playback,
  will press Space in Praat and get nothing (or, if a text field has
  focus, a literal space character typed into a label). The play/pause
  concept is standard; the key it lives on is not.
- **Surprising: what Tab plays depends on selection AND cursor position,
  with a fallback chain.** Selection first, else cursor-to-window-end
  (only if the cursor is strictly inside the window), else whole window.
  DAWs that support "play from cursor" or "play selection" typically pick
  one governed by an explicit mode toggle (e.g. Reaper's loop-selection
  button), not an implicit three-way fallback baked into a single key.
- **Expected: toggle-to-stop.** Pressing the same play key again to stop,
  with the cursor left where playback halted, is exactly the "scrub and
  transcribe" idiom described in the TextGridEditor manual itself, and
  matches how Space behaves in Audacity/Pro Tools/Reaper. Nothing unusual
  here except the key.
- **Expected-shaped but incomplete: Shift-Tab as a distinct "play
  everything visible" command** is a reasonable secondary transport action
  (comparable to a DAW's "play from start of view"), but its
  non-toggling behavior (it always restarts rather than stopping on a
  second press) is inconsistent with Tab's own toggle behavior one key
  over — a DAW user who has just learned "press again to stop" from Tab
  would reasonably expect the same from Shift-Tab and be wrong.
- **Surprising: Home/End do nothing.** Jumping to the start or end of the
  file/selection via Home/End is a near-universal media-player and DAW
  convention. **[source-observed]** Praat has no `GuiMenu_HOME` or
  `GuiMenu_END` binding anywhere in the editor source, even though menu
  commands for "move cursor to start/end of selection" exist — they are
  simply never bound to a key.
- **Surprising: numbered-tier boundary shortcuts split across two modifier
  families.** Command-1…8 vs. Extra-Command-1…8 for closely related
  "add interval" vs. "add boundary" actions on the same tier is a
  finer-grained modifier distinction than most editors make for two
  operations a user would consider variants of the same gesture — most
  DAW/annotation tools use one modifier tier for "add marker on track N"
  and don't split by an internal implementation detail (whether a
  cross-tier alignment lookup also runs).
- **Not surprising, and worth stating plainly for scope-setting:** zoom
  (Command-I/O/A/N/B), page-scroll (Page Up/Down), and selection-nudging
  (arrow keys, with Shift/Command variants for the two selection edges) all
  land close to ordinary editor/DAW conventions — modifier-plus-letter for
  zoom, arrow keys for stepwise navigation. These are unlikely to surprise
  anyone porting habits from another audio tool.

## Not confirmed

- Whether clicking a play rectangle while a sound is already playing
  overlaps the two, cuts the first short, or is dropped — the rectangle
  click handler does not call an explicit stop before playing, unlike
  Tab/Shift-Tab/`Play...`, but the actual arbitration happens inside
  `MelderAudio`/`Sound_playPart`, which was not traced.
- Whether the "Diarize interval" (Command-D) and its settings dialog
  reflect a stable, released feature or in-development work specific to
  the pre-`7.0` branch inspected — the feature exists in this commit but
  its manual page was only located as a "Links to this page" title, not
  read in full.
- Whether v6.6.30 (the latest tagged stable release) actually contains the
  Command-D/Command-E/Extra-Command-L changes described above, or whether
  they landed only after that tag — this document inspected `master`
  directly, not the `v6.6.30` tag's source tree, so a user running the
  stable release could still be on the older Command-D/Command-N bindings
  the manual describes. This is worth resolving before Phonia commits to
  matching "current Praat" bindings, since "current" is ambiguous between
  the tagged release and `master`.
