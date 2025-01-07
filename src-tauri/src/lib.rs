// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use hound::WavReader;
use serde::Serialize;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, Clone, Serialize)]
struct WavInfo {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    length: u64,
    duration: f32,
    rms: f64,
}

#[tauri::command]
fn read_wav(buffer: Vec<u8>) -> WavInfo {
    let mut reader = WavReader::new(buffer.as_slice()).unwrap();

    // Collect necessary metadata first
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let bits_per_sample = spec.bits_per_sample;
    let length = reader.len();
    let duration = reader.duration();

    // Then process samples
    let sqr_sum = reader.samples::<i16>().fold(0.0, |acc, s| {
        let sample = s.unwrap() as f64;
        acc + sample * sample
    });

    // Format the final string
    WavInfo {
        sample_rate,
        channels,
        bits_per_sample,
        length: length as u64,
        duration: duration as f32,
        rms: (sqr_sum / length as f64).sqrt(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .invoke_handler(tauri::generate_handler![read_wav])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
