# phx-annot

An annotation model for speech and phonetics. A document holds ordered tiers
over a finite time domain: interval tiers with contiguous, boundary-shared
spans and point tiers with strictly increasing points. Tiers carry typed
relations — aligned boundaries between interval tiers, and parent/child
nesting — that `validate` checks for cross-tier integrity.

Every mutation (insert or remove a boundary, move a boundary or point, edit a
label, reorder a tier) returns a payload whose `inverse()` reconstructs the
operation that undoes it, so a caller can build undo/redo without snapshotting
the whole document.

## Example

```rust
use phx_annot::{Annotation, TierRelation};

let mut doc = Annotation::new(0.0, 2.0)?;
let tier = doc.add_interval_tier("words", TierRelation::Independent)?;

// Split the tier's single interval at 1.0 s.
let split = doc.insert_boundary(tier, 1.0)?;

// Undo it with the inverse the split reported.
doc.apply_inverse(&split.inverse_insert())?;

assert!(doc.validate().is_empty());
# Ok::<(), phx_annot::AnnotationError>(())
```

## Compatibility

Requires Rust 1.88 or newer (edition 2024).

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE) at
your option.
