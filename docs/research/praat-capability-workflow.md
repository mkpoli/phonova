# Praat capability inventory and the Phonia workflow map

Ground-truth inventory of what Praat can do across annotation, scripting,
data, and output, checked against what Phonia implements today. Feeds the
long-range goal in `horizon.md` item 16 (Praat-script compatibility) and item
6 (a tracked I/O parity table) — this document is the first full pass at
both.

## Method and license note

Phonia is MIT-licensed and clean-room; Praat is GPL. The primary source for
this inventory is Praat's own manual at
[fon.hum.uva.nl/praat/manual](https://www.fon.hum.uva.nl/praat/manual/) — the
Intro chapters, the Scripting tutorial, and per-command pages, cited inline.

The project owner authorized reading Praat's source
(`github.com/praat/praat`) for behavior the manual leaves thin or ambiguous,
specifically: the scripting interpreter's real semantics, exact file-format
layouts, and the Picture window's drawing/retention model. Every such finding
is marked **[source-observed]** and describes *behavior only* — mechanism and
observable effect, in this document's own words — never code, code comments,
or identifiers copied verbatim, and never internal data-structure detail
beyond what a file format or language semantic requires to specify. The
distinction stays live in this document because manual-documented behavior is
stable across releases; source-observed behavior can drift as Praat changes.
File-format internals derived from source are not re-derived here — they are
cited to `docs/formats/praat-formats.md` (commit `3ba4d26`), which inspected
Praat source at commit `626bc1f` (self-identified as `7.0beta`) under the same
rules and is the authoritative account for this repository.

The Phonia column in every table was checked directly against this
repository's crates and apps as of the commit at the head of this branch, not
assumed from plans or roadmaps.

**Verdict categories:**

- **core-parity** — everyday-workflow feature; Phonia should match or already
  matches it before calling itself a Praat replacement.
- **valuable** — meaningfully useful, not required for parity; worth building
  once core-parity items are done.
- **niche** — specialist or rare-use; low priority, may never be built.
- **deliberately-out-of-scope** — an explicit non-goal, with the reasoning
  stated per row (usually: Praat's own design is the pain point being fixed,
  see `praat-features-and-pain-points.md`).

---

## 1. Annotation

### 1.1 TextGrid object model

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Interval tier | Ordered, contiguous, non-overlapping intervals over a time domain; each interval carries one text label. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | `phx_annot::IntervalTier` — same contiguity invariant, enforced by `Annotation::validate`. | core-parity — done |
| Point tier (`TextTier`) | Ordered time-stamped marks, no duration, no contiguity constraint. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | `phx_annot::PointTier`. | core-parity — done |
| Document time domain (`xmin`/`xmax`) | One domain for the whole TextGrid; a tier's own `xmin`/`xmax` is normally equal but is not a maintained invariant — Praat only ever widens the document domain when a tier is added, never forces tier domains to match [source-observed, `docs/formats/praat-formats.md` §2, §7.6]. | `Annotation::xmin`/`xmax`; `IntervalTier`/`PointTier` carry independent domains; a domain mismatch is an *advisory* integrity issue, not a hard error, matching Praat's own looseness here. | core-parity — done |
| Cross-tier relations (declared, validated) | None. Boundary alignment across tiers is an editor *gesture* (drag/Shift-drag snapping, see §1.2), never a persisted, checked relation; nothing stops two "aligned" tiers from drifting apart after independent edits. | `phx_annot::TierRelation`: `Independent`, `AlignedBoundaries { with }` (boundary times must match another tier, enforced and repairable via `AlignMode::Linked`/`SingleTier`), `ChildOf { parent }` (interval nesting, enforced). Validated continuously by `Annotation::validate`. | **superset already** — this is the fix for pain point 2.15 (no cross-tier integrity) and is further along than Praat's own model |
| Creation from audio | `Sound: To TextGrid...` / `LongSound: To TextGrid...`, tier names supplied as a list, empty grid matching the sound's domain. [Intro 7](https://www.fon.hum.uva.nl/praat/manual/Intro_7__Annotation.html) | `AttachAnnotation` command creates/attaches an `Annotation` to imported audio via `phx-engine`; tier add is a separate journaled command (`AddIntervalTier`/`AddPointTier`). | core-parity — done, different shape (compose vs. one dialog) |
| `TextGrid: To TextGridNavigator...` — conditional multi-tier query object | Builds a reusable multi-tier boolean-query object (tier N matches pattern AND/OR tier M matches pattern, with time-relation constraints) for later searches. [To TextGridNavigator](https://www.fon.hum.uva.nl/praat/manual/TextGrid__To_TextGridNavigator___.html) | No equivalent object; `LabelQuery` (below) is single-pass, not a reusable multi-tier query object. | valuable — real gap for corpus-scale queries |

### 1.2 TextGridEditor

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Boundary/point insertion | Click inside the cursor circle in a tier, or a Boundary/Point menu command; keyboard shortcuts per tier (Cmd/Ctrl-F1..F9). [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | `insert_boundary`/`insert_point` engine commands, wired to `TierPane`/`TierLane` UI with keyboard-first loop (per `roadmap.md` phase-3 gate). | core-parity — done |
| Drag to move a boundary/point | Free drag repositions a single element. | `move_boundary`/`move_point`, journaled, with `AlignMode` controlling whether linked tiers move together. | core-parity — done |
| Shift-drag / cross-tier alignment | Dragging near a matching timestamp on another tier snaps to it; this is a per-gesture snap, not a persisted relation. [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | Superseded: `AlignedBoundaries` relations move together *because they are declared*, not because of a snap heuristic — the alignment survives independent future edits, which Praat's gesture-only model does not guarantee. | core-parity — done, structurally better |
| Boundary removal merges intervals | Removing a boundary concatenates the two intervals' text. [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | `remove_boundary` → `Merged`/`TierMerge`, same concatenation semantics, invertible. | core-parity — done |
| Text entry, special symbols | Direct text editing; a special-symbols palette (IPA, Greek, math, superscripts). [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | `set_label` command; no IPA input palette yet — tracked separately as horizon.md H1 item 5 ("IPA input, stage 1"), not built. | core-parity for text entry (done) / valuable for the palette (planned, not shipped) |
| Find / Find again | Cmd-F / Cmd-G, searches text in the current tier. [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | `Annotation::search(LabelQuery)` — substring or regex, filterable to specific tiers, exposed via `SearchBar.svelte`/`TierPane.svelte`'s label search. Regex is a strict superset of Praat's plain-text Find. | core-parity — done, superset (regex) |
| Extract selected sound | File menu command copies the current selection to a new `Sound` object. [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | `Engine::export_span_wav(audio, t0, t1, bit_depth)` — selection-to-WAV export, bit-for-bit equal to a direct sample slice per its own test. | core-parity — done |
| Playback of a selection | Tab plays the selected interval; clicking a boundary rectangle plays a short stretch around it. | Playback engine (`phx-playback`, `NativePlayback.ts`/`Playback.ts`) with sample-counter clock; selection playback exists in the UI. | core-parity — done |
| Spelling checker on labels | Optional `SpellingChecker` object enables Cmd-N "check spelling in tier." [TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html) | None. | niche — specialist add-on even in Praat |

### 1.3 Tier operations (Objects window / scripted)

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Add interval/point tier | `TextGrid: Insert interval tier...` / `Insert point tier...`, position + name. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | `add_interval_tier`/`add_point_tier`, journaled, invertible (`TierAdded`). | core-parity — done |
| Remove tier | `Remove tier...`. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | `remove_tier` → `TierRemoval`, invertible. | core-parity — done |
| Duplicate tier | `Duplicate tier...`, copies a tier's content to a new position/name. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | No dedicated command; not exposed in `phx-annot`'s public API or `phx-engine`'s command enum. | valuable — clean, small gap |
| Rename tier | Tier name is set at creation and via `Set tier name...`. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | No rename operation on a tier (audio buffers have `RenameAudio`; tiers do not have an equivalent). | valuable — clean, small gap, same shape as the audio rename that already exists |
| Merge TextGrids | `TextGrids: Merge...` combines multiple TextGrid objects' tiers into one, with an `equalizeDomains` option [source-observed, `docs/formats/praat-formats.md` §7.6]. | No cross-annotation merge; only within-tier boundary merge exists (§1.2). | valuable |
| Reorder tiers | Drag tier order in the editor; no separate scripted command found beyond the editor gesture. | `reorder_tier`, journaled. | core-parity — done, and scriptable (Praat's is editor-only) |
| Scale times / DurationTier-based warping | `TextGrid: To DurationTier...` then reapply to time-warp a grid; `Scale times...` variants exist for stretching a grid's domain. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | None. | niche — used for corpus time-alignment repair, rare in everyday work |
| Count labels | `Count labels...` reports label-frequency stats across tiers. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | None as a dedicated command (label search returns hits; no frequency-count report). | niche |

### 1.4 IntervalTier / TextTier standalone, conversions

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Standalone `IntervalTier`/`TextTier` objects and operations | Extractable from a TextGrid (`Extract one tier...`) and operable independently (same query/draw commands as a tier inside a grid). [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | `phx_annot::Tier` (`Interval`/`Point` variants inside `TierSlot`) is always part of an `Annotation`; no standalone extraction API. | niche — Phonia's document-first model makes this less necessary, not a blocking gap |
| TextGrid ↔ Table | Tier content tabulated to a `Table` for spreadsheet-style export/analysis. | No `Table` object exists in Phonia at all (§3.1). | valuable — depends on Table existing first |
| PointProcess → TextGrid (voiced/unvoiced) | `PointProcess: To TextGrid (vuv)...` builds interval tiers from pulse voicing decisions. [TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html) | Phonia has pulse extraction (`phx_voice::pulses`) but no auto-segmentation into tiers from it. | niche |

### 1.5 Semi-automatic segmentation and search (adjacent to annotation)

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `Sound: To TextGrid (silences)...` | Intensity-threshold silence detection with min-interval parameters, produces a two-label interval tier. [Sound: To TextGrid (silences)](https://www.fon.hum.uva.nl/praat/manual/Sound__To_TextGrid__silences____.html) | None. | valuable — cheap, high-value automation; not yet on the roadmap |
| Silero VAD / Whisper transcription / diarization | 2025–2026 Praat additions: neural voice-activity segmentation, whisper.cpp interval transcription, pyannote diarization. [Sound: To TextGrid (speech activity, Silero)](https://www.fon.hum.uva.nl/praat/manual/Sound__To_TextGrid__speech_activity__Silero____.html) | None; tracked as horizon.md H2 item 7, explicitly investigation-first. | valuable, already on the roadmap as future work |
| `TextGridNavigator`-based search | See §1.1. | `LabelQuery` (substring/regex, tier-scoped), single-pass. | core-parity for simple search; valuable gap for compound multi-tier queries |

---

## 2. Scripting

Praat's scripting language has no analog in Phonia by design — the
architecture replaces it with a typed engine API plus bindings
(`BRIEF.md`), not a bespoke interpreter. This section inventories what such
an API (and, per `horizon.md` item 16, an eventual `.praat`-script
compatibility layer) would need to account for.

### 2.1 Control flow

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `if/elsif/else/endif` | Standard branching; `elsif` also spellable `elif`. [Scripting 5.3](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_3__Jumps.html) | N/A — no scripting language; the engine command API expresses control flow in the host language (Rust/TS/Python). | deliberately-out-of-scope as a *language feature* — see §2 preamble |
| `for/endfor`, `while/endwhile`, `repeat/until` | Fixed step-1 for-loop, pre-test while, post-test repeat. [Scripting 5.4](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_4__Loops.html) | N/A, same reasoning. | deliberately-out-of-scope |
| `goto`/label | Not documented; no such construct exists in Praat. | N/A | n/a |
| `assert`, `asserterror` | Script-aborting assertion; `asserterror "substring"` is a negative-test idiom checking the next line raises a matching error [source-observed]. | N/A (Rust's own `assert!`/test harness serves this role for Phonia's own code, not for user scripts). | deliberately-out-of-scope |
| Error handling | A runtime error unwinds the whole script by default; `nocheck <command>` swallows exactly one command's error [source-observed]. | The engine returns `Result` per command; no error-recovery language construct is needed because there is no interpreter. | deliberately-out-of-scope |

### 2.2 Variables and types

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Numeric / string scalars (`$`-suffix typing) | Untyped-looking but strictly separated at the formula-evaluation level; no implicit numeric↔string coercion [source-observed]. [Scripting 3.4](https://www.fon.hum.uva.nl/praat/manual/Scripting_3_4__String_variables.html) | Rust's static type system replaces this outright; every engine command has typed fields. | deliberately-out-of-scope |
| Indexed array/dictionary variables (`name[index]`) | Dynamically created global variables keyed by the literal string `"name[index]"`, unbounded, never bounds-checked [source-observed]. [Scripting 5.6](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_6__Arrays_and_dictionaries.html) | N/A. | deliberately-out-of-scope — this is exactly the kind of footgun `BRIEF.md` cites as a design fix |
| Numeric/string vectors and matrices (`#`, `##`, `$#`) | Bounds-checked, unlike bracket-indexed scalars [source-observed]. Literal syntax, `zero#()`/`randomGauss#()`/`mul##()` etc. [Scripting 5.7](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_7__Vectors_and_matrices.html) | `Vec<f64>`/`ndarray` types throughout the Rust core, always bounds-checked. | deliberately-out-of-scope as language syntax; the underlying capability (vector math) is already core to every `phx-*` crate |
| Predefined variables (`pi`, platform flags, `praatVersion`) | Interpreter globals. [Scripting 5.1](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_1__Variables.html) | N/A. | deliberately-out-of-scope |

### 2.3 String handling

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Concatenation, `length`/`mid$`/`replace$`/case-conversion/etc. | A large fixed function library, mostly ASCII-oriented. [Formulas](https://www.fon.hum.uva.nl/praat/manual/Formulas.html) | Rust's `std::str`/Unicode-aware string handling throughout. | deliberately-out-of-scope as a scripting surface |
| Regex via `_regex`-suffixed functions | `index_regex`, `rindex_regex`, `replace_regex$` — not a general regex literal syntax, just these functions [source-observed]. [Formulas 3](https://www.fon.hum.uva.nl/praat/manual/Formulas_3__Operators.html) | `LabelQuery::regex` uses the `regex` crate directly — already a cleaner, more general surface than Praat's bespoke functions. | core-parity — done, superset |

### 2.4 `do()` / command invocation

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `Command: arg1, arg2` (modern) / `Command... arg1 arg2` (legacy) syntax | GUI-button-label command strings, resolved at runtime in two ordered phases (selection-dependent actions, then fixed menu commands) [source-observed]. [Scripting 2](https://www.fon.hum.uva.nl/praat/manual/Scripting_2__How_to_script_settings_windows.html) | Every operation is a typed `Command` enum variant with named fields — no string dispatch, no selection dependency (see §2.5). | deliberately-out-of-scope as a string-dispatch mechanism — this is the exact pain point `BRIEF.md`/pain-point 2.4 targets |
| `execute`, `runScript` | Run another script's text/file, `runScript` passing explicit positional args bypassing the form. [Scripting 5.8](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_8__Including_other_scripts.html), [Scripting 6.1](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_1__Arguments_to_the_script.html) | N/A directly; a future Python-binding "macro" surface (horizon.md item 15) would cover the equivalent need. | valuable, tracked as future work (not v0.1) |

### 2.5 Object selection semantics

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `selectObject`/`plusObject`/`minusObject`, `selected()` | One process-global selection flag per object in a single shared list; no save/restore mechanism, no per-scope isolation — a `@procedure` call sees and can mutate the caller's selection state [source-observed]. [Scripting 4.1](https://www.fon.hum.uva.nl/praat/manual/Scripting_4_1__Selecting_objects.html), [4.3](https://www.fon.hum.uva.nl/praat/manual/Scripting_4_3__Querying_objects.html) | No selection state at all in the engine API — every command takes explicit object IDs as arguments. This is the structural fix `BRIEF.md` names for pain point 2.1. | **already fixed at the architecture level** — core-parity in the sense that the *capability* (act on a specific object) exists and is strictly safer |
| Selection-failure semantics | Selecting a nonexistent ID/name throws a script-aborting error [source-observed]. | A command referencing a missing ID returns a typed `Err`, not a panic. | core-parity — done, safer |

### 2.6 Procedures

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `procedure`/`@name`, dot-prefixed "local" variables | No native return value (convention: write to `.name` fields the caller reads after return); dot-variables are not true lexical locals — they're the same flat global table, keyed per call-stack depth so recursive calls get independent slots, but any non-dot variable inside a procedure is a genuine global [source-observed]. [Scripting 5.5](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_5__Procedures.html) | Rust functions with real return values and lexical scoping throughout. | deliberately-out-of-scope as a language feature |

### 2.7 Include files

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `include filename.praat` | Textual inclusion before script execution; no separate namespace, shares the parent's flat variable table; relative paths resolve against the *outermost* script's folder even when nested [source-observed]. [Scripting 5.8](https://www.fon.hum.uva.nl/praat/manual/Scripting_5_8__Including_other_scripts.html) | N/A — Rust's module system (crates) is the equivalent for the core; a Python-binding layer would use ordinary `import`. | deliberately-out-of-scope |

### 2.8 `form...endform` dialogs

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Field types: `real`/`positive`/`integer`/`natural`, `word`/`sentence`/`text`, `choice`/`optionmenu`, `boolean`, `infile`/`outfile`/`folder`, `*vector` types, `comment` | One shared field vocabulary reused by `form`, `beginPause`/`endPause`. [Scripting 6.1](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_1__Arguments_to_the_script.html) | The UI's non-modal inspector panels (per `BRIEF.md`'s fix for pain point 2.2) and the command-palette's parameterized actions cover the same *need* — parameter capture — but as live-updating UI, not a blocking modal form language. | core-parity for the underlying need, structurally different and better (non-modal, live re-analysis) — matches the explicit redesign direction in `praat-features-and-pain-points.md` §2.2 |

### 2.9 Output

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `writeInfoLine`/`appendInfoLine`, file writers, `pauseScript`/`beginPause` | Info-window text output; file read/write/append; blocking pause dialogs. [Scripting 6.2](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_2__Writing_to_the_Info_window.html), [6.4](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_4__Files.html), [6.6](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_6__Controlling_the_user.html) | The UI surfaces results directly (voice-report cards, inspectors) rather than a text log; project/figure export covers the file-writing need (§3, §4). No scripting-console equivalent exists or is planned for v0.1. | deliberately-out-of-scope for v0.1; a future Python binding would use ordinary `print`/file I/O |

### 2.10–2.11 Table creation and file looping (brief, see §3 for Table depth)

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `Create Table with column names...` | Entry point into Table objects (§3.1). [Create Table with column names...](https://www.fon.hum.uva.nl/praat/manual/Create_Table_with_column_names___.html) | No Table object exists (§3.1). | valuable, blocked on §3.1 |
| `Create Strings as file list`, `fileNames$#()` | Corpus batch-processing backbone: enumerate files matching a glob into a script-iterable list. [Scripting 6.4](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_4__Files.html) | Project folder-drop import (`phx-project`) covers the *use case* (bring a folder of recordings into one session) but there is no scripting-level file-enumeration primitive, because there is no scripting layer. | core-parity for the underlying workflow (folder import), deliberately-out-of-scope as a scripting primitive |

### 2.12–2.17 Demo window, editor blocks, command line, sendpraat, plugins, standalone programs

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Demo window (`demo`, `demoWaitForInput`, click/key events) | Separate interactive-graphics window, scripted event loop; also the UI surface for standalone Demo-only programs. [Demo window](https://www.fon.hum.uva.nl/praat/manual/Demo_window.html) | N/A. | niche |
| `editor...endeditor` blocks | Switches script execution into a specific open editor window's own command namespace [source-observed confirms this is a genuinely separate namespace, `Scripting_7_2`]. | N/A — no editor/object namespace split exists to begin with (§2.5). | deliberately-out-of-scope |
| `--run`/`--send`/`--no-pref-files`/`--utf8` etc. | Headless batch execution and GUI-driving command-line flags. [Scripting 6.9](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_9__Calling_from_the_command_line.html) | The Tauri desktop app takes OS open-with/file-association launches; no headless batch CLI exists yet (the engine is a library, so one is straightforward to add later). | valuable — natural fit for a future CLI over the same engine crate |
| `sendpraat` | Message an already-running instance from another program. [Scripting 8](https://www.fon.hum.uva.nl/praat/manual/Scripting_8__Controlling_Praat_from_another_program.html) | N/A. | niche |
| Plugins (`plugin_*` folders, `Add menu command`) | Auto-loaded `setup.praat` scripts register new menu commands at each launch. [preferences folder](https://www.fon.hum.uva.nl/praat/manual/preferences_folder.html) | Tracked as horizon.md H3 item 18 ("analysis plugin surface"), investigation-first, candidate shape WASM components — not built. | valuable, already scoped as future work |
| Turning a script into a standalone program | Requires recompiling Praat with the script embedded; GPL-v3 redistribution obligation stated explicitly by the manual. [Scripting 9](https://www.fon.hum.uva.nl/praat/manual/Scripting_9__Turning_a_script_into_a_stand-alone_progra.html) | N/A — Phonia's apps already ship as standalone products by design. | deliberately-out-of-scope |

### 2.18 Dependency-weight assessment (how much of the ecosystem this blocks)

Ranked by how central each area is to scripts actually circulating in
phonetics teaching and corpus pipelines (judgment call from the manual's own
tutorial ordering and emphasis, not measured usage data):

1. **Very heavy** — command invocation (§2.4), the select-then-act model
   (§2.5), scalar variables (§2.2), control flow (§2.1), Info-window output
   (§2.9). Unavoidable in literally any script.
2. **Heavy** — `form` dialogs (§2.8), file-list looping (§2.11), string
   handling (§2.3).
3. **Moderate** — procedures (§2.6), include files (§2.7), pause dialogs
   (§2.9).
4. **Light** — vectors/matrices (§2.2), Table creation (§2.10), command-line
   flags (§2.14–2.17 boundary).
5. **Rare** — editor blocks, `sendpraat`, Demo window, plugins, standalone
   programs.

---

## 3. Data

### 3.1 Tables and statistics

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| `Table` object (mixed string/numeric columns) | Rows/columns, per-column name, string or numeric cells; created via dialogs, canned example datasets, or conversion from other object types. [Table](https://www.fon.hum.uva.nl/praat/manual/Table.html) | No Table/TableOfReal crate exists anywhere in the workspace (confirmed by search — no `Table`, `PCA`, `regression`, `correlation` hits outside colormap/UI code). | valuable — the single largest structural gap in the DATA area |
| Query (`Get value`, quantile/min/max/mean/stdev/group mean) | Dialog and scripting-only "dynamic menu" query commands. [Table](https://www.fon.hum.uva.nl/praat/manual/Table.html) | None. | valuable, blocked on the Table object existing |
| Manipulation (`Formula...`, `Sort rows...`, `Extract rows where...`, `Append/Remove column/row`) | Column-wise formula application, filtering, sorting. [Table](https://www.fon.hum.uva.nl/praat/manual/Table.html) | None. | valuable |
| Table ↔ file (tab/comma/semicolon-separated) | Read/write delimited text, plus a headerless-spreadsheet reader that produces a `TableOfReal`. [Table](https://www.fon.hum.uva.nl/praat/manual/Table.html) | `phx-figure`'s code backend emits CSV sidecars for generated plotting scripts, but there is no general-purpose, standalone data-table export independent of a figure. | valuable — a real gap: Phonia currently has no "export my measurements as a spreadsheet" path outside figure generation |
| `TableOfReal` (pure numeric matrix + row/column labels) | Required input type for the multivariate stats below. [TableOfReal](https://www.fon.hum.uva.nl/praat/manual/TableOfReal.html) | None. | valuable |
| Correlation (Pearson, Spearman rank) | `TableOfReal: To Correlation`, `To Correlation (rank)`; confidence intervals via Fisher's z / Ruben's transformation. No Kendall's tau in Praat at all. [TableOfReal: To Correlation](https://www.fon.hum.uva.nl/praat/manual/TableOfReal__To_Correlation.html) | None. | valuable — routine phoneticians' work (the manual's own worked examples are vowel-formant datasets) |
| Principal Component Analysis | `TableOfReal: To PCA`, scree plots, projection to `Configuration`, reconstruction. [Principal component analysis](https://www.fon.hum.uva.nl/praat/manual/Principal_component_analysis.html) | None. | valuable — routine for vowel-space work, "next tier up" in complexity per the manual's own framing |
| Discriminant analysis | `TableOfReal: To Discriminant`, classification, confusion-matrix validation. [Discriminant analysis](https://www.fon.hum.uva.nl/praat/manual/Discriminant_analysis.html) | None. | valuable |
| Canonical correlation analysis | `TableOfReal: To CCA...`. [Canonical correlation analysis](https://www.fon.hum.uva.nl/praat/manual/Canonical_correlation_analysis.html) | None. | niche — specialist multivariate tool |
| Linear regression | **Not present in Praat** — no `Table`/`TableOfReal` "To/Report linear regression" command exists in the current manual (checked directly). | None. | n/a — nothing to match |
| Logistic regression | `To logistic regression...`, MLE fit of a binary/categorical response, standard for categorical-perception experiments. [Logistic regression](https://www.fon.hum.uva.nl/praat/manual/Logistic_regression.html) | None. | niche-to-valuable — common specifically for perception studies, not general acoustic work |
| t-tests, one-/two-way ANOVA, Kruskal-Wallis | `Table: Report mean/group mean (Student t)...`, `Report one-way/two-way anova...` (with Tukey HSD option), `Report one-way Kruskal-Wallis...`. [T-test](https://www.fon.hum.uva.nl/praat/manual/T-test.html), [Table: Report one-way anova...](https://www.fon.hum.uva.nl/praat/manual/Table__Report_one-way_anova___.html) | None. | valuable — routine significance-testing surface phoneticians reach for constantly |

### 3.2 File I/O — audio

| Format | Praat read | Praat write | Phonia read | Phonia write | Verdict |
|---|---|---|---|---|---|
| WAV | Linear 16-bit, 8-bit µ-law/A-law, linear 8-bit unsigned. [Sound files 3](https://www.fon.hum.uva.nl/praat/manual/Sound_files_3__Files_that_Praat_can_read.html) | Linear 16-bit. [Sound files 4](https://www.fon.hum.uva.nl/praat/manual/Sound_files_4__Files_that_Praat_can_write.html) | Yes, via `hound`, all standard PCM formats. | Yes — PCM 8/16/24/32-bit and Float32 (a superset of Praat's own write depths). | core-parity — done, superset on write |
| AIFF/AIFC | Linear 16-bit, linear 8-bit signed. | Same as WAV note, AIFF-form. | Yes, via `symphonia`. | No. | valuable — read parity done, write is the gap |
| NeXT/Sun (`.au`) | Linear 16-bit BE, µ-law, A-law, linear 8-bit signed. | Same depths. | No. | No. | niche — legacy Unix format, rare in modern corpora |
| NIST (Sphere) | Linear 16-bit LE/BE, µ-law, A-law, linear 8-bit signed. | Same depths. | No. | No. | niche-to-valuable — still used by some speech corpora (e.g. TIMIT-lineage sets) |
| FLAC | 8/16/24/32-bit, any sample rate. | Same. | Yes, via `symphonia`. | No. | valuable — read parity done, write is a real gap (lossless archival export) |
| MP3 | Any CBR/VBR. Read-only in Praat too. | — | No. | — | niche — Praat itself doesn't write it either; low priority given licensing complexity |
| Ogg Vorbis / Opus, Shorten | Read-only. | — | No. | — | niche |
| LongSound (disk-backed streaming) | Cannot edit directly; view/label via `LongSoundEditor`, extract to `Sound`, ~2 GB / ~3 hr practical ceiling. [LongSound](https://www.fon.hum.uva.nl/praat/manual/LongSound.html) | — | `phx-audio` has a `stream.rs` module (streamed source path referenced in horizon.md T8.3); full long-file streaming UX is tracked, not yet gated as complete. | — | core-parity direction, in progress |

### 3.3 File I/O — annotation formats

| Format | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| TextGrid long text format | Header `File type = "ooTextFile"` / `Object class = "TextGrid"`, then positional `xmin`/`xmax`/tiers-flag/tier-count/tier-blocks; structural labels (`xmin =`, `item [n]:`) are comments, ignored on read. [TextGrid file formats](https://www.fon.hum.uva.nl/praat/manual/TextGrid_file_formats.html); generic serialization mechanism per `docs/formats/praat-formats.md` §1. | Read and write, UTF-8. | core-parity — done |
| TextGrid short text format | Same data and order, comments stripped. [TextGrid file formats](https://www.fon.hum.uva.nl/praat/manual/TextGrid_file_formats.html) | Read (variant auto-detected structurally, not by filename); write always emits long format only (a deliberate simplification — one canonical write shape, per `praat-features-and-pain-points.md` §2.7's redesign direction). | core-parity — read done; write-short intentionally not offered |
| TextGrid binary format | Undocumented by the manual ("we can add it here on request"). Full field layout — `ooBinaryFile` magic, class-name/tier-name `w8`/`w16` string codecs, big-endian `f64`/`i32`/`bool8` fields, no padding/checksums — reconstructed from source in `docs/formats/praat-formats.md` §1, §3. | Read, independently reverse-engineered via oracle sample-pairs (`crates/phx-textgrid/docs/binary-format.md`, since corrected against source per `praat-formats.md` §7) before this task ever consulted Praat source. Write intentionally not offered — the writer always emits text (§ TextGrid long text format row above). | **already ahead of the manual** — Phonia understands and reads a format Praat itself declines to document; core-parity on read, write is a deliberate one-canonical-format choice, not a gap |
| Chronological TextGrid text format | Real, currently supported, text-only. A flat, time-and-tier-sorted record stream, `"Praat chronological TextGrid text file"` header tag. Introduced Praat 4.2 (2004). [source-observed, `docs/formats/praat-formats.md` §4] | No support (read or write). | niche — a corpus-interchange convenience, not a primary annotation format; low priority relative to the Table gap |
| Xwaves/ESPS label files | Asymmetric: reader accepts multi-tier, multi-field, point- or interval-tier files with a header (`nfields`, `separator`); writer only ever emits a single interval tier, no `xmin`/tier-domain preserved, hardcoded colour code. [source-observed, `docs/formats/praat-formats.md` §5] | No support. | niche — legacy ESPS-suite interchange, rare outside older corpora |
| Generic `ooTextFile`/`ooBinaryFile` object serialization (any object type) | Shared substrate for every Praat object type, text and binary; positional field order from a compile-time class description; class auto-detected from the file's own header. [Save as text file...](https://www.fon.hum.uva.nl/praat/manual/Save_as_text_file___.html), full mechanism in `docs/formats/praat-formats.md` §1 | Not applicable — Phonia has no generic "any object" serialization scheme; each domain object (`Annotation`, `Project`) has its own purpose-built format. | deliberately-out-of-scope — this generic-but-undiscoverable serialization is itself one of Praat's named pain points (2.7); a typed, versioned, per-domain format is the stated redesign direction |
| Collection files (multi-object bundle) | Selecting >1 object and saving wraps them as one `Collection`-class file; restores every object under its original name. [Save as text file...](https://www.fon.hum.uva.nl/praat/manual/Save_as_text_file___.html) | The `phonix-project` container (`docs/formats/project.md`) is the direct, stronger replacement: a versioned ZIP bundling media references, annotations, parameter profiles, and view state — not just a flat object dump. | **superset already** — this is the fix for pain point 2.12 (no project/session concept) |

### 3.4 Text encoding

| Behavior | Praat | Phonia | Verdict |
|---|---|---|---|
| Write-time encoding selection | User preference (`kMelder_textOutputEncoding`), default "try ASCII, then UTF-16": ASCII with no BOM if every string is 7-bit-representable, otherwise the whole file is promoted to UTF-16BE with a leading `FE FF` BOM. Alternative preference values force UTF-8 or "try Latin-1, then UTF-16." [source-observed, `docs/formats/praat-formats.md` §6.1] | Always writes UTF-8, no BOM, unconditionally — one canonical output encoding regardless of content. | core-parity — done, and structurally the fix for pain point 2.6 (files silently switching encoding based on label content) |
| Read-time detection | Two layers: BOM sniff (`FE FF`/`FF FE`/`EF BB BF`) for UTF-16BE/LE/UTF-8-with-BOM; otherwise, UTF-8 validity test; if that fails, a single fixed 8-bit codec chosen by a *user preference*, defaulting per build platform (Windows Latin-1 / MacRoman / ISO Latin-1) — **not content-based detection**, confirmed by source: Praat cannot distinguish MacRoman from Latin-1 from content either, and doesn't try. [source-observed, `docs/formats/praat-formats.md` §6.2] | Detects BOM for UTF-8/UTF-16, then UTF-8 validity, then a single fixed Latin-1 fallback (no platform-dependent default, no user-selectable legacy codec). | core-parity on the detection ceiling — Phonia is not missing a capability Praat has (Praat has no content-based MacRoman/Latin-1 detection either); the real, narrower gap is a user-selectable legacy-codec *preference*, which Phonia doesn't expose |
| Line endings | Windows builds rewrite `\n` → `\r\n` on write (platform default, not user-facing); reads normalize `\n`/`\r\n`/`\r` uniformly. [source-observed, `docs/formats/praat-formats.md` §6.4] | Writer emits `LF` unconditionally, on every platform. | core-parity — done, simpler (no platform-conditional writer branch) |

---

## 4. Output — Picture window and drawing

### 4.1 Canvas and menu structure

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Accumulate-only canvas | Draw commands from any selected object append strokes to a persistent canvas; "the Picture window is one of Praat's two main windows." [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `phx-figure::Figure` is a retained scene-graph model (panels, layers, axes, provenance) rebuilt from data on every render, not an accumulate-and-replay canvas. | **already the redesign target** — this is exactly the fix `praat-features-and-pain-points.md` §2.3 calls for (retained-mode plot objects vs. append-only canvas) |
| Erase all / Undo | `Erase all` clears the canvas; a documented one-level-ish Undo reverses the last draw action. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | Not applicable in the same sense — a `Figure` is rebuilt from its model, not incrementally drawn-and-undone; the export dialog (`ExportDialog.svelte`) re-renders from current state on every change. | core-parity for the underlying need (revise a figure), structurally different |
| Viewport selection (inner/outer) | Sub-rectangle targeting via the Select menu, for composing multi-panel figures. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `Panel`/`Figure` model composes multiple panels natively (`Figure::panel()` in the builder) — declared layout, not manual viewport arithmetic. | core-parity — done, structurally simpler |
| Margins / World menus (axes, ticks, data-coordinate labels) | Axis range, tick marks, "Text left/right/top/bottom," mark placement at data coordinates. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `Axis`/`AxisScale` (linear; log support present as an enum variant) in `phx-figure::model`. | core-parity — done |
| Pen menu (line type/width/color/arrows) | Stroke state for subsequent draws. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `LineStyle`/`DashStyle`/`RgbaColor` in `phx-figure::style`, per-layer rather than global pen state. | core-parity — done |
| Font menu | Typeface/size for subsequent text draws, including IPA-capable fonts. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | Text sizing in typographic points (`phx-figure::style`); font-family selection is theme-driven, no free font picker yet. | valuable — matches horizon.md H1 item 5's user-selectable IPA display font, not yet shipped |

### 4.2 Object-specific draw/paint commands

| Capability | Praat behavior | Phonia today | Verdict |
|---|---|---|---|
| Sound: Draw (waveform) | Waveform trace over a time range. | `waveform_layer`/`waveform_minmax` in `phx-figure::builder`. | core-parity — done |
| Spectrogram: Paint | dB-mapped tile image. [Intro 3.2](https://www.fon.hum.uva.nl/praat/manual/Intro_3_2__Configuring_the_spectrogram.html) | `spectrogram_layer` with `DisplayMapping`/`Colormap` (9 colormaps in `phx-render`, exceeding Praat's stock set — see `praat-features-and-pain-points.md` history of viridis/magma/inferno/plasma/cividis additions). | core-parity — done, superset (colormap choice) |
| Pitch: Draw / Speckle | Contour or candidate-point rendering. | `pitch_layer` with `PitchUnit`. | core-parity — done |
| Formant: Speckle / Draw tracks | Formant candidate points or smoothed tracks. | `formant_layer` with `smoothed` toggle and `SpeckleStyle`. | core-parity — done |
| Intensity: Draw | dB contour. | `intensity_layer`. | core-parity — done |
| TextGrid: Draw (alone, or with Sound/Pitch) | Tier rendering, optionally composited with another track. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `tiers_layer`/`tier_data`, composable into any panel alongside other layers (the figure model is layer-stacking by design, not per-combination special cases). | core-parity — done, structurally more general |
| Spectrum: Draw / spectral slice | Single-frame spectral slice plot. [Intro 3.6](https://www.fon.hum.uva.nl/praat/manual/Intro_3_6__Viewing_a_spectral_slice.html) | `spectral_slice_layer`. | core-parity — done |
| Drawing primitives (lines, rectangles, text, insert external picture) | Free-standing draw commands independent of any object. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | Not exposed as a general free-drawing primitive API; the figure model is data-layer-driven, not a general canvas. | niche — a deliberate scope choice (Phonia figures are data visualizations, not a general drawing tool), reasonable to leave out |

### 4.3 Export

| Format | Praat | Phonia | Verdict |
|---|---|---|---|
| PDF | Vector export from the canvas. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | `to_pdf` — SVG scene graph converted via `svg2pdf`, vector text/tracks/axes preserved. | core-parity — done |
| PNG | Raster export, documented silent-failure ceiling above ~240 inches of canvas width (pain-point catalog 2.13). | `to_png` — rasterizes the same SVG source via `resvg`/`tiny-skia` at a chosen DPI; no documented size ceiling. | core-parity — done, more robust |
| EPS | Vector export. | Not offered. | niche — EPS is a legacy publisher-workflow format; PDF/SVG supersede it for virtually every modern venue |
| Windows metafile | Vector export, Windows-specific. | Not offered. | niche — platform-specific legacy format |
| Praat picture file (round-trippable) | Native format for re-opening/re-editing a saved figure state. | `Figure::to_json`/`from_json` — a documented, versioned JSON serialization of the whole figure model. | core-parity — done, and self-describing rather than a bespoke binary |
| SVG | **Not offered by Praat at all** — a named pain point (2.13). | `to_svg` — the scene-graph source of truth every other backend derives from. | **already the fix** — closes a gap Praat has never closed |
| PGFPlots/TikZ, Typst/CeTZ, Vega-Lite, GraphML, generated matplotlib/ggplot2/Makie code + data sidecars | **Not offered by Praat at all.** | `phx-figure` ships all of these as backends (`backends/tikz.rs`, `typst.rs`, `vega.rs`, `graphml.rs`, `code.rs`). | **already a superset** — this is the "better than anything Praat ships" gate from `roadmap.md` phase 5, met |
| PostScript settings (resolution/paper/greyscale) | Print-era controls for the above exports. [Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html) | DPI is a parameter to `to_png`; no separate print/paper-size settings panel, since PDF/SVG are resolution-independent by construction. | niche — print-era concern, largely obviated by vector-first export |

---

## 5. Summary

### Verdict counts

Counted directly from every inventory-table row in §1–§4 (108 rows total; a
handful carry a compound verdict and are counted under their primary
category):

| Verdict | Count |
|---|---|
| core-parity (Phonia already matches, done) | 40 |
| core-parity (matches, in progress) | 1 |
| superset already (Phonia exceeds Praat) | 7 |
| valuable (real gap, worth building) | 23 |
| niche (specialist/rare, low priority) | 19 |
| deliberately-out-of-scope | 16 |
| n/a (nothing in Praat to match, or vice versa) | 2 |

### Biggest gaps, ranked

1. **No Table/TableOfReal object or statistics surface at all** (§3.1). This
   is the single largest missing subsystem — not one feature but an entire
   category: query/manipulate a data table, correlation, PCA, discriminant
   analysis, ANOVA/t-tests/Kruskal-Wallis, and a standalone CSV/TSV export
   path independent of figure generation. Every one of these is described by
   Praat's own manual using vowel-formant worked examples — squarely
   everyday phonetics work, not edge-case statistics.
2. **Audio write breadth** (§3.2): FLAC and AIFF are read but not written;
   NIST/NeXT-Sun are neither read nor written. WAV write already exceeds
   Praat's own bit-depth range, so this is a narrower gap than it looks —
   closing FLAC write specifically (lossless archival export) is the
   highest-value single item here.
3. **Small, well-defined annotation-editing gaps** (§1.3): tier rename,
   tier duplicate, and cross-TextGrid merge. All three are structurally
   simple — `RenameAudio` already exists as a template for tier rename, and
   `TierMerge`/boundary-merge machinery already exists for the merge case at
   the interval level, just not at the tier/document level yet.
4. **Reusable multi-tier search** (`TextGridNavigator`, §1.1/§1.5):
   Phonia's `LabelQuery` is single-pass and already exceeds Praat on regex
   support, but has no persisted, reusable, boolean multi-tier query object
   for corpus-scale conditional search.
5. **Cheap automatic segmentation entry points** (§1.5): `To TextGrid
   (silences)...` has no equivalent. Low implementation cost relative to
   value, and distinct from the heavier VAD/Whisper/diarization work already
   tracked as future investigation.
6. **Legacy annotation interchange formats** (§3.3): chronological TextGrid
   and Xwaves/ESPS label files are unsupported. Both are niche relative to
   the Table gap, but chronological TextGrid in particular is a real,
   still-current Praat feature (not deprecated), so it belongs on a tracked
   list even at low priority.

### How large is the scripting-compatibility problem, really

Large, but concentrated. The scripting inventory (§2) and its dependency-
weight ranking (§2.18) show that a small number of areas — command
invocation, the select-then-act object model, scalar variables, control flow,
and Info-window output — account for the overwhelming majority of what any
everyday phonetics-course or corpus-pipeline script actually does. Those are
also exactly the areas Phonia's architecture already replaces by design
(typed commands instead of string dispatch, explicit object arguments instead
of global selection state, a real type system instead of untyped scalars)
rather than needing to *emulate*.

The honest difficulty is not reproducing Praat's language semantics — it's
the long tail: `.praat` scripts in the wild lean on quirks that are individually
small but numerous — the `dictionary["key"]` string-keyed-global array trick,
dot-variable pseudo-locals inside procedures that are secretly stack-depth-keyed
globals, `nocheck`-style partial error suppression, `editor...endeditor`'s
separate command namespace, and legacy space-separated command syntax living
alongside the modern colon syntax in the same corpus of real-world scripts.
Building a compatibility *interpreter* that runs existing `.praat` files
correctly would mean replicating all of that fringe behavior faithfully, not
just the core 20%. That is precisely why `horizon.md` item 16 scopes this as
*investigation-first* with "a corpus study of real scripts (what fraction
uses which commands) before any interpreter design" as its first step, rather
than committing to a build. This inventory supports that caution: the
core-workflow surface is small and already superseded by Phonia's typed API;
the compatibility-surface tail is what would actually be expensive, and its
size can only be answered empirically, by the corpus study horizon.md already
calls for — not by reading the manual further.

A Python-bindings layer (horizon.md item 15, a parselmouth-shaped `phonia`
package over the same typed engine) captures most of the *practical* value
routine Praat scripts provide — parameterized, repeatable, file-looping
analysis — without needing to emulate `.praat` syntax or its selection-state
semantics at all. That path is lower-risk and higher-value than a
compatibility interpreter and should stay the priority over `.praat`-script
emulation.
