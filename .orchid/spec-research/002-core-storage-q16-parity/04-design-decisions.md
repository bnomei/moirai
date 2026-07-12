# Design Decisions

- `EntityId` is an opaque Copy 8-byte `u32` slot + `u32` generation with no public raw conversion;
  `ComponentId` is an Rc-owner-scoped Clone handle.
- The allocator tracks Free, Reserved, Live, and Retired separately from generation.
- World performs entity validation before storage lookup.
- Registration conflicts are transactional contextual errors; only exact repeats are idempotent.
- `ComponentOptions` has private fields and sparse/table/tag constructors.
- Typed tags are zero-sized non-dropping markers; authored untyped tags have no payload.
- Allocator, registry, erased storage, and `SparseSet` remain crate-private.
- `Q16(i32)` is `#[repr(transparent)]` with private bits, exact bit boundaries, checked division,
  named checked/saturating operations, and i64 nearest/half-away multiply/divide rounding.
- f32 construction uses power-of-two scaling and nearest/half-away rounding; NaN is always an
  error, while the explicitly saturating path clamps infinities/range.
- Arithmetic operator traits are omitted so overflow and division policy stay explicit.
- Wyrd domain conversion is deferred to the Wyrd-owned adapter.
- Phase 2 includes an end-to-end sparse-world test using intended public paths.
