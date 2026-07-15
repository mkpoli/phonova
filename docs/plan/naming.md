# Naming

The bare crate `phonix` belongs to an unrelated AGPL wake-word project
(registered June 2026), and `phonix.dev` is taken. The product name and the
crate prefix are separate decisions; the prefix `phx-` is verified free and
serves regardless of the product name. Availability checked 2026-07-16 via
the crates.io API and RDAP.

| Candidate | crates.io | Domains | Notes |
|---|---|---|---|
| **Phonix** (keep) | bare name taken | phonix.dev taken | Established working name; crates.io search will surface the wake-word crate first; trademark-adjacent collision risk low but nonzero |
| **Phonia** | `phonia` free | phonia.app, phonia.dev free (.org taken) | Closest to "phonetics"; -ia reads as a domain/field; clean pronunciation across EN/JA |
| **Phonos** | `phonos` free | .org and .app taken | Greek root, but domain situation weak |
| **Fonix** | `fonix` free | .org/.app taken; Fonix is an existing brand (payments) | Spelling reads informal |
| **Vocalys** | `vocalys` free | unchecked | Leans clinical/voice, under-sells annotation and plotting |

Recommendation: rename to **Phonia** — the bare crate, `phonia.app`, and
`phonia.dev` are all free, so the product, the flagship crate, and the web
demo can share one name; publish crates as `phx-*` anyway so a future rename
never touches the ecosystem. Second choice: keep Phonix as a product-only
name over `phx-*` crates and accept the crates.io search collision.

Decision owner: the user. Until decided, "Phonix" stays the working name in
docs; nothing user-facing ships before v0.1 demo prep, so the decision can
wait until phase 5 without cost. Whichever way it goes, register the bare
crate (`phonia` or nothing) and `phx-core` immediately (crates.io has no
namespace protection; RFC 3243 unimplemented).
