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
    project.normalize_library();
    project
}

/// Builds a v2 project carrying nested groups and full metadata.
fn sample_project_v2() -> Project {
    let mut project = sample_project();
    project.description = "Two speakers recorded on site.".to_string();
    project.authors = vec!["Fieldworker A".to_string(), "Fieldworker B".to_string()];
    project.tags = vec!["fieldwork".to_string(), "2026".to_string()];

    if let Some(m) = project.media.iter_mut().find(|m| m.id == MediaId::new(1)) {
        m.description = "Elicitation, morning session.".to_string();
        m.authors = vec!["Speaker A".to_string()];
        m.tags = vec!["elicitation".to_string(), "vowels".to_string()];
    }

    // A nested tree: recording 1 sits inside a group, recording 2 at the root.
    let inner = Group {
        id: GroupId::new(20),
        name: "Session 1".to_string(),
        children: vec![LibraryNode::Media(MediaId::new(1))],
    };
    let outer = Group {
        id: GroupId::new(10),
        name: "Speaker A".to_string(),
        children: vec![LibraryNode::Group(inner)],
    };
    project.groups = vec![
        LibraryNode::Group(outer),
        LibraryNode::Media(MediaId::new(2)),
    ];
    project.normalize_library();
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
fn round_trip_v2_preserves_groups_and_metadata() {
    let project = sample_project_v2();
    let loaded = load(&save(&project)).expect("load");
    assert_eq!(project, loaded);
    // The nested tree and per-media metadata survived intact.
    assert_eq!(
        loaded.library_media_ids(),
        vec![MediaId::new(1), MediaId::new(2)]
    );
    assert_eq!(loaded.media[0].tags, vec!["elicitation", "vowels"]);
    assert_eq!(loaded.authors, vec!["Fieldworker A", "Fieldworker B"]);
}

#[test]
fn round_trip_v2_is_deterministic() {
    let project = sample_project_v2();
    assert_eq!(save(&project), save(&project));
    // Deterministic across an intervening load, too.
    let once = save(&project);
    assert_eq!(once, save(&load(&once).expect("load")));
}

#[test]
fn saves_write_version_two() {
    assert_eq!(manifest_version(&save(&sample_project_v2())), 2);
    assert_eq!(manifest_version(&save(&Project::new("empty"))), 2);
}

#[test]
fn v1_fixture_loads_with_defaults() {
    let original = sample_project();
    let loaded = load(&v1_container(&original)).expect("load v1");

    // v1 has no metadata and no tree; both default, so the loaded project
    // equals the normalized in-memory one (flat root in manifest order).
    assert_eq!(loaded, original);
    assert!(loaded.description.is_empty());
    assert!(loaded.authors.is_empty());
    assert!(loaded.tags.is_empty());
    assert!(loaded.media.iter().all(|m| m.tags.is_empty()));
    assert_eq!(
        loaded.library_media_ids(),
        vec![MediaId::new(1), MediaId::new(2)]
    );
}

#[test]
fn v1_fixture_resaves_as_v2_preserving_structure() {
    let loaded = load(&v1_container(&sample_project())).expect("load v1");
    let resaved = save(&loaded);
    assert_eq!(manifest_version(&resaved), 2);
    // Re-loading the v2 file reproduces the same value: annotations, media,
    // profiles, and the flat root all preserved through the upgrade.
    assert_eq!(load(&resaved).expect("reload"), loaded);
}

#[test]
fn load_repairs_dangling_and_duplicate_leaves() {
    // A hand-built container whose tree references a missing recording, repeats
    // one, and omits another. The loader repairs to the invariant.
    let mut project = sample_project();
    project.groups = vec![
        LibraryNode::Media(MediaId::new(2)),
        LibraryNode::Media(MediaId::new(2)), // duplicate, dropped
        LibraryNode::Media(MediaId::new(99)), // unknown, dropped
                                             // recording 1 omitted, appended at root during repair
    ];
    let loaded = load(&save(&project)).expect("load");
    // Recording 2 stays where it was placed; recording 1 lands at root end.
    assert_eq!(
        loaded.library_media_ids(),
        vec![MediaId::new(2), MediaId::new(1)]
    );
}

#[test]
fn normalize_is_idempotent() {
    let mut project = sample_project_v2();
    let once = project.clone();
    project.normalize_library();
    assert_eq!(project, once);
}

#[test]
fn random_group_trees_round_trip() {
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
    for _ in 0..256 {
        let mut project = Project::new("random");
        let media_count = 1 + (rng.next() % 6) as usize;
        for i in 0..media_count {
            let bytes = wav_bytes(i);
            project.media.push(
                MediaRef::from_wav_bytes(MediaId::new(i as u64 + 1), format!("m{i}.wav"), &bytes)
                    .expect("media"),
            );
        }
        // A random (possibly malformed) tree over these recordings plus noise.
        project.groups = random_forest(&mut rng, media_count, 0);
        project.normalize_library();

        // Invariant: every recording appears exactly once, no stranger present.
        let ids = project.library_media_ids();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), media_count, "each recording appears once");
        assert!(
            ids.iter()
                .all(|id| id.get() >= 1 && id.get() <= media_count as u64)
        );

        // The normalized tree round-trips byte-deterministically.
        let bytes = save(&project);
        let loaded = load(&bytes).expect("load");
        assert_eq!(loaded, project);
        assert_eq!(save(&loaded), bytes);
    }
}

/// Builds a random forest of nodes over `media_count` recordings.
///
/// Leaves reference ids in `1..=media_count`, plus out-of-range ids the loader
/// must drop; groups nest up to a small depth. The result is deliberately
/// malformed so normalization is exercised.
fn random_forest(rng: &mut Rng, media_count: usize, depth: u32) -> Vec<LibraryNode> {
    let count = (rng.next() % 4) as usize;
    let mut nodes = Vec::new();
    for _ in 0..count {
        let make_group = depth < 3 && rng.next() % 3 == 0;
        if make_group {
            let id = GroupId::new(1_000 + rng.next() % 1_000);
            nodes.push(LibraryNode::Group(Group {
                id,
                name: format!("g{}", rng.next() % 100),
                children: random_forest(rng, media_count, depth + 1),
            }));
        } else {
            // Bias toward valid ids but allow strays the loader must drop.
            let id = 1 + rng.next() % (media_count as u64 + 2);
            nodes.push(LibraryNode::Media(MediaId::new(id)));
        }
    }
    nodes
}

/// A tiny SplitMix64 generator: deterministic randomness with no dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Reads the `version` integer from a container's manifest.
fn manifest_version(bytes: &[u8]) -> u64 {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name("manifest.json").unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["version"].as_u64().unwrap()
}

/// Serializes `project` as a version-1 container: no metadata, no library tree,
/// media entries lacking the v2 per-recording fields.
fn v1_container(project: &Project) -> Vec<u8> {
    let media: Vec<serde_json::Value> = project
        .media
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "relative_path": m.relative_path,
                "hash": m.hash,
                "duration": m.duration,
                "sample_rate": m.sample_rate,
                "channels": m.channels,
            })
        })
        .collect();
    let manifest = json!({
        "format": FORMAT_TAG,
        "version": 1,
        "saved_at": project.saved_at,
        "name": project.name,
        "media": media,
    });

    let opts = zip::write::SimpleFileOptions::default();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let write_entry = |writer: &mut zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
                       name: &str,
                       value: &serde_json::Value| {
        writer.start_file(name, opts).unwrap();
        writer
            .write_all(serde_json::to_string_pretty(value).unwrap().as_bytes())
            .unwrap();
    };
    write_entry(&mut writer, "manifest.json", &manifest);
    write_entry(
        &mut writer,
        "profiles.json",
        &serde_json::to_value(&project.profiles).unwrap(),
    );
    write_entry(&mut writer, "view.json", &project.view);
    for (id, annotation) in &project.annotations {
        write_entry(
            &mut writer,
            &format!("annotations/{id}.json"),
            &serde_json::to_value(annotation).unwrap(),
        );
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn bundle_embeds_media_and_container_still_loads() {
    let wav0 = wav_bytes(0);
    let wav1 = wav_bytes(7);
    let project = sample_project_v2();
    let media = vec![(MediaId::new(1), wav0.clone()), (MediaId::new(2), wav1.clone())];
    let bytes = save_bundle(&project, &media);

    // A reader ignoring the embedded entries loads the same project a
    // references-only container would: embedding carries no version bump.
    let loaded = load(&bytes).expect("bundle loads as a project");
    assert_eq!(loaded, project);
    assert_eq!(manifest_version(&bytes), 2);

    // The embedded bytes come back exactly, ascending by id.
    let embedded = load_embedded_media(&bytes).expect("embedded media");
    assert_eq!(
        embedded,
        vec![(MediaId::new(1), wav0), (MediaId::new(2), wav1)]
    );
}

#[test]
fn references_only_container_embeds_nothing() {
    let project = sample_project_v2();
    // save and save_bundle with no media produce byte-identical references-only
    // containers, and neither carries embedded media.
    assert_eq!(save(&project), save_bundle(&project, &[]));
    assert!(load_embedded_media(&save(&project)).unwrap().is_empty());
}

#[test]
fn bundle_is_deterministic_regardless_of_media_order() {
    let wav0 = wav_bytes(0);
    let wav1 = wav_bytes(7);
    let project = sample_project_v2();
    let forward = vec![(MediaId::new(1), wav0.clone()), (MediaId::new(2), wav1.clone())];
    let reversed = vec![(MediaId::new(2), wav1), (MediaId::new(1), wav0)];
    assert_eq!(save_bundle(&project, &forward), save_bundle(&project, &reversed));
}

#[test]
fn bundle_may_embed_a_subset() {
    let wav0 = wav_bytes(0);
    let project = sample_project_v2();
    // Only recording 1 travels with the bundle; recording 2 stays references-only.
    let bytes = save_bundle(&project, &[(MediaId::new(1), wav0.clone())]);
    let embedded = load_embedded_media(&bytes).expect("embedded media");
    assert_eq!(embedded, vec![(MediaId::new(1), wav0)]);
    assert_eq!(load(&bytes).expect("load"), project);
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
