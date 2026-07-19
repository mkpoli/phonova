# Praat capability analysis: analysis and signal processing

Reference for planning Phonia's analysis/DSP roadmap against Praat's actual
surface area. Scope is Praat's *analysis* and *signal-processing* half:
Pitch, Formant, Intensity, Harmonicity, Spectrogram, Spectrum, Ltas, Cepstrum
family, Excitation/Cochleagram, LPC/MFCC/LFCC, PointProcess and periodicity
queries, the Query submenu pattern, the Voice report and its constituent
measures, and modification/resynthesis (Manipulation/PSOLA, tiers, filtering,
resampling, convolution, Sound arithmetic). Audio file I/O, TextGrid
annotation, the Picture window, and scripting are covered by
[praat-features-and-pain-points.md](praat-features-and-pain-points.md) and are
only mentioned here where they intersect signal processing (resampling,
filtering).

## Method and license boundary

Phonia is MIT and clean-room; Praat is GPL. The primary source throughout is
Praat's public manual (<https://www.fon.hum.uva.nl/praat/manual/>), fetched
page by page — each command and object type has its own reference page there.
Partway through this research the task owner relaxed the original manual-only
restriction to allow reading Praat's source (github.com/praat/praat) for
behavioral detail the manual leaves ambiguous. In practice every finding below
still comes from the manual: the three research passes that covered Praat's
documentation independently chose to stay manual-only (one explicitly declined
a mid-task instruction to consult source, since it did not verifiably
originate from the task owner), and manual coverage turned out to be complete
enough that no source consultation was needed. Where the manual itself is
silent on a detail — a missing default, an unconfirmed command name, an
object with no documented creation path — that is recorded explicitly as
"manual does not specify" or "not found in the current manual," rather than
filled in by inference. Nothing below was copied or paraphrased from Praat's
source or its exact wording; descriptions are written independently from the
documented behavior.

The Phonia side comes from reading every source file in the crates under
`crates/phx-audio`, `crates/phx-dsp`, `crates/phx-spectrogram`,
`crates/phx-pitch`, `crates/phx-formant`, `crates/phx-intensity`,
`crates/phx-voice`, `crates/phx-render`, `crates/phx-figure`, and the
query-orchestration layer in `crates/phx-engine` — not inferred from crate
names or doc comments alone.

**Verdict vocabulary** used in every table:

- **core-parity** — a phonetician would consider Phonia broken without this;
  either a baseline method/query every comparable tool has, or something
  Praat's own documentation names as the recommended/default approach.
- **valuable** — commonly used, meaningfully improves coverage, but Phonia is
  usable without it.
- **niche** — advanced-chapter or special-purpose material; Praat's own
  manual often hedges these as "not recommended for general use."
- **out-of-scope** — deliberately not planned, with the reason stated inline
  (usually: superseded by Phonia's typed-API design philosophy in place of
  Praat's bespoke scripting/formula language, or the capability doesn't
  actually exist in current Praat either).

---

## 1. Pitch (F0) analysis

Phonia (`crates/phx-pitch`) implements exactly one of Praat's five pitch
methods — window-corrected raw autocorrelation, following Boersma (1993) —
with a genuine Viterbi dynamic-programming path finder
(`crates/phx-pitch/src/path.rs`) and Praat-matching default parameters
(floor 75 Hz, ceiling 600 Hz, 15 candidates, octave cost 0.01, octave-jump
cost 0.35, voiced/unvoiced cost 0.14). This is the right baseline to have
first, but it is one of five documented methods, and the missing ones are not
exotic: Praat's own manual now recommends the *filtered* autocorrelation
method over raw for everyday intonation work, and recommends *cross*-correlation
specifically for voice-pathology analysis.

### 1.1 Creation methods

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Pitch (raw autocorrelation) | Window-corrected ACF peak-picking (Boersma 1993) + Viterbi path. | [Sound: To Pitch (raw autocorrelation)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__raw_autocorrelation____.html) | Implemented — only method in the crate. | core-parity (satisfied) |
| To Pitch (raw cross-correlation) | Same path-finder, NCCF-style candidate strength instead of ACF; manual recommends it for voice/pathology work because it doesn't low-pass away irregularities. | [Sound: To Pitch (raw cross-correlation)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__raw_cross-correlation____.html) | Absent. | **core-parity** |
| To Pitch (filtered autocorrelation) | Gaussian low-pass pre-filter, then raw-ac; Praat's manual states this has been the *preferred* method for intonation and vocal-fold-vibration measurement since 2023 (defaults: floor 50 Hz, ceiling 800 Hz, octave cost 0.055, voicing threshold 0.50). | [Sound: To Pitch (filtered autocorrelation)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__filtered_autocorrelation____.html) | Absent. | **core-parity** — this is Praat's current recommended default, not a niche variant |
| To Pitch (filtered cross-correlation) | Low-pass filter + cc. Manual: developers "know of no situation yet" where it beats the other three. | [Sound: To Pitch (filtered cross-correlation)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__filtered_cross-correlation____.html) | Absent. | niche |
| To Pitch (shs) | Subharmonic summation (Hermes 1988): compress spectral peaks onto integer-rank sub-spectra, sum, pick the peak. | [Sound: To Pitch (shs)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__shs____.html) | Absent. | niche |
| To Pitch (legacy 3-parameter wrapper) | Thin wrapper over raw-ac with everything else at standard values. | [Sound: To Pitch...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch___.html) | Superseded — Phonia's raw-ac defaults already match. | n/a |
| SPINET (auditory-model pitch) | Task scope named this as a target; the manual survey found **zero** references to "spinet" anywhere in the current manual. | — (not found) | Absent, and not a real gap — Praat itself doesn't currently document this. | out-of-scope (not present in current Praat) |

### 1.2 Query surface

The Pitch object has **no dedicated per-command reference pages** in the
current manual — unlike Formant, Intensity, Harmonicity, and Ltas, which all
have individual `Get mean...`/`Get minimum...` pages, Pitch's query commands
are confirmed to exist only via the manual's own
[Query submenu](https://www.fon.hum.uva.nl/praat/manual/Query_submenu.html)
master list and a worked
[F0-statistics script example](https://www.fon.hum.uva.nl/praat/manual/Script_for_listing_F0_statistics.html).
This is noted per row below as "no dedicated page."

| Feature | Purpose | Manual citation | Phonia | Verdict |
|---|---|---|---|---|
| Get value at time... | Interpolated F0 at a time point. | no dedicated page (Query submenu) | Absent — `PitchTrack` has no interpolated point read. | core-parity |
| Get value in frame... | Raw F0 of a frame index. | no dedicated page | Absent as a named method. | valuable |
| Get mean... | Mean F0 over a span, Hz or semitones. | no dedicated page | Implemented — `PitchTrack::mean_hz`/`mean_semitones`. | satisfied |
| Get standard deviation... | Std. dev. over a span. | no dedicated page | Absent. | **core-parity** |
| Get quantile... | Arbitrary-percentile quantile. | no dedicated page | Partial — only `median_hz`/`median_semitones` (the 50th percentile) exist. | valuable |
| Get minimum.../Get maximum... | Extremal F0, with interpolation-method choice. | no dedicated page | Implemented — `min_hz`/`max_hz`/`min_semitones`/`max_semitones`. | satisfied |
| Get time of minimum.../Get time of maximum... | Time at the extremum. | no dedicated page | Absent. | valuable |
| Count voiced frames | Count of voiced frames in a span. | no dedicated page | Absent — needed downstream for a fraction-unvoiced statistic. | valuable |
| Get number of frames | Frame count. | [generic, shared page](https://www.fon.hum.uva.nl/praat/manual/Get_number_of_frames.html) | Implemented implicitly via `frames().len()`. | satisfied |
| Get frame number from time.../Get time from frame number... | Frame↔time conversion. | [generic](https://www.fon.hum.uva.nl/praat/manual/Get_frame_number_from_time___.html) | Implemented indirectly via the shared `FrameGrid`. | satisfied |
| Get mean absolute slope / Get slope without octave jumps | Rate-of-change statistics used for intonation research. | no dedicated page | Absent. | niche |

### 1.3 Editing / PitchTier

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| PitchEditor (interactive candidate/path editor) | Manual correction of the tracker's chosen path. | [PitchEditor](https://www.fon.hum.uva.nl/praat/manual/PitchEditor.html) | Out of scope for this doc — UI concern. | out-of-scope (UI) |
| Pitch: Interpolate | Fills unvoiced gaps in a Pitch object. | [Pitch: Interpolate](https://www.fon.hum.uva.nl/praat/manual/Pitch__Interpolate.html) | Absent. | valuable |
| Pitch: Smooth... | Smooths the F0 contour. | [Pitch: Smooth...](https://www.fon.hum.uva.nl/praat/manual/Pitch__Smooth___.html) | Absent. | valuable |
| Subtract linear fit / Kill octave jumps | Not confirmed to exist as current-manual commands on the Pitch object. | not found | n/a | out-of-scope (not confirmed in current Praat) |
| PitchTier (object + editing) | See §13 (Manipulation/PSOLA). | — | Absent. | see §13 |

---

## 2. Formant analysis

Phonia (`crates/phx-formant`) implements Burg LPC only, with a genuine
Aberth-Ehrlich polynomial root-solver (`roots.rs`) and a documented
cross-frame Viterbi tracker following Xia & Espy-Wilson (2000)
(`track.rs`) — the crate's tracking weights are explicitly flagged in its own
doc comments as provisional, not values published in the cited paper. Praat
itself has four creation methods plus a multi-ceiling path-selection object
(FormantPath) that exists specifically to work around the ceiling-mismatch
problem the project's own pain-points research already flagged (see
[praat-features-and-pain-points.md §2.8](praat-features-and-pain-points.md)).

### 2.1 Creation methods

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Formant (burg) | Burg LPC + root-finding, standard method. | [Sound: To Formant (burg)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__burg____.html) | Implemented. | satisfied |
| To Formant (keep all) | Same algorithm, but keeps candidates below 50 Hz / above ceiling−50 Hz that Burg normally discards; manual: "not recommended for general use." | [Sound: To Formant (keep all)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__keep_all____.html) | Absent. | niche |
| To Formant (robust) | Autocorrelation-method LPC + iterative reweighting (Lee 1988) against outliers. | [Sound: To Formant (robust)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__robust____.html) | Absent. | **valuable** — common choice for noisy/pathological recordings |
| To Formant (sl) | Split-Levinson (Willems 1986); always returns the requested formant count, bandwidths fixed at 50 Hz; manual: "not recommended for general use." | [Sound: To Formant (sl)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__sl____.html) | Absent. | niche |
| To FormantPath (burg) | Runs `2n+1` Burg analyses at exponentially spaced ceilings, returns a per-interval "which ceiling fit best" selection. | [Sound: To FormantPath (burg)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_FormantPath__burg____.html) | Absent. | **valuable** — directly answers the formant-ceiling pain point already on record |
| LPC: To Formant | Root-extraction from a standalone LPC object. | [LPC: To Formant](https://www.fon.hum.uva.nl/praat/manual/LPC__To_Formant.html) | Partial — the same root-finding math exists internally (`roots.rs`), but there's no standalone LPC object to run it from. | see §10 |

### 2.2 Query surface

| Feature | Purpose | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Get value at time... | Interpolated formant frequency at a time point (Hz or Bark). | [Formant: Get value at time...](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_value_at_time___.html) | Absent — no interpolated per-slot query on `FormantTrack`. | **core-parity** |
| Get bandwidth at time... | Formant bandwidth at a time point. | [Formant: Get bandwidth at time...](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_bandwidth_at_time___.html) | Absent. | **core-parity** — bandwidth is central to judging formant-track quality |
| Get mean... | Mean of a formant slot over a span. | [Formant: Get mean...](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_mean___.html) | Partial — `phx-engine::formant_span_means` gives an unweighted arithmetic mean per slot; Praat's own averaging (amplitude/bandwidth-aware) is not reproduced. | valuable |
| Get quantile... | Distribution quantile of a slot over a span. | [Formant: Get quantile...](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_quantile___.html) | Absent. | valuable |
| Get standard deviation | Std. dev. of a slot over a span. | [Formant: Get standard deviation](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_standard_deviation.html) | Absent. | valuable |
| Get minimum.../Get maximum... | Extremal value of a slot. | [min](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_minimum___.html) / [max](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_maximum___.html) | Absent. | valuable |
| Get time of minimum.../maximum... | Time of the extremum. | [min](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_time_of_minimum___.html) / [max](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_time_of_maximum___.html) | Absent. | niche |
| Get number of formants | Formants present in a given frame. | [Formant: Get number of formants](https://www.fon.hum.uva.nl/praat/manual/Formant__Get_number_of_formants.html) | Implemented implicitly (frame data carries all found formants). | satisfied |
| Get number of frames / frame↔time conversion | Structural queries. | [generic pages](https://www.fon.hum.uva.nl/praat/manual/Get_number_of_frames.html) | Implemented via the shared `FrameGrid`. | satisfied |

### 2.3 Drawing / tracking / output

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Speckle... / Draw tracks... | Per-frame dots / connected-segment plots. | [Speckle](https://www.fon.hum.uva.nl/praat/manual/Formant__Speckle___.html), [Draw tracks](https://www.fon.hum.uva.nl/praat/manual/Formant__Draw_tracks___.html) | Implemented — `phx-figure` embeds formant "speckle" point series. | satisfied |
| Track... (DP re-tracking to N target tracks) | Cheapest-path re-tracking across frames. | [Formant: Track...](https://www.fon.hum.uva.nl/praat/manual/Formant__Track___.html) | Implemented — `track_smoothed`, following a specific published cost formulation (Xia & Espy-Wilson 2000), more precisely documented than Praat's own default. | satisfied, arguably ahead — see §"Where Phonia already exceeds Praat" |
| List formant slope... | Curve fit + slope report over a span. | [Formant: List formant slope...](https://www.fon.hum.uva.nl/praat/manual/Formant__List_formant_slope___.html) | Absent. | niche |
| Down to FormantGrid | Converts frames to an editable FormantGrid. | [Formant: Down to FormantGrid](https://www.fon.hum.uva.nl/praat/manual/Formant__Down_to_FormantGrid.html) | Absent — no FormantGrid object at all. | see §13 |

---

## 3. Intensity

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Intensity | Squared signal convolved with a Gaussian-equivalent (Kaiser-20) window, effective duration `3.2/floor`, dB SPL re 2×10⁻⁵ Pa. | [Sound: To Intensity...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Intensity___.html) | Implemented (`crates/phx-intensity`) — matches the documented window/reference; local-mean subtraction is the crate's own defensible extrapolation since the manual doesn't name the local window it uses. | satisfied |
| Get value at time... | Interpolated dB value; manual specifies cubic interpolation as the standard, chosen over sinc because of intensity's log nonlinearity. | [Intensity: Get value at time...](https://www.fon.hum.uva.nl/praat/manual/Intensity__Get_value_at_time___.html) | Absent — `IntensityTrack` exposes only per-frame `db(i)`, no interpolated read. | **core-parity** |
| Get mean... | Mean dB over a span, with a choice of averaging semantics: energy, dB, or sones. | [Intensity: Get mean...](https://www.fon.hum.uva.nl/praat/manual/Intensity__Get_mean___.html) | Partial — computed ad hoc at the `phx-engine` level via a free `mean_in_span` helper, not a crate method, and with no choice of averaging semantics. | **core-parity** |
| Get minimum.../Get maximum... | Extremal dB over a span. | [min](https://www.fon.hum.uva.nl/praat/manual/Intensity__Get_minimum___.html) / [max](https://www.fon.hum.uva.nl/praat/manual/Intensity__Get_maximum___.html) | Absent — not implemented even at the engine level. | **core-parity** |
| Get standard deviation... | Std. dev. over a span. | linked from [Intensity](https://www.fon.hum.uva.nl/praat/manual/Intensity.html) | Absent. | valuable |
| Get time of maximum.../minimum... | Time of the extremum. | linked from Intensity.html | Absent. | niche |

Intensity has the thinnest query surface of any Phonia analysis crate — no
mean/min/max/stdev live on `IntensityTrack` itself, unlike `PitchTrack` and
`HnrTrack`, which both carry real stats methods. This is worth treating as an
engineering-consistency item independent of the Praat-parity gaps: unify the
query surface across track types before adding new analysis methods.

---

## 4. Harmonicity (HNR)

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Harmonicity (ac) | Window-corrected-ACF-peak HNR (Boersma 1993); up to ~80 dB sensitivity. | [Sound: To Harmonicity (ac)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Harmonicity__ac____.html) | Implemented (`hnr_track` in `crates/phx-voice`). | satisfied |
| To Harmonicity (cc) | Cross-correlation HNR; manual explicitly recommends this over ac for better time resolution (ac ≈37–80 dB sensitivity depending on periods/window vs cc's better time localization at ~60 dB). | [Sound: To Harmonicity (cc)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Harmonicity__cc____.html) | Absent — the crate's own default-method doc explicitly names "ac" only. | **core-parity** — Praat's manual names cc as the better everyday choice |
| To Harmonicity (gne) | Glottal-to-Noise Excitation ratio, a cross-band-correlation breathiness measure distinct from ac/cc's whole-waveform periodicity measure. | not found — confirmed absent from both the Harmonicity object page and the Periodicity menu's full command listing | Absent. | niche — existence in current Praat itself is unconfirmed by the manual, despite being attested in third-party command indexes |
| Get value at time... / Get value in frame... | Point queries. | [value at time](https://www.fon.hum.uva.nl/praat/manual/Harmonicity__Get_value_at_time___.html) / [value in frame](https://www.fon.hum.uva.nl/praat/manual/Harmonicity__Get_value_in_frame___.html) | Absent. | valuable |
| Get mean... | Mean HNR over a span, silent frames excluded. | [Harmonicity: Get mean...](https://www.fon.hum.uva.nl/praat/manual/Harmonicity__Get_mean___.html) | Implemented — `HnrTrack::mean_db`. | satisfied |
| Get maximum.../minimum.../standard deviation/time of max/min | Further stats. | linked from [Harmonicity](https://www.fon.hum.uva.nl/praat/manual/Harmonicity.html) | Absent. | niche |

---

## 5. Spectrogram

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Spectrogram | Gaussian-window STFT PSD in Pa²/Hz, worked-example defaults 0.005 s window / 5000 Hz range / 0.002 s time step / 20 Hz freq step. | [Sound: To Spectrogram...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Spectrogram___.html) | Implemented (`crates/phx-spectrogram`) — matches every documented default and the min-step clamp rules (`windowLength/(8√π)`, `√π/(8·windowLength)`), validated against a scipy oracle to 1e-6 relative tolerance. | satisfied, arguably ahead — see closing section |
| Window shape choice (Gaussian/Square/Hamming/Bartlett/Welch/Hanning) | Advanced spectrogram settings expose 6 window shapes. | [Advanced spectrogram settings...](https://www.fon.hum.uva.nl/praat/manual/Advanced_spectrogram_settings___.html) | Partial — `phx-dsp` has Hanning, Gaussian, Kaiser; Hamming, Bartlett, Welch, and Square are absent, and `SpectrogramParams` always defaults to Gaussian (Praat's own recommended shape). | niche |
| To Spectrogram (pitch-dependent) / Sound & Pitch: To Spectrogram | Pitch-synchronous analysis variant. | [pitch-dependent](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Spectrogram__pitch-dependent____.html) | Absent. | niche |
| Point queries (Get power at..., Get frequency of nearest maximum...) | Task scope assumed these exist as scripting commands. | not found — confirmed absent from the Spectrogram object page's full link list; only reachable interactively via editor cross-hairs | Phonia has no per-cell query API on raw tiles either, but **does** have `phx-engine::band_energy` (mean PSD in a time-frequency box) and a bundled `selection_readout`, neither of which Praat exposes as a documented Spectrogram-object command. | Phonia ahead here — see closing section |
| Display: dB clip/dynamic range, pre-emphasis, colormap | Autoscale, dynamic range, +6 dB/oct pre-emphasis, grayscale mapping. | [Advanced spectrogram settings...](https://www.fon.hum.uva.nl/praat/manual/Advanced_spectrogram_settings___.html) | Implemented (`phx-render::DisplayMapping`/`colorize`), with a materially wider colormap set than Praat's grayscale-only display. | satisfied, ahead |

---

## 6. Spectrum

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| To Spectrum... / To Spectrum (resampled)... | Whole-signal FFT to a persistent Spectrum object; "resampled" upsamples to a power of two instead of zero-padding. | [To Spectrum](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Spectrum___.html), [resampled](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Spectrum__resampled____.html) | Absent as a standalone persistent object — spectral analysis is always ad hoc (`SpectrumSlice` built from a spectrogram frame). | **valuable** |
| Get centre of gravity.../standard deviation.../skewness.../kurtosis.../central moment... | Power-weighted spectral moments. | [centre of gravity](https://www.fon.hum.uva.nl/praat/manual/Spectrum__Get_centre_of_gravity___.html) (+ stdev/skewness/kurtosis/central-moment siblings) | Implemented (`spectral_moments()` in `crates/phx-voice`) — all four moments present, driven off a spectrogram-frame slice rather than a true selection-wide FFT. | satisfied (with the object-model caveat above) |
| Filter (pass Hann band)... / Filter (stop Hann band)... | Hann-skirt band gain multiplication. | [pass](https://www.fon.hum.uva.nl/praat/manual/Spectrum__Filter__pass_Hann_band____.html) / [stop](https://www.fon.hum.uva.nl/praat/manual/Spectrum__Filter__stop_Hann_band____.html) | Partial — `phx-dsp::band_pass_filter` implements pass-band only, with a fixed 100 Hz skirt (not user-adjustable smoothing); no stop-band. | **valuable** |
| Formula... | Arbitrary per-cell formula on real/imaginary rows. | [Spectrum: Formula...](https://www.fon.hum.uva.nl/praat/manual/Spectrum__Formula___.html) | Absent (no expression evaluator anywhere in the repo). | out-of-scope — see §17 |
| To Sound / To Sound (resampled) | Inverse FFT round trip. | [To Sound](https://www.fon.hum.uva.nl/praat/manual/Spectrum__To_Sound.html) | Absent as an explicit operation (the underlying inverse FFT exists in `phx-dsp::RealFftPlan` but isn't exposed as a Spectrum→Sound round trip). | niche |

Praat's manual does **not** document "Get band energy...", "Get band
density...", "Get value at frequency...", or "Get value in bin..." on the
Spectrum object — those live on Ltas instead (§7). Any Phonia roadmap item
planning those as Spectrum methods should be redirected to the Ltas design.

---

## 7. Ltas (long-term average spectrum)

Entirely absent from Phonia. Praat's Ltas is the whole-recording spectral
summary tool used for register/voice-quality and dialectological work (e.g.
average spectral tilt, /s/ spectral characterization over a corpus) — distinct
from the per-frame spectral moments Phonia already has via `phx-voice`.

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Ltas (object) | Time-averaged log PSD vs. frequency, dB/Hz. | [Ltas](https://www.fon.hum.uva.nl/praat/manual/Ltas.html) | Absent. | **valuable** |
| Sound: To Ltas (pitch-corrected)... | F0-corrected spectral envelope, isolating vocal-tract/glottal-source shape without sacrificing frequency resolution (Boersma & Kovacic 2006). | [Sound: To Ltas (pitch-corrected)...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_Ltas__pitch-corrected____.html) | Absent. | valuable |
| Spectrum: To Ltas (1-to-1) | Basic (non-pitch-corrected) route: Sound → Spectrum → Ltas, one Spectrum bin per Ltas band. Note: no plain "Sound: To Ltas..." command exists in the current manual — only this route and the pitch-corrected variant are documented. | [Spectrum: To Ltas (1-to-1)](https://www.fon.hum.uva.nl/praat/manual/Spectrum__To_Ltas__1-to-1_.html) | Absent. | valuable |
| Get mean.../Get standard deviation.../Get maximum.../Get minimum.../Get frequency of maximum.../Get frequency of minimum.../Get value at frequency.../Get value in bin.../bin↔frequency conversions | Full statistical + point-query surface over the averaged spectrum. | [Ltas: Get mean...](https://www.fon.hum.uva.nl/praat/manual/Ltas__Get_mean___.html) (+ sibling pages) | Absent. | valuable |

---

## 8. Cepstrum / PowerCepstrogram / CPPS

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Cepstrum (object) | Log spectrum of the log power spectrum. Praat's own manual page for this is a near-empty stub with no algorithmic content. | [Cepstrum](https://www.fon.hum.uva.nl/praat/manual/Cepstrum.html) | No bare "Cepstrum" object, but the real substance (below) is covered. | out-of-scope — Praat itself doesn't build meaningful functionality on this object either |
| Sound: To PowerCepstrogram... | Time-varying quefrency representation: resample → pre-emphasize → Gaussian-window each frame → Spectrum → PowerCepstrum, stacked over time. | [Sound: To PowerCepstrogram...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_PowerCepstrogram___.html) | Absent as a persistent time-varying object — Phonia computes cepstra per-frame internally but doesn't expose a queryable/paintable PowerCepstrogram. | **valuable** — a real cepstrogram would enable time-resolved dysphonia visualization, not just a scalar per span |
| PowerCepstrogram: Get CPPS... | Cepstral Peak Prominence Smoothed: time+quefrency-smoothed cepstrum, peak height above a regression trend line (Hillenbrand et al. 1994/1996; Maryn & Weenink 2015 for the smoothed variant). Standard fit method is "Robust slow" (Theil's line fit). | [PowerCepstrogram: Get CPPS...](https://www.fon.hum.uva.nl/praat/manual/PowerCepstrogram__Get_CPPS___.html) | Implemented — `cpps()` in `crates/phx-voice`, scalar-over-span (averages cepstra across the span, smooths across quefrency, then measures CPP on the result). Regression is a plain linear fit (not the "Robust slow" Theil fit Praat names as its standard). | satisfied for the headline measure; minor regression-method gap |
| PowerCepstrogram: Smooth... / Paint... | Explicit time/quefrency smoothing step and grayscale rendering. | [Smooth](https://www.fon.hum.uva.nl/praat/manual/PowerCepstrogram__Smooth___.html), [Paint](https://www.fon.hum.uva.nl/praat/manual/PowerCepstrogram__Paint___.html) | Smoothing happens inline inside `cpps()`; no standalone smoothed object and no paint/render surface. | niche (Smooth is functionally covered; Paint is a visualization gap tied to the missing PowerCepstrogram object above) |

Note: Phonia's `voice_report()` already bundles CPP and CPPS into the same
call as jitter/shimmer/HNR/voice-breaks — genuinely more convenient than
Praat's own workflow, where CPPS requires separately creating a
PowerCepstrogram object outside the Voice report entirely (§12).

---

## 9. Excitation / Cochleagram

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Excitation (object) | Basilar-membrane excitation pattern, Bark scale (0–25.6 Bark), used for loudness (sone) modeling. | [Excitation](https://www.fon.hum.uva.nl/praat/manual/Excitation.html) | Absent. | out-of-scope |
| Cochleagram (object) | Same Bark-scale model, extended along a time axis — nominally distinct from Excitation but described in the manual as literally "the excitation pattern... as a function of time." | [Cochleagram](https://www.fon.hum.uva.nl/praat/manual/Cochleagram.html) | Absent. | out-of-scope |

Both object pages are near-stubs, and — more importantly — **the current
manual documents no command that creates either object from a Sound.**
Direct URL checks for `Sound: To Excitation...` and `Sound: To
Cochleagram...` 404, and neither command appears in the Sound, Excitation, or
Cochleagram page link lists. Building Sound→Excitation/Cochleagram machinery
would mean implementing an entry point Praat itself doesn't currently expose
to users through any documented command — not a parity gap worth chasing.

---

## 10. LPC / MFCC / LFCC

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| LPC (object) | Time-varying linear-prediction coefficients, as a first-class queryable/filterable object. | [LPC](https://www.fon.hum.uva.nl/praat/manual/LPC.html) | Partial — Burg LPC coefficients are computed internally in `crates/phx-formant/src/burg.rs` en route to formant roots, but never surfaced as a standalone object with its own query/filter/spectrum-slice operations. | **valuable** |
| To LPC (autocorrelation).../ (covariance).../ (burg).../ (marple)... | Four LPC estimation methods (autocorrelation and covariance per Markel & Gray 1976, Burg per Anderson 1978, Marple's adaptive-order method 1980). Manual explicitly warns not to use any of these for formant analysis — use `To Formant (burg)` instead, since `To LPC` always uses Nyquist as its ceiling. | [autocorrelation](https://www.fon.hum.uva.nl/praat/manual/Sound__To_LPC__autocorrelation____.html), [covariance](https://www.fon.hum.uva.nl/praat/manual/Sound__To_LPC__covariance____.html), [burg](https://www.fon.hum.uva.nl/praat/manual/Sound__To_LPC__burg____.html), [marple](https://www.fon.hum.uva.nl/praat/manual/Sound__To_LPC__marple____.html) | Burg only, and only internally (see above). | valuable |
| LPC: To Spectrum (slice)... | Spectrum from one LPC frame, with bandwidth-reduction sharpening. | [LPC: To Spectrum (slice)...](https://www.fon.hum.uva.nl/praat/manual/LPC__To_Spectrum__slice____.html) | Absent. | niche |
| LPC: To LFCC... / LFCC (object) / LFCC: To LPC... | LPC-coefficient ↔ linear-frequency cepstral coefficient conversion. Praat **does** have LFCC as a first-class object type. | [LPC: To LFCC](https://www.fon.hum.uva.nl/praat/manual/LPC__To_LFCC___.html), [LFCC](https://www.fon.hum.uva.nl/praat/manual/LFCC.html) | Absent — zero references anywhere in the repo. | niche (LFCC sees far less real-world use than MFCC) |
| Sound: To MFCC... (via MelSpectrogram) | Two-stage pipeline: triangular mel-filterbank → DCT of log filter powers (Davis & Mermelstein 1980). | [Sound: To MFCC...](https://www.fon.hum.uva.nl/praat/manual/Sound__To_MFCC___.html), [To MelSpectrogram](https://www.fon.hum.uva.nl/praat/manual/Sound__To_MelSpectrogram___.html) | Absent. | **valuable** — MFCC is the standard feature for forced-alignment/ML-adjacent phonetics pipelines, more so today than when Praat's classic toolset was designed |
| MFCC: To MelSpectrogram... / To TableOfReal... | Inverse DCT reconstruction; frame-flattening to a table (no MFCC→Sound path exists in Praat at all). | [To MelSpectrogram](https://www.fon.hum.uva.nl/praat/manual/MFCC__To_MelSpectrogram___.html), [To TableOfReal](https://www.fon.hum.uva.nl/praat/manual/MFCC__To_TableOfReal___.html) | Absent. | niche |

---

## 11. PointProcess and periodicity queries

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Sound & Pitch: To PointProcess (cc) | Cross-correlation-maximizing glottal-pulse placement, seeded by the pitch-voiced intervals. | [Sound & Pitch: To PointProcess (cc)](https://www.fon.hum.uva.nl/praat/manual/Sound___Pitch__To_PointProcess__cc_.html) | Implemented — `pulses()` in `crates/phx-voice`, the same cross-correlation search-outward-from-a-seed-point approach, with matching correlation/edge thresholds (0.3 drop, 0.7 edge-keep). | satisfied |
| Sound & Pitch: To PointProcess (peaks) | Local-extremum pulse placement instead of cross-correlation; manual itself says this is "less useful for analysis and subsequent overlap-add synthesis." | [Sound & Pitch: To PointProcess (peaks)...](https://www.fon.hum.uva.nl/praat/manual/Sound___Pitch__To_PointProcess__peaks____.html) | Absent. | niche |
| Sound: To PointProcess (periodic, cc)... / (periodic, peaks)... | One-step convenience wrappers combining pitch analysis + the above. | [periodic, cc](https://www.fon.hum.uva.nl/praat/manual/Sound__To_PointProcess__periodic__cc____.html) | Absent as a single call (achievable by composing existing pitch + pulse functions). | niche (ergonomics only) |
| Pitch: To PointProcess | Builds pulses from a PitchTier's own timing rather than the waveform; manual notes this is **not** waveform-aligned and unsuitable for voice analysis. | [Pitch: To PointProcess](https://www.fon.hum.uva.nl/praat/manual/Pitch__To_PointProcess.html) | Absent. | niche |
| Sound: To PointProcess (extrema)... / (zeroes)... | Task scope named these; the manual survey confirms **neither exists** in the current manual — only the two `periodic` wrappers above are documented. | not found | n/a | out-of-scope (not present in current Praat) |
| Get low index.../Get high index.../Get nearest index.../Get interval... | Index/interval lookups around a time point. | [low](https://www.fon.hum.uva.nl/praat/manual/PointProcess__Get_low_index___.html) (+ siblings) | Absent as named methods — `times()`/`times_in()` give raw access but no index-lookup API. | niche |
| Get number of points / Get number of periods.../Get mean period... | Basic pulse-count/period-averaging queries. | not found — none of the three appear in the manual's PointProcess command list either | Absent, and Praat itself doesn't document these as commands. | out-of-scope (not confirmed in current Praat) |
| Get jitter (local).../(local, absolute).../(rap).../(ppq5).../(ddp) | All five jitter measures, computable directly from a PointProcess. | [local](https://www.fon.hum.uva.nl/praat/manual/PointProcess__Get_jitter__local____.html) (+ 4 siblings) | Implemented — all five, in `jitter()`/`JitterKind` (`crates/phx-voice`). | satisfied |

---

## 12. Voice report

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Voice report (aggregate) | Text report from Sound + Pitch + PointProcess covering pitch range, jitter, shimmer, and voice breaks; the manual does not give one exhaustive field list (only two exact field names are quoted verbatim: "Jitter (local): " and "Shimmer (local): "). HNR and CPPS are **not** part of the classic Voice report — they require separately creating a Harmonicity or PowerCepstrogram object. | [Voice report](https://www.fon.hum.uva.nl/praat/manual/Voice_report.html), tutorial: [Voice](https://www.fon.hum.uva.nl/praat/manual/Voice.html) | Implemented as a structured `VoiceReport` (`voice_report()` in `crates/phx-voice`) — and it already bundles pitch summary, all jitter/shimmer measures, mean HNR, CPP, CPPS, and voice breaks into one call, which is *more* integrated than Praat's own multi-object workflow. | satisfied, ahead — see closing section |
| Fraction of locally unvoiced frames (MDVP: DUV) | % of pitch frames locally unvoiced, computed with the path-finder switched off. | [Voice 1: Voice breaks](https://www.fon.hum.uva.nl/praat/manual/Voice_1__Voice_breaks.html) | Absent as a percentage — Phonia has no locally-unvoiced-frame statistic at all (only the whole-path voiced/unvoiced classification from the normal path finder). | **core-parity** — a headline clinical measure |
| Number of voice breaks | Count of inter-pulse gaps exceeding `1.25/floor`. | same page | Partial — `voice_breaks()` returns the raw gap list (so the count is derivable), but not as a named field. | valuable |
| Degree of voice breaks (MDVP: DVB) | Total break duration ÷ total analyzed duration, as a percentage. | same page | Partial — `VoiceBreaks` returns total gap seconds, not normalized to % of span. | **core-parity** — paired MDVP measure with DUV, expected alongside it in any clinical report |
| Jitter: local, local absolute, rap, ppq5, ddp | All five, with MDVP pathology thresholds documented (e.g. Jitt 1.040%, RAP 0.680%, PPQ 0.840%). | [Voice 2: Jitter](https://www.fon.hum.uva.nl/praat/manual/Voice_2__Jitter.html) | Implemented, all five. | satisfied |
| Shimmer: local, local dB, apq3, apq5, apq11, dda | All six, MDVP thresholds partially documented (Shim 3.810%, ShdB 0.350 dB, APQ11 3.070%). | [Voice 3: Shimmer](https://www.fon.hum.uva.nl/praat/manual/Voice_3__Shimmer.html) | Implemented, all six. | satisfied |
| Additive noise (HNR) | SNR = HNR for a periodic signal, via `To Harmonicity (ac)`/`(cc)`; manual calls this "the world's most sensitive HNR measurement (up to 90 dB)." | [Voice 4: Additive noise](https://www.fon.hum.uva.nl/praat/manual/Voice_4__Additive_noise.html) | Implemented for ac; cc variant absent (§4). | see §4 |

---

## 13. Manipulation / PSOLA / tiers

This is the single largest structural gap. Every piece of it is absent from
Phonia — there is no Manipulation object, no PSOLA resynthesis, and none of
the four tier types (PitchTier, DurationTier, IntensityTier, FormantGrid)
exist anywhere in the repo. This whole area underlies Praat's *Intro* chapter
8 (perception-experiment stimulus construction) and any pitch/duration
manipulation workflow, which is core to the stated goal of Phonia eventually
being a Praat superset rather than an analysis-only viewer.

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Manipulation (object) | Bundles the original Sound, a hidden glottal-pulse PointProcess, an editable PitchTier, and an editable DurationTier. | [Manipulation](https://www.fon.hum.uva.nl/praat/manual/Manipulation.html) | Absent. | **core-parity** |
| To Manipulation (creation) | Pipeline: pitch analysis → cc-method pulses → Pitch: To PitchTier → empty DurationTier. No dedicated manual page exists for the command itself; described inline on the Manipulation object page. | described on [Manipulation](https://www.fon.hum.uva.nl/praat/manual/Manipulation.html) | Absent, though every prerequisite (pitch, pulses) already exists in Phonia and just needs bundling. | **core-parity** |
| Overlap-add / PSOLA resynthesis | Pitch-synchronous time-domain overlap-add (TD-PSOLA, Moulines & Charpentier 1990): retime via a PointProcess derived from the target PitchTier, copy voiceless stretches directly, and for voiced stretches overlap-add a bell-shaped window per target point sized from the neighboring source/target periods. | [overlap-add](https://www.fon.hum.uva.nl/praat/manual/overlap-add.html) | Absent. | **core-parity** |
| Get resynthesis (overlap-add) / Play (overlap-add) | Extract or play the resynthesized Sound. | [Get resynthesis](https://www.fon.hum.uva.nl/praat/manual/Manipulation__Get_resynthesis__overlap-add_.html), [Play](https://www.fon.hum.uva.nl/praat/manual/Manipulation__Play__overlap-add_.html) | Absent. | core-parity |
| ManipulationEditor | Interactive pitch/duration point editing UI, with a recommended pre-editing pitch-stylize step. | [ManipulationEditor](https://www.fon.hum.uva.nl/praat/manual/ManipulationEditor.html) | Absent — UI concern. | out-of-scope for this doc (UI) |
| PitchTier (object): Add point..., Modify interval..., Modify interval (tone levels)..., Get mean/stdev (curve/points), Down to PointProcess, Stylize... | Editable pitch-contour tier; "Modify interval (tone levels)" is specifically for tone-language stimulus work (T = numberOfLevels·log(f/Fmin)/log(Fmax/Fmin)). | [PitchTier](https://www.fon.hum.uva.nl/praat/manual/PitchTier.html), [Stylize](https://www.fon.hum.uva.nl/praat/manual/PitchTier__Stylize___.html) | Absent entirely. | **core-parity** |
| DurationTier (object): Add point..., Get target duration... | Editable relative-duration tier consumed by PSOLA time-scaling. | [DurationTier](https://www.fon.hum.uva.nl/praat/manual/DurationTier.html) | Absent. | **core-parity** — paired with PitchTier for any resynthesis workflow |
| IntensityTier (object): Add point..., Sound & IntensityTier: Multiply | Editable relative-dB tier; Multiply scales a Sound by the interpolated tier, then re-peaks to 0.9. | [IntensityTier](https://www.fon.hum.uva.nl/praat/manual/IntensityTier.html), [Sound & IntensityTier: Multiply](https://www.fon.hum.uva.nl/praat/manual/Sound___IntensityTier__Multiply.html) | Absent. | valuable |
| FormantGrid (object): Add formant/bandwidth point..., Sound & FormantGrid: Filter [(no scale)] | Editable formant-frequency/bandwidth tiers, drivable as a source-filter synthesizer. | [FormantGrid](https://www.fon.hum.uva.nl/praat/manual/FormantGrid.html), [Sound & FormantGrid: Filter](https://www.fon.hum.uva.nl/praat/manual/Sound___FormantGrid__Filter.html) | Absent. | valuable |

---

## 14. Filtering

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Filter (pass Hann band)... / Filter (stop Hann band)... | Hann-skirt frequency-domain gain, equivalent to Sound→Spectrum→Spectrum-filter→Sound. Standard from/to 500/1000 Hz, smoothing 100 Hz. | [pass](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__pass_Hann_band____.html), [stop](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__stop_Hann_band____.html) | Partial — `phx-dsp::band_pass_filter` covers pass-band with a fixed 100 Hz skirt; no stop-band, no adjustable smoothing. | **valuable** |
| Filter (formula)... | Arbitrary frequency-domain formula, e.g. rectangular bands (with a documented ringing warning). | [Sound: Filter (formula)...](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__formula____.html) | Absent (no formula evaluator). | out-of-scope — see §17 |
| Filter (one formant)... [+ in-place] | Time-domain 2-pole IIR resonator filter, `p = -2e^{-πB Δt}cos(2πF Δt)`, `q = e^{-2BΔt}`. | [Filter (one formant)](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__one_formant____.html) | Absent. | niche |
| Filter (pre-emphasis)... / Pre-emphasize (in-place)... | +6 dB/oct high-pass above a corner frequency. | [Filter (pre-emphasis)](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__pre-emphasis____.html) | Partial — `phx-dsp::preemphasis_in_place` implements the same first-difference formula internally (used to prep formant/pitch analysis), but isn't exposed as a user-facing Sound-modification command. | valuable |
| Filter (de-emphasis)... / De-emphasize (in-place)... | Inverse of pre-emphasis, recursive low-pass. | [Filter (de-emphasis)](https://www.fon.hum.uva.nl/praat/manual/Sound__Filter__de-emphasis____.html) | Absent — no de-emphasis anywhere in the repo. | valuable |
| Remove noise... | Spectral-subtraction denoiser (Boll 1979 method, "modelled after a script by Ton Wempe"), with noise-region auto-detection. | [Sound: Remove noise...](https://www.fon.hum.uva.nl/praat/manual/Sound__Remove_noise___.html) | Absent. | **valuable** — commonly requested for field recordings |
| Sound & Formant: Filter [(no scale)] / Sound & FormantGrid: Filter [(no scale)] / LPC & Sound: Filter... [(inverse)] | Source-filter resynthesis through an existing Formant/FormantGrid/LPC object. | [Sound & Formant: Filter](https://www.fon.hum.uva.nl/praat/manual/Sound___Formant__Filter.html) | Absent. | valuable — ties to §13's Manipulation gap |

---

## 15. Resampling and time-scaling

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Sound: Resample... | Precision-parametrized sinc interpolation (Precision 1 = linear, higher = deeper sinc depth); anti-aliasing low-pass applied automatically when downsampling. Standard Precision: 50. | [Sound: Resample...](https://www.fon.hum.uva.nl/praat/manual/Sound__Resample___.html) | Implemented (`crates/phx-audio`, `rubato` windowed sinc, Blackman-Harris2 window, sinc length 256, cutoff pinned to destination Nyquist) — a single fixed high-quality preset, no user-adjustable precision dial. | satisfied, minor configurability gap (valuable) |
| Sound: Extract part... | Time-range extraction with a choice of 12 window shapes, relative-width scaling, and a preserve-original-timing option. | [Sound: Extract part...](https://www.fon.hum.uva.nl/praat/manual/Sound__Extract_part___.html) | Partial — raw slicing exists (`slice_samples`/`AudioView`), but no window-shaped tapering or relative-width option. | valuable |
| Sound: Lengthen (overlap-add)... | Constant-factor time-scaling via the same overlap-add machinery as Manipulation (factor capped near 3×). | [Sound: Lengthen (overlap-add)...](https://www.fon.hum.uva.nl/praat/manual/Sound__Lengthen__overlap-add____.html) | Absent. | **core-parity** — common need even outside a full Manipulation workflow, and shares its implementation with §13 |
| Sound: Deepen band modulation... | Bark-band spectral decomposition + selective modulation-rate enhancement to sharpen fast spectral changes (F2 transitions); explicitly "not identical to" its cited inspiration (Nagarajan et al. 1998). | [Sound: Deepen band modulation...](https://www.fon.hum.uva.nl/praat/manual/Sound__Deepen_band_modulation___.html) | Absent. | niche |

---

## 16. Convolution and cross-correlation of sounds

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Sounds: Convolve... | Two-sound convolution, with scaling options (integral/sum/normalize/peak 0.99) and an "outside time domain = zero" default. | [Sounds: Convolve...](https://www.fon.hum.uva.nl/praat/manual/Sounds__Convolve___.html) | Absent as a first-class operation (cross-correlation logic exists only inline inside pulse-finding in `phx-voice`). | valuable |
| Sounds: Cross-correlate... | Same scaling options; not commutative (order-reversal gives the time-reversed result). | [Sounds: Cross-correlate...](https://www.fon.hum.uva.nl/praat/manual/Sounds__Cross-correlate___.html) | Absent as a first-class operation. | valuable |
| Sound: Autocorrelate... | Single-sound autocorrelation as a user-facing command (distinct from the internal ACF inside pitch analysis). | [Sound: Autocorrelate...](https://www.fon.hum.uva.nl/praat/manual/Sound__Autocorrelate___.html) | Absent as a first-class operation. | niche |
| Sounds: Concatenate / Concatenate with overlap... | List-order concatenation; the overlap variant cross-fades with raised-cosine tails. | [Concatenate](https://www.fon.hum.uva.nl/praat/manual/Sounds__Concatenate.html), [Concatenate with overlap](https://www.fon.hum.uva.nl/praat/manual/Sounds__Concatenate_with_overlap___.html) | Absent. | niche (combination utility, not signal processing proper) |

---

## 17. Sound arithmetic and Formula

Praat's Formula language is the mechanism by which several "missing"
commands below actually get done in real Praat usage — there is no dedicated
`Sound: Multiply...`/`Add...`/`Reverse` command in the current manual either
(confirmed absent from the full Sound.html link enumeration); users write
`self * factor` or `self + constant` instead. Phonia's design direction
already rejects a bespoke expression language in favor of a typed API (see
[praat-features-and-pain-points.md §2.4–2.5](praat-features-and-pain-points.md)),
so the right roadmap answer here is small dedicated functions for the common
cases, not a formula evaluator.

| Feature | What it does | Manual page | Phonia | Verdict |
|---|---|---|---|---|
| Formula... (general expression evaluator) | Per-sample arbitrary formula; `self`, `self[idx]`, `self(t)` (interpolated), `x`/`row`/`col`, plus built-in psychoacoustic scale conversions (`hertzToBark`/`barkToHertz`, `hertzToMel`/`melToHertz`, `hertzToSemitones`/`semitonesToHertz`, `hertzToErb`/`erbToHertz`, `erb`, `phonToDifferenceLimens`/`differenceLimensToPhon`). | [Formulas 1.7](https://www.fon.hum.uva.nl/praat/manual/Formulas_1_7__Formulas_for_modification.html), [Formulas 5](https://www.fon.hum.uva.nl/praat/manual/Formulas_5__Mathematical_functions.html) | Absent — no expression evaluator anywhere in the repo. | **out-of-scope** — superseded by native typed functions per the project's redesign direction, not a gap to fill with a scripting language |
| Multiply..., Add..., Scale..., Reverse | Task scope assumed these exist as discrete commands. Confirmed absent from the current manual (done via Formula instead, as above). | not found | Absent. | out-of-scope (not real Praat commands either — see native-equivalents note below) |
| Scale peak... | Multiply amplitude so the absolute peak hits a target (standard 0.99). | [Sound: Scale peak...](https://www.fon.hum.uva.nl/praat/manual/Sound__Scale_peak___.html) | Absent as a dedicated function. | **valuable** — worth a native function even without a formula language |
| Scale intensity... | Multiply amplitude so RMS intensity hits a target dB SPL (standard 70 dB). | [Sound: Scale intensity...](https://www.fon.hum.uva.nl/praat/manual/Sound__Scale_intensity___.html) | Absent. | valuable |
| Set value at sample number... | Direct single-sample overwrite by index. | [Sound: Set value at sample number...](https://www.fon.hum.uva.nl/praat/manual/Sound__Set_value_at_sample_number___.html) | Absent as a dedicated op (though trivially achievable). | niche |
| Multiply by window... | Multiplies the whole Sound by one of 12 window shapes. | [Sound: Multiply by window...](https://www.fon.hum.uva.nl/praat/manual/Sound__Multiply_by_window___.html) | Partial — windowing exists internally in `phx-dsp` for analysis frames only, not as a general Sound-transform with named shape choices. | niche |

---

## Where Phonia already exceeds Praat

A handful of findings from this survey are genuine advantages, not just
parity gaps in the other direction:

1. **Viewport-independent measurement.** Every analysis crate rides on one
   shared `FrameGrid` (`crates/phx-dsp/src/frame_grid.rs`) that is a pure
   function of `(duration, window, step)` with centered symmetric margins.
   Query results for an unchanged selection are therefore bit-identical
   regardless of zoom/scroll state. Praat has a documented, acknowledged bug
   class here — the maintainer's own response to a user report was "Zooming
   and scrolling lead to pitch values being measured at different time
   points, that's all" (tracked as pain point 2.9 in
   [praat-features-and-pain-points.md](praat-features-and-pain-points.md)).
2. **Numerically validated spectrogram tiling.** `phx-spectrogram`'s tile
   computation is checked against a scipy oracle fixture to 1e-6 relative
   tolerance, and overlapping-tile requests are verified bit-identical by
   test — a stronger reproducibility guarantee than anything Praat's manual
   documents for its own spectrogram renderer.
3. **Bundled composite queries.** `phx-engine::selection_readout` returns
   F0 range, band energy, mean intensity, and mean HNR from one call, and
   `voice_report()` returns pitch summary + all jitter/shimmer measures +
   mean HNR + CPP + CPPS + voice breaks from one call. Praat's equivalent
   requires separately creating and querying five or six different object
   types (Pitch, Harmonicity, PowerCepstrogram, PointProcess, plus the
   Voice-report command itself, which doesn't even include HNR/CPPS).
4. **A documented, published cost function for formant tracking.**
   `phx-formant::track_smoothed` implements a specific, citable
   cross-frame Viterbi cost formulation (Xia & Espy-Wilson 2000), whereas
   Praat's default `To Formant (burg)` performs no cross-frame optimization
   at all — full-track dynamic-programming re-tracking is a separate,
   opt-in `Track...` command in Praat.
5. **Wider display colormap set.** `phx-render` ships viridis, magma,
   turbo, inferno, plasma, cividis, cmrmap, gnuplot, ocean, and cubehelix
   ramps alongside a project-specific one, versus Praat's grayscale-only
   spectrogram display.
6. **Streaming architecture for long files.** `phx-audio::stream` computes
   spectrogram tiles from a bounded LRU-cached chunk reader over an
   arbitrary-length WAV file without ever materializing the whole file in
   memory — addressing the LongSound-style whole-file problem
   (pain point 2.10) at the architecture level rather than via a separate
   object type users have to remember to switch to.

---

## Summary by verdict

Counting each distinct feature row across all seventeen sections above
(commands confirmed absent from current Praat itself — SPINET,
`extrema`/`zeroes` PointProcess creation, `Get number of points`, `Subtract
linear fit`/`Kill octave jumps`, `Multiply`/`Add`/`Reverse` — are excluded
from the tally as non-gaps, not counted toward Phonia):

| Verdict | Count | What it means for the roadmap |
|---|---|---|
| **core-parity** | 20 | Cross-correlation and filtered-autocorrelation pitch methods; Formant/Intensity per-time-point and bandwidth queries; Intensity min/max/mean; Harmonicity (cc); the entire Manipulation/PSOLA/PitchTier/DurationTier core (7 rows); Sound: Lengthen (overlap-add); fraction-unvoiced and degree-of-voice-breaks statistics. |
| **valuable** | 37 | FormantPath, robust-formant, Ltas (5 rows), standalone Spectrum/LPC objects, MFCC, PowerCepstrogram as a real time-varying object, Remove noise, stop-band filtering, pre-/de-emphasis as user-facing filters, convolution/cross-correlation as first-class ops, IntensityTier/FormantGrid, and most of the thinner query-surface rows (quantile, stdev, time-of-extremum) across Pitch/Formant/Harmonicity. |
| **niche** | 27 | Split-Levinson and keep-all formant methods, SHS and filtered-cc pitch, GNE (existence itself unconfirmed), LFCC, Deepen band modulation, one-formant filter, Autocorrelate, most drawing/listing commands, PointProcess index lookups. |
| **out-of-scope** | 13 | Formula/expression evaluator (superseded by typed API), Excitation/Cochleagram (no documented Sound creation path even in Praat), Cepstrum-the-stub-object, SPINET and the two nonexistent PointProcess creation variants, PitchEditor/ManipulationEditor (UI, not analysis). |

The single highest-priority item is **Manipulation/PSOLA and its supporting
tiers** — currently 0% implemented, and the only area where "core-parity"
rows cluster into one coherent, currently entirely-missing subsystem rather
than scattered query-surface gaps. The second-highest-priority cluster is
**query-surface completeness** on the analysis objects Phonia already has
(Pitch, Formant, Intensity, Harmonicity): the algorithms exist, but
`Get value at time`, `Get standard deviation`, `Get quantile`, and
min/max are inconsistently present across track types, and Praat's own
Query submenu is exactly the pattern users expect scripting parity with.
