# Praat binary TextGrid format

Praat's manual page "TextGrid file formats" documents the long and short
text formats but explicitly declines to document the binary variant ("we
can add it here on request" — it never was). Every field below was derived
clean-room from sample pairs: a TextGrid built or loaded inside Praat (via
parselmouth, out-of-process, GPLv3, never linked into this crate) and
saved once as text and once as binary. The binary bytes were hexdumped
and matched, byte by byte, against the corresponding text file's
already-understood values. Praat's own source was never read for this
derivation.

Every claim below cites the fixture file and byte offset that demonstrates
it, decimal and 0-indexed from the start of the file. The fixtures
themselves are development-only test data, generated and checked by this
repository's own test suite rather than shipped with this package; each
name below identifies the sample pair a claim was checked against.

## Overview

```
"ooBinaryFile"                     12 bytes, literal ASCII, no length prefix
<class-name string>                "TextGrid"
<f64> xmin
<f64> xmax
<u8> tiers-exist flag              0x00 or 0x01
if flag == 0x01:
    <i32> tier count
    <tier record> * tier count
```

All multi-byte integers and floats are **big-endian**. There is no
byte-order mark and no encoding-selection header of any kind — a binary
TextGrid is recognized purely by its 12-byte magic prefix, `ooBinaryFile`
(`crates/phx-textgrid/src/lib.rs`'s `read` checks this before dispatching
to the binary reader; every other stream is decoded as text).

Evidence: `adjacent_empty_intervals_binary.TextGrid` bytes 0–11 are exactly
`6f 6f 42 69 6e 61 72 79 46 69 6c 65` = `"ooBinaryFile"` in every fixture in
the corpus.

## Class-name strings

Two fields in the format are drawn from Praat's small, fixed vocabulary of
object/tier class names: the top-level object class (`"TextGrid"`) and each
tier's class (`"IntervalTier"` or `"TextTier"`). Both use the same encoding:

```
<u8> length
<length bytes>                     one byte per character
```

Evidence: in `adjacent_empty_intervals_binary.TextGrid`, byte 12 is `0x08`
(8), followed by bytes 13–20 `54 65 78 74 47 72 69 64` = `"TextGrid"` — no
byte-order flag, no 2-byte length; the class-name field is structurally
different from the "text" field described below (see next section), which
*does* carry a 2-byte signed length immediately before its content. The
same fixture's tier-class field at byte 42 is `0x0c` (12) followed by bytes
43–54 `"IntervalTier"`, confirming the same 1-byte-length scheme is reused
for tier class names. `synthetic_binary_complex.TextGrid` and every other
fixture in the corpus repeat this pattern for both `"IntervalTier"` and
`"TextTier"` tiers.

Every sample in the corpus stores a class name as plain 7-bit ASCII; this
reader accepts any byte value here (mapping each byte to the Unicode code
point of the same numeric value, i.e. Latin-1), since a class name is
immediately compared against the fixed strings `"TextGrid"`, `"IntervalTier"`,
and `"TextTier"` and any other content simply fails that comparison.

## Text fields (tier names, interval/point labels)

Tier names and interval/point labels use a different, richer encoding than
class names — a signed 2-byte length that doubles as a narrow/wide flag:

```
<i16> n
if n == -1:                        wide (UTF-16BE) string
    <u16> char_count
    <UTF-16BE code units>          char_count *codepoints*, 2 or 4 bytes each
if n >= 0:                         narrow string
    <n bytes>                      one Latin-1 byte per character
```

### Narrow strings are ASCII-only in practice

Evidence for the two-scheme split: `adjacent_empty_intervals_binary.TextGrid`
byte 55 is `0x00`, byte 56 is `0x06` (`n = 6` as a 2-byte signed int), followed
by bytes 57–62 `70 68 6f 6e 65 73` = `"phones"` (the tier name) — one byte
per character, no wide flag, confirming the narrow branch. The same
fixture's interval label `"p"` at bytes 99–101 is `00 01 70`: `n = 1`,
followed by the single byte `0x70` = `'p'`.

Whether narrow strings can carry bytes ≥ `0x80` is not settled by any
sample: `latin1_legacy_binary.TextGrid`, generated from
`latin1_legacy.TextGrid` (itself Latin-1-encoded text containing "Über",
"Straße", "Café", "garçon", "Mädchen", "naïve"), stores every one of those
labels in the **wide** form (`ff ff` sentinel), even though every character
in them fits in a single Latin-1 byte — e.g. bytes 169–174 of that fixture
are `ff ff 00 04 00 43 00 61 00 66 00 e9` = `n = -1`, count 4, UTF-16BE
`0043 0061 0066 00e9` = `"Café"`. Praat evidently chooses the wide encoding
for any string containing a non-ASCII character, regardless of whether that
character would fit Latin-1's single-byte range. This reader still decodes
a narrow byte ≥ `0x80` as its Latin-1 code point rather than rejecting it —
consistent with the text reader's own Latin-1 fallback for legacy input —
since no sample requires this path and the safer assumption for malformed
or hand-crafted binary input is to degrade gracefully rather than error.

### Wide strings: codepoint count, not code-unit count

The `u16` following the `-1` sentinel counts **Unicode scalar values**
(`char`s), not 16-bit code units. `ipa_diacritics_binary.TextGrid`'s ninth
interval on the `segments` tier carries the label `"🗣dˤ"` — a
speaking-head emoji (U+1F5E3, encoded as a surrogate pair in UTF-16),
`'d'` (U+0064), and `'ˤ'` (U+02E4) — three Unicode scalar values but four
UTF-16 code units. At byte offset 309 of that file, the count field reads
`00 03` (3), followed by 8 bytes of UTF-16BE data: `d8 3d dd e3 00 64 02 e4`
— `D83D DDE3` (the surrogate pair for 🗣), `0064` (`'d'`), `02E4` (`'ˤ'`).
Reading exactly `count` `char`s (decoding surrogate pairs as they occur,
consuming 4 bytes each) rather than `count` code units is required to land
on the correct byte offset for the field that follows (verified: byte 319
of that fixture is `0x0c`, the length prefix that opens the next tier's
`"IntervalTier"` class name, exactly where the grammar expects it after
consuming those 8 bytes of text data).

The eighth interval on the same tier, label `"t̪ʰə"` (4 BMP codepoints:
`'t'`, a combining bridge-below, `'ʰ'`, `'ə'`), confirms the ordinary case
where codepoint count and code-unit count coincide: bytes 279–290 are
`ff ff 00 04 00 74 03 2a 02 b0 02 59` — count 4, followed by exactly 4
UTF-16BE code units (`0074`, `032A`, `02B0`, `0259`), 8 bytes total.

An unpaired surrogate (a high surrogate not followed by a low surrogate in
range `0xDC00..=0xDFFF`, or a low surrogate not preceded by a high one) is
not attested by any sample and is rejected with
[`TextGridError::InvalidUtf16`], matching how the text reader's UTF-16
decoding for whole-file content already treats an unpaired surrogate.

## Document header

```
<f64> xmin
<f64> xmax
```

immediately follow the object class name. Evidence:
`adjacent_empty_intervals_binary.TextGrid` bytes 21–28 are
`00 00 00 00 00 00 00 00` = `0.0` (xmin), bytes 29–36 are
`40 08 00 00 00 00 00 00` = `3.0` (xmax) — matching
`adjacent_empty_intervals_long_utf8.TextGrid`'s `xmin = 0` / `xmax = 3`.

## Tiers-exist flag and tier count

```
<u8> tiers-exist flag
if flag == 0x01:
    <i32> tier count
```

Evidence: byte 37 of `adjacent_empty_intervals_binary.TextGrid` is `0x01`,
immediately followed by bytes 38–41 `00 00 00 01` (tier count 1) — matching
the source's `tiers? <exists>` / `size = 1`. Every fixture in the corpus
with at least one tier repeats `0x01` at this position.

**Not derived from a sample:** no binary fixture demonstrates a zero-tier
document (`tiers? <absent>` in text form). The parselmouth-based generation
environment segfaults when reading a zero-tier TextGrid text file
(`parselmouth.Data.read` on a zero-tier fixture crashes the interpreter
before any binary sample can be produced), so this case could not be
generated. This reader treats a flag byte of `0x00` as
"no tiers, document ends here" by direct structural analogy with the text
grammar's `<absent>` case (which also carries no tier count or item block
after the flag) — an inference from the text format's symmetry, not a
derived fact. Any flag byte other than `0x00` or `0x01` is rejected as
[`TextGridError::InvalidTiersFlag`] rather than guessed at.

## Tier record

```
<class-name string>                "IntervalTier" or "TextTier"
<text field>                       tier name
<f64> tier xmin
<f64> tier xmax
<i32> entry count
<entry> * entry count              interval or point, per class
```

A tier's own `xmin`/`xmax` usually equals the document's, but can differ:
`narrow_tier_domain_binary.TextGrid` (from
`narrow_tier_domain_long_utf8.TextGrid`, document domain `[0, 2]`, tier
domain `[0.5, 1.5]`) stores the *document* xmin/xmax at bytes 21–36
(`0.0`, then `40 00 00 00 00 00 00 00` = `2.0`), and the *tier's own*
xmin/xmax separately at bytes 61–76 (`3f e0 00 00 00 00 00 00` = `0.5`,
then `3f f8 00 00 00 00 00 00` = `1.5`) — two independent `f64` pairs, not
one shared value. `phx_annot::IntervalTier`/`PointTier` already carry their
own `xmin`/`xmax` fields distinct from the document's (see
`crates/phx-annot/src/lib.rs`); the binary reader populates them from these
tier-local bytes, the same as the text reader already does for the text
grammar's per-tier `xmin =` / `xmax =` fields.

Tier class is checked against exactly `"IntervalTier"` and `"TextTier"`;
anything else is [`TextGridError::UnknownTierClass`].

### IntervalTier entries

```
<f64> xmin
<f64> xmax
<text field>                       label
```

repeated `entry count` times. There is no explicit boundary identifier:
adjacent intervals repeat the same time value for one interval's `xmax` and
the next interval's `xmin`, exactly as the text format does. Evidence:
`adjacent_empty_intervals_binary.TextGrid`'s first two intervals store
`xmax = 0.5` (bytes 91–98) and the very next interval's `xmin = 0.5` (bytes
102–109) as two separately-encoded but bit-identical `f64` values, not a
shared reference of any kind. `crates/phx-textgrid/src/tier_build.rs`'s
`build_interval_tier` mints one [`phx_annot::BoundaryId`] per such
run-length-1 join, identically for both the text and binary readers.

An interval's label uses the text-field encoding above; an empty label is
narrow with `n = 0` (zero bytes follow) in every sample — e.g.
`adjacent_empty_intervals_binary.TextGrid` bytes 118–119 are `00 00` for its
second interval's empty text, never the wide `-1` sentinel with a zero
count.

### TextTier (point) entries

```
<f64> time
<text field>                       mark
```

repeated `entry count` times — no `xmin`/`xmax` pair, a point tier stores
only a time and a label per entry. Evidence: `points_only_binary.TextGrid`
(from `points_only_long_utf8.TextGrid`) tier 1 (`"landmarks"`, class
`"TextTier"`) has entry count 4 (bytes 77–80: `00 00 00 04`), and its first
entry is exactly 10 bytes — `3f c3 33 33 33 33 33 33` (`0.15`) followed by
`00 05 6f 6e 73 65 74` (narrow, `"onset"`) — with no interleaved xmin/xmax
fields, unlike the `IntervalTier` case above.

A point tier with zero points ends its tier record immediately after the
entry count: `points_only_binary.TextGrid`'s second tier
(`"silence-marks"`) has entry count `00 00 00 00` at its final 4 bytes, and
the file ends there;
`synthetic_binary_complex.TextGrid`'s fourth tier (`"markers"`) does the
same mid-file, immediately followed by the next field the grammar expects
(there being none, since it is the last tier).

## Fixture coverage

| Fixture | Text twin | Demonstrates |
| --- | --- | --- |
| `adjacent_empty_intervals_binary.TextGrid` | `adjacent_empty_intervals_long_utf8.TextGrid` | Header, class-name strings, narrow text fields, empty labels, interval boundary sharing — the primary derivation fixture for this document |
| `ipa_diacritics_binary.TextGrid` | `ipa_diacritics_long_utf8.TextGrid` | Wide (UTF-16) text fields, codepoint-count semantics, a surrogate-pair (astral-plane emoji) label, two interval tiers |
| `narrow_tier_domain_binary.TextGrid` | `narrow_tier_domain_long_utf8.TextGrid` | Tier `xmin`/`xmax` distinct from the document domain |
| `points_only_binary.TextGrid` | `points_only_long_utf8.TextGrid` | `TextTier` (point) entry layout, a wide point label, a zero-point tier |
| `mixed_multitier_binary.TextGrid` | `mixed_multitier_short_utf8.TextGrid` | Interleaved interval/point tiers in one document, short-format text twin |
| `latin1_legacy_binary.TextGrid` | `latin1_legacy.TextGrid` | Latin-1-range characters always encoded wide, never narrow |
| `synthetic_binary_complex.TextGrid` | `synthetic_complex_long_utf8.TextGrid` | Four tiers (two interval, two point), non-round fractional boundary times, a zero-point tier at end of file |

`binary_truncated_mid_header.TextGrid`, `binary_truncated_mid_interval.TextGrid`,
and `binary_bad_tier_class.TextGrid` are hand-corrupted copies of
`adjacent_empty_intervals_binary.TextGrid` exercising the reader's
truncation and tier-class error paths.

This crate's fixture-driven test suite reads every binary fixture above and
asserts it parses structurally equal to its text twin, parsed independently
by the text reader.

[`TextGridError::InvalidUtf16`]: ../src/error.rs
[`TextGridError::InvalidTiersFlag`]: ../src/error.rs
[`TextGridError::UnknownTierClass`]: ../src/error.rs
