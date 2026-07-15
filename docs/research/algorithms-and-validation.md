# Praat analysis core: algorithms and validation

Reference notes for a clean-room, MIT-licensed Rust reimplementation of Praat's
acoustic-analysis routines. Every algorithm below is grounded in published
literature or Praat's own public manual, so an implementer can work from cited
sources without reading GPL source. Praat manual pages are cited as documentation
of Praat's documented behaviour and default parameters; the underlying algorithms
trace to the primary papers cited alongside them.

Scope of "clean-room" here: read papers and Praat's public documentation freely;
Praat's source may inform understanding but no code is ported. Where the manual
under-specifies a step (noted inline), the implementer picks a defensible choice
from the cited primary literature.

## Contents

1. Pitch (F0)
2. Formants
3. Spectrogram
4. Intensity
5. Voice-quality and spectral measures (HNR, jitter, shimmer, spectral moments, CPP/CPPS, duration)
6. Resampling and filtering fundamentals
7. Validation strategy

---

## 1. Pitch (F0)

### 1.1 Primary reference

Boersma, P. (1993). "Accurate short-term analysis of the fundamental frequency
and the harmonics-to-noise ratio of a sampled sound." *Proceedings of the
Institute of Phonetic Sciences (IFA), University of Amsterdam*, 17, 97–110. PDF:
<https://www.fon.hum.uva.nl/david/ba_shs/2010/Boersma_Proceedings_1993.pdf>

(Note: the printed title is the one above; it covers both the F0 method and the
HNR method of section 5.)

Praat manual, method and defaults: "Sound: To Pitch (raw autocorrelation)..."
<https://www.fon.hum.uva.nl/praat/manual/pitch_analysis_by_raw_autocorrelation.html>
(mirror: <https://praat.org/manual/pitch_analysis_by_raw_autocorrelation.html>),
and the older "Sound: To Pitch (ac)..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_Pitch__ac____.html>.

### 1.2 Core computation

The central innovation is dividing the autocorrelation of the windowed signal by
the autocorrelation of the window itself, correcting the windowing bias that
tapers the signal ACF toward zero at longer lags (Boersma 1993, eq. 9):

    r_x(τ) ≈ r_a(τ) / r_w(τ)

Per-frame procedure (Boersma 1993, §4):

1. Soft-lowpass the signal near Nyquist to suppress Hanning-transform sidelobe
   aliasing.
2. Compute the global absolute peak of the signal (used later by the unvoiced
   candidate's strength).
3. For each frame: take a segment of length `3 / pitchFloor` (pitch) or
   `6 / pitchFloor` (HNR); subtract the local mean; multiply by a Hanning window
   `w(t) = 0.5 − 0.5 cos(2πt/T)` (eq. 6); zero-pad to ~1.5× then to a power of
   two; FFT; square the magnitude spectrum; inverse FFT to obtain `r_a(τ)`;
   divide by the analytic window ACF `r_w(τ)` (eq. 8) to obtain `r_x(τ)`.
4. Find local maxima of `r_x(τ)` for lags between `1/pitchCeiling` and
   `1/pitchFloor`, located to sub-sample precision by windowed-sinc interpolation
   in the lag domain (eq. 22), refined with Brent maximisation. Sinc
   interpolation, not mere parabolic interpolation, is what recovers accurate
   peak heights (needed for HNR); parabolic interpolation alone leaves ~0.1
   sample error.

The analytic window ACF (eq. 8):

    r_w(τ) = (1 − |τ|/T)·(2/3 + (1/3)cos(2πτ/T)) + (1/(2π))·sin(2π|τ|/T)

Postscript addendum in the paper: a Gaussian window over `[−T/2, 3T/2]` gives
markedly better HNR resolution than Hanning at equal length. Praat's "Very
accurate" option implements this — Hanning at physical length `3/pitchFloor` when
off, Gaussian at `6/pitchFloor` when on.

### 1.3 Candidate generation

Up to `maxCandidates` lag–strength pairs per frame. One candidate is always the
explicit **unvoiced** candidate (frequency 0), with local strength (eq. 23):

    R_unvoiced = voicingThreshold
               + max(0, 2 − (localPeak/globalPeak) / (silenceThreshold/(1+voicingThreshold)))

Each voiced candidate at lag `τ_max` has strength (eq. 24):

    R = r_x(τ_max) − octaveCost · log2(pitchFloor · τ_max)

`octaveCost` biases the choice among equally strong peaks toward higher
frequency, countering perceived-vs-acoustic F0 mismatches in amplitude-modulated
signals.

### 1.4 Path-finder (Viterbi dynamic programming)

Across all frames, a path selects one candidate per frame; its total cost
(eq. 26) is the sum of inter-frame transition costs minus the sum of chosen
candidate strengths. The transition cost (eq. 27), with F=0 meaning unvoiced:

    transitionCost(F1, F2) =
        0                                if F1 = 0 and F2 = 0
        voicedUnvoicedCost               if exactly one of F1, F2 is 0
        octaveJumpCost · |log2(F1/F2)|   if F1 ≠ 0 and F2 ≠ 0

Boersma cites the Viterbi algorithm as described for HMMs by Van Alphen & Van
Bergem (1989). The globally cheapest path is found by dynamic programming; it
removes essentially all local octave errors on stationary signals even when up to
40% of per-frame best candidates are wrong.

### 1.5 Praat documented defaults

From "Sound: To Pitch (raw autocorrelation)..." /
"Sound: To Pitch (ac)...":

| Parameter | Default |
|---|---|
| Time step | 0.0 s (automatic; = 0.25 / pitchFloor · number-of-windows heuristic) |
| Pitch floor | 75 Hz |
| Pitch ceiling | 600 Hz |
| Max. candidates | 15 |
| Very accurate | off (Hanning, length 3/floor) |
| Silence threshold | 0.03 |
| Voicing threshold | 0.45 |
| Octave cost | 0.01 per octave |
| Octave-jump cost | 0.35 |
| Voiced/unvoiced cost | 0.14 |

Note the discrepancy: Praat's "FAQ: Pitch analysis"
(<https://www.fon.hum.uva.nl/praat/manual/FAQ__Pitch_analysis.html>) mentions in
prose a "standard 0.50" voicing and "0.09" silence threshold; the
command-reference pages (0.45 / 0.03) are authoritative for the current version.

### 1.6 Pitfalls

- Octave errors on amplitude-modulated signals if `octaveCost` too low; the path
  finder fixes most on stationary signals but not all on rapidly changing pitch.
- Voicing errors in noise: lower `voicingThreshold` (~0.25) and `silenceThreshold`
  (~0.01) for low SNR (per the FAQ). Fast pitch movement (tone languages) and
  irregular periods (pathological voice) cause spurious unvoiced frames.
- Cost: one FFT per frame (size = next power of two above 1.5× window), plus a
  fine sinc search per candidate maximum (up to N=500 interpolation samples per
  side). Fine time steps with low pitch floors make the per-frame FFT dominate.

### 1.7 Alternative F0 algorithms

**RAPT** — Talkin, D. (1995). "A Robust Algorithm for Pitch Tracking (RAPT)." In
Kleijn & Paliwal (eds.), *Speech Coding and Synthesis*, Elsevier, pp. 495–518.
(Paywalled edited volume; formulas corroborated in Azarov et al. 2013
<https://www.bsuir.by/m/12_100229_1_142365.pdf> and Stone et al. 2017, DOI
<https://doi.org/10.21437/Interspeech.2017-382>.) Uses a normalized
cross-correlation function (NCCF) with a short (~one glottal pulse) window,
normalised per lag by local energies, over a two-pass coarse (decimated) then
fine (full-rate) search, followed by a Viterbi path with the same three-term cost
structure as Boersma's (octave-jump, candidate goodness, voicing transition).
Trade-off: lower latency and cost than the FFT-ACF method, no window-ACF
correction needed (energy normalisation absorbs the bias); comparable accuracy,
often ranked just behind Praat's ac method.

**YIN** — de Cheveigné, A. & Kawahara, H. (2002). "YIN, a fundamental frequency
estimator for speech and music." *JASA* 111(4), 1917–1930. DOI
<https://doi.org/10.1121/1.1458024>. Squared-difference function
`d_t(τ) = Σ (x_j − x_{j+τ})²`, converted to a cumulative mean normalised
difference `d'_t(τ)`, absolute threshold (~0.1–0.15), first below-threshold local
minimum, parabolic refinement. Trade-off: simple, no FFT strictly required, no
built-in multi-candidate smoothing (one estimate per frame — the shortcoming
pYIN fixes). Generally less octave/voicing-robust than Praat-ac or RAPT.

**pYIN** — Mauch, S. & Dixon, S. (2014). "pYIN: A fundamental frequency estimator
using probabilistic threshold distributions." *ICASSP 2014*, 659–663. DOI
<https://doi.org/10.1109/ICASSP.2014.6853678>. PDF
<https://webspace.eecs.qmul.ac.uk/s.e.dixon/pub/2014/MauchDixon-PYIN-ICASSP2014.pdf>.
Runs YIN over a prior distribution of thresholds (Beta distributions), producing
frequency candidates with probabilities, decoded by an HMM/Viterbi over
`(pitch-bin, voiced/unvoiced)` states. Structurally a probabilistic cousin of
Boersma's DP path-finder. Trade-off: better recall and octave-error rate than
plain YIN at low added cost; needs an HMM layer of similar engineering scope to
Boersma's path-finder.

**SWIPE'** — Camacho, A. & Harris, J. G. (2008). "A sawtooth waveform inspired
pitch estimator for speech and music." *JASA* 124(3), 1638–1652. DOI
<https://doi.org/10.1121/1.2951592>. Spectral template matching against a
sawtooth-like kernel with candidate-frequency-dependent window lengths; the prime
variant sums only the first and prime-numbered harmonics to suppress subharmonic
errors. Trade-off: frequency-domain, heavier (FFT per candidate window length);
robust to certain octave errors by construction, but fragile under
low-frequency-heavy noise.

**CREPE** — Kim, J. W., Salamon, J., Li, P. & Bello, J. P. (2018). "CREPE: A
Convolutional Representation for Pitch Estimation." *ICASSP 2018*. arXiv
<https://arxiv.org/abs/1802.06182>. Code/weights
<https://github.com/marl/crepe>. Six 1-D conv layers over a 1024-sample 16 kHz
frame → 360 sigmoid outputs on a 20-cent pitch grid; estimate is a local weighted
average of activations. Trade-off: highest reported accuracy and noise robustness,
but depends on a trained weight set and synthetic-ground-truth training pipeline,
not a small set of interpretable DSP parameters. For a from-scratch MIT DSP port,
best treated as an accuracy benchmark, not an algorithm to reimplement; embedding
the `marl/crepe` weights carries separate licensing/provenance questions.

---

## 2. Formants

### 2.1 Primary references

- Burg, J. P. (1968). "A new analysis technique for time series data." NATO
  Advanced Study Institute on Signal Processing. Reprinted in Childers (ed.),
  *Modern Spectrum Analysis*, IEEE Press, 1978.
- Burg, J. P. (1975). *Maximum Entropy Spectral Analysis*. PhD thesis, Stanford
  (SEP-6). <https://sep.sites.stanford.edu/publications/theses/maximum-entropy-spectral-analysis-sep-6-1975>
- Andersen, N. (1974). "On the calculation of filter coefficients for maximum
  entropy spectral analysis." *Geophysics* 39(1), 69–72. DOI
  <https://doi.org/10.1190/1.1440413>. (This is the reference Praat's manual cites
  as "Anderson (1978)" — the Childers reprint; note the original spelling is
  Andersen and the year 1974.)
- Press, Teukolsky, Vetterling, Flannery, *Numerical Recipes*, 2nd ed. (1992),
  §13.6 (`memcof`, the Burg recursion) and §13.7. §13.6 PDF
  <https://phys.uri.edu/nigh/NumRec/bookfpdf/f13-6.pdf>.
- Markel, J. D. & Gray, A. H. Jr. (1976). *Linear Prediction of Speech*, Springer.
  <https://link.springer.com/book/10.1007/978-3-642-66286-7>
- Rabiner, L. R. & Schafer, R. W. (1978). *Digital Processing of Speech Signals*,
  Prentice-Hall. <https://archive.org/details/digitalprocessin00rabi_0>

Praat manual: "Sound: To Formant (burg)..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_Formant__burg____.html>;
"Sound: To LPC (burg)..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_LPC__burg____.html>;
"LPC: To Formant" <https://www.fon.hum.uva.nl/praat/manual/LPC__To_Formant.html>;
"Sound: Pre-emphasize (in-place)..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__Pre-emphasize__in-place____.html>.

### 2.2 Burg recursion (implementer detail)

Standard specification is the Numerical Recipes `memcof` routine. For a real
sequence of length `n`, target order `m`:

1. Seed forward-error array `f` from `data[0..n-1]` and backward-error array `b`
   from `data[1..n]`.
2. At each order `k = 1..m`, the reflection coefficient is twice the
   forward/backward cross-correlation over combined power:
   `k_ref = 2 Σ f_j b_j / Σ (f_j² + b_j²)` (sum over `j = 0..n-k-1`).
   This is Burg's defining criterion: choose each reflection coefficient to
   minimise the sum of forward and backward prediction-error powers simultaneously
   (autocorrelation-method LPC minimises only the forward error and implicitly
   windows the data).
3. Update mean-square error: `E ← E·(1 − k_ref²)`.
4. Levinson-style update of lower-order coefficients:
   `a_i ← a_i^{prev} − k_ref · a_{k-i}^{prev}` for `i = 1..k-1`.
5. Update forward/backward error arrays for the next order.

Burg is preferred for formants because it needs no windowing beyond the frame,
uses both forward and backward errors (usable information on short frames), and
gives sharper spectral peaks on short records.

### 2.3 Pre-emphasis

First-difference high-pass applied before LPC (Markel & Gray 1976), compensating
the ~−6 dB/octave source+radiation tilt of voiced speech. Praat's form
("Sound: To LPC (burg)...", "Sound: Pre-emphasize (in-place)..."):

    y_n = x_n − a·x_{n-1},  a = exp(−2π · preemphFrequency / samplingFrequency)

Default pre-emphasis-from frequency: 50 Hz (the +3 dB point of a +6 dB/octave
filter). Worked example in the manual: 48.47 Hz at 10 kHz gives a ≈ 0.97.

### 2.4 Root-solving for frequency and bandwidth

Find the complex roots of the LPC (denominator) polynomial; reflect roots outside
the unit circle back inside; keep roots with positive imaginary part. Praat uses
Laguerre's method plus root polishing (citing Press et al. 1992). For a root
`z = r·e^{jθ}` at sampling rate `fs` (standard Markel & Gray / Rabiner & Schafer
formulation):

    F = θ · fs / (2π)          (formant frequency from root angle)
    B = −(fs/π) · ln(r)        (bandwidth from root radius; closer to unit circle → narrower)

Praat discards formants below 50 Hz or above `ceiling − 50 Hz` as LPC artifacts,
copies LPC gain into the formant-frame intensity.

### 2.5 Formant ceiling and LPC order

Praat resamples the input to twice the formant ceiling before analysis (Nyquist
for the target range), and sets LPC order = 2 × (max number of formants). This is
why the ceiling parameter is speaker-sex dependent. Documented defaults
("Sound: To Formant (burg)..."):

| Parameter | Default |
|---|---|
| Formant ceiling | 5500 Hz (adult female / overall default); 5000 Hz (adult male); 8000 Hz (child) |
| Max. number of formants | 5 |
| LPC order (poles) | 2 × max formants |
| Window length (nominal / effective) | 0.025 s / 0.05 s (Gaussian, doubled) |
| Time step | 0.0 → 25% of window length |
| Pre-emphasis from | 50 Hz |

Pitfall: using the female-default 5500 Hz ceiling on male speakers inflates F1 by
~120 Hz. See Schiel & Zitzelsberger, "Evaluation of Automatic Formant Trackers,"
LREC 2018, <https://aclanthology.org/L18-1449.pdf>.

### 2.6 Formant tracking (Viterbi)

- McCandless, S. (1974). "An algorithm for automatic formant extraction using
  linear prediction spectra." *IEEE Trans. ASSP* 22(2), 135–141. DOI
  <https://doi.org/10.1109/TASSP.1974.1162559>. Peak-slotting heuristic with
  frame-to-frame continuity — the origin of continuity as an organising
  principle, predating full DP.
- Talkin, D. (1987). "Speech formant trajectory estimation using dynamic
  programming with modulated transition costs." *JASA Suppl. 1*, 82, S55. The
  canonical DP formulation.
- Xia, K. & Espy-Wilson, C. (2000). "A new strategy of formant tracking based on
  dynamic programming." *ICSLP 2000*, III 55–58. DOI
  <https://doi.org/10.21437/ICSLP.2000-476>. Full implementable cost functions:

  Viterbi recursion:
      C(t,n) = C_local(t,n) + min_m [ C_tran((t,n),(t-1,m)) + C(t-1,m) ]
  Local cost (bandwidth penalty + deviation from neutral-tract values):
      C_local(t,n) = Σ_i [ α_i B_i² + β_i |F_i − Fn_i| / Fn_i ]
      with neutral references Fn = (500, 1500, 2500, 3500) Hz and per-formant
      frequency gates.
  Transition (continuity) cost:
      C_tran = Σ_i γ_i (F_i(t) − F_i(t-1))²

  Weights α, β, γ tuned empirically per formant. Pitfall documented in the paper:
  pure frame-by-frame Viterbi can make gross errors when candidates are missing
  (merged poles), motivating segmentation + anchor-formant post-processing.

Kalman alternative — Mehta, D. D., Rudoy, D. & Wolfe, P. J. (2012).
"Kalman-based autoregressive moving average modeling and inference for formant
and antiformant tracking." *JASA* 132(3), 1732–1746. DOI
<https://doi.org/10.1121/1.4739462>. Preprint <https://arxiv.org/abs/1107.0076>.
ARMA cepstral observations (MA part tracks antiformants/nasals, which all-pole
LPC cannot) with an extended Kalman filter over a state-space model. Trade-offs
vs DP/Viterbi: continuous probabilistic smoothing rather than a best discrete
path; naturally causal/streaming (Viterbi backtracing needs the whole utterance
or a lookahead buffer); yields per-formant uncertainty; tuning is
noise-covariance/model-order selection rather than empirical cost weights. The
paper reports lower error than Praat and WaveSurfer and adds antiformant tracking.

---

## 3. Spectrogram

### 3.1 References

- Allen, J. B. & Rabiner, L. R. (1977). "A unified approach to short-time Fourier
  analysis and synthesis." *Proc. IEEE* 65(11), 1558–1564. DOI
  <https://doi.org/10.1109/PROC.1977.10770>.
- Gabor, D. (1946). "Theory of communication. Part 1." *J. IEE* 93(26), 429–441.
  DOI <https://doi.org/10.1049/ji-3-2.1946.0074>. (Gaussian minimises the joint
  time-bandwidth product.)
- Oppenheim & Schafer, *Discrete-Time Signal Processing* (3rd ed., 2010), STFT
  chapter.

Praat manual: "Sound: To Spectrogram..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_Spectrogram___.html>;
"Intro 3.2. Configuring the spectrogram"
<https://www.fon.hum.uva.nl/praat/manual/Intro_3_2__Configuring_the_spectrogram.html>;
"Advanced spectrogram settings..."
<https://www.fon.hum.uva.nl/praat/manual/Advanced_spectrogram_settings___.html>;
"Spectrogram" <https://www.fon.hum.uva.nl/praat/manual/Spectrogram.html>.

### 3.2 Computation and Gaussian window

STFT per frame; the underlying object is a power spectral density in Pa²/Hz.
Praat uses a Gaussian window because it gives no sidelobes (a pure sine renders as
a single clean line); the manual argues from sidelobes rather than from Gabor's
theorem explicitly, though Gabor 1946 is the formal justification. Cost: Gaussian
analysis uses roughly twice the samples per frame.

For a Gaussian window the −3 dB bandwidth is
`2·√(6·ln2)/(π·windowLength) = 1.2982804 / windowLength`.

Window length vs bandwidth:

- Wideband (broadband): 5 ms window → ≈260 Hz bandwidth; resolves glottal pulses
  and vertical striations, smears harmonics.
- Narrowband: 30 ms window → ≈43 Hz bandwidth; resolves individual harmonics,
  smears transients.

Praat clamps effective steps for speed: time step ≥ windowLength/(8√π), frequency
step ≥ √π/(8·windowLength). At 5 ms the frequency step never falls below 44.31 Hz.

Textbook framing of wideband/narrowband: Johnson, *Acoustic and Auditory
Phonetics* (3rd ed.); Kent & Read, *The Acoustic Analysis of Speech* (2nd ed.).

### 3.3 Display mapping

Pipeline as documented: PSD (Pa²/Hz) → dB/Hz → optional dynamic compression
(per-frame, off by default) → preemphasis → clip to
`[globalMax − dynamicRange, globalMax]` (with a 100 dB/Hz absolute ceiling when
autoscaling is off) → map to grayscale (white at floor, black at ceiling). The
exact grayscale curve (linear in dB vs perceptual) is not specified in the manual;
default to linear-in-dB.

Pre-emphasis for display: +6 dB/octave, flattening the ~−6 dB/octave vowel slope
so higher formants render as strongly as lower ones. The manual states the effect
qualitatively (bands above 1000 Hz darken, below lighten) but gives no closed-form
corner frequency; a defensible choice is `+6·log2(f/1000 Hz)` dB, display-only.

Documented defaults:

| Parameter | Default |
|---|---|
| Window length | 0.005 s (wideband) |
| Window shape | Gaussian |
| Frequency view range | 0–5000 Hz |
| Dynamic range | 50 dB |
| Maximum (autoscale off) | 100 dB/Hz (autoscale on by default) |
| Pre-emphasis | +6 dB/octave |
| Dynamic compression | 0 (off) |

---

## 4. Intensity

Praat manual: "Sound: To Intensity..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_Intensity___.html>;
"Intensity" <https://www.fon.hum.uva.nl/praat/manual/Intensity.html>;
"Intro 6.2. Configuring the intensity contour"
<https://www.fon.hum.uva.nl/praat/manual/Intro_6_2__Configuring_the_intensity_contour.html>.

### 4.1 Computation

Values are in dB SPL relative to 2×10⁻⁵ Pa (the normative auditory threshold for
a 1000 Hz sine). Standard acoustics reference: Kinsler, Frey, Coppens & Sanders,
*Fundamentals of Acoustics* (4th ed., 2000); equivalently ANSI/ASA S1.1. Level in
power terms: `L = 10·log10(⟨p²⟩ / p_ref²)`, `p_ref² = 4×10⁻¹⁰ Pa²` — identical to
`20·log10(p_rms/p_ref)`.

Algorithm (manual, verbatim): the samples are squared, then convolved with a
Gaussian analysis window (Praat implements it as a Kaiser-20 approximation,
sidelobes below −190 dB). The effective window duration is `3.2 / pitchFloor`,
guaranteeing pitch-synchronous intensity ripple below 0.00001 dB. Automatic time
step is `0.8 / pitchFloor`. "Subtract mean pressure" (DC removal, computed locally
around each point) is on by default.

A clean analytic Gaussian of the same effective duration (`3.2/pitchFloor`) is a
legitimate clean-room substitute for the Kaiser-20 approximation.

### 4.2 Pitfall

Pitch floor too high → window shorter than a few pitch periods → periodicity
leaks through as amplitude ripple; too low → genuine intensity changes (onsets,
stress) smear. Set the floor as high as the signal's lowest F0 allows for a sharp
contour. Display view range defaults to 50–100 dB (display only).

---

## 5. Voice-quality and spectral measures

### 5.1 Harmonicity / HNR

Same primary paper as pitch: Boersma (1993). HNR from the normalized
autocorrelation peak `r` (the fraction of energy that is periodic):

    HNR(dB) = 10·log10( r / (1 − r) )

Praat manual "Harmonicity"
<https://www.fon.hum.uva.nl/praat/manual/Harmonicity.html> states it as
`10·log10(periodic_energy / noise_energy)` (e.g. 99% periodic → 20 dB; 0 dB means
equal periodic and noise energy) — algebraically the same with `r` the periodic
fraction. Defaults from "Sound: To Harmonicity (ac)..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__To_Harmonicity__ac____.html>:
time step 0.01 s, pitch floor 75 Hz, silence threshold 0.1, periods per window
4.5. The cc variant uses cross-correlation instead of the window-corrected ACF.

### 5.2 Jitter (period perturbation)

Praat manual "Voice 2. Jitter"
<https://www.fon.hum.uva.nl/praat/manual/Voice_2__Jitter.html>. Over the sequence
of glottal periods `T_i`:

- **local**: average absolute difference between consecutive periods, divided by
  the average period.
- **local, absolute**: same numerator, in seconds (not divided).
- **rap** (relative average perturbation): average absolute difference between a
  period and the mean of it and its two neighbours, divided by the average period.
- **ppq5** (5-point period perturbation quotient): average absolute difference
  between a period and the mean of it and its four closest neighbours, divided by
  the average period.
- **ddp**: average absolute difference between consecutive differences of
  consecutive periods, divided by the average period (= 3 × rap).

Classic perturbation literature: Baken & Orlikoff, *Clinical Measurement of
Speech and Voice* (2nd ed., 2000); Horii (1979), "Fundamental frequency
perturbation observed in sustained phonation," *J. Speech Hear. Res.*

### 5.3 Shimmer (amplitude perturbation)

Praat manual "Voice 3. Shimmer"
<https://www.fon.hum.uva.nl/praat/manual/Voice_3__Shimmer.html>. Over per-period
peak amplitudes `A_i`:

- **local**: average absolute difference between consecutive amplitudes, divided
  by the average amplitude.
- **local, dB**: average of `|20·log10(A_{i+1}/A_i)|`.
- **apq3**: average absolute difference between an amplitude and the mean of its
  two neighbours, divided by the average amplitude.
- **apq5**: same over it and its four closest neighbours.
- **apq11**: same over it and its ten closest neighbours.
- **dda**: average absolute difference between consecutive differences of
  consecutive amplitudes (= 3 × apq3).

### 5.4 Spectral moments

Praat manual "Spectrum: Get centre of gravity..."
<https://www.fon.hum.uva.nl/praat/manual/Spectrum__Get_centre_of_gravity___.html>
and the companion "Get standard deviation / skewness / kurtosis" pages. Power-p
weighted moments of the spectrum `S(f)`:

    centre of gravity = ∫ f·|S(f)|^p df / ∫ |S(f)|^p df

with `p = 2` (power spectrum, the default), `p = 1` (magnitude), or `p = 2/3`.
Standard deviation, skewness, and kurtosis are the corresponding higher
power-weighted central moments. Primary application reference: Forrest, Weismer,
Milenkovic & Dolan (1988), "Statistical analysis of word-initial voiceless
obstruents," *JASA* 84.

### 5.5 Cepstral Peak Prominence (CPP / CPPS)

- Hillenbrand, J., Cleveland, R. A. & Erickson, R. L. (1994). "Acoustic
  correlates of breathy vocal quality." *J. Speech Hear. Res.* 37, 769–778. DOI
  <https://doi.org/10.1044/jshr.3705.769>.
- Hillenbrand, J. & Houde, R. A. (1996). "Acoustic correlates of breathy vocal
  quality: dysphonic voices and continuous speech." *J. Speech Hear. Res.* 39,
  311–321. DOI <https://doi.org/10.1044/jshr.3902.311>.
- Maryn, Y. & Weenink, D. (2015). "Objective dysphonia measures in the program
  Praat: smoothed cepstral peak prominence and acoustic voice quality index."
  *J. Voice* 29(1), 35–43. DOI <https://doi.org/10.1016/j.jvoice.2014.06.015>.

Computation: cepstrum = inverse FFT of the log magnitude spectrum. Locate the
cepstral peak at the quefrency corresponding to F0. Fit a regression line (log
magnitude vs quefrency) across the cepstrum; CPP is the dB distance from the peak
to that regression baseline at the peak's quefrency. CPPS (smoothed) averages the
cepstrum over both time (across frames) and quefrency before the peak/baseline
measurement, giving a more stable dysphonia correlate.

### 5.6 Duration

No dedicated algorithm beyond interval arithmetic on annotated boundaries: total
duration is `xmax − xmin`; a segment duration is the difference of its TextGrid
boundary times. Automatic onset/offset placement (intensity or voicing
thresholds, zero-crossing snapping) is a separate practice, not a Praat
"algorithm" per se. Praat manual "TextGrid"
<https://www.fon.hum.uva.nl/praat/manual/TextGrid.html>.

---

## 6. Resampling and filtering fundamentals

### 6.1 Sinc resampling

Ideal bandlimited reconstruction is the Whittaker–Shannon interpolation formula
`x(t) = Σ_n x[n]·sinc((t − nT)/T)`, valid when the signal was sampled above the
Nyquist rate (Nyquist–Shannon sampling theorem). Standard textbook: Oppenheim &
Schafer, *Discrete-Time Signal Processing* (interpolation and multirate
chapters). Practical windowed-sinc resampling reference: J. O. Smith, "Digital
Audio Resampling Home Page," CCRMA, Stanford,
<https://ccrma.stanford.edu/~jos/resample/>.

Praat's documented algorithm ("Sound: Resample..."
<https://www.fon.hum.uva.nl/praat/manual/Sound__Resample___.html>): a **Precision**
parameter (default 50) sets the interpolation depth in samples. Precision 1 is
linear interpolation; Precision > 1 is sin(x)/x (sinc) interpolation with depth
equal to Precision — higher is slower but more accurate. When the target rate is
below the source rate, Praat applies anti-aliasing low-pass filtering before
resampling. The exact window over the truncated sinc is not published; treat it as
a parameter to calibrate against parselmouth output if numeric parity matters.

### 6.2 Anti-aliasing

Downsampling without a prior low-pass filter folds energy above the new Nyquist
back into band (aliasing) — filter first, then decimate (Oppenheim & Schafer,
multirate chapter; Lyons, *Understanding Digital Signal Processing*). FIR
windowed-sinc low-pass design trade-off: length `N ≈ C/Δf` (inverse of transition
bandwidth), with window choice setting stopband attenuation (Hamming ≈ 53 dB,
Blackman ≈ 74 dB, Kaiser tunable via β at the cost of length).

---

## 7. Validation strategy

### 7.1 Parselmouth as a black-box oracle

Jadoul, Y., Thompson, B. & de Boer, B. (2018). "Introducing Parselmouth: A Python
interface to Praat." *Journal of Phonetics* 71, 1–15. DOI
<https://doi.org/10.1016/j.wocn.2018.07.001>. Repo
<https://github.com/YannickJadoul/Parselmouth>, GPLv3 (it wraps Praat's GPL
source). Install `pip install praat-parselmouth`.

Usage: in a separate Python process,
`snd = parselmouth.Sound(path)`, then `snd.to_pitch(...)`,
`snd.to_formant_burg(...)`, `snd.to_spectrogram(...)`, `snd.to_intensity(...)`,
`snd.to_harmonicity(...)` with parameters matched to the Rust defaults; dump the
numeric arrays (CSV/JSON) and diff against the Rust output on the same audio and
parameters.

Licensing note (reasoned analysis, not legal advice — confirm before
distributing): because Parselmouth is GPLv3, statically linking or vendoring it
would create a copyleft obligation. Invoking it **out-of-process** — a standalone
Python comparison script, never compiled into or shipped inside the Rust crate —
follows the standard "mere aggregation" / independent-process pattern used to test
against a GPL reference implementation without inheriting its license (see the FSF
GPL FAQ <https://www.gnu.org/licenses/gpl-faq.html>). Two cautions: (1) committing
Praat's exact numeric output as "expected" fixtures is more contestable than
shipping only the script that regenerates them at test time from a user-installed
Parselmouth; (2) the algorithms in this Rust project must derive from the papers
and public docs cited here, not from reading GPL source. This is the author's
reasoning, not a documented legal fact.

### 7.2 Test audio corpora

| Corpus | License | Suitability | URL |
|---|---|---|---|
| TIMIT (LDC93S1) | Not free; LDC User Agreement, membership/fee required | Not redistributable in an open test suite without a paid license | <https://catalog.ldc.upenn.edu/LDC93S1> |
| LibriSpeech | CC BY 4.0 (verified), 16 kHz | Large clean read English; good default | <https://www.openslr.org/12> |
| VCTK 0.92 | CC BY 4.0, 48 kHz | Multi-accent read speech | <https://datashare.ed.ac.uk/handle/10283/3443> |
| CMU ARCTIC | Permissive (CMU/BSD-like) | Single-speaker phonetically balanced, 16 kHz | <http://www.festvox.org/cmu_arctic/> |
| Mozilla Common Voice | CC0 | Crowdsourced, variable quality — robustness testing | <https://commonvoice.mozilla.org/> |
| Buckeye | Registration + signed agreement (not fully free) | Rich phonetic transcription; licensing overhead for CI | <https://buckeyecorpus.osu.edu/> |
| Free ST American English (SLR45) | CC BY-NC-ND 4.0 (NC/ND — not bundleable in MIT repo) | Internal testing only | <https://www.openslr.org/45/> |

Ground-truth F0 corpora (laryngograph/EGG-derived reference, for validating pitch
against truth rather than only against Praat):

- Keele Pitch Extraction Reference Database — Plante, Meyer & Ainsworth (1995),
  *EUROSPEECH '95*. Reference F0 + voicing labels from a laryngograph. Confirm
  current hosting/licensing before use.
- PTDB-TUG — Pirker, Wohlmayr, Petrik & Pernkopf (2011), *Interspeech 2011*,
  <https://www.spsc.tugraz.at/databases-and-tools/ptdb-tug-pitch-tracking-database-from-graz-university-of-technology.html>.
  Synchronised laryngograph reference. Sentence content is TIMIT-derived — check
  its redistribution terms.

Recommended default for CI: LibriSpeech or CMU ARCTIC clips (permissive, clean
read speech), with a PTDB-TUG/Keele subset for pitch-against-ground-truth checks
if their licenses permit.

### 7.3 Tolerance guidance

Benchmarking literature: Rabiner, Cheng, Rosenberg & McGonegal (1976), "A
comparative performance study of several pitch detection algorithms," *IEEE Trans.
ASSP* 24(5), 399–418; Babacan, Drugman, d'Alessandro, Henrich & Dutoit (2013), "A
comparative study of pitch extraction algorithms on a large variety of singing
sounds," *ICASSP 2013*, <https://arxiv.org/abs/1912.12609>. Standard metrics:

- **Gross Pitch Error (GPE)**: fraction of voiced frames with F0 deviating >20%
  from reference (captures octave errors).
- **Mean Fine Pitch Error (MFPE)**: mean deviation (cents or Hz) over
  non-gross-error voiced frames.

Proposed tolerance bands when diffing the Rust implementation against Parselmouth
(tune empirically; both implement the same nominal formulas so agreement should be
tight where voicing/octave decisions match):

| Measure | Suggested tolerance |
|---|---|
| F0, both voiced at same octave | ~0.5–1% relative (≈5–17 cents) |
| F0 gross error flag | >20% relative (GPE) — investigate separately |
| Voicing decision | majority frame match on clean speech; report boundary frames separately |
| Formants F1–F3 (clear vowels) | ~20–50 Hz absolute or ~2–3% relative, whichever larger |
| Intensity | ~0.5–1 dB |
| HNR | ~0.5–1 dB well-voiced; larger near voicing boundaries |

The dominant source of large disagreement between independent pitch trackers is
voicing-decision mismatch and octave jumps, not floating-point differences.
Separate gross errors (voicing mismatch, octave) from fine errors (small numeric
deviation where both agree voiced at the same octave), as GPE/MFPE do — a single
blended RMS metric would be dominated by rare gross errors and mask core numeric
correctness.

---

## Open items flagged for the implementer

1. Spectrogram display pre-emphasis has no closed-form corner frequency in the
   manual — pick `+6·log2(f/1000 Hz)` dB.
2. Praat's intensity window is a Kaiser-20 approximation to Gaussian; an analytic
   Gaussian of effective duration `3.2/pitchFloor` is a documented-equivalent
   substitute.
3. The exact spectrogram grayscale curve is unspecified — default linear-in-dB.
4. RAPT (Talkin 1995) is secondary-sourced here (paywalled volume); its formulas
   are corroborated but not read from the primary text.
5. The GPL out-of-process reasoning in §7.1 is analysis, not documented legal
   fact — confirm before shipping Praat-derived test fixtures.
6. Re-verify Buckeye, Keele, and PTDB-TUG current licensing/hosting before use.
