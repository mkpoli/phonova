# Phonix project container

A Phonix project is a single file that stores an editing session: which
recordings are open, the annotations on each, named analysis parameter
profiles, and the view state the interface last showed. Audio is referenced by
path and content hash; the recordings themselves stay where they are on disk.

This document specifies version 1 of the format so a third-party tool can read
or write a project without the Phonix source.

## Physical layout

The file is a ZIP archive (APPNOTE 6.3.x, the common subset every ZIP reader
supports). Entries are stored with Deflate compression. The archive holds these
entries:

| Entry | Presence | Contents |
| --- | --- | --- |
| `manifest.json` | required | Format tag, version, save time, project name, media references. |
| `profiles.json` | optional | Array of named parameter profiles. Absence means no profiles. |
| `view.json` | optional | Opaque view-state value. Absence is read as JSON `null`. |
| `annotations/<id>.json` | optional, one per annotated recording | The annotation document for the recording whose `id` matches. |

Every entry is UTF-8 JSON. A reader that does not recognise an entry name
ignores it. A writer emits entries in the order above, media in manifest order,
and annotations in ascending `id` order, so a given project value serializes to
the same bytes each time.

## `manifest.json`

```json
{
  "format": "phonix-project",
  "version": 1,
  "saved_at": 1737000000000,
  "name": "Fieldwork 2026",
  "media": [
    {
      "id": 1,
      "relative_path": "audio/interview-01.wav",
      "hash": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
      "duration": 42.13,
      "sample_rate": 48000.0,
      "channels": 1
    }
  ]
}
```

- `format` — the constant string `phonix-project`. A reader rejects any other
  value.
- `version` — the schema version, currently `1`. A reader rejects a version
  higher than it understands rather than dropping fields it cannot represent.
- `saved_at` — milliseconds since the Unix epoch when the file was written.
  Recovery compares this between a project file and its autosave sidecar.
- `name` — the project's display name.
- `media` — the referenced recordings.

### Media references

Each media entry names a recording without embedding it.

- `relative_path` — location of the media file relative to the directory that
  holds the project file, using `/` as the separator.
- `hash` — BLAKE3 digest of the media file's bytes, as 64 lowercase hex
  characters. This is the content address used to re-link a moved file.
- `duration` — length in seconds.
- `sample_rate` — sample rate in hertz.
- `channels` — channel count.

When the file at `relative_path` is absent, a reader searches sibling
directories, hashes each candidate, and re-links the reference to a file whose
hash equals `hash`. One match re-links silently. Two or more equal matches are
reported to the user with the candidate paths, because the choice is theirs. No
match leaves the recording unresolved for the user to locate.

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
represent. A reader accepts its own version and lower, and rejects a higher
one. New optional entries can be added within a version, since readers ignore
entries they do not recognise.
