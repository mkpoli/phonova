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
mod platform_cmds;
mod playback_cmds;
mod project_cmds;
mod state;

use state::AppState;
use tauri::{Emitter, Manager};

/// Event the frontend listens for after launch: a second instance (Windows,
/// Linux) or a macOS open-file event queued more paths in
/// [`AppState::pending_opens`] for `take_pending_opens` to drain.
const FILES_OPENED_EVENT: &str = "phonix://files-opened";

/// Builds and runs the desktop application.
///
/// # Panics
/// Panics if the app data directory cannot be resolved or the Tauri runtime
/// fails to start — both are unrecoverable at launch.
pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin registered (see the plugin's own docs):
        // Windows and Linux hand a file-association open-with to a *second*
        // process, which this forwards to the one already running instead of
        // opening a second window onto the same project store.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let state = app.state::<AppState>();
            platform_cmds::record_pending_opens(&state, argv.into_iter().skip(1));
            let _ = app.emit(FILES_OPENED_EVENT, ());
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir")
                .join("projects");
            app.manage(AppState::new(root));
            // The paths this launch itself was opened with (first instance on
            // Windows/Linux, or a macOS launch with a file argument).
            let state = app.state::<AppState>();
            platform_cmds::record_pending_opens(&state, std::env::args().skip(1));
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
            engine_cmds::journal_head_id,
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
            platform_cmds::take_pending_opens,
            platform_cmds::read_external_file,
            platform_cmds::dmabuf_advisory,
        ])
        .build(tauri::generate_context!())
        .expect("build the Phonia desktop application");

    // `Builder::run` doesn't expose the run-loop event stream `Opened` arrives
    // on, so open-with on macOS (double-click, or `Open With` while already
    // running) needs the lower-level `build` + `run(callback)` split.
    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = event {
            let state = app_handle.state::<AppState>();
            let paths = urls
                .into_iter()
                .filter_map(|url| url.to_file_path().ok())
                .map(|path| path.to_string_lossy().into_owned());
            platform_cmds::record_pending_opens(&state, paths);
            let _ = app_handle.emit(FILES_OPENED_EVENT, ());
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app_handle, event);
    });
}
