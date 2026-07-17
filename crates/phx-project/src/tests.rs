//! Round-trip, recovery, re-link, and error-path coverage.

use crate::*;
use phx_annot::{Annotation, TierRelation};
use phx_audio::{Audio, BitDepth};
use serde_json::json;
use std::io::Write;

/// Builds deterministic WAV bytes for a tone of the given seed length.
fn wav_bytes(seed: usize) -> Vec<u8> {
    let samples: Vec<f32> = (0..(64 + seed))
        .map(|i| ((i as f32) * 0.01 * (seed as f32 + 1.0)).sin() * 0.5)
        .collect();
    let audio = Audio::new(vec![samples], 16_000.0).expect("valid audio");
    audio.to_wav_bytes(BitDepth::Pcm16).expect("wav encode")
}

/// Builds a small annotation with one interval tier.
fn sample_annotation() -> Annotation {
    let mut annotation = Annotation::new(0.0, 1.0).expect("valid domain");
    annotation
        .add_interval_tier("words", TierRelation::Independent)
        .expect("interval tier");
    annotation
        .add_point_tier(
            "events",
            vec![(0.25, "onset".to_string()), (0.75, "offset".to_string())],
            TierRelation::Independent,
        )
        .expect("point tier");
    annotation
}

/// Builds a populated project fixture for round-trip checks.
fn sample_project() -> Project {
    let media0 =
        MediaRef::from_wav_bytes(MediaId::new(1), "audio/a.wav", &wav_bytes(0)).expect("media ref");
    let media1 =
        MediaRef::from_wav_bytes(MediaId::new(2), "audio/b.wav", &wav_bytes(7)).expect("media ref");

    let mut project = Project::new("Fieldwork 2026");
    project.saved_at = 1_000;
    project.media = vec![media0, media1];
    project
        .annotations
        .insert(MediaId::new(1), sample_annotation());
    project.profiles = vec![
        Profile {
            name: "female speaker".to_string(),
            params: json!({ "pitch_floor": 100.0, "pitch_ceiling": 500.0 }),
        },
        Profile {
            name: "male speaker".to_string(),
            params: json!({ "pitch_floor": 75.0, "pitch_ceiling": 300.0 }),
        },
    ];
    project.view = json!({ "zoom": [0.0, 1.0], "palette": "grayscale", "tracks": ["pitch"] });
    project
}

#[test]
fn round_trip_preserves_structure() {
    let project = sample_project();
    let bytes = save(&project);
    let loaded = load(&bytes).expect("load");
    assert_eq!(project, loaded);
}

#[test]
fn round_trip_default_project() {
    let project = Project::new("empty");
    let loaded = load(&save(&project)).expect("load");
    assert_eq!(project, loaded);
}

#[test]
fn round_trip_is_deterministic() {
    let project = sample_project();
    assert_eq!(save(&project), save(&project));
}

#[test]
fn media_hash_matches_bytes() {
    let bytes = wav_bytes(3);
    let media = MediaRef::from_wav_bytes(MediaId::new(1), "x.wav", &bytes).expect("media");
    assert_eq!(media.hash, ContentHash::of(&bytes));
    assert!(media.duration > 0.0);
    assert_eq!(media.sample_rate, 16_000.0);
    assert_eq!(media.channels, 1);
}

#[test]
fn content_hash_hex_round_trips() {
    let hash = ContentHash::of(b"phonia");
    let hex = hash.to_hex();
    assert_eq!(hex.len(), 64);
    assert_eq!(ContentHash::from_hex(&hex).unwrap(), hash);
    assert!(ContentHash::from_hex("zz").is_err());
    assert!(ContentHash::from_hex(&"g".repeat(64)).is_err());
}

#[test]
fn corrupted_container_is_typed() {
    let err = load(b"not a zip archive at all").unwrap_err();
    assert!(matches!(err, ProjectError::Container(_)));
}

#[test]
fn missing_manifest_is_typed() {
    // A valid, empty ZIP with no manifest entry.
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("unrelated.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"x").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let err = load(&bytes).unwrap_err();
    assert!(matches!(err, ProjectError::MissingEntry(entry) if entry == "manifest.json"));
}

/// Builds a container whose manifest carries an arbitrary version and tag.
fn container_with_manifest(manifest: serde_json::Value) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    writer.finish().unwrap().into_inner()
}

#[test]
fn newer_version_is_rejected() {
    let bytes = container_with_manifest(json!({
        "format": FORMAT_TAG,
        "version": FORMAT_VERSION + 1,
        "saved_at": 0,
        "name": "future",
        "media": [],
    }));
    let err = load(&bytes).unwrap_err();
    assert!(matches!(
        err,
        ProjectError::UnsupportedVersion { found, supported }
            if found == FORMAT_VERSION + 1 && supported == FORMAT_VERSION
    ));
}

#[test]
fn unknown_format_is_rejected() {
    let bytes = container_with_manifest(json!({
        "format": "some-other-tool",
        "version": 1,
        "saved_at": 0,
        "name": "alien",
        "media": [],
    }));
    let err = load(&bytes).unwrap_err();
    assert!(matches!(err, ProjectError::UnknownFormat(tag) if tag == "some-other-tool"));
}

#[test]
fn malformed_entry_is_typed() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    writer.start_file("manifest.json", opts).unwrap();
    writer
        .write_all(
            serde_json::to_string(&json!({
                "format": FORMAT_TAG,
                "version": 1,
                "saved_at": 0,
                "name": "n",
                "media": [],
            }))
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
    writer.start_file("profiles.json", opts).unwrap();
    writer.write_all(b"{ this is not valid json").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let err = load(&bytes).unwrap_err();
    assert!(matches!(err, ProjectError::MalformedEntry { entry, .. } if entry == "profiles.json"));
}

#[test]
fn autosave_debounce_timing() {
    let mut saver = Autosaver::new("p.phxproj").with_timing(2_000, 15_000);
    assert!(!saver.due(0));
    saver.note_change(1_000);
    assert!(saver.is_pending());
    assert!(!saver.due(1_500)); // still within debounce, edits recent
    assert!(saver.due(3_000)); // quiet for 2s
    // Continuous edits still checkpoint via max_wait.
    let mut busy = Autosaver::new("p.phxproj").with_timing(2_000, 15_000);
    for t in (0..=15_000).step_by(500) {
        busy.note_change(t);
    }
    assert!(busy.due(15_000));
}

#[test]
fn kill_and_recover_restores_unsaved_state() {
    let store = MemStore::new();
    let path = "projects/session.phxproj";

    // Saved baseline on disk.
    let saved = {
        let mut p = sample_project();
        p.saved_at = 1_000;
        p
    };
    store.write(path, &save(&saved)).unwrap();

    // Unsaved edits captured only by autosave, then a "crash".
    let edited = {
        let mut p = saved.clone();
        p.name = "renamed after edits".to_string();
        p.annotations.insert(MediaId::new(2), sample_annotation());
        p
    };
    let mut saver = Autosaver::new(path);
    saver.note_change(4_000);
    assert!(saver.due(7_000));
    saver.flush(&store, &edited, 7_000).unwrap();
    drop(saver); // engine dropped without an explicit save

    // On reopen, recovery finds the newer sidecar.
    match detect_recovery(&store, path).unwrap() {
        Recovery::Recoverable(recovered) => {
            let mut expected = edited.clone();
            expected.saved_at = 7_000; // stamped by the flush
            assert_eq!(*recovered, expected);
        }
        other => panic!("expected recoverable, got {other:?}"),
    }
}

#[test]
fn recovery_none_and_up_to_date() {
    let store = MemStore::new();
    let path = "s.phxproj";
    let mut project = sample_project();
    project.saved_at = 5_000;
    store.write(path, &save(&project)).unwrap();

    assert_eq!(detect_recovery(&store, path).unwrap(), Recovery::None);

    // A sidecar no newer than the file is up to date.
    let mut saver = Autosaver::new(path);
    saver.note_change(1_000);
    saver.flush(&store, &project, 3_000).unwrap(); // older than file's 5_000
    assert_eq!(detect_recovery(&store, path).unwrap(), Recovery::UpToDate);
}

#[test]
fn discard_removes_sidecar() {
    let store = MemStore::new();
    let path = "s.phxproj";
    let mut saver = Autosaver::new(path);
    saver.note_change(1_000);
    saver.flush(&store, &sample_project(), 2_000).unwrap();
    assert!(store.exists(saver.sidecar_path()));
    saver.discard(&store).unwrap();
    assert!(!store.exists(saver.sidecar_path()));
    assert!(!saver.is_pending());
}

#[test]
fn relink_present_media_resolves_in_place() {
    let store = MemStore::new();
    let bytes = wav_bytes(1);
    store.write("proj/audio/a.wav", &bytes).unwrap();

    let mut project = Project::new("p");
    project.media = vec![MediaRef::from_wav_bytes(MediaId::new(1), "audio/a.wav", &bytes).unwrap()];

    let outcomes = resolve_media(&mut project, "proj/session.phxproj", &[], &store).unwrap();
    assert_eq!(outcomes, vec![MediaResolution::Present(MediaId::new(1))]);
    assert_eq!(project.media[0].relative_path, "audio/a.wav");
}

#[test]
fn relink_moved_media_by_hash() {
    let store = MemStore::new();
    let bytes = wav_bytes(2);
    // The file is not where the reference points; it now lives in a sibling dir.
    store.write("proj/recovered/a.wav", &bytes).unwrap();

    let mut project = Project::new("p");
    project.media = vec![MediaRef::from_wav_bytes(MediaId::new(1), "audio/a.wav", &bytes).unwrap()];

    let outcomes = resolve_media(
        &mut project,
        "proj/session.phxproj",
        &["proj/recovered"],
        &store,
    )
    .unwrap();
    assert_eq!(
        outcomes,
        vec![MediaResolution::Relinked {
            media: MediaId::new(1),
            from: "audio/a.wav".to_string(),
            to: "recovered/a.wav".to_string(),
        }]
    );
    assert_eq!(project.media[0].relative_path, "recovered/a.wav");
}

#[test]
fn relink_missing_media_reports_gap() {
    let store = MemStore::new();
    // Nothing that hashes to the reference exists anywhere.
    store
        .write("proj/other/unrelated.wav", &wav_bytes(99))
        .unwrap();

    let mut project = Project::new("p");
    let media = MediaRef::from_wav_bytes(MediaId::new(1), "audio/gone.wav", &wav_bytes(2)).unwrap();
    let expected_hash = media.hash;
    project.media = vec![media];

    let gaps = resolve_media(
        &mut project,
        "proj/session.phxproj",
        &["proj/other"],
        &store,
    )
    .unwrap_err();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].media, MediaId::new(1));
    assert_eq!(gaps[0].expected_hash, expected_hash);
    assert!(gaps[0].candidates.is_empty());
}

#[test]
fn relink_ambiguous_lists_candidates() {
    let store = MemStore::new();
    let bytes = wav_bytes(4);
    // Two byte-identical copies: cannot choose without the user.
    store.write("proj/one/a.wav", &bytes).unwrap();
    store.write("proj/two/b.wav", &bytes).unwrap();

    let mut project = Project::new("p");
    project.media = vec![MediaRef::from_wav_bytes(MediaId::new(1), "audio/a.wav", &bytes).unwrap()];

    let gaps = resolve_media(
        &mut project,
        "proj/session.phxproj",
        &["proj/one", "proj/two"],
        &store,
    )
    .unwrap_err();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].candidates.len(), 2);
    // Reference left untouched until the user picks.
    assert_eq!(project.media[0].relative_path, "audio/a.wav");
}

#[test]
fn memstore_lists_immediate_entries_only() {
    let store = MemStore::new();
    store.write("dir/a.wav", b"1").unwrap();
    store.write("dir/b.wav", b"2").unwrap();
    store.write("dir/nested/c.wav", b"3").unwrap();
    store.write("top.wav", b"4").unwrap();

    let mut listed = store.list_dir("dir").unwrap();
    listed.sort();
    assert_eq!(listed, vec!["a.wav".to_string(), "b.wav".to_string()]);

    let root = store.list_dir("").unwrap();
    assert_eq!(root, vec!["top.wav".to_string()]);
}
