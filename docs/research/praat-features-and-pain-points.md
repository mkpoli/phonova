# Praat: feature inventory and pain-point catalog

Reference for the design of a replacement phonetics toolkit. Covers Praat as of the 6.4.x series (2024–2026). Primary sources: the Praat manual at fon.hum.uva.nl/praat/manual, the issue tracker at github.com/praat/praat.github.io, tool papers written to extend or bypass Praat (Parselmouth, praatpicture, Praaline, Praat on the Web), and community tutorials (Styler, Chodroff, Bosker).

Feature classification: **core** = taught in the manual's Intro or Voice tutorial chapters and used in everyday measurement work; **niche** = advanced-chapter or special-purpose material.

---

## 1. Feature inventory

### 1.1 Audio management

| Feature | Class | Notes |
|---|---|---|
| Read sound from file | core | WAV, AIFF, AIFC, NeXT/Sun (.au), NIST, FLAC, MP3, Ogg Vorbis, Ogg Opus readable; write support limited to WAV/AIFF/AIFC/NeXT-Sun/NIST/FLAC ([Sound files](https://www.fon.hum.uva.nl/praat/manual/Sound_files.html)) |
| Record from microphone | core | SoundRecorder window: device choice, record/stop/play, "Save to list" preview loop before committing ([Intro 1.1](https://www.fon.hum.uva.nl/praat/manual/Intro_1_1__Recording_a_sound.html)); recording of long files and multi-input setups is limited ([Styler, Phonetician's Software Toolkit](https://wstyler.ucsd.edu/posts/phoneticians_software.html)) |
| SoundEditor (View & Edit) | core | Waveform + analysis panes (spectrogram, pitch, formants, intensity, pulses), zoom/scroll, region selection, playback of selected stretch, cut/copy/paste between sound windows ([Intro 2.2](https://www.fon.hum.uva.nl/praat/manual/Intro_2_2__Viewing_and_editing_a_sound.html)) |
| LongSound | core for long files | Disk-backed streaming object (~60 s in memory at a time); exists because ordinary Sound objects must fit in RAM ([Intro 2.2](https://www.fon.hum.uva.nl/praat/manual/Intro_2_2__Viewing_and_editing_a_sound.html)) |
| Sound from formula / synthesis | niche | Tone complexes, gammatones, Shepard tones ([Sound](https://www.fon.hum.uva.nl/praat/manual/Sound.html)) |
| Combine sounds (concatenate, convolve, cross-correlate) | niche | Exposed when two or more Sound objects are selected together ([Sounds: Concatenate](https://www.fon.hum.uva.nl/praat/manual/Sounds__Concatenate.html)) |

### 1.2 Pitch analysis

| Feature | Class | Notes |
|---|---|---|
| To Pitch (raw autocorrelation) | core | Defaults: time step 0.0 (= 0.75/floor), floor 75 Hz, ceiling 600 Hz ([Sound: To Pitch](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch___.html)) |
| Filtered autocorrelation / filtered cross-correlation | niche (added 2023) | Gaussian low-pass before correlation; recommended for intonation work; raw cross-correlation recommended for voice pathology ([choosing a pitch method](https://www.fon.hum.uva.nl/praat/manual/how_to_choose_a_pitch_analysis_method.html)) |
| Advanced pitch parameters | niche | Silence threshold 0.03, voicing threshold 0.45, octave cost, octave-jump cost, voiced/unvoiced cost, max candidates, "very accurate" ([Sound: To Pitch (ac)](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__ac______.html)) |
| Pitch contour view/query in editor | core | Get pitch at cursor, mean over selection; view range configured separately from analysis range ([Intro 4.2](https://www.fon.hum.uva.nl/praat/manual/Intro_4_2__Configuring_the_pitch_contour.html), [4.3](https://www.fon.hum.uva.nl/praat/manual/Intro_4_3__Querying_the_pitch_contour.html)) |
| Pitch candidate editing (PitchEditor), Interpolate, Smooth | niche | Manual correction of tracker output ([Pitch](https://www.fon.hum.uva.nl/praat/manual/Pitch.html)) |
| Manipulation (PSOLA resynthesis of pitch/duration) | core for perception-experiment work | Intro 8 chapter; PitchTier / DurationTier editing ([Pitch](https://www.fon.hum.uva.nl/praat/manual/Pitch.html)) |

### 1.3 Formant analysis

| Feature | Class | Notes |
|---|---|---|
| To Formant (burg) | core | Defaults: ceiling 5500 Hz, 5 formants, window 0.025 s, pre-emphasis from 50 Hz; recommended ceilings 5000 Hz adult male, 5500 Hz adult female, ~8000 Hz child ([Sound: To Formant (burg)](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__burg____.html)) |
| Formant queries (value/bandwidth at time, mean, min/max, quantiles) | core | ([Formant](https://www.fon.hum.uva.nl/praat/manual/Formant.html)) |
| Draw tracks / Speckle | core | Figure output to the Picture window ([Formant](https://www.fon.hum.uva.nl/praat/manual/Formant.html)) |
| To Formant (robust), (sl), LPC family | niche | Iteratively reweighted Burg; Split Levinson; raw LPC objects ([robust](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__robust____.html), [sl](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__sl____.html)) |
| FormantPath | niche (recent) | Runs analyses across several ceilings and lets the user pick per-interval; addresses the single-ceiling problem inside Praat itself ([To FormantPath](https://www.fon.hum.uva.nl/praat/manual/Sound__To_FormantPath__burg____.html)) |

### 1.4 Intensity

| Feature | Class | Notes |
|---|---|---|
| To Intensity | core | Pitch floor sets window (3.2/floor); time step default 0.8/floor; subtract-mean option; dB SPL re 2×10⁻⁵ Pa ([Sound: To Intensity](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Intensity___.html)) |
| Contour view/query in editor | core | View range default 50–100 dB; mean via energy, sones, dB, or median ([Intro 6.2](https://www.fon.hum.uva.nl/praat/manual/Intro_6_2__Configuring_the_intensity_contour.html)) |
| IntensityTier, silence detection interactions | niche | ([Intensity](https://www.fon.hum.uva.nl/praat/manual/Intensity.html)) |

### 1.5 Voice quality

| Feature | Class | Notes |
|---|---|---|
| Voice report | core (clinical/voice work) | Aggregate report from Sound + Pitch + PointProcess; output depends on the active pitch settings, with the manual warning to set the range per speaker ([Voice report](https://www.fon.hum.uva.nl/praat/manual/Voice_report.html)) |
| Jitter: local, local absolute, rap, ppq5, ddp | core (voice work) | MDVP-equivalent pathology thresholds documented per measure ([Voice 2](https://www.fon.hum.uva.nl/praat/manual/Voice_2__Jitter.html)) |
| Shimmer: local, local dB, apq3, apq5, apq11, dda | core (voice work) | ([Voice 3](https://www.fon.hum.uva.nl/praat/manual/Voice_3__Shimmer.html)) |
| Voice breaks (fraction unvoiced, count, degree) | core (voice work) | Break = inter-pulse gap > 1.25/floor ([Voice 1](https://www.fon.hum.uva.nl/praat/manual/Voice_1__Voice_breaks.html)) |
| Harmonicity (HNR), cc and ac variants | core (voice work) | 10·log10(harmonic/noise energy); cc preferred ([Harmonicity](https://www.fon.hum.uva.nl/praat/manual/Harmonicity.html)) |
| Spectral moments (centre of gravity, SD, skewness, kurtosis) | niche | Spectrum-object queries used mainly for fricative research ([Spectrum: Get centre of gravity](https://www.fon.hum.uva.nl/praat/manual/Spectrum__Get_centre_of_gravity___.html)) |

### 1.6 Spectrograms and spectra

| Feature | Class | Notes |
|---|---|---|
| Spectrogram view | core | Defaults: 0–5000 Hz, window 0.005 s, dynamic range 50 dB; window length is the wideband/narrowband dial ([Intro 3.2](https://www.fon.hum.uva.nl/praat/manual/Intro_3_2__Configuring_the_spectrogram.html)) |
| Advanced spectrogram settings | niche | Window shapes: Gaussian (no sidelobes, ~2× compute), square, Hamming, Bartlett, Welch, Hanning; time/frequency step counts ([Advanced spectrogram settings](https://www.fon.hum.uva.nl/praat/manual/Advanced_spectrogram_settings___.html)) |
| Spectral slice, Spectrum object | core | ([Intro 3.6](https://www.fon.hum.uva.nl/praat/manual/Intro_3_6__Viewing_a_spectral_slice.html)) |
| Spectrogram computed only for the visible window | design constraint | Long files never get a whole-file spectrogram; see pain point 2.10 |

### 1.7 Annotation (TextGrid)

| Feature | Class | Notes |
|---|---|---|
| TextGrid object: interval tiers + point tiers | core | ([TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html)) |
| Sound/LongSound: To TextGrid | core | Empty grid matching the sound's time domain ([Intro 7](https://www.fon.hum.uva.nl/praat/manual/Intro_7__Annotation.html)) |
| TextGridEditor | core | Boundary insert (click circle / Enter / Cmd-F1..F9 per tier), drag to move, Shift-drag moves aligned boundaries across tiers, boundary removal merges intervals, text field with IPA special symbols, yellow selection playback, Find/Find again, extract selected sound ([TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html)) |
| Tier operations (add/remove/duplicate, merge TextGrids) | core | ([TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html)) |
| Automatic segmentation: To TextGrid (silences) | semi-core | Intensity-threshold silence marking with min-interval parameters ([silences](https://www.fon.hum.uva.nl/praat/manual/Sound__To_TextGrid__silences____.html)) |
| VAD segmentation (Silero), Whisper transcription, diarization | niche (2025–2026 additions) | Neural VAD, whisper.cpp interval transcription, pyannote diarization ([Silero](https://www.fon.hum.uva.nl/praat/manual/Sound__To_TextGrid__speech_activity__Silero____.html), [Transcribe interval](https://www.fon.hum.uva.nl/praat/manual/TextGrid___Sound__Transcribe_interval___.html)) — where upstream development is currently investing |
| TextGridNavigator (conditional multi-tier search) | niche | ([To TextGridNavigator](https://www.fon.hum.uva.nl/praat/manual/TextGrid__To_TextGridNavigator___.html)) |
| SpellingChecker on labels | niche | ([TextGridEditor](https://www.fon.hum.uva.nl/praat/manual/TextGridEditor.html)) |
| Count labels, DurationTier scaling, DTW time warping of grids | niche | ([TextGrid](https://www.fon.hum.uva.nl/praat/manual/TextGrid.html)) |

### 1.8 Plotting and drawing (Picture window)

| Feature | Class | Notes |
|---|---|---|
| Picture window canvas | core | Second of Praat's two main windows; Draw/Paint commands from any object accumulate on it ([Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html)) |
| Object-specific Draw/Paint commands | core | Sound: Draw, Spectrogram: Paint, Pitch: Draw, Formant: Speckle, TextGrid: Draw (alone or with Sound/Pitch), plus primitives (lines, rectangles, text) |
| Viewport selection (inner/outer) | core | Sub-rectangle targeting for multi-panel figures |
| Margins/World menus (axes, ticks, labels at data coordinates) | core | |
| Pen/Font menus | core | Stroke and typeface state for subsequent draws |
| Erase all; one-level Undo | core | Canvas clearing is the primary revision mechanism ([Erase all](https://www.fon.hum.uva.nl/praat/manual/Erase_all.html)) |
| Export: PDF, PNG, EPS, Windows metafile; Praat picture file | core | No SVG; see pain point 2.13 |
| PostScript settings | niche | Print-era resolution/paper controls ([PostScript settings](https://www.fon.hum.uva.nl/praat/manual/PostScript_settings___.html)) |

### 1.9 Scripting and batch

| Feature | Class | Notes |
|---|---|---|
| Praat script = recorded menu commands | core | Commands are the literal GUI button labels; menu "..." becomes ":" ([Scripting](https://www.fon.hum.uva.nl/praat/manual/Scripting.html)) |
| Variables: numeric, string ($-suffix), vectors/matrices, dictionaries | core | ([Scripting 3](https://www.fon.hum.uva.nl/praat/manual/Scripting_3__Simple_language_elements.html)) |
| Control flow: if/for/while/repeat/goto; procedures (@) | core | |
| form...endform dialog declaration | core | Field types real/positive/integer/word/sentence/text/choice/optionmenu/boolean/infile/outfile/folder/vectors ([Scripting 6.1](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_1__Arguments_to_the_script.html)) |
| selectObject/plusObject/minusObject, selected(), IDs | core | Selection state is the argument-passing mechanism ([Scripting 4](https://www.fon.hum.uva.nl/praat/manual/Scripting_4__Object_selection.html)) |
| Info window I/O, file I/O, pause dialogs | core | |
| editor...endeditor blocks | niche | Driving commands inside an open editor window ([Scripting 7](https://www.fon.hum.uva.nl/praat/manual/Scripting_7__Scripting_the_editors.html)) |
| Command line: praat --run, --send, --no-pref-files, --utf8 | core for pipelines | Headless batch execution; callable from other languages via subprocess ([Scripting 6.9](https://www.fon.hum.uva.nl/praat/manual/Scripting_6_9__Calling_from_the_command_line.html)) |
| sendpraat | niche | Message an already-running instance ([Scripting 8](https://www.fon.hum.uva.nl/praat/manual/Scripting_8__Controlling_Praat_from_another_program.html)) |
| Plugins (plugin_* folders in prefs dir), menu customization | niche but ecosystem-defining | Auto-loaded at startup; basis of Praat Vocal Toolkit, EasyAlign, etc. ([preferences folder](https://www.fon.hum.uva.nl/praat/manual/preferences_folder.html), [Add menu command](https://www.fon.hum.uva.nl/praat/manual/Add_menu_command___.html)) |
| Script → standalone C++ program | niche | Requires compiling against Praat's GPL source ([Scripting 9](https://www.fon.hum.uva.nl/praat/manual/Scripting_9__Turning_a_script_into_a_stand-alone_progra.html)) |

### 1.10 Objects-window workflow

| Feature | Class | Notes |
|---|---|---|
| Object list + dynamic menu | core | The dynamic (right-pane) menu changes with the selected object type(s); this select-then-act pattern is the program's central paradigm ([Objects window](https://www.fon.hum.uva.nl/praat/manual/Objects_window.html)) |
| Multi-object combination commands | core | TextGrid+Sound → View & Edit; Sound+Sound → Combine menu |
| Query submenu (Get.../Count... → Info window) | core | Same commands capture into variables under scripting ([Query submenu](https://www.fon.hum.uva.nl/praat/manual/Query_submenu.html)) |
| Modify menu (Formula..., in-place edits) | core | |
| Rename/Copy/Remove/Info/Inspect buttons | core | |
| Unique numeric object IDs | core | Basis of script addressing |

### 1.11 File formats

| Format | Class | Notes |
|---|---|---|
| TextGrid long text format | core | De-facto interchange format for the whole annotation ecosystem (ELAN, forced aligners) ([TextGrid file formats](https://www.fon.hum.uva.nl/praat/manual/TextGrid_file_formats.html)) |
| TextGrid short text format | core | Same data, labels stripped; external tools must parse both |
| Praat binary object format | niche | Manual gives no format description for the TextGrid case ("we can add it here on request") |
| Generic ooTextFile serialization | core | Every object type saves as text/short text; class auto-detected on read ([Save as text file](https://www.fon.hum.uva.nl/praat/manual/Save_as_text_file___.html)) |
| Collection files | core | Multiple selected objects saved to one file, restored together — the closest thing to a session save |
| Praat picture file | niche | Round-trippable Picture-window state |
| Text encoding: reads UTF-8/UTF-16/Latin-1/MacRoman, writes UTF-8 or UTF-16 | core, problematic | See pain point 2.6 ([Unicode](https://www.fon.hum.uva.nl/praat/manual/Unicode.html)) |

---

## 2. Pain-point catalog

Each entry: the problem, its impact on research work, and the redesign direction.

### 2.1 Select-then-act object model

**Problem.** Every command's availability depends on the current selection in the Objects window; commands take no explicit object arguments. Scripts inherit this: `selectObject`/`plusObject` boilerplate precedes almost every call, and a mismatch yields errors such as `Command "To TextGrid (vuv)..." not available for current selection`. A Praat contributor described the mechanic as "useful in the GUI but a source of endless headaches for inexperienced script writers" and proposed passing objects as arguments ([issue #229](https://github.com/praat/praat.github.io/issues/229)). Recurring confusion threads: [Linguistics SE 12733](https://linguistics.stackexchange.com/questions/12733), [15155](https://linguistics.stackexchange.com/questions/15155), [SO 9567546](https://stackoverflow.com/q/9567546), [SO 39534742](https://stackoverflow.com/q/39534742). Gibbon characterizes the object+methods interface as "conceptually clear, but, for most new users, unfamiliar" ([arXiv:1903.08718](https://arxiv.org/abs/1903.08718)). Editor-window commands live in a separate namespace from Objects-window commands, so the same conceptual action needs different code depending on "focus" ([issue #790](https://github.com/praat/praat.github.io/issues/790)).

**Impact.** Hidden global state makes scripts fragile and order-dependent; beginners cannot discover which selection unlocks which command; batch scripts silently operate on the wrong object when selection drifts.

**Redesign.** Functions take explicit object references; the GUI derives contextual menus from the type system without making selection the calling convention. One namespace for an operation regardless of which view invokes it.

### 2.2 Modal form dialogs

**Problem.** Most commands open a blocking form dialog. Long forms overflow the screen with no scrollbar ([issue #1976](https://github.com/praat/praat.github.io/issues/1976)); clicking OK/Cancel only hides a dialog, and the surviving hidden window shows up as a ghost in the Windows taskbar ([#2664](https://github.com/praat/praat.github.io/issues/2664)); a script opening two forms can segfault ([#193](https://github.com/praat/praat.github.io/issues/193)); combining an editor with a pause dialog breaks ([#3108](https://github.com/praat/praat.github.io/issues/3108)); the maintainer's own patches are titled "quick fix for the editor-and-pause problem" ([PR #3130](https://github.com/praat/praat.github.io/pull/3130)).

**Impact.** No live preview while adjusting analysis parameters; parameter exploration is a modal round-trip per attempt; dialogs block the rest of the application.

**Redesign.** Non-modal parameter panels with live re-analysis of the visible region; parameters bound to the view so changing a ceiling immediately redraws the track.

### 2.3 Picture-window painter canvas

**Problem.** The Picture window is an accumulate-only canvas: draw commands append strokes; changing anything means erasing and re-issuing every command. The praatpicture paper names as its advantages over Praat "the ability to resize figures dynamically (i.e. without regenerating figures with new size parameters)" and system-font access ([Puggaard-Rode 2024, ISSP](https://rasmuspuggaard.wordpress.com/wp-content/uploads/2024/02/2024_issp.pdf)). Styler's guide recommends OS screenshots as "the fastest (and usually sufficient) means of getting images from Praat" ([Styler guide](https://phonetique.uqam.ca/wp-content/uploads/sites/27/2017/05/Styler_2013_2.pdf)). PNG export silently writes a 0-byte file above 240 inches of canvas width ([#189](https://github.com/praat/praat.github.io/issues/189)); a margin-free export needs scripting workarounds ([#1584](https://github.com/praat/praat.github.io/issues/1584)).

**Impact.** Publication figures require either full scripting of the drawing sequence or repeated manual redraws; an entire R package exists to reproduce "Praat-style" figures outside Praat.

**Redesign.** Retained-mode plot objects: each panel is an editable object (data binding, styles, layout) rendered on demand; export to SVG/PDF/PNG at any size at any time; constraint-based panel layout with no manual viewport arithmetic.

### 2.4 Scripting language design

**Problem.** Documented oddities: commands are GUI button labels, giving names like `To PointProcess (periodic, cc)...` ("very strictly linked to the labels of the buttons in the GUI... command names that can get very unwieldy" — [issue #229](https://github.com/praat/praat.github.io/issues/229)); three historical syntax generations still parse, so tutorials from different eras conflict ([Chodroff tutorial](https://eleanorchodroff.com/tutorial/praat-scripting.html)); `'var'` single-quote interpolation vs double-quoted literals ([Bosker how-to](https://hrbosker.github.io/resources/how-to/script-in-praat/)); `$`-suffixed string typing; `=` for both assignment and comparison, `<>` for inequality; comments only at line start; the Objects window mangles filenames (dots/spaces → underscores), breaking select-by-name ([Chodroff](https://eleanorchodroff.com/tutorial/praat-scripting.html)); logic errors run silently and debugging means sprinkling `writeInfo`/`pauseScript` ([Gouskova](https://www.gouskova.com/2021/11/10/using-praat-scripts/)); the built-in ScriptEditor has no highlighting, completion, or linting ([#591 "Terrible user interface"](https://github.com/praat/praat.github.io/issues/591)); recursive `runScript` has produced heap corruption ([#58](https://github.com/praat/praat.github.io/issues/58)). Command naming across object types is inconsistent (`Get start time` / `Get end point` / `Get starting point...`), conceded by the maintainer but left in place for backward compatibility across 8000+ commands ([#229](https://github.com/praat/praat.github.io/issues/229)). Hacker News commenters who used it in graduate work: "Ugh, the scripting", "that horrible scripting language" ([HN 40060838](https://news.ycombinator.com/item?id=40060838)).

**Impact.** Every analysis pipeline of any size migrates to Python/R wrappers; scripts are write-only for collaborators; student onboarding cost is high.

**Redesign.** No bespoke language. Expose a typed Rust core through first-class Python (and optionally Lua/JS) bindings with a stable API whose function names follow programming conventions; keep a macro-recorder that emits code in the host language.

### 2.5 No API layer

**Problem.** Praat's functionality was reachable only through the GUI, the script interpreter, or `sendpraat`; no C library or typed API exists ([issue #229](https://github.com/praat/praat.github.io/issues/229), open since 2016). Parselmouth had to bind Praat's C++ internals directly via pybind11 because reimplementation "does not guarantee the same results as Praat" and shelling out has "compromised performance," while still forcing users to learn Praat command strings through `praat.call()` ([Jadoul, Thompson & de Boer 2018, *Journal of Phonetics* 71](https://doi.org/10.1016/j.wocn.2018.07.001)). PraatR and praat-py exist for the same reason ([PraatR](http://www.aaronalbin.com/praatr/index.html), [praat-py](https://github.com/JoshData/praat-py)).

**Impact.** The ecosystem around Praat consists of wrappers compensating for a missing API; numerical reproducibility across tools depends on unofficial bindings tracking upstream.

**Redesign.** Library-first architecture: the DSP core is a documented, versioned crate/C-ABI; GUI and bindings are equal consumers. Algorithms specified precisely enough that results are reproducible from the docs.

### 2.6 Text encoding of saved files

**Problem.** Praat does not record a file's origin encoding; any non-ASCII label makes it write UTF-16 by default. Maintainer: "There is no way that Praat remembers whether a TextGrid... comes from a binary file, a UTF-8 text file, or a UTF-16 text file" ([#2662](https://github.com/praat/praat.github.io/issues/2662)). Third-party parsers break on the resulting mix — "Praat's annoying habit of using UTF-8 or UTF-16 with no apparent pattern" ([Legisign/Praat-textgrids#2](https://github.com/Legisign/Praat-textgrids/issues/2), also [#11](https://github.com/Legisign/Praat-textgrids/issues/11), [kylebgorman/textgrid#2](https://github.com/kylebgorman/textgrid/issues/2)). Pasted labels can carry tabs/CR/LF into files unfiltered ([#857](https://github.com/praat/praat.github.io/issues/857)).

**Impact.** Corpus pipelines encounter files in four encodings; IPA-labeled TextGrids silently flip encoding on resave; downstream statistical tools choke.

**Redesign.** UTF-8 everywhere, always, with BOM-tolerant reading of legacy files; label input sanitized (control characters rejected or escaped); encoding never depends on content.

### 2.7 TextGrid format multiplicity

**Problem.** Three on-disk representations (long text, short text, undocumented binary) carry identical data ([TextGrid file formats](https://www.fon.hum.uva.nl/praat/manual/TextGrid_file_formats.html)). Praat ignores the long format's descriptive labels when reading, so they are decoration; external tools must implement two text parsers plus encoding detection, and several libraries exist mainly to hide this (praatio, textgrid, Praat-textgrids, readtextgrid).

**Impact.** Every interoperating tool pays a parser tax; the binary format is a black box.

**Redesign.** One canonical, versioned, documented format (self-describing, UTF-8), plus import filters for legacy long/short/binary TextGrid. Publish a formal grammar.

### 2.8 Analysis defaults that produce silently wrong measurements

**Problem.** Formant ceiling ships as a single static 5500 Hz (tuned to an average adult female); the manual instructs 5000 Hz for adult males and ~8000 Hz for children, with a stated failure mode of "analysing an [u] as having a single formant near 500 Hz whereas you want two formants at 300 and 600 Hz" ([Sound: To Formant (burg)](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__burg____.html), [FAQ: Formant analysis](https://www.fon.hum.uva.nl/praat/manual/FAQ__Formant_analysis.html)). The default itself drifted across versions (5000 → 5500 Hz), so older tutorials disagree with current builds ([Styler guide](https://phonetique.uqam.ca/wp-content/uploads/sites/27/2017/05/Styler_2013_2.pdf)). Nothing in the UI flags a speaker mismatch. Pitch defaults (75–600 Hz) mis-serve creaky voice (falls below the 75 Hz floor and drops out; the FAQ's suggested 30 Hz floor over-smooths elsewhere), children, and song; Styler: "You will often see it jump up and down, doubling and halving the actual F0" ([Styler guide](https://phonetique.uqam.ca/wp-content/uploads/sites/27/2017/05/Styler_2013_2.pdf), [FAQ: Pitch analysis](https://www.fon.hum.uva.nl/praat/manual/FAQ__Pitch_analysis.html)). Fast Track exists specifically to sweep ceilings per token and pick the best-fitting track ([Barreda 2021, *Linguistics Vanguard*](https://doi.org/10.1515/lingvan-2020-0051)); Praat's own FormantPath is the in-house response.

**Impact.** Published measurements are wrong when users trust defaults; the error is silent — plausible-looking tracks with halved F0 or merged formants.

**Redesign.** Per-speaker analysis profiles as a first-class concept; ceiling/floor sweeps with goodness-of-fit selection (Fast-Track-style) as the default path; visible warnings when tracked values crowd the ceiling/floor; defaults declared next to every displayed measurement.

### 2.9 Viewport-dependent measurement values

**Problem.** Measured pitch statistics over an unchanged selection shift (reportedly up to ~20 Hz) when the user zooms, because analysis frames are placed relative to the visible window; maintainer response: "Zooming and scrolling lead to pitch values being measured at different time points, that's all" ([#1156](https://github.com/praat/praat.github.io/issues/1156)). A frame-count truncation bug drops one analysis frame when the window divides the duration exactly ([#2011](https://github.com/praat/praat.github.io/issues/2011)).

**Impact.** Reproducibility depends on GUI state; two researchers measuring the same selection get different numbers.

**Redesign.** Analysis frame placement anchored to the signal (absolute time grid), never to the viewport; editor queries and batch queries guaranteed identical.

### 2.10 Navigation and long-recording handling

**Problem.** Long-file work is zoom-and-scroll only; spectrograms render only for the visible chunk; LongSound exists because whole files cannot be held. Platform gaps in scroll/zoom gestures persisted for years (Shift+wheel absent on Windows [#2985](https://github.com/praat/praat.github.io/issues/2985); Linux wheel scrolling arrived via an outside patch, "It was annoying me that I couldn't scroll through my TextGrids on Linux" [PR #3186](https://github.com/praat/praat.github.io/pull/3186); Mac gestures [#3218](https://github.com/praat/praat.github.io/issues/3218)). No way to hide/collapse tiers in crowded TextGrids ([#2601](https://github.com/praat/praat.github.io/issues/2601)); no split view of non-contiguous regions.

**Impact.** Corpus annotators spend their day scrolling; multi-tier grids become unreadable; long fieldwork recordings hit rendering and export ceilings ([#189](https://github.com/praat/praat.github.io/issues/189)).

**Redesign.** Tiled/streamed rendering of arbitrarily long files with overview + detail views, bookmarks, tier show/hide and grouping, standard scroll/zoom gestures on all platforms.

### 2.11 Undo

**Problem.** No unified undo. A generic ten-level editor undo arrived only via [PR #2493](https://github.com/praat/praat.github.io/pull/2493); individual editors gained, lost, and regained undo in separate patches ("corrected Undo in ManipulationEditor" [#2105](https://github.com/praat/praat.github.io/pull/2105), "enabled Undo in FormantPathEditor" [#2101](https://github.com/praat/praat.github.io/pull/2101), "reactivate Undo in decibel tiers" [#2103](https://github.com/praat/praat.github.io/pull/2103)). Objects-window operations (remove, rename, Modify-menu formulas) have no undo at all.

**Impact.** A stray Formula... or Remove destroys in-memory work; annotators fear boundary edits.

**Redesign.** Application-wide undo/redo over an immutable or journaled document model, covering object lifecycle and annotation edits alike, with unlimited depth.

### 2.12 No project or session concept

**Problem.** Objects live in RAM and vanish on quit; the manual's Save menu handles one object at a time, and the Collection file (save multiple selected objects together) is the only bundling mechanism — it stores no editor layout, zoom, cursor, or window state. No autosave or crash recovery is documented; community scripts exist solely to dump all Sounds+TextGrids to a directory ([example](https://github.com/michellecohn/praat-scripts/blob/master/!save.all.wav.files.and.textgrids.to.a.directory)).

**Impact.** Every session is reconstructed by hand; a crash loses unsaved annotation; multi-file studies have no home.

**Redesign.** A project file as the root abstraction: referenced media, derived analyses (cached, recomputable), annotations, per-speaker parameter profiles, window layout; autosave and crash recovery by default.

### 2.13 Export

**Problem.** Figures: PDF/PNG/EPS/WMF only, no SVG; publisher-workflow caveats acknowledged in the manual itself ([Picture window](https://www.fon.hum.uva.nl/praat/manual/Picture_window.html)); silent 0-byte PNGs past a width ceiling ([#189](https://github.com/praat/praat.github.io/issues/189)). Data: tab-separated output only, no CSV writer with delimiter/locale control (community scripts post-process, e.g. [this gist](https://gist.github.com/fauxneticien/33f8895892ffc0f4f44c72361856aabf)); non-ASCII tables hit the UTF-16 default problem. Audio: no compressed export ([mp3 request #2533](https://github.com/praat/praat.github.io/issues/2533)); no m4a import ([#2561](https://github.com/praat/praat.github.io/issues/2561)); WAV save without a forced extension produced files other software would not open ([#1290](https://github.com/praat/praat.github.io/issues/1290)).

**Impact.** Every result leaves Praat through a conversion step owned by the user.

**Redesign.** SVG/PDF/PNG figure export at arbitrary size; CSV/TSV/Parquet data export with explicit encoding and delimiter; audio import/export through a mature codec library (Opus, FLAC, MP3, AAC both directions where licensing allows); file extensions always enforced.

### 2.14 Distribution, updates, packaging

**Problem.** Manual per-OS/per-architecture binary downloads (six current Linux variants) with no first-party package-manager channel ([download page](https://www.fon.hum.uva.nl/praat/download_linux.html)); no update mechanism by design ("the Praat program will never... access the Internet by itself" — [Checking for updates](https://www.fon.hum.uva.nl/praat/manual/Checking_for_updates.html)) against a once-or-twice-monthly release cadence; packaging left to volunteers (Flatpak sandboxing conflicts with scripts' free filesystem access [#2992](https://github.com/praat/praat.github.io/issues/2992); missing .desktop file [#1438](https://github.com/praat/praat.github.io/issues/1438); build "spews compiler warnings by the thousand" per the Arch maintainer [#1238](https://github.com/praat/praat.github.io/issues/1238)); GLIBC mismatches block older distros ([#1342](https://github.com/praat/praat.github.io/issues/1342)). The custom GUI layer (Motif heritage, GTK3 on Linux) draws "relic from the past" reactions ([HN 40060838](https://news.ycombinator.com/item?id=40060838)); a 2025 issue proposes a web port so students "don't need to install an application" ([#3261](https://github.com/praat/praat.github.io/issues/3261)).

**Impact.** Lab-wide version skew; classroom setup friction; distro packages lag or break.

**Redesign.** Reproducible CI builds published to cargo/homebrew/winget/Flatpak from day one; opt-in update checks; a sandbox-compatible file-access model (portals/pickers); clean warning-free builds enforced in CI.

### 2.15 No corpus structure or cross-tier integrity

**Problem.** Praat scripts are monolithic — "all automatic routines a user is interested in... must be programmed together in a single script"; labels are "opaque blocks of strings, and there is no function for a linguistic analysis of the labels" ([Domínguez et al., COLING 2016](https://aclanthology.org/C16-2046/)). Duplicating information across supposedly aligned tiers "quickly leads to problems when corpora get larger: discrepancies in tier boundaries that should had been aligned... data incoherence between tiers that are supposedly linked" ([Christodoulides, Praaline](https://arxiv.org/abs/1802.02914)).

**Impact.** Large annotation projects drift into inconsistency with no tooling to detect it; multi-file studies rely on external databases bolted on afterward.

**Redesign.** Typed annotation schema with declared tier relationships (alignment, hierarchy) validated continuously; corpus-level queries across files; composable analysis pipeline steps.

### 2.16 Learning curve

**Problem.** Pedagogy literature reports Praat as "too complicated for self-training — learners would not be able to operate the program unless they receive close assistance from an instructor" ([Osatananda & Thinchan 2021, LEARN Journal 14(2)](https://so04.tci-thaijo.org/index.php/LEARN/article/view/253272)); separate front-ends have been built solely to simplify it (Brett 2004 Flash interface; Gibbon's CRAFT, motivated by tools with "a steep learning curve" [arXiv:1903.08718](https://arxiv.org/abs/1903.08718)). Praat's own authors wrote that the program "is not unlike the magic broom that takes off with the sorcerer's apprentice... do not try this at home, and always consult your local phonetician" (Boersma & van Heuven 2001, *Glot International* 5(9/10)).

**Impact.** Teaching time is spent on the tool; students produce measurements they cannot vet (compounding 2.8).

**Redesign.** Progressive disclosure: a measurement-first default UI with the full parameter surface reachable underneath; parameter explanations and sanity indicators at the point of use.

---

## 3. Source index

- Praat manual: https://www.fon.hum.uva.nl/praat/manual/ (Intro, Voice, Scripting chapters; per-command pages cited inline)
- Issue tracker: https://github.com/praat/praat.github.io/issues (source repository: https://github.com/praat/praat)
- Jadoul, Y., Thompson, B. & de Boer, B. (2018). Introducing Parselmouth: A Python interface to Praat. *Journal of Phonetics* 71, 1–15. https://doi.org/10.1016/j.wocn.2018.07.001
- Barreda, S. (2021). Fast Track: fast (nearly) automatic formant-tracking using Praat. *Linguistics Vanguard* 7(1). https://doi.org/10.1515/lingvan-2020-0051
- Puggaard-Rode, R. (2024). praatpicture: a library for making flexible Praat Picture-style figures in R. *Proc. ISSP 2024*. https://rasmuspuggaard.wordpress.com/wp-content/uploads/2024/02/2024_issp.pdf
- Domínguez, M. et al. (2016). Praat on the Web: an upgrade of Praat for semi-automatic speech annotation. *Proc. COLING 2016 System Demonstrations*, 218–222. https://aclanthology.org/C16-2046/
- Christodoulides, G. (2014). Praaline: integrating tools for speech corpus research. *Proc. LREC 2014*. https://arxiv.org/abs/1802.02914
- Gibbon, D. (2019). CRAFT: a multifunction online platform for speech prosody visualisation. https://arxiv.org/abs/1903.08718
- Osatananda, V. & Thinchan, W. (2021). Using Praat for English pronunciation self-practice outside the classroom. *LEARN Journal* 14(2), 372–396.
- Styler, W. Using Praat for Linguistic Research. https://phonetique.uqam.ca/wp-content/uploads/sites/27/2017/05/Styler_2013_2.pdf ; A Phonetician's Software Toolkit. https://wstyler.ucsd.edu/posts/phoneticians_software.html
- Chodroff, E. Corpus Phonetics Tutorial: Praat Scripting. https://eleanorchodroff.com/tutorial/praat-scripting.html
- Bosker, H. R. How to script in Praat. https://hrbosker.github.io/resources/how-to/script-in-praat/
- Gouskova, M. (2021). Using Praat Scripts. https://www.gouskova.com/2021/11/10/using-praat-scripts/
- Hacker News discussion of praat.org (2024). https://news.ycombinator.com/item?id=40060838
