# Phonia project container

A Phonia project is a single file that stores an editing session: which
recordings are open, how they are grouped and ordered, descriptive metadata on
the project and each recording, the annotations on each, named analysis
parameter profiles, and the view state the interface last showed. Audio is
referenced by path and content hash; the recordings themselves stay where they
are on disk.

This document specifies the format through version 2 so a third-party tool can
read or write a project without the Phonia source. Version 2 adds the library
tree and descriptive metadata; a version 1 file omits those and reads with them
defaulted (see [Versioning](#versioning)).

## Physical layout

The file is a ZIP archive (APPNOTE 6.3.x, the common subset every ZIP reader
supports). Entries are stored with Deflate compression. The archive holds these
entries:

| Entry | Presence | Contents |
| --- | --- | --- |
| `manifest.json` | required | Format tag, version, save time, project name, project metadata, media references, library tree. |
| `profiles.json` | optional | Array of named parameter profiles. Absence means no profiles. |
| `view.json` | optional | Opaque view-state value. Absence is read as JSON `null`. |
| `annotations/<id>.json` | optional, one per annotated recording | The annotation document for the recording whose `id` matches. |
| `media/<id>.wav` | optional, one per embedded recording | The recording's own bytes, present only in a self-contained bundle (see [Self-contained bundle](#self-contained-bundle)). |

Every JSON entry is UTF-8. A reader that does not recognise an entry name
ignores it. A writer emits entries in the order above, media in manifest order,
tree nodes in display order, annotations in ascending `id` order, and embedded
`media/<id>.wav` entries in ascending `id` order, so a given project value
serializes to the same bytes each time whether or not it carries media.

## `manifest.json`

```json
{
  "format": "phonix-project",
  "version": 2,
  "saved_at": 1737000000000,
  "name": "Fieldwork 2026",
  "description": "Two speakers recorded on site.",
  "authors": ["Fieldworker A", "Fieldworker B"],
  "tags": ["fieldwork", "2026"],
  "media": [
    {
      "id": 1,
      "relative_path": "audio/interview-01.wav",
      "hash": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
      "duration": 42.13,
      "sample_rate": 48000.0,
      "channels": 1,
      "description": "Elicitation, morning session.",
      "authors": ["Speaker A"],
      "tags": ["elicitation", "vowels"]
    }
  ],
  "groups": [
    {
      "Group": {
        "id": 10,
        "name": "Speaker A",
        "children": [{ "Media": 1 }]
      }
    }
  ]
}
```

Fields serialize in this order: `format`, `version`, `saved_at`, `name`,
`description`, `authors`, `tags`, `media`, `groups`.

- `format` — the constant string `phonix-project`, unchanged since the format
  predates the product's current name. A reader rejects any other value.
- `version` — the schema version, `2` for a file a current writer emits. A
  reader rejects a version higher than it understands rather than dropping
  fields it cannot represent.
- `saved_at` — milliseconds since the Unix epoch when the file was written.
  Recovery compares this between a project file and its autosave sidecar.
- `name` — the project's display name.
- `description` — free-form text about the project. Empty when unset. Added in
  version 2; absent from a version 1 file, where it reads as empty.
- `authors` — contributors credited for the project, an ordered array of
  strings. Empty when unset. Added in version 2.
- `tags` — free-form tags on the project, an ordered array of strings. Empty
  when unset. Added in version 2.
- `media` — the referenced recordings, the flat source of truth for which
  recordings the project holds.
- `groups` — the library tree over those recordings. Added in version 2; see
  [The library tree](#the-library-tree).

### Media references

Each media entry names a recording without embedding it.

- `relative_path` — location of the media file relative to the directory that
  holds the project file, using `/` as the separator.
- `hash` — BLAKE3 digest of the media file's bytes, as 64 lowercase hex
  characters. This is the content address used to re-link a moved file.
- `duration` — length in seconds.
- `sample_rate` — sample rate in hertz.
- `channels` — channel count.
- `description` — free-form text about this recording. Empty when unset. Added
  in version 2.
- `authors` — contributors credited for this recording, an ordered array of
  strings. Empty when unset. Added in version 2.
- `tags` — free-form tags on this recording, an ordered array of strings. Empty
  when unset. Added in version 2.

The three metadata fields close each media entry, after `channels`. A version 1
media entry omits them and they read as empty.

When the file at `relative_path` is absent, a reader searches sibling
directories, hashes each candidate, and re-links the reference to a file whose
hash equals `hash`. One match re-links silently. Two or more equal matches are
reported to the user with the candidate paths, because the choice is theirs. No
match leaves the recording unresolved for the user to locate.

### The library tree

`groups` is an ordered tree that organizes the recordings for display. The
recordings live in the `media` array; the tree only references them by id, so a
reader that skips `groups` still sees every recording. On top of the flat array
the tree adds nesting into named groups and an explicit display order that runs
independent of the `media` array's order.

Each element of `groups` is a node, one of two shapes:

```json
{ "Media": 1 }
```

A recording leaf. The value is the `id` of a media entry.

```json
{ "Group": { "id": 10, "name": "Speaker A", "children": [ /* nodes */ ] } }
```

A group. `id` identifies the group within the project, `name` is its display
name, and `children` is an ordered array of nodes, groups and recordings
intermixed. Groups nest to any depth; a group may be empty.

Group ids are their own space, unrelated to media ids. Node order in each
`children` array, and in the root `groups` array, is the display order and is
preserved verbatim on read and write.

#### Integrity

A well-formed tree references every media `id` exactly once as a `Media` leaf.
A reader repairs a tree that departs from this rather than rejecting the file:

- a `Media` leaf whose id is not in `media` is dropped;
- a second `Media` leaf for an id already seen (walking depth-first in display
  order) is dropped;
- a recording in `media` that no leaf references is appended as a `Media` leaf
  to the end of the root array, in `media` order;
- groups are kept even when empty.

A version 1 file has no `groups`; the repair lays every recording out flat at
the root in `media` order, which is the manifest order. Repair is idempotent:
running it on an already-well-formed tree changes nothing, so a file round-trips
to identical bytes.

## Self-contained bundle

A project file travels two ways. The default references the media externally,
as [Media references](#media-references) describes: the recordings stay on disk
and the file carries only their paths and hashes. A self-contained bundle
instead embeds each recording, so the file opens on a machine that never held
the source audio.

A bundle embeds a recording as a `media/<id>.wav` entry, where `<id>` is the
recording's manifest `id`. The entry holds the recording's exact bytes; the ZIP
stores it Deflate-compressed like every other entry, and the bytes carry no
further compression, so a lossless source stays lossless. A bundle may embed
every recording, a subset, or none — a recording the archive does not embed
stays references-only and resolves through the re-link flow below.

Embedding is orthogonal to the schema `version`: a bundle is a version-2 file,
and the manifest is identical to the references-only form. A reader that ignores
the `media/` entries sees the same project and resolves each recording against
the filesystem; a reader that reads them restores the recording from the bundle
before falling back to that resolution. So a bundle needs no version bump, and
an older reader opens it as a references-only project.

On import, a reader restores each embedded recording to the location its
`relative_path` names, then resolves any recording the bundle did not embed by
content hash against media already present, exactly as a moved reference
re-links (see [Media references](#media-references)). A recording that resolves
to no local file is reported to the user as a gap and left for them to locate.

## `profiles.json`

An array of parameter profiles:

```json
[
  { "name": "female speaker", "params": { "pitch_floor": 100.0, "pitch_ceiling": 500.0 } },
  { "name": "male speaker",   "params": { "pitch_floor": 75.0,  "pitch_ceiling": 300.0 } }
]
```

- `name` — the profile's display name, shown in the toolbar.
- `params` — any JSON value. The container stores and returns it unchanged; the
  interface owns its shape (pitch floor and ceiling, formant ceiling, palette,
  and so on).

## `view.json`

A single JSON value carrying the view state the interface last showed — zoom
span, palette, visible tracks. The container treats it as opaque and returns it
unchanged.

## `annotations/<id>.json`

One entry per recording that carries an annotation, named by the recording's
`id` from the manifest. The payload is a serialized annotation document: a time
domain and a list of interval and point tiers with their labels, boundaries,
and cross-tier relations. A recording with no annotation has no entry.

## Autosave sidecar

While a project is open, edits are checkpointed to a sidecar file named by
appending `.autosave` to the project path (for example
`session.phxproj.autosave`). The sidecar is a full project container in this
same format; its `saved_at` records when the snapshot was taken. On open, a
sidecar whose `saved_at` is newer than the project file's holds unsaved work
from an interrupted session and is offered for recovery. An explicit save
writes the project file and removes the sidecar.

## Versioning

`version` increments when the schema changes in a way older readers cannot
represent. A reader accepts its own version and lower, and rejects a higher one
with a version error. New optional entries can be added within a version, since
readers ignore entries they do not recognise.

The current version is `2`. A writer always emits `2`.

Version 2 added the project `description`, `authors`, and `tags`; the same three
fields per media entry; and the `groups` library tree. Each addition defaults
when absent, so a version 1 file loads without loss: its metadata reads as empty
and its recordings lay out flat at the tree root in manifest order. A version 2
reader accepts both a version 1 and a version 2 file. A version 1 reader accepts
a version 1 file and rejects a version 2 file with the version error, since the
tree and metadata it cannot represent would otherwise be dropped silently.
