/// Parameters for Boersma-style raw-autocorrelation pitch analysis.
///
/// Defaults follow Praat's documented "Sound: To Pitch (raw autocorrelation)"
/// command reference: automatic time step, 75 Hz floor, 600 Hz ceiling, 15
/// candidates, normal accuracy, silence threshold 0.03, voicing threshold 0.45,
/// octave cost 0.01, octave-jump cost 0.35, and voiced/unvoiced cost 0.14. The
/// Praat FAQ text mentions older 0.09/0.50 threshold values; the command
/// reference is authoritative here. Automatic time step is resolved as
/// `0.25 / floor_hz`, the simple part of Praat's documented heuristic.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchParams {
    /// Frame hop in seconds, or `None` for `0.25 / floor_hz`.
    pub time_step: Option<f64>,
    /// Lowest voiced frequency considered, in hertz.
    pub floor_hz: f64,
    /// Highest voiced frequency considered, in hertz.
    pub ceiling_hz: f64,
    /// Maximum candidates retained per frame, including the unvoiced candidate.
    pub max_candidates: usize,
    /// Uses the longer Gaussian window when `true`; otherwise uses Hanning.
    pub very_accurate: bool,
    /// Silence threshold in the unvoiced-candidate strength formula.
    pub silence_threshold: f64,
    /// Voicing threshold in the unvoiced-candidate strength formula.
    pub voicing_threshold: f64,
    /// Per-octave bias applied to voiced candidate strength.
    pub octave_cost: f64,
    /// Per-octave cost for voiced-to-voiced frequency jumps.
    pub octave_jump_cost: f64,
    /// Cost for transitions between voiced and unvoiced candidates.
    pub voiced_unvoiced_cost: f64,
}

impl Default for PitchParams {
    fn default() -> Self {
        Self {
            time_step: None,
            floor_hz: 75.0,
            ceiling_hz: 600.0,
            max_candidates: 15,
            very_accurate: false,
            silence_threshold: 0.03,
            voicing_threshold: 0.45,
            octave_cost: 0.01,
            octave_jump_cost: 0.35,
            voiced_unvoiced_cost: 0.14,
        }
    }
}

impl PitchParams {
    pub(crate) fn resolved_step(&self) -> Option<f64> {
        let step = self.time_step.unwrap_or(0.25 / self.floor_hz);
        (step.is_finite() && step > 0.0).then_some(step)
    }

    pub(crate) fn window_seconds(&self) -> f64 {
        if self.very_accurate {
            6.0 / self.floor_hz
        } else {
            3.0 / self.floor_hz
        }
    }

    pub(crate) fn is_valid_for_analysis(&self) -> bool {
        self.floor_hz.is_finite()
            && self.ceiling_hz.is_finite()
            && self.floor_hz > 0.0
            && self.ceiling_hz > self.floor_hz
            && self.max_candidates > 0
            && self.resolved_step().is_some()
    }
}
