# Praat file formats

This document specifies the on-disk formats Phonia needs to read and write
Praat-compatible files: the generic object-serialization scheme Praat builds
every format on, the binary TextGrid layout, the chronological TextGrid text
format, the Xwaves/ESPS label-file format, and Praat's text-encoding rules.

Praat's own manual documents the long and short TextGrid text formats in
detail (see [TextGrid file formats](#textgrid-text-format-long-and-short)
below) but explicitly declines to document the binary variant, and says
nothing about the chronological format or Xwaves label files beyond a
changelog line. Everything not cited to a manual page below was derived by
reading Praat's source (github.com/praat/praat, GPL), with the owner's
explicit authorization to do so for this project; every claim is described in
this document's own words and cited to file and line rather than reproduced
verbatim, and no Praat source was copied into this repository.

**Source inspected**: commit `626bc1f2686d42a60c8f5472780d6fec9bac5906` of
`github.com/praat/praat` (default branch, fetched 2026), which identifies
itself as version `7.0beta` (`PRAAT_VERSION_NUM 7000`, `main/main_Praat.h`).
Praat's file formats have historically been stable across major versions —
version history is noted per format below where the changelog documents when
a format or behavior was introduced or fixed — but a reader should treat any
claim not backed by the manual as accurate for this commit and check for
drift against newer Praat sources if exact fidelity to a different version
matters.

All multi-byte binary integers and floats described in this document are
**big-endian** unless stated otherwise.

## 1. Generic object serialization: `ooTextFile` and `ooBinaryFile`

Every object type Praat can save — Sound, Pitch, TextGrid, and hundreds of
others — is serialized by the same generic mechanism, referred to internally
by the "oo" prefix. Understanding this substrate first makes the binary
TextGrid layout (§3) mostly a matter of reading off one class's field list.

### 1.1 The class-description macro system

A class's on-disk shape is declared once, in a `*_def.h` file, as a sequence
of macro invocations — `oo_DOUBLE`, `oo_STRING`, `oo_COLLECTION_OF`,
`oo_OBJECT`, and others — bracketed by `oo_DEFINE_CLASS` / `oo_END_CLASS`.
Praat generates nine different operations from that single field list — read
text, write text, read binary, write binary, copy, equality test, destroy,
description, and an "encoding capability" check — by including the same
`_def.h` file nine times, each time after a header that redefines the field
macros to do something different for that operation (`sys/oo_undef.h`;
operation headers `sys/oo_WRITE_BINARY.h`, `sys/oo_READ_BINARY.h`,
`sys/oo_WRITE_TEXT.h`, `sys/oo_READ_TEXT.h`, `sys/oo_COPY.h`, `sys/oo_EQUAL.h`,
`sys/oo_DESTROY.h`, `sys/oo_DESCRIPTION.h`,
`sys/oo_CAN_WRITE_AS_ENCODING.h`). A concrete class's `.cpp` file includes its
`_def.h` nine times, once per operation (e.g. `fon/TextGrid.cpp:22-39`).

Field order in the `_def.h` file is authoritative for on-disk order, for both
text and binary, because reading and writing are purely positional — there is
no field tagging or keyed lookup. A class's `oo_DEFINE_CLASS` expansion always
invokes its parent class's corresponding method first, so inherited fields
are always serialized before a class's own fields, recursively up the
hierarchy, identically on read and write (`sys/oo_WRITE_BINARY.h:127-132`,
`sys/oo_READ_BINARY.h:135-142`).

### 1.2 File header

**Binary.** A binary object file begins with the literal 12-byte ASCII
sequence `ooBinaryFile`, no length prefix, no trailing NUL or newline
(`sys/Data.cpp:131-148` writes it; `sys/Data.cpp:216-256` reads it). This is
immediately followed, with no separator, by the class-name string (§1.3),
then the object's own field data starts immediately. Before matching
`ooBinaryFile`, the reader checks for a reserved-but-unimplemented
`ooBinary2File` token and rejects it with a "download a newer version of
Praat" style message (`sys/Data.cpp:228-229`) — a placeholder for an
anticipated but never-built second binary container generation. A legacy,
pre-"oo"-era fallback (`<ClassName>BinaryFile`, no `oo` prefix) is also
recognized and sets a sentinel `formatVersion = -1` for objects saved by very
old Praat versions (`sys/Data.cpp:237-247`); TextGrid was never serialized in
this pre-oo format (its class version is 0, see §1.4), so this path is not
relevant to TextGrid specifically.

**Text (long and short).** A text object file begins with exactly:

```
File type = "ooTextFile"
Object class = "ClassName"
```

as two literal lines (`sys/Data.cpp:85-88`), where `ClassName` is the class
name, with a space and the class's version number appended inside the quotes
only if that class's version is greater than 0 (e.g. `"Sound 2"`) — see §1.4.
The object's own field data follows directly; the file ends with one trailing
newline appended after the object body (`sys/Data.cpp:90`). A reader first
rejects a reserved `ooText2File` token (the text-format counterpart of
`ooBinary2File`, same unbuilt-extension-point status), then matches
`ooTextFile`, then reads the class name as a quoted string (§1.3) rather than
a fixed-width field (`sys/Data.cpp:174-189`). A legacy pre-oo fallback
(`<ClassName>TextFile`) is also recognized, again setting `formatVersion = -1`.

**Long text vs. short text are the same format at different verbosity, not
two grammars.** Both are produced by the same internal writer, differing only
in a `verbose` flag that controls whether field-name labels (`xmin =`),
`item [n]:` indices, and indentation are printed — purely decorative, never
parsed back on read (`sys/Data.cpp:105-119`; label printing itself in
`melder/abcio.cpp:440-479`). The header, quoting convention, and positional
field order are identical between long and short text. This matches what the
manual page documents for TextGrid specifically (see §2).

**Byte-order mark.** If the output encoding resolves to UTF-16 (§6), a BOM
(`U+FEFF`, written big-endian as bytes `FE FF`) is written as the very first
two bytes of the file, before the `File type = ...` line
(`sys/Data.cpp:72-73`). Binary object files carry no file-level BOM or
encoding preamble of any kind — see §6.3.

**Generic file-type detection.** When Praat opens a file without knowing its
type in advance, it sniffs the first 512 bytes for these magic substrings,
also tolerating a leading gzip magic (`1F 8B`) on the assumption the file is
a gzip-compressed text file, before falling back to other installed
recognizers (`sys/Data.cpp:281-365`). Whether gzip-wrapped *binary* object
files are also transparently supported was not confirmed — only the text-file
gzip path was found in source.

### 1.3 Class-name string and version encoding

The binary class-name field uses **`binputw8`** (`melder/abcio.cpp:1985-2015`),
which is a general string codec with a narrow/wide duality parallel to the
`w16` string codec used for ordinary string fields (§1.5), just with a 1-byte
length instead of 2:

- If the string is strict 7-bit ASCII (`Melder_isValidAscii`, codepoint ≤
  127; `melder/melder_textencoding.cpp:48-54`): a `u8` length, then that many
  raw bytes.
- Otherwise: a `u8` value `0xFF` (escape), a `u8` character count, then that
  many UTF-16BE code units (with surrogate-pair splitting for astral
  characters).

In practice every class name and tier-class name Praat writes is 7-bit ASCII,
so the escape branch never fires for this field — but the mechanism is the
general `w8` codec reused, not a bespoke class-name format. Note that the
*reader* for the top-level class name uses a plainer function, `bingets8`
(`u8` length + raw bytes, no escape handling at all, `sys/Data.cpp:235`,
`melder/abcio.cpp:1763-1774`) — asymmetric with the writer, but harmless
because class names are always ASCII in practice. A tier's own class name
inside a `TextGrid` (`"IntervalTier"` / `"TextTier"`) is written through a
different path — see §3.1 — using the full `w8` writer, not the plain
reader-side function.

A class's version number is not a separate field; it is textually appended
to the class-name string before encoding, as a space plus the version's
decimal digits (e.g. `"Sound 2"`), for both binary and text
(`sys/Data.cpp:138-142`). On read, the decoded class-name string is split at
the first space, and the remainder parsed as an integer
(`sys/Thing.cpp:88-100`); no space means version 0. `TextGrid`,
`IntervalTier`, `TextTier`, `TextInterval`, and `TextPoint` are all declared
at version 0 (`Thing_implement (TextGrid, Function, 0)`,
`fon/TextGrid.cpp:329`, and correspondingly for the other TextGrid-family
classes), so no version suffix is ever present on these class names in a
TextGrid file.

The text-format class name uses `texputw16`/`texgetw16` — a quoted string
with no length prefix, delimited by double quotes, internal quotes doubled,
scanned to the matching close quote (`melder/abcio.cpp:296-338`,
`583-593`) — with the same version-as-space-suffix convention inside the
quotes.

### 1.4 String field encoding (`oo_STRING`)

Every `oo_STRING` field in every Praat class, TextGrid's included, goes
through one shared implementation — TextGrid gets no special-cased string
handling (`sys/oo.h:116`; TextGrid's own `mark`/`text` fields,
`fon/TextGrid_def.h:23,31`, expand through the same path as any other class's
string field).

**Binary** (`bingetw16`/`binputw16`, `melder/abcio.cpp:1840-1876,2017-2047`):

```
<u16> n                     length, big-endian, UNSIGNED
if n == 0xFFFF:             wide escape (bit-identical to -1 read as i16)
    <u16> char_count         Unicode scalar-value count, not UTF-16 code-unit count
    <UTF-16BE code units>    char_count codepoints, surrogate pairs for astral characters
else:                        narrow
    <n bytes>                 one byte per character, zero-extended to a codepoint on read
```

Two points correct a natural reading of the "narrow/wide" split as
Latin-1-vs-Unicode:

- The length prefix is an **unsigned** 16-bit integer (0–65534 = narrow
  length, 65535 = wide escape), not a signed one. Testing the raw bits
  against `-1` as a signed `i16` happens to work because `0xFFFF` is
  bit-identical either way, but an implementation that does signed
  comparisons like "length < 0" is relying on that coincidence rather than
  the actual field type.
- The writer's narrow/wide decision is gated by `Melder_isValidAscii`, which
  is **strict 7-bit ASCII** (codepoint ≤ 127), not "fits in one Latin-1
  byte." A string containing any character in the Latin-1 upper range
  (0x80–0xFF, e.g. "é") is still promoted to the wide UTF-16 escape on
  write, even though it would fit in a single narrow byte. Praat's own
  writer therefore never emits a narrow-branch byte ≥ 0x80. On *read*, a
  narrow-branch byte is decoded by zero-extension (raw byte value becomes
  the codepoint, i.e. genuine Latin-1 semantics) — a reader that accepts
  narrow bytes ≥ 0x80 as Latin-1 is being more permissive than any file a
  real Praat writer produces, which is a reasonable robustness choice but
  should not be read as "Praat writes Latin-1 in the narrow branch."

**Text** (`texputw16`/`texgetw16`): no length prefix at all — delimited by
double quotes, an internal `"` doubled to escape it, scanned to the matching
close quote (`melder/abcio.cpp:296-338`). The character encoding of the text
*file* as a whole (ASCII, ISO Latin-1, or UTF-16) is chosen once per file, not
per string — see §6.

### 1.5 Numeric field encoding

All fixed-width binary numeric writers in `melder/abcio.cpp` (`binputi16`,
`binputu16`, `binputi32`, `binputr32`, `binputr64`, and their `binget*`
counterparts) produce big-endian byte order unconditionally, regardless of
host endianness — on a little-endian host they explicitly byte-swap; the
double/float writers construct genuine IEEE 754 bit patterns even on a
(hypothetical) non-IEEE host, rather than assuming the native representation.
Storage-tag names used inside a `_def.h` file (`i8`, `u16`, `integer32BE`,
`r32`, `r64`, …) map textually to the `binget<tag>`/`binput<tag>` (binary)
and `texget<tag>`/`texput<tag>` (text) function families
(`melder/abcio.h`); `oo_INTEGER` uses tag `integer32BE`, i.e. a 32-bit
big-endian integer regardless of the in-memory integer width Praat itself
uses internally (`melder/abcio.h:94`).

### 1.6 Versioning and backward-compatible reads

Every class carries one compile-time integer version, set via
`Thing_implement(Class, Parent, version)` (`sys/Thing.h:65-68`; e.g.
`Thing_implement (Sound, Vector, 2)`, `fon/Sound.cpp:34`). This number is
written into a file only as the class-name version suffix described in
§1.3 — never as an independent field — and becomes the `_formatVersion_`
value threaded through every class's read methods, including into parent
classes' methods, when reading that file back.

Two macro families gate field presence on `_formatVersion_` at *runtime*
(not `#if` — the logic must exist in every build, to read any older file a
user might still have):

- `oo_FROM(n) ... oo_ENDFROM` wraps enclosed fields in
  `if (_formatVersion_ >= n)` on read; on write it is a no-op, since an
  outgoing file is always written at the class's current version and so
  always includes every field (`sys/oo_READ_BINARY.h:144-151`,
  `sys/oo_WRITE_BINARY.h:134-136`).
- `oo_VERSION_UNTIL(n) / oo_VERSION_ELSE / oo_VERSION_END` switches between
  entirely different old-format layouts for one field, not just its
  presence — used for fields whose on-disk type or shape itself changed
  across a version boundary.

Every class's read path also inserts a hard guard,
`_formatVersion_ <= this class's own compiled-in version`, rejecting a file
newer than the reading Praat build understands rather than attempting to
parse it (`sys/oo_READ_BINARY.h:136-138`, `sys/oo_READ_TEXT.h:152-154`) —
checked independently at every level of the class hierarchy.

TextGrid and every class in its object graph (`IntervalTier`, `TextTier`,
`TextInterval`, `TextPoint`) are version 0, so none of this version-gating
logic is currently active for TextGrid files — the field list in §3.1 is the
complete, unconditional layout for every TextGrid ever written by any Praat
release using the modern "oo" format.

## 2. TextGrid text format (long and short)

Manual page: [TextGrid file formats](https://www.fon.hum.uva.nl/praat/manual/TextGrid_file_formats.html)
(`docs/manual/TextGrid_file_formats.html` in the Praat source tree, dated
2018-08-21 with a 2026 revision note). This page documents the long and short
text formats thoroughly and is the authoritative reference — this section
only extracts the facts most load-bearing for an implementer and for
cross-checking §3 and §5.

- Header: `File type = "ooTextFile"` then `Object class = "TextGrid"`, each
  alone on its own line, exactly as described generically in §1.2.
- Body, in order: `xmin`, `xmax` (document domain), a flag written as the
  literal token `<exists>` or `<absent>` (`tiers? <exists>` in long form),
  then — only if `<exists>` — the tier count and one block per tier: class
  (`"IntervalTier"` or `"TextTier"`), name, tier `xmin`/`xmax`, an
  interval/point count, then that many interval or point records. An
  `<absent>` flag ends the file immediately with no tier count or tier
  blocks. This is Praat's own documented semantics for the `<absent>` case —
  see §3.2 for how it maps onto the binary format's boolean flag byte.
- A file is parsed as a stream of free-standing tokens: quoted strings,
  bare numbers, and the two literal flag tokens `<exists>`/`<absent>`
  (TextGrid files use no other flag value). Everything else — labels like
  `xmin =`, `item [2]:`, indentation, and anything after a `!` on a line — is
  a comment and is ignored on read; the short format simply omits all of it.
- A literal double quote inside a quoted string is escaped by doubling it
  (`""`), the same convention used for the class-name and tier-name fields
  documented generically in §1.3–§1.4.
- Interval-tier invariants Praat maintains and expects: contiguous,
  non-overlapping intervals (each interval's start equals the previous
  interval's end); Praat may refuse to load or may attempt to repair a file
  that violates this. A tier's `xmin`/`xmax` need not equal the document's —
  this is explicitly a normal, weaker invariant that real usage (e.g.
  combining TextGrids of different durations) can and does leave unequal;
  see the correction to our own binary-format doc in §7.

## 3. Binary TextGrid format

### 3.1 Field layout

TextGrid's binary I/O is produced entirely by the generic `oo_*` mechanism of
§1 — there is no hand-written binary reader or writer for the standard
`.TextGrid` binary format (hand-written I/O in `fon/TextGrid_files.cpp`
exists only for the *foreign* formats: chronological text, Xwaves/ESPS, and
corpus-specific importers — §4, §5). The complete field list, reconstructed
from `fon/TextGrid_def.h`, `fon/Function_def.h`, `sys/Simple_def.h`, and the
generic collection-serialization code:

```
TextGrid:
    f64 xmin, f64 xmax                        [Function base class]
    bool8 tiersExist                          0x00 or 0x01, one byte
    if tiersExist:
        i32 tierCount
        tierCount × {
            w8  tierClassName                 "IntervalTier" | "TextTier"
            w16 tierName                      not a TextGrid-family field — the generic
                                               Thing "name" slot, see below
            f64 tier.xmin, f64 tier.xmax       [Function base class, tier's own]
            i32 itemCount
            itemCount × <item, per tier class below>
        }

IntervalTier item (TextInterval):
    f64 xmin, f64 xmax                        [Function base class]
    w16 text

TextTier item (TextPoint):
    f64 number                                the point's time; inherited from
                                               SimpleDouble via AnyPoint, not a
                                               TextGrid-family field
    w16 mark
```

`w8`/`w16` are the class-name and string codecs from §1.3–§1.4. `bool8` is a
single byte, `0x00` or `0x01` (`binputbool8`/`binputex`,
`melder/abcio.cpp:785-791`). `i32`/`f64` are big-endian per §1.5. There is no
checksum, padding, or alignment byte anywhere in the format — every field is
written back-to-back with nothing interleaved
(`sys/oo_WRITE_BINARY.h` in full; `sys/Data.cpp:131-148` appends nothing
after the last field).

**Why the tier list carries a class-name and name tag but interval/point
lists don't.** `TextGrid`'s `tiers` field is declared
`oo_OBJECT (FunctionList, 0, tiers)` — a single, nullable, *polymorphic*
object reference, because a tier can be either concrete class
(`fon/TextGrid_def.h:80`). Serializing it requires a runtime class tag so the
reader knows which concrete type to reconstruct; that tag, plus the tier's
display name, come from Praat's generic `Thing`/`Collection` layer, not from
any field TextGrid itself declares (`sys/Collection.cpp:156-168`) — hence
"tier name" above being the borrowed `Thing.name` slot rather than a
`TextTier`/`IntervalTier` field. `IntervalTier.intervals` and
`TextTier.points`, by contrast, are declared `oo_COLLECTION_OF` — a
homogeneous, statically-typed collection — so each entry is written by a
direct, non-virtual call to its known class's own write method, with no
class tag and no name field at all (`sys/oo_WRITE_BINARY.h:97-102`). This is
the structural reason interval and point entries carry no per-entry
metadata while the outer tier list does.

**Tier count and item count** are written via the generic collection writer
as `i32` (`binputi32`/`bingeti32`, or the equivalent `binputinteger32BE` —
both produce the same on-disk 4-byte big-endian representation;
`sys/Collection.cpp:156-168`, `sys/oo_WRITE_BINARY.h:97`).

### 3.2 The zero-tier case, resolved

`TextGrid`'s `tiers` field, being `oo_OBJECT`-declared, serializes as a
single boolean byte (§3.1's `tiersExist`) followed by the tier list only if
that byte is `0x01`. This is Praat's general "nullable object" encoding
(§1.1), not a bespoke TextGrid convention, and confirms the derived doc's
inferred structure exactly: `0x00` legitimately means "the object reference
is null, nothing else follows for this field."

However, **no exposed Praat code path ever produces a savable TextGrid with
a null or empty tier list.** `TextGrid_createWithoutTiers`
(`fon/TextGrid.cpp:331-341`) unconditionally allocates a (possibly empty)
`FunctionList` — `tiers` is never left null. The public "Create TextGrid..."
command refuses to create a TextGrid with zero tier names supplied
(`fon/TextGrid.cpp:371`, error message to that effect), and "Remove tier..."
refuses to remove a tier if it is the last one (`fon/praat_TextGrid_init.cpp:1262`).
So while the *format* can represent zero tiers two different ways — `0x00`
with nothing further, or `0x01` followed by `i32(0)` and an empty tier
list — only the second is reachable by constructing a `TextGrid` the way
Praat's own object model always does (allocate the list, then add zero or
more tiers), and neither is actually reachable from the stock UI/scripting
commands, which refuse to leave a TextGrid tier-less at all.

There is a further, sharper reason to treat `0x00` as reader-robustness-only
rather than a state to imitate on write: TextGrid's own display and info code
iterates `tiers->size` without checking `tiers` for null in multiple places
(`fon/TextGrid.cpp:284,304,315,323`), so a real Praat build handed a
hand-crafted `flag=0x00` file would very likely crash the moment it tried to
inspect the resulting object, rather than displaying "0 tiers" gracefully.

**Resolution for Phonia**: a reader should accept `0x00` as "zero tiers,
document ends here" (matching the format's literal semantics and the text
grammar's `<absent>` case), since a malformed or hand-crafted file might
carry it and Praat's format grammar makes it well-defined. A writer aiming
to produce a byte-for-byte plausible "file a real Praat could have written,"
for the degenerate zero-tier case specifically, should prefer `0x01`
followed by `i32(0)` — an allocated-but-empty tier list — since that is the
form `TextGrid_createWithoutTiers` would actually serialize to, and stock
Praat never produces the `0x00` form at all.

### 3.3 Other corrections and confirmations

See §7 for the full itemized diff against `crates/phx-textgrid/docs/binary-format.md`.

## 4. Chronological TextGrid text format

This format exists, is currently supported (not deprecated), text-only (no
binary variant), and is built on Praat's ordinary object-text writer
primitives rather than the generic `_def.h`-driven object serialization —
it has its own hand-written reader and writer because its on-disk shape
(one flat, time-sorted stream across all tiers) doesn't correspond to any
single object's field list.

- Write: `TextGrid_writeToChronologicalTextFile`
  (`fon/TextGrid.h:178`, `fon/TextGrid_files.cpp:479-561`). Read:
  `TextGrid_readFromChronologicalTextFile` (`fon/TextGrid.h:179`,
  `fon/TextGrid_files.cpp:408-464`).
- Menu: **Save as chronological text file...**, with a hidden legacy-named
  alias **Write to chronological text file...**
  (`fon/praat_TextGrid_init.cpp:1678-1679`). No dedicated read-menu command
  was found in this search; a chronological file is presumably picked up by
  Praat's generic "recognize a text object file by its first token" read
  path (unresolved which exact dispatcher — Praat auto-detects many foreign
  text formats this way, but the specific registration point for this format
  was not traced).
- Introduced in Praat 4.2 (4 March 2004); corrected reading in 4.6.13
  (16 August 2007); made UTF-16-encoded chronological files readable in
  5.0.32 (12 August 2008); fixed a further reading bug in 5.2.24
  (10 May 2011) — all per `fon/manual_whatsnew.cpp`'s changelog entries for
  those version headers. No dedicated manual page documents the grammar in
  prose; the source and this changelog are the only in-repository
  documentation.

### Grammar

```
"Praat chronological TextGrid text file"      literal quoted tag, first token in file
<f64> xmin  <f64> xmax                        document time domain
<i32> numberOfTiers

numberOfTiers × {
    <quoted string> tierClassName             "IntervalTier" | "TextTier"; anything
                                               else is rejected
    <quoted string> tierName
    <f64> tier.xmin  <f64> tier.xmax
}

then a flat stream of records, in file order, until end of input:
    <i32> tierNumber                          1-based index into the tier list above
    if that tier is an IntervalTier:
        <f64> xmin  <f64> xmax  <quoted string> text
    if that tier is a TextTier:
        <f64> number                          the point's time
        <quoted string> mark
```

Records are written sorted primarily by time and secondarily by tier index
— this is the "chronological" property the format is named for
(`fon/TextGrid_files.cpp:483-534`). The writer emits a `! tierName:` comment
line whenever the tier changes between consecutive records; this is purely
cosmetic — comments starting with `!` are skipped generically by Praat's
object-text tokenizer wherever a number or string is expected, not
specifically parsed by this format. There is no explicit record count or
terminator: end of file is detected on read by catching the "ran out of
input" error that the integer parser raises when asked to read a
`tierNumber` past the last record (`fon/TextGrid_files.cpp:438-446`).

Quoting follows the same convention as every other Praat object-text string:
double-quote delimited, an internal `"` doubled to escape it. Text encoding
of the file as a whole follows the same rules as any other Praat text file
(§6) — ASCII by default, promoted to UTF-16 with a BOM if the content isn't
representable, or whatever the user's output-encoding preference specifies.

## 5. Xwaves / ESPS label file format

This format exists and is supported, but **asymmetrically**: the reader is
considerably more general than the writer, and Praat treats "Xwaves label
file" and "ESPS label file" as the same thing — Xwaves is the ESPS
software suite's waveform editor, and its label format is the general ESPS
label-file convention.

- Read: `TextGrid_readFromEspsLabelFile` (`fon/TextGrid.h:171-175`,
  `fon/TextGrid_files.cpp:91-336`), menu **Read TextGrid from Xwaves...**
  (alias **Read TextGrid from ESPS label file...**,
  `fon/praat_uvafon_init.cpp:3119-3120`). Produces a full `TextGrid`, one
  tier per label-file field.
- Write: `IntervalTier_writeToXwaves` (`fon/TextGrid.h:176`,
  `fon/TextGrid_files.cpp:338-350`), menu **Save as Xwaves label file...**
  (alias **Write to Xwaves label file...**,
  `fon/praat_TextGrid_init.cpp:1652`), registered on `IntervalTier`, not on
  `TextGrid`. **There is no whole-TextGrid Xwaves writer and no point-tier
  writer** — only a single `IntervalTier` can be exported this way in stock
  Praat; a multi-tier export would need to be done tier-by-tier into
  separate files, and there is no built-in command that automates that.
- Version history: originally added as **Read IntervalTier from Xwaves...**
  in 4.3.01 (9 February 2005) — producing a bare `IntervalTier`, later
  generalized to return a full `TextGrid`; the write direction was added in
  4.4.05 (26 January 2006); both directions were refined in 4.4.06
  (30 January 2006) — per `fon/manual_whatsnew.cpp`. No dedicated manual page
  documents the grammar.

### Read-side grammar (the general case Praat accepts)

```
header lines, read until one starts with '#' (the literal end-of-header marker):
    a line starting with "nfields "      sets the tier/field count (default 1)
    a line starting with "separator "    sets the field separator character (default ';')
    any other header line is ignored

data lines, one boundary per line, until end of file:
    <f64 endTime>  ' '  <int colourCode>  ' '  <field_1><sep><field_2><sep>...<field_N>
```

`colourCode` is parsed but discarded. Field count `N` equals the header's
`nfields` (optionally overridden by a caller-supplied parameter). Each field
maps 1:1 to a tier — this multi-field-per-line convention is a Praat/ESPS
extension beyond the single-field label file Xwaves itself traditionally
uses, and is how multiple tiers round-trip through one file.

Interval vs. point reconciliation is controlled by a boolean parameter,
not read from the file: if the caller requests point tiers, each field's
time+label becomes a point directly; otherwise (the default), Praat
synthesizes interval boundaries from consecutive end-times — each line's
time is treated as the *end* of an interval whose *start* is the previous
line's end time, with the very first interval implicitly starting at `0.0`.
A zero-duration line (`start == end`) is merged into the previous interval's
text (joined with `//`) rather than creating a degenerate empty interval.
Encoding on read is UTF-8 (a source comment describes the reader as
"going to be UTF-8-compatible", `fon/TextGrid_files.cpp:97`).

The reader also contains an undocumented `;`-prefixed continuation-line
convention and a narrowly-targeted repair heuristic for 4-tier files
matching the Buckeye corpus's specific label conventions
(`fon/TextGrid_files.cpp:142-256`). This is corpus-compatibility cruft
outside the general ESPS/Xwaves grammar and is not worth replicating for a
clean-room implementation unless Buckeye-corpus compatibility is itself a
goal.

### Write-side grammar (narrower than what the reader accepts)

```
"separator ;\n"
"nfields 1\n"
"#\n"

then one line per interval:
    '\t' <xmax formatted %.6f> ' ' "26" '\t' <text, raw UTF-8, no escaping> '\n'
```

Always single-field (`nfields 1`) — confirming only one `IntervalTier` at a
time is ever exported. The interval's `xmin` (and the tier's own `xmin`) is
never written; because the reader always reconstructs the first interval's
start as `0.0`, round-tripping a tier whose real `xmin` isn't 0 silently
loses that offset. The hardcoded colour code `26` is written for every line;
its significance (if any, in the ESPS/Xwaves convention) is not documented
anywhere in Praat's source or comments. Text is written as raw UTF-8 bytes
with no quoting or escaping of any kind — a label containing a tab, newline,
or the separator character would corrupt the file; the writer does not guard
against this.

## 6. Text encoding: BOM, detection, and legacy 8-bit files

### 6.1 On write

Output encoding is governed by a user preference,
`kMelder_textOutputEncoding` (menu **Text writing settings...**, alias
**Text writing preferences...**, `sys/praat_objectMenus.cpp:287-296`), with
four possible values and **"try ASCII, then UTF-16" as the default**
(`melder/melder_enums.h:68-73`):

| Value | Behavior |
| --- | --- |
| `UTF-8` | Always UTF-8. No BOM. |
| `UTF-16` | Always UTF-16, big-endian, with a leading `FE FF` BOM. |
| `try ASCII, then UTF-16` (**default**) | Writes plain ASCII, no BOM, if every string in the object is representable in ASCII; otherwise promotes the whole file to UTF-16 with a BOM. |
| `try ISO Latin-1, then UTF-16` | Same idea, checking ISO Latin-1 representability instead of ASCII. |

This resolution happens once per file, checking every string field the
object being written contains (`Data_createTextFile`,
`sys/Data.cpp:59-75`; representability check `sys/oo_CAN_WRITE_AS_ENCODING.h`).
Praat's own writer, per §1.4, never emits a narrow string byte ≥ 0x80 even in
the binary format — the "try ASCII" resolution here is the text-format
version of the same underlying rule. UTF-16 output is **always big-endian**;
Praat's own writer never produces UTF-16LE output (`binputu16`, the
unsuffixed/big-endian variant, is what writes both the BOM and the body;
`binputu16LE` exists only for reading/writing other little-endian-specific
formats elsewhere in the codebase, not for object text files).

This preference is honored uniformly for every text file writer that goes
through `Data_createTextFile` — TextGrid included, both long and short
format, and the chronological format (§4), since it also goes through the
shared `Data_createTextFile` entry point (`fon/TextGrid_files.cpp:489`).

### 6.2 On read

Detection happens in two layers.

**Layer 1 — BOM sniff** (`MelderFile_readText`, `melder/melder_files.cpp:974-1088`):
read the first 2–3 bytes.

| Bytes | Encoding |
| --- | --- |
| `FE FF` | UTF-16, big-endian |
| `FF FE` | UTF-16, little-endian |
| `EF BB BF` | UTF-8 (BOM consumed/stripped before decoding) |
| anything else | 8-bit, encoding undetermined — falls through to layer 2 |

A UTF-16 file (either byte order) is decoded directly, with surrogate-pair
combination, once its byte order is known from the BOM — there is no
BOM-less UTF-16 detection.

**Layer 2 — 8-bit fallback, when no UTF-16/UTF-8-with-BOM signal was found**
(`melder/melder_textencoding.cpp:366-433`, invoked with
`kMelder_textInputEncoding::UNDEFINED`, which resolves to the input-encoding
preference below): first, the raw bytes are tested for well-formed UTF-8
(`Melder_str8IsValidUtf8`); if they validate, the file is treated as UTF-8
**with no BOM** — this is Praat's answer to "detecting" BOM-less UTF-8, and
is the case the manual page tells implementers to expect (§2). If the bytes
do **not** validate as UTF-8, Praat falls back to a single fixed 8-bit
codec — **this is not content-based detection at all, it is a user
preference**, `kMelder_textInputEncoding` (menu **Text reading settings...**,
alias **Text reading preferences...**, `sys/praat_objectMenus.cpp:276-285`),
with these values (`melder/melder_enums.h:52-66`):

| Value | Fallback codec if UTF-8 validation fails |
| --- | --- |
| `UTF-8` | none — throws an error instead |
| `try UTF-8, then ISO Latin-1` | ISO Latin-1 (byte value = codepoint) |
| `ISO Latin-1` | ISO Latin-1 unconditionally, UTF-8 not even attempted |
| `try UTF-8, then Windows Latin-1` | Windows-1252-style "Windows Latin-1" table |
| `Windows Latin-1` | same, unconditionally |
| `try UTF-8, then MacRoman` | MacRoman codepage table |
| `MacRoman` | same, unconditionally |

**The default value of this preference is platform-dependent at compile
time**: `try UTF-8, then Windows Latin-1` on a Windows build, `try UTF-8,
then MacRoman` on a macOS build, `try UTF-8, then ISO Latin-1` on every other
build (`melder/melder_enums.h:60-66`, and the same platform switch repeated
as the initialization fallback in `melder/melder_textencoding.cpp:374-381`).

**This directly answers the question of whether Praat can distinguish
MacRoman from Latin-1 for a legacy 8-bit file with no signal beyond its
bytes: it cannot, and doesn't try to.** There is no content-based heuristic
anywhere in this path — the manual page's own text (§2, "it may be difficult
to figure out which is which") is Praat's authors acknowledging exactly this
limitation, not describing an unstated algorithm. Praat's own reader resolves
the ambiguity the same way any single-assumption reader would: by picking one
fixed legacy codec (configurable, defaulting to whichever legacy encoding
matches the platform the Praat binary was built for) and applying it
unconditionally to every BOM-less, non-UTF-8 file. A Rust reader that
defaults to a single fixed 8-bit encoding (as Phonia's currently does, with
Latin-1) is not missing a detection capability Praat has and it lacks —
Praat has no such capability either. What Phonia's reader is missing,
relative to Praat, is the equivalent of Praat's *preference* (letting the
caller specify which legacy 8-bit codec to assume, rather than hard-coding
one) — that is a real, worthwhile gap to close, but "auto-detect MacRoman
from content" is not a thing to build toward, because Praat itself doesn't
do it.

### 6.3 Binary files sidestep this entirely

A binary object file carries no file-level encoding preamble and needs
none, because every string field carries its own narrow/wide-UTF-16 flag
inline (§1.4) — there is no BOM, no encoding preference, and no detection
step anywhere in the binary read/write path. This confirms the phx-textgrid
crate's existing assumption that binary TextGrids sidestep the whole
encoding-detection question by construction.

### 6.4 Line endings

A Windows build sets a per-file `requiresCRLF` flag that rewrites every `\n`
written to `\r\n` on output (`melder/MelderFile.cpp:94,102,122,142,147,164`);
other platforms write plain `\n`. This is a compile-time platform default,
not a user-facing preference. On read, Praat normalizes `\n`, `\r\n`, and
bare `\r` line endings uniformly (`Melder_killReturns_inplace`, invoked in
both the UTF-16 and 8-bit read paths of `MelderFile_readText`, and again in
`MelderReadText_createFromFile`) — consistent with the manual's guidance
(§2) that a reader should accept any of the three conventions.

### 6.5 Version history

Praat's transition from an 8-bit-only text model to Unicode began around
version 4.6.07 (20 June 2007) — the changelog for the immediately following
releases (4.6.08 onward) describes several follow-on fixes attributed to
"large changes in the Praat source code as a result of the transition to
Unicode" (`fon/manual_whatsnew.cpp`, changelog entries near the 4.6.07–4.6.34
version headers). A TextGrid file written by a Praat release meaningfully
older than this predates reliable Unicode support and should be treated as
one of the legacy 8-bit cases in §6.2.

## 7. Corrections to `crates/phx-textgrid/docs/binary-format.md`

The crate's binary-format documentation was derived independently, without
source access, from hexdumps of sample file pairs. It holds up well overall:
the field layout, big-endian claim, absence of padding/checksums, and the
observation that non-ASCII always promotes strings to the wide encoding are
all confirmed correct by source. The corrections below are refinements, not
reversals.

1. **The zero-tier case is now resolved** (was explicitly flagged as
   inferred, not observed). See §3.2. The crate's guess — `0x00` means "no
   tiers, nothing follows" — is confirmed correct as a matter of format
   grammar. The additional fact source adds: stock Praat never actually
   produces this byte value, because every code path that can create a
   TextGrid refuses to leave it tier-less; the reachable "empty" form, if
   Phonia ever needs to emit one, is `0x01` followed by a tier count of
   `0`, matching what `TextGrid_createWithoutTiers` would serialize to.
   Recommend keeping `0x00` acceptance in the reader (for robustness against
   hand-crafted or corrupted input) but not treating it as the form a writer
   aiming for Praat-realistic output should prefer.

2. **The tier-name (and every other `oo_STRING`) length prefix is an
   unsigned 16-bit integer, not a signed one.** The crate's doc describes it
   as `<i16> n`. Source shows the field is `u16`, with the wide-string
   escape being the specific bit pattern `0xFFFF` (65535) — which is
   bit-identical to `-1` when reinterpreted as a signed 16-bit value, so a
   `read i16; if n == -1` implementation produces the same behavior as the
   correct `read u16; if n == 0xFFFF` implementation for every value that
   actually occurs. The distinction matters for a fully spec-compliant
   implementation because it changes how the non-escape range should be
   validated (0..65534, not "any non-negative i16"); it does not change the
   crate's existing behavior on any input it can currently parse correctly.

3. **The narrow/wide promotion rule is "any character outside 7-bit ASCII,"
   not "any character outside Latin-1."** The crate's doc already documents,
   correctly, that Praat promotes any non-ASCII character to the wide
   encoding even when it would fit a single Latin-1 byte (citing the "Café"
   example) — source confirms this precisely and explains the mechanism:
   the writer's gate is `Melder_isValidAscii` (strict ≤ 127), not any
   Latin-1-range check. Recommend rephrasing the doc's "narrow Latin-1
   bytes" language: the narrow *write* path is 7-bit-ASCII-only in every
   file a real Praat produces; describing the narrow branch as "Latin-1" is
   only accurate for how a *reader* should interpret narrow bytes it
   encounters (raw byte value as codepoint), which is a reasonable
   reader-side liberality, not a description of Praat's own write behavior.
   The crate's existing prose already gets this distinction functionally
   right; this is a clarity note, not a behavior change.

4. **The per-tier "class-name string" field is `w8`
   (u8 length + ASCII, with an unreachable-in-practice wide-escape branch
   via a `0xFF` sentinel byte), not a bespoke fixed unsigned-byte-length
   ASCII-only scheme.** The crate's doc describes the common case correctly
   but doesn't mention that the same general string codec used for the
   top-level object class name (§1.3) is reused here, escape branch
   included. Since Praat's own tier class names (`"IntervalTier"`,
   `"TextTier"`) are fixed C identifiers, this branch cannot fire in any
   file Praat itself produces, so this is a completeness note rather than a
   functional bug in the crate's reader.

5. **The tier's "name" field is not actually a `TextTier`/`IntervalTier`
   field at all** — it's the generic `Thing.name` slot, populated by Praat's
   polymorphic-collection writer specifically because the tier list itself
   (`TextGrid.tiers`) is stored as a nullable, runtime-typed single object
   reference (`oo_OBJECT`), not because tiers declare a name field the way
   `TextInterval`/`TextPoint` declare `text`/`mark`. This explains, and
   confirms as intentional rather than arbitrary, the crate's already-correct
   observation that interval and point entries inside a tier carry no
   equivalent per-entry class tag or name — they're serialized through a
   different, homogeneous-collection mechanism that has no such metadata.
   No change needed to the crate's byte-level claims; this is the "why," for
   anyone maintaining the reader later.

6. **A tier's `xmin`/`xmax` being narrower than the document's domain is
   normal, expected TextGrid state, not merely an edge case a sample
   happened to demonstrate.** The crate's doc frames this correctly at the
   byte level but describes it, in tone, as a special case worth noting;
   source shows Praat's own tier-adding code only ever widens the document's
   domain to enclose a new tier and never forces a tier's domain to match
   the document's, and that equalizing domains across tiers is an explicit,
   opt-in operation (`TextGrids_merge`'s `equalizeDomains` flag) rather than
   an invariant Praat otherwise maintains. Worth softening the framing from
   "edge case" to "the normal, unforced state," so a future maintainer
   doesn't treat domain-matching as an invariant to assert on read.

No other discrepancies were found. The fixture-derived field order, endianness,
interval/point layout, and empty-label handling in the existing document all
match source exactly.
