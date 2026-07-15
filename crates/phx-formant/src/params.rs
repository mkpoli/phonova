use phx_dsp::FrameGrid;

const DEFAULT_CEILING_HZ: f64 = 5500.0;
const DEFAULT_MAX_FORMANTS: usize = 5;
const DEFAULT_WINDOW_LENGTH: f64 = 0.025;
const DEFAULT_PREEMPHASIS_FROM_HZ: f64 = 50.0;

/// Burg formant analysis parameters.
///
/// Defaults follow Praat's documented "Sound: To Formant (burg)..." values:
/// formant ceiling 5500 Hz, maximum formants 5, Gaussian effective window
/// length 25 ms, time step `0.0` meaning 25% of the window length, and
/// pre-emphasis from 50 Hz. The 5500 Hz ceiling default is Praat's
/// overall/adult-female default; Praat's documented adult-male default is
/// 5000 Hz and using the female default on male speakers inflates F1 by
/// roughly 120 Hz (Schiel & Zitzelsberger, LREC 2018). Callers analysing male
/// or child speech should override `ceiling_hz` explicitly (5000 Hz adult male,
/// 8000 Hz child, per the Praat manual).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormantParams {
    /// Formant ceiling in hertz; Praat's overall/adult-female default is 5500 Hz.
    pub ceiling_hz: f64,
    /// Maximum number of formants; Praat's default is 5.
    pub max_formants: usize,
    /// Gaussian effective window length in seconds; Praat's default is 0.025 s.
    pub window_length: f64,
    /// Frame hop in seconds; `None` follows Praat's `0.0` meaning 25% of the window length.
    pub time_step: Option<f64>,
    /// Pre-emphasis corner in hertz; Praat's default is 50 Hz.
    pub preemphasis_from_hz: f64,
}

impl Default for FormantParams {
    fn default() -> Self {
        Self {
            ceiling_hz: DEFAULT_CEILING_HZ,
            max_formants: DEFAULT_MAX_FORMANTS,
            window_length: DEFAULT_WINDOW_LENGTH,
            time_step: None,
            preemphasis_from_hz: DEFAULT_PREEMPHASIS_FROM_HZ,
        }
    }
}

/// Returns the analysis hop in seconds after Praat's `0.0` time-step rule.
#[must_use]
pub fn effective_time_step(params: &FormantParams) -> f64 {
    validate_params(params);
    params.time_step.unwrap_or(0.25 * params.window_length)
}

/// Builds the frame grid used by formant analysis.
///
/// The grid margin uses the physical Gaussian window length, twice the
/// `window_length` parameter. Praat's manual ("Sound: To Formant (burg)...")
/// states the actual analysis window "is twice this value, because Praat uses
/// a Gaussian-like analysis window", and the frame count subtracts that actual
/// window from the signal duration before dividing by the step. `duration`
/// must be the analyzed signal's sample count times its sampling period, the
/// same discrete duration Praat measures frames over.
#[must_use]
pub fn frame_grid(duration: f64, params: &FormantParams) -> FrameGrid {
    FrameGrid::new(
        duration,
        2.0 * params.window_length,
        effective_time_step(params),
    )
}

pub(crate) fn validate_params(params: &FormantParams) {
    assert!(
        params.ceiling_hz.is_finite() && params.ceiling_hz > 100.0,
        "ceiling_hz must be finite and greater than 100 Hz"
    );
    assert!(params.max_formants > 0, "max_formants must be positive");
    assert!(
        params.window_length.is_finite() && params.window_length > 0.0,
        "window_length must be finite and positive"
    );
    if let Some(time_step) = params.time_step {
        assert!(
            time_step.is_finite() && time_step > 0.0,
            "time_step must be finite and positive"
        );
    }
    assert!(
        params.preemphasis_from_hz.is_finite(),
        "preemphasis_from_hz must be finite"
    );
}
