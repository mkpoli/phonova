//! The Phonia desktop shell: the shared Svelte UI in a Tauri webview over a
//! native [`phx_engine`] core.
//!
//! The engine sits behind a `Mutex<Engine>` in Tauri-managed state, and every
//! `#[tauri::command]` mirrors one method of the web worker protocol, so the
//! `TauriCoreClient` in the frontend implements the same `CoreClient` interface
//! the `WasmCoreClient` does — one interface, two transports. Project storage is
//! the real filesystem under the app data directory; the dialog plugin only
//! hands the frontend paths.

mod engine_cmds;
mod playback_cmds;
mod project_cmds;
mod state;

use state::AppState;
use tauri::Manager;

/// Builds and runs the desktop application.
///
/// # Panics
/// Panics if the app data directory cannot be resolved or the Tauri runtime
/// fails to start — both are unrecoverable at launch.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir")
                .join("projects");
            app.manage(AppState::new(root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            project_cmds::import_audio,
            project_cmds::open_audio_streaming,
            project_cmds::save_project_container,
            project_cmds::load_project_container,
            project_cmds::rename_project_container,
            project_cmds::fs_read,
            project_cmds::fs_write,
            project_cmds::fs_exists,
            project_cmds::fs_remove,
            project_cmds::fs_list,
            project_cmds::fs_list_dirs,
            project_cmds::fs_remove_dir,
            project_cmds::fs_copy_dir,
            engine_cmds::waveform_slice,
            engine_cmds::spectrogram_tile,
            engine_cmds::pitch_track,
            engine_cmds::pitch_track_span,
            engine_cmds::formant_track,
            engine_cmds::intensity_track,
            engine_cmds::band_energy,
            engine_cmds::selection_readout,
            engine_cmds::formant_span_means,
            engine_cmds::voice_report,
            engine_cmds::create_annotation,
            engine_cmds::add_interval_tier,
            engine_cmds::add_point_tier,
            engine_cmds::rename_audio,
            engine_cmds::detach_audio,
            engine_cmds::remove_tier,
            engine_cmds::insert_boundary,
            engine_cmds::move_boundary,
            engine_cmds::remove_boundary,
            engine_cmds::set_interval_label,
            engine_cmds::set_point_label,
            engine_cmds::undo,
            engine_cmds::redo,
            engine_cmds::undo_depth,
            engine_cmds::redo_depth,
            engine_cmds::state_hash,
            engine_cmds::list_annotations,
            engine_cmds::annotation_tiers,
            engine_cmds::intervals_in_range,
            engine_cmds::points_in_range,
            engine_cmds::search_labels,
            engine_cmds::import_text_grid,
            engine_cmds::export_text_grid,
            engine_cmds::annotation_json,
            engine_cmds::attach_annotation_json,
            engine_cmds::build_figure,
            engine_cmds::render_figure_svg,
            engine_cmds::export_figure,
            playback_cmds::playback_available,
            playback_cmds::playback_load,
            playback_cmds::playback_play,
            playback_cmds::playback_play_range,
            playback_cmds::playback_pause,
            playback_cmds::playback_stop,
            playback_cmds::playback_seek,
            playback_cmds::playback_status,
        ])
        .run(tauri::generate_context!())
        .expect("run the Phonia desktop application");
}
