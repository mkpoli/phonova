//! Pre-emphasis, Burg recursion, polynomial roots → frequency/bandwidth,
//! Xia–Espy-Wilson DP tracking.
#![warn(missing_docs)]

mod burg;
mod params;
mod roots;
mod track;
mod types;

pub use params::{FormantParams, effective_time_step, frame_grid};
pub use track::{TrackingRefs, formant_track, track_smoothed};
pub use types::{FormantFrame, FormantPoint, FormantTrack};

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;
    use std::fs;
    use std::path::Path;

    use phx_audio::Audio;

    use super::*;

    #[test]
    fn synthetic_vowel_recovers_first_three_formants() {
        let sample_rate = 11_000.0;
        let targets = [(850.0, 50.0), (1220.0, 50.0), (2600.0, 120.0)];
        let audio = synthetic_vowel(sample_rate, 0.5, 90.0, &targets);
        let params = FormantParams::default();
        let track = formant_track(audio.slice_samples(0..audio.frames()), &params);
        let mid = best_synthetic_frame(&track, &targets).expect("non-empty track");

        println!("synthetic frame {:.6}s: {:?}", mid.time, mid.formants);
        assert!(mid.formants.len() >= 3, "formants {:?}", mid.formants);
        for (index, &(frequency, bandwidth)) in targets.iter().enumerate() {
            let actual = nearest_frequency(&mid.formants, frequency).expect("matching formant");
            assert!(
                (actual.frequency - frequency).abs() <= 0.02 * frequency,
                "F{} recovered {} Hz, expected {frequency} Hz",
                index + 1,
                actual.frequency
            );
            assert!(
                (actual.bandwidth - bandwidth).abs() <= 0.20 * bandwidth,
                "B{} recovered {} Hz, expected {bandwidth} Hz",
                index + 1,
                actual.bandwidth
            );
        }
    }

    #[test]
    #[ignore = "ad-hoc fixture check for the documented ceiling-direction pitfall"]
    fn arctic_bdl_ceiling_direction_check() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|crates_dir| crates_dir.parent())
            .expect("crate manifest lives under the workspace crates directory")
            .join("tests/fixtures/audio/arctic_bdl_a0001.wav");
        let bytes = fs::read(fixture).unwrap();
        let audio = Audio::from_wav_bytes(&bytes).unwrap();
        let male = formant_track(
            audio.slice_samples(0..audio.frames()),
            &FormantParams {
                ceiling_hz: 5000.0,
                ..FormantParams::default()
            },
        );
        let female_default = formant_track(
            audio.slice_samples(0..audio.frames()),
            &FormantParams {
                ceiling_hz: 5500.0,
                ..FormantParams::default()
            },
        );
        let male_f1 = mean_f1(&male);
        let female_default_f1 = mean_f1(&female_default);
        println!("mean F1 ceiling 5000 Hz: {male_f1:.3}");
        println!("mean F1 ceiling 5500 Hz: {female_default_f1:.3}");
        println!(
            "difference 5500-5000 Hz: {:.3}",
            female_default_f1 - male_f1
        );
        assert!(female_default_f1 > male_f1);
    }

    fn synthetic_vowel(
        sample_rate: f64,
        duration: f64,
        fundamental: f64,
        targets: &[(f64, f64)],
    ) -> Audio {
        let frames = (sample_rate * duration).round() as usize;
        let period = (sample_rate / fundamental).round() as usize;
        let mut signal = (0..frames)
            .map(|i| if i.is_multiple_of(period) { 1.0 } else { 0.0 })
            .collect::<Vec<_>>();

        for &(frequency, bandwidth) in targets {
            signal = resonator(signal, sample_rate, frequency, bandwidth);
        }

        let peak = signal
            .iter()
            .fold(0.0_f64, |acc, value| acc.max(value.abs()))
            .max(1.0);
        let samples = signal
            .into_iter()
            .map(|sample| (0.5 * sample / peak) as f32)
            .collect();
        Audio::new(vec![samples], sample_rate).unwrap()
    }

    fn resonator(input: Vec<f64>, sample_rate: f64, frequency: f64, bandwidth: f64) -> Vec<f64> {
        let radius = (-PI * bandwidth / sample_rate).exp();
        let theta = 2.0 * PI * frequency / sample_rate;
        let c1 = 2.0 * radius * theta.cos();
        let c2 = -(radius * radius);
        let mut y1 = 0.0;
        let mut y2 = 0.0;
        input
            .into_iter()
            .map(|x| {
                let y = c1 * y1 + c2 * y2 + x;
                y2 = y1;
                y1 = y;
                y
            })
            .collect()
    }

    fn best_synthetic_frame<'a>(
        track: &'a FormantTrack,
        targets: &[(f64, f64)],
    ) -> Option<&'a FormantFrame> {
        track
            .frames
            .iter()
            .filter(|frame| frame.time > 0.1 && frame.time < 0.4)
            .min_by(|left, right| {
                frame_error(left, targets).total_cmp(&frame_error(right, targets))
            })
    }

    fn frame_error(frame: &FormantFrame, targets: &[(f64, f64)]) -> f64 {
        targets
            .iter()
            .map(|&(frequency, bandwidth)| {
                nearest_frequency(&frame.formants, frequency).map_or(f64::INFINITY, |actual| {
                    ((actual.frequency - frequency).abs() / frequency)
                        .max((actual.bandwidth - bandwidth).abs() / bandwidth)
                })
            })
            .fold(0.0, f64::max)
    }

    fn nearest_frequency(formants: &[FormantPoint], frequency: f64) -> Option<FormantPoint> {
        formants.iter().copied().min_by(|left, right| {
            (left.frequency - frequency)
                .abs()
                .total_cmp(&(right.frequency - frequency).abs())
        })
    }

    fn mean_f1(track: &FormantTrack) -> f64 {
        let values = track
            .frames
            .iter()
            .filter_map(|frame| frame.formants.first().map(|formant| formant.frequency))
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        values.iter().sum::<f64>() / values.len() as f64
    }
}
