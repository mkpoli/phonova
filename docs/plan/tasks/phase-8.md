# Task specs — phase 8 (library management, navigation, I/O)

Standing constraints as in `phase-1.md`. Visual work in this phase is
verified against tests and `docs/DESIGN.md` on every diff.
DESIGN.md is binding for every UI task in this phase; its principle tests are
part of each task's gate. Container-format changes follow
`docs/formats/project.md` versioning: the format tag stays `phonix-project`,
`version` moves 1 → 2, readers accept both, writers emit 2.

Waves order the work by dependency and file collision. Wave A never touches
`apps/`; wave B tasks each own `apps/` exclusively while they run.

## Wave A — engine and container

### T8.1 · container v2: groups, order, metadata
**Objective.** The project container carries recording order, a group tree,
and descriptive metadata.
**Files.** `crates/phx-project/src/{model,container,media}.rs`,
`docs/formats/project.md` (new v2 section).
**Interfaces.**
```rust
pub struct Group { pub id: GroupId, pub name: String,
  pub children: Vec<LibraryNode> }        // LibraryNode = Media(MediaId) | Group(GroupId)
pub struct Project { /* + groups: Vec<LibraryNode> (ordered root),
  description: String, authors: Vec<String>, tags: Vec<String> */ }
pub struct MediaRef { /* + description: String, authors: Vec<String>,
  tags: Vec<String> */ }
```
**Constraints.** Standing set. v1 files load with empty metadata and a flat
root in manifest order; a v2 file's `media` array stays the flat source of
truth and `groups` references it by id (a reader ignoring `groups` still sees
every recording). Deterministic serialization order documented. Integrity:
every media id appears in the tree exactly once; loader repairs or rejects
per the format doc.
**Verification.** Round-trip v2; v1 fixture loads (defaults applied);
deterministic bytes; tree-integrity property test over random group edits;
`docs/formats/project.md` updated in the same commit.

### T8.2 · engine: rename, remove, band-filtered render, span export
**Objective.** Session-level library operations and selection audio out.
**Files.** `crates/phx-engine/src/*`, `crates/phx-dsp/src/*` (filter),
`crates/phx-wasm/src/lib.rs`.
**Interfaces.**
```rust
Command::RenameAudio { id, name }         // journaled, inverse = old name
Command::DetachAudio { id }               // journaled; restore keeps AudioId
pub fn export_span_wav(&self, id, t0, t1, bits) -> Result<Vec<u8>, _>;
pub fn band_filtered_span(&mut self, id, t0, t1, f_low, f_high)
    -> Result<Vec<f32>, _>;               // mono, source rate, for playback
```
**Constraints.** Standing set. Band filtering by spectral multiplication:
forward real FFT of the padded span, unity gain inside `[f_low, f_high]`,
zero outside, raised-cosine skirts of 100 Hz (half-cosine rolloff; the
smoothing form Praat's manual documents for "Spectrum: Filter (pass Hz)…" —
cite the manual page; skirt width a documented constant), inverse FFT,
edge-taper the first/last 5 ms. DetachAudio refuses while annotations
reference the audio unless they are detached in the same command (design the
inverse so undo restores both).
**Verification.** Filtered sine inside band unchanged (< 0.1 dB), outside
band < −60 dB, skirt monotonic; span export equals eager slice bit-for-bit;
rename/detach journal round-trips hash-stable in the 50-op mix; wasm
bindings tested in node.

### T8.3 · streamed-source integration (task #15)
**Objective.** The streaming core reaches users end-to-end.
**Files.** `apps/web/src/lib/core/wasm-worker.ts` (+client),
`crates/phx-spectrogram/src/lib.rs`, `crates/phx-engine/src/*`,
`apps/desktop/src-tauri/src/*` (environment permitting).
**Interfaces.** Worker import path: files over the eager threshold persist to
OPFS first, then `openStreamingWav` over a sync access handle — whole-file
bytes never cross into wasm. `phx-spectrogram` gains an offset-aware
column-block entry so streamed tiles decode only the block's sample range.
Desktop: dialog → path → `Send + Sync` file reader command.
**Constraints.** Standing set; streamed and eager results stay bit-identical
(existing equality suites extend to the new paths); the ALSA build gap may
defer the desktop command — report, do not shim system state.
**Verification.** Web e2e: import a generated 10-minute WAV, time-to-corpus
< 3 s, RSS bound reported, scroll gate green; equality tests; existing
suites green.

## Wave B — interface (each task owns `apps/` while active)

### T8.4 · library management (feedback A, B, E, G)
**Objective.** The corpus becomes a library: rename anywhere, clear
affordances, groups, order, deletion, metadata, search.
**Files.** `apps/ui/src/lib/{ProjectView,HomeView,EditorView}.svelte`, new
`LibraryTree.svelte`/`MetadataPanel.svelte`, `apps/web/src/lib/project/
ProjectStore.ts`, `apps/web/src/routes/+page.svelte`, e2e.
**Interfaces.** Row click opens; the name is a distinct inline-edit target
(pencil icon on hover, `F2`, double-click on the name only) — the two
affordances visibly separate (feedback B). Rename works on home cards,
corpus rows, and the editor breadcrumb, all through the same store/engine
path. Group tree in the layers register: drag to reorder, drag onto a group
to nest, create/rename/dissolve groups, collapse state persisted in view
state. Delete asks once, names the recording, and routes through the
journaled detach so undo restores. Metadata panel (description, authors,
tags as chips) per recording and per project; a search field filters the
tree by name, tag, and annotation labels (`searchLabels` already exists).
**Constraints.** DESIGN.md throughout: identity accent for selection only,
action blue for destructive-confirm primary, Carbon table discipline, every
action palette-registered with shortcuts, keyboard reachable (tree
navigation per WAI-ARIA treegrid).
**Verification.** e2e: rename from all three surfaces persists after reload;
group/drag/reorder round-trips the container; delete + undo restores;
search narrows by tag and label; screenshots light+dark reviewed against
DESIGN.md's tests. Depends on T8.1 + T8.2.

### T8.5 · switcher and recording flow (feedback C, D)
**Objective.** Rich recording switcher; recording without a project
announces the project it creates.
**Files.** `apps/ui/src/lib/{EditorView,RecordingStrip}.svelte`, new
`RecordingSwitcher.svelte`, `apps/web/src/routes/+page.svelte`, e2e.
**Interfaces.** The breadcrumb `<select>` becomes a popover listing each
recording as a row — waveform thumbnail (existing `WaveThumb`), name,
duration, tiers badge — keyboard navigable, filter-as-you-type over names.
Recording started from Home creates the project first and the strip states
the destination ("Recording into <name>") with the name editable in place;
stopping lands in that project's corpus (feedback D).
**Constraints.** DESIGN.md; popover uses `--radius-xl`/`--shadow-lg`; no new
selection concepts.
**Verification.** e2e: switch via popover updates all panes; record-from-home
creates and names a project, banner asserted; screenshots both themes.

### T8.6 · navigation and view control (feedback H, I, J, K)
**Objective.** Zoom, scale, and pane control that is fast, hard to break,
and one gesture from recovery.
**Files.** `apps/ui/src/lib/{EditorView,WaveformPane,SpectrogramPane,
SelectionLayer,ReadoutBar,OverviewStrip}.svelte`, e2e.
**Interfaces.** Double-click inside a selection zooms to it; double-click
empty pane space fits the file. Box-selection play button renders through
`band_filtered_span` (T8.2) and states the band in the readout while
playing. Vertical control, RX register: waveform gain and spectrogram
frequency ceiling adjust by drag on their rulers (or Alt+wheel per the
binding table), always paired with a visible reset chip that appears the
moment either departs default — one click restores, nothing hides state
(the failure mode to avoid is Audacity's unrecoverable vertical mess).
Overview strip slims to 60 % of current height; the waveform pane gains a
toggle (palette + `W`) and an overlay mode ghosting the envelope over the
spectrogram at documented opacity. App-wide UI scale (90–150 %, Ctrl+±,
persisted) built on rem-based layout — verify layout tolerates both ends.
**Constraints.** DESIGN.md motion rules (data panes never ease); overlay
conventions frozen; every new action palette-registered.
**Verification.** e2e: dblclick zoom; filtered-play audibly banded is
asserted numerically (rendered buffer band energy), reset chip round-trip;
UI scale extremes screenshot-reviewed; both themes; suite green. Depends on
T8.2; runs after T8.4/T8.5.

## Wave C — interchange

### T8.7 · project and audio I/O (feedback F)
**Objective.** Projects and audio move in and out as files.
**Files.** `crates/phx-project/src/*` (bundle option), `crates/phx-wasm`,
`apps/web` (download/upload UI), `apps/ui` menus, e2e; `crates/phx-audio`
(decode formats) as scoped.
**Interfaces.** Export project: self-contained bundle (media embedded in the
zip) or references-only, chosen in the export dialog, default bundled on
web; import accepts both, relinks by hash, reports gaps through the
existing `MediaGap` flow. Audio export: whole recording or current
selection as WAV (16/24/32f) from the editor and the library row. Audio
import beyond WAV: AIFF via a clean-room reader in `phx-audio` (format is
publicly specified) and FLAC via symphonia where the target permits —
investigate wasm fitness first and report; Praat-parity for the common
fieldwork set (WAV, AIFF, FLAC) is the bar.
**Constraints.** Standing set; format doc updated for the bundle layout;
no compression of embedded WAVs beyond the zip's own Deflate.
**Verification.** Bundle export → fresh browser profile import round-trips
annotations and metadata; selection WAV equals `export_span_wav` bytes;
AIFF/FLAC fixtures decode bit-equal to references generated by a documented
tool; suite green.

**wasm fitness of the symphonia decode path.** `wasm-pack build crates/phx-wasm
--target web --release` with `RUSTFLAGS="-C target-feature=+simd128"`,
`phx_wasm_bg.wasm` measured before and after the AIFF/FLAC import path became
reachable from `WasmEngine::import_audio_bytes` (previously only WAV decoded,
so the linker's dead-code elimination dropped `symphonia` and its `aiff`,
`flac`, and `pcm` codec features entirely):

| build | raw | gzip -9 | brotli -q11 |
|---|---|---|---|
| WAV-only (dead-code-eliminated) | 2,602,603 B | 954,934 B | 672,673 B |
| WAV + AIFF + FLAC reachable | 3,200,698 B | 1,167,618 B | 820,232 B |
| delta | +598,095 B (+23.0%) | +212,684 B (+22.3%) | +147,559 B (+21.9%) |

`symphonia` (RIFF probe, the FLAC bundle, and the shared PCM/bit-reader core)
costs roughly 580 KiB raw / 208 KiB gzipped on top of the WAV-only binary —
a quarter more wasm shipped to every session, including the large majority
that only ever opens WAV. The decode path is reachable only from
`WasmEngine::import_audio_bytes` and the `openAudioFile` worker branch; no
other engine surface touches it, which makes it a clean split candidate:
move `phx-audio`'s AIFF/FLAC decode behind a second, lazily-loaded wasm
module (or a dynamic `import()` of a `symphonia`-only wasm-pack target),
fetched the first time a worker receives a non-WAV file, rather than
shipping it in the module every session downloads at launch. Splitting the
wasm build is unscheduled.

### T8.8 · phase gate
Screenshot review of every wave-B surface against DESIGN.md, container v2
compatibility spot-check on a v1 file from before the phase, filtered-play
listening check, gate record appended to `docs/plan/gates.md`.

## Sequencing

- Wave A starts immediately (no `apps/` contention); T8.1 → T8.4 and
  T8.2 → {T8.4, T8.6} are the hard edges; T8.3 is independent of the rest.
- Wave B runs strictly one task at a time in `apps/`, order T8.4 → T8.5 →
  T8.6, each behind the currently-running polish task.
- T8.7 follows T8.1 (bundle layout builds on v2) and can overlap wave B's
  tail once `apps/` frees.
- DaVinci-influenced density (feedback H) is realized inside T8.4/T8.6 as
  layout discipline plus the UI-scale control; a workspace/tab
  re-architecture is out of scope for this phase.
