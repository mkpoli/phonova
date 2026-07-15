# Task specs — phase 4 (project, voice report, selection tooling)

Lane key and **standing constraints block** as in `phase-1.md` — copy the
block into every delegation. Voice-measure definitions:
`../../research/algorithms-and-validation.md` §5 (cite the exact subsection
in each delegation and paste its text in; lanes cannot follow references).

**Formant caveat for this phase and later:** the Xia & Espy-Wilson excerpt in
the algorithms report carries the DP cost *structure* but no numeric weights;
`phx-formant` documents its α/β/γ constants as implementation choices. Any
task that displays, exports, or aggregates *tracked* (smoothed) formants must
treat those weights as provisional: the T2.6 gate compares tracked-vs-raw
output against the oracle first, and until it closes, UI and export default
to raw Burg candidates with tracking behind an explicit toggle.

### T4.1 · codex · phx-project
**Objective.** Project persistence: one file, autosave, crash recovery.
**Files.** `crates/phx-project/src/*`, engine integration points in
`crates/phx-engine`.
**Interfaces.**
```rust
pub struct Project { /* referenced media (relative path + BLAKE3 content hash),
  annotations per audio, parameter profiles (named PitchParams/FormantParams/…),
  view state (zoom span, palette, visible tracks) */ }
pub fn save(p: &Project) -> Vec<u8>;                    // versioned container
pub fn load(bytes: &[u8]) -> Result<Project, ProjectError>;
pub struct Autosaver { /* debounced journal snapshots; sidecar file;
  recover() detects a newer sidecar than the project file on open */ }
```
Container format: a ZIP archive (`zip` crate, permissive — verify) with
`manifest.json` (format version, schema-documented), `annotations/*.json`,
`profiles.json`, `view.json`. Media stays external by reference; the
manifest records hash + duration so a moved file is detected and re-linked
by hash search in sibling directories before asking the user.
**Constraints.** Standing set. Format documented in
`docs/formats/project.md` (new file, part of this task) precisely enough for
third-party readers — self-describing, versioned, UTF-8 (design rules 4/
pain point 2.7). Autosave must not block the engine thread; WASM path writes
the sidecar to OPFS.
**Verification.** Round-trip property test over generated projects; recovery
test: apply commands, snapshot, drop the engine without saving, reopen →
recovered state hash equals pre-drop hash (roadmap phase-4 gate); a project
saved on desktop opens in the web build (fixture-based integration test).

### T4.2 · codex · phx-voice
**Objective.** Pulse extraction and the voice-report measure family.
**Files.** `crates/phx-voice/src/*`.
**Interfaces.**
```rust
pub fn pulses(audio: AudioView, pitch: &PitchTrack, params: &PulseParams) -> PointProcess;
pub fn jitter(pp: &PointProcess, span: TimeSpan, kind: JitterKind) -> Option<f64>;
   // JitterKind: Local | LocalAbsolute | Rap | Ppq5 | Ddp     (report §5.2)
pub fn shimmer(audio: AudioView, pp: &PointProcess, span: TimeSpan, kind: ShimmerKind) -> Option<f64>;
   // ShimmerKind: Local | LocalDb | Apq3 | Apq5 | Apq11 | Dda (report §5.3)
pub fn hnr_track(audio: AudioView, params: &HarmonicityParams) -> HnrTrack;   // §5.1
pub fn cpp(audio: AudioView, at: f64, params: &CppParams) -> f64;             // §5.5, plus cpps()
pub fn spectral_moments(slice: &Slice, power: f64) -> Moments;                // §5.4
pub fn voice_report(audio: AudioView, span: TimeSpan, pitch_params: &PitchParams) -> VoiceReport;
   // aggregates the above + voice breaks (gap > 1.25/floor), with the
   // parameters used embedded in the report struct
```
Pulse placement: period-by-period peak alignment guided by the pitch track's
voiced spans (Praat manual "Voice" intro describes the observable behaviour;
where under-specified, document the chosen procedure and validate against
the oracle empirically).
**Constraints.** Standing set. HNR from the window-corrected ACF peak per
Boersma 1993 (`10·log10(r/(1−r))`), reusing phx-pitch's ACF machinery rather
than duplicating it. Every measure returns `Option`/typed absence for spans
with too few periods — no NaN surprises.
**Verification.** Synthetic ground truth first: pulse trains with injected
period perturbation (known jitter %) and amplitude perturbation (known
shimmer %) recover the injected values within 5% relative; harmonic+noise
mixes at constructed HNR within 0.5 dB. Then oracle: voice report on
sustained-vowel fixtures within validation.md bands (jitter/shimmer ≤ 10%
relative), disagreements listed with pulse-count context.

### T4.3 · grok · home screen / project manager
**Depends on T4.1 and T1.6.**
**Objective.** `ux.md` §Home: projects grid, whole-screen drop target,
one-screen creation.
**Files.** `apps/ui/src/lib/` (HomeView, ProjectCard, DropImport),
`apps/web` routing + `CoreClient` additions (create/open/list projects,
import batch).
**Requirements.** Folder drop imports every audio file inside (WAV now;
extended formats desktop-only later), attaches same-name TextGrids, and
shows per-file waveform thumbnails as they compute (progressive, worker-
side); rename/delete/duplicate in place with undo via the journal; recent
projects persisted (OPFS on web); empty state explains drop-to-start in one
sentence.
**Constraints.** Standing set; both themes; no blocking spinners — import
streams results.
**Verification.** Playwright: drop a fixture folder (5 audio + 2 TextGrids)
→ project with 5 entries, 2 with tiers attached, thumbnails rendered; delete
then undo restores; screenshots light+dark of grid and empty state.

### T4.4 · grok · spectrogram box selection + readout
**Depends on T1.6; touches formant display — see the formant caveat above.**
**Objective.** `ux.md` wow-moment 2: click-drag a time–frequency box, read
measurements, act on it.
**Files.** `apps/ui/src/lib/` (SelectionLayer, ReadoutBar), engine additions
(`band_energy(id, span, band) -> f64`, selection-scoped stats reusing
existing track queries).
**Requirements.** Drag on the spectrogram creates a t×f box (waveform drag
stays time-only); readout shows duration, F0 mean/min/max over the span
(voiced frames), band energy dB, and — only when the tracking toggle is on —
mean F1–F3 with a "provisional tracking" marker until T2.6 closes; actions:
play span, zoom to span, voice report on span, clear. Escape clears; the box
survives zoom/pan (it is anchored in signal coordinates, not pixels).
**Constraints.** Standing set. Readout numbers come from engine queries —
never recomputed frontend-side (batch = GUI invariant).
**Verification.** Playwright: drag a box on a fixture vowel, assert readout
values equal the engine's direct API answers for the same span (test hook);
box position pixel-checked after a 4× zoom; screenshots light+dark.

### T4.5 · sonnet · voice fixtures + oracle references
**Objective.** Fixture and reference coverage for T4.2.
**Files.** `tests/fixtures/audio/` additions, `tools/oracle` case additions,
MANIFEST update.
**Requirements.** Two openly licensed real sustained vowels (different
speakers/sexes; verify license), one breathy and one pressed phonation
sample if freely available; oracle reference dumps (voice report per
fixture) regenerated at test time per validation.md; synthetic
perturbation-injected WAVs generated by a committed script, never
hand-edited.
**Verification.** Oracle runs green on the new cases; MANIFEST complete.

### T4.6 · architect · phase gate review
Diff review; run recovery, voice-oracle, and folder-drop verifications;
judge tracked-formant status against the T2.6 comparison; screenshots both
themes; close against roadmap.md phase-4 gate.

## Sequencing

T4.1 and T4.2 are independent codex lanes and start immediately (T4.2 needs
only committed phase-2 crates). T4.3 waits on T4.1's engine surface; T4.4
waits only on T1.6 and can run parallel to everything. T4.5 runs any time.
Gate T2.6 (architect, outstanding) should close before T4.4 ships its
formant readout default.
