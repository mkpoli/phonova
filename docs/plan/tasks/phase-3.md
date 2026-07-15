# Task specs — phase 3 (annotation)

Lane key and **standing constraints block** as in `phase-1.md` — copy that
block into every delegation. Algorithm/format sources for this phase:
`../../research/praat-features-and-pain-points.md` §1.7/§1.11 (TextGrid
behaviour and format landscape) and the Praat manual pages cited inline.

Committed context lanes can rely on: `phx_engine::Engine` (audio store,
`waveform_slice`, `spectrogram_tile_rgba` — no journal yet),
`phx_annot`/`phx_textgrid` are empty skeletons; `apps/ui` and `apps/web` are
being built in T1.6 (codex) — T3.4 depends on its merge.

### T3.1 · codex · phx-annot
**Objective.** Annotation document model with typed tier relations and
integrity validation.
**Files.** `crates/phx-annot/src/*`.
**Interfaces.**
```rust
pub struct Annotation { /* tiers ordered; time domain [xmin, xmax] in seconds */ }
pub enum Tier { Interval(IntervalTier), Point(PointTier) }
pub struct IntervalTier { /* name; sorted, gap-free, non-overlapping intervals
                             covering the time domain; empty label = unlabeled */ }
pub struct PointTier { /* name; sorted, strictly increasing point times */ }
pub enum TierRelation {
  Independent,
  AlignedBoundaries { with: TierId },        // shared boundaries move together
  ChildOf { parent: TierId },                // every interval nested in a parent span
}
pub struct AnnotationError { /* typed: overlap, gap, orphan child, unsorted, … */ }
impl Annotation {
  pub fn validate(&self) -> Vec<IntegrityIssue>;   // never panics; reports all issues
  pub fn insert_boundary(&mut self, tier: TierId, at: f64) -> Result<BoundaryId, AnnotationError>;
  pub fn move_boundary(&mut self, id: BoundaryId, to: f64, mode: AlignMode) -> Result<Moved, AnnotationError>;
  pub fn remove_boundary(&mut self, id: BoundaryId) -> Result<Merged, AnnotationError>;
  pub fn set_label(&mut self, target: LabelTarget, text: &str) -> Result<(), AnnotationError>;
  pub fn search(&self, query: &LabelQuery) -> Vec<Hit>;   // substring | regex, per-tier filter
}
```
Every mutator returns enough information to invert it (the engine journal in
T3.3 stores these as inverse operations).
**Constraints.** Standing set. Stable ids for tiers/boundaries/intervals
(indices shift under edits; ids must not). Label text is sanitized on entry:
control characters (tab/CR/LF from pastes) rejected with a typed error, never
stored (pain point 2.6). `AlignedBoundaries` moves are atomic across tiers.
No dependency on any other phx crate.
**Verification.** Property tests: any sequence of successful mutations keeps
`validate()` empty; every mutator's returned inverse restores a
structurally equal document. Unit tests per `IntegrityIssue` variant. Search
tested against a document with IPA and combining-character labels.

### T3.2 · codex · phx-textgrid
**Objective.** Read every TextGrid Praat writes; write exactly one format.
**Files.** `crates/phx-textgrid/src/*`, new fixtures under
`tests/fixtures/textgrids/`.
**Interfaces.**
```rust
pub fn read(bytes: &[u8]) -> Result<(Annotation, SourceInfo), TextGridError>;
   // SourceInfo: detected variant (Long/Short) + encoding — reported, never round-tripped implicitly
pub fn write(a: &Annotation) -> Vec<u8>;   // long text format, UTF-8, LF, no BOM — always
```
Encoding detection: UTF-8 (accept BOM), UTF-16 LE/BE via BOM, Latin-1
fallback for legacy files; format detection long vs short from structure, no
filename heuristics. Number formatting on write matches Praat's decimal
serialization closely enough that Praat re-opens files without complaint
(verify empirically via the oracle's Praat, not by reading source).
**Constraints.** Standing set. Format knowledge comes from the Praat manual
page "TextGrid file formats", the fixture files, and files generated through
`tools/oracle` (black-box). The binary TextGrid variant is undocumented:
derive it only from oracle-generated sample files and permissively licensed
third-party parsers (verify each one's license first); if that is
insufficient for a full reader, ship long/short support and file the binary
reader as a follow-up with findings — do not guess and do not read GPL
parsers. `TierRelation` metadata does not exist in TextGrid; import as
`Independent`, and document that relations are a project-level concept.
**Verification.** Round-trip: every fixture reads → writes → re-reads to a
structurally equal `Annotation`; UTF-16 fixture re-emerges as UTF-8; a
written file with IPA labels is byte-stable across a second round-trip.
Oracle check: `tools/oracle` opens each written file in Praat and re-saves
it; the re-read matches. Fuzz the reader (arbitrary bytes → error, never
panic).

### T3.3 · codex · engine journal + annotation commands
**Objective.** The unified undo journal (design rule 5) and the annotation
surface of the engine.
**Files.** `crates/phx-engine/src/{journal,document,commands}.rs`, additions
to `lib.rs`; `crates/phx-wasm` plumbing.
**Interfaces.**
```rust
pub enum Command {  // serializable (serde), self-describing
  ImportAudio { bytes: Vec<u8>, name: String },
  AttachAnnotation { audio: AudioId, annotation: Annotation },
  AddTier {..}, RemoveTier {..}, InsertBoundary {..}, MoveBoundary {..},
  RemoveBoundary {..}, SetLabel {..},
}
impl Engine {
  pub fn apply(&mut self, cmd: Command) -> Result<Applied, EngineError>;
  pub fn undo(&mut self) -> Result<Option<Applied>, EngineError>;
  pub fn redo(&mut self) -> Result<Option<Applied>, EngineError>;
  pub fn state_hash(&self) -> u64;          // document-model hash, for tests
  pub fn search_labels(&self, q: &LabelQuery) -> Vec<Hit>;   // across all annotations
}
```
`Applied` carries what changed (ids, spans) so the UI patches incrementally.
Journal depth unlimited; analyses stay outside it (pure cache). Existing
read-only methods (`waveform_slice`, tiles) are untouched.
**Constraints.** Standing set. Undo restores state-hash-identical documents
(validation.md invariant 5). Redo stack clears on a new command. The journal
is the only mutation path — no public method mutates the document directly.
**Verification.** Property test: random 50-command sequences, undo-all →
hash equals initial, redo-all → hash equals final (roadmap phase-3 gate).
wasm-pack test covering apply/undo through the bindings.

### T3.4 · grok · tier UI + keyboard annotation loop
**Depends on T1.6 (apps/ui, apps/web merged).**
**Objective.** The annotation panes and interaction loop of `ux.md` §Editor.
**Files.** `apps/ui/src/lib/` (TierPane, TierLane, BoundaryHandle,
LabelEditor, SearchBar components), `apps/web` wiring through `CoreClient`
(extend the interface with the T3.3 command/undo/search calls).
**Requirements.** Tiers render under the spectrogram, synchronized to the
timeline transform; collapsible and reorderable; boundary drag with
`AlignedBoundaries` visual feedback; the full keyboard map from ux.md
(`Tab`/`Shift-Tab`, `Enter`, `S`, `M`, arrow nudges with `Alt` = one frame,
digit tier focus); label editing inline with IPA input untouched by any
autocorrect/composition interference; label search with hit navigation;
every action calls `Engine::apply` — no local mutation, undo/redo bound to
Ctrl/Cmd-Z/-Shift-Z globally.
**Constraints.** Standing set; Svelte 5 runes; both themes; TextGrid
import/export reachable from the palette-less interim menu until phase 7.
**Verification.** Playwright: keyboard-only annotation of a fixture sentence
(insert boundaries, label three intervals, split, merge, undo×5, redo×5),
asserting engine state via a test hook after each step; screenshots
light+dark of a 4-tier document; drag a shared boundary and assert both
tiers moved.

### T3.5 · sonnet · TextGrid fixture expansion
**Objective.** Broaden `tests/fixtures/textgrids/` to cover the wild.
**Files.** New fixtures + MANIFEST.md updates.
**Requirements.** Add: point-tier file, mixed interval+point multi-tier
file, IPA/diacritic-heavy labels, an empty-tier file, a Latin-1 legacy file,
and binary-format samples generated via `tools/oracle` from existing
fixtures (script committed alongside). Real-world files only where the
license is verifiable; author the rest by hand or through the oracle.
**Verification.** MANIFEST lists provenance per file; every fixture loads in
Praat via the oracle without error.

### T3.6 · architect · phase gate review
Diff review across T3.1–T3.4; run the round-trip corpus, the 50-op
undo property, and the keyboard-only Playwright script; screenshot review
both themes; close against roadmap.md phase-3 gate.

## Sequencing

T3.1 → {T3.2, T3.3} → T3.4; T3.5 parallel with everything after T3.2's
reader exists in draft (its oracle scripts help generate fixtures). T3.2 and
T3.3 touch disjoint crates and run in parallel lanes.
