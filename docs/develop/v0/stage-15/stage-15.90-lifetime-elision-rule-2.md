# Stage 15.90 — Lifetime Elision Rule 2 (Output Lifetime = Input Lifetime)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.214.0 → v0.215.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.90 implements **Lifetime Elision Rule 2** (RFC 141): if a
function has exactly one input lifetime (elided or explicit), that
lifetime is assigned to all elided output lifetimes.

This is the first stage of **Task 12 (Lifetime elision)** — the last
remaining P1 task for v0.2 release.

**What changed**:
- Modified `lower_hir_body_to_mir_full` in `src/mir/lower/mod.rs` to
  lower param types BEFORE the return type, collecting their region vids.
- Added `collect_region_vids` helper — recursively collects all
  `RegionVid`s from a `Ty`'s reference types.
- Added `apply_elision_rule_2` helper — if there's exactly one input
  lifetime, replaces all output lifetimes with that vid.
- Param types are now lowered once (reused for both elision collection
  and local allocation), ensuring region vids match.

**Rust Elision Rules (RFC 141)**:
1. Each elided input lifetime gets its own fresh lifetime. ✅ (Stage 15.49)
2. If there's exactly one input lifetime, it's assigned to all elided
   output lifetimes. ✅ **(Stage 15.90 — this stage)**
3. If there are multiple input lifetimes but one is `&self`/`&mut self`,
   that lifetime is assigned to all elided output lifetimes. ⏳ (deferred)

**Test impact**:
- 5 new Rust unit tests (collect_region_vids × 2, apply_elision_rule_2 × 3)
- 0 conformance test changes
- **Total: 7601 tests passing** (241 lib [was 236, +5 new] + 2144
  integration + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
Per §23 (API Naming): `collect_region_vids` and `apply_elision_rule_2`
follow `<verb>_<noun>` pattern.

## 2. Why This Matters

Before Stage 15.90, the MIR lowerer assigned a **fresh** `RegionVid` to
every reference type — both input AND output. This meant:

```landin
fn first(arr: &[i32]) -> &i32 { &arr[0] }
```

Would produce MIR with:
- `arr: Ref(Region::Var(0), _, i32)` — input lifetime vid 0
- return: `Ref(Region::Var(1), _, i32)` — output lifetime vid 1

The region inference would then see no relationship between vid 0 and
vid 1, so it couldn't verify that the returned reference doesn't outlive
the input. This is semantically incorrect — in Rust, the elision rule
makes the output lifetime equal to the input lifetime, so the borrow
checker can verify the reference is valid.

Stage 15.90 fixes this: with exactly one input lifetime, the output
lifetime is replaced with the input lifetime's vid, so the region
inference sees the correct relationship.

## 3. The Implementation

### 3.1 Reordered lowering: params before return

Previously, `lower_hir_body_to_mir_full` lowered the return type first,
then params. This meant the return type's region vids were allocated
before the params', making it impossible to apply elision rule 2.

Stage 15.90 reorders: params are lowered first (collecting their region
vids), then the return type is lowered and `apply_elision_rule_2` is
called with the collected input vids.

### 3.2 `collect_region_vids` helper

```rust
fn collect_region_vids(ty: &Ty, vids: &mut Vec<RegionVid>) {
    match &ty.kind {
        TyKind::Ref(region, _, inner) => {
            if let Region::Var(vid) = region {
                vids.push(*vid);
            }
            collect_region_vids(inner, vids);
        }
        // ... RawPtr, Array, Slice, Tuple, FnPtr (recursive)
        _ => {}
    }
}
```

### 3.3 `apply_elision_rule_2` helper

```rust
fn apply_elision_rule_2(return_ty: &Ty, input_vids: &[RegionVid]) -> Ty {
    // Rule 2 applies only when there's exactly one input lifetime.
    if input_vids.len() != 1 {
        return return_ty.clone();
    }
    let target_vid = input_vids[0];
    // Recursively replace all region vids in the return type.
    replace_regions(return_ty, target_vid)
}
```

### 3.4 Param type reuse

Param types are lowered once and stored in `lowered_param_types: Vec<Option<Ty>>`.
The param local allocation loop reuses these types instead of re-lowering,
ensuring the region vids match what was collected for elision.

## 4. API Naming Compliance (§23)

**New private functions**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `collect_region_vids(ty, vids)` | `mir::lower::mod` | ✅ `<verb>_<noun>` |
| `apply_elision_rule_2(return_ty, input_vids)` | `mir::lower::mod` | ✅ `<verb>_<noun>_<number>` |

Both are private (`fn`, not `pub fn`). No public API changes.

## 5. §16 Interface Isolation

The new functions are entirely within `mir::lower::mod`. They read only
`Ty`/`TyKind`/`Region`/`RegionVid` data (MIR types). No HIR lookup, no
resolver access, no borrowck access.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Param-before-return ordering enables elision |
| D2 Tech Debt | ✅ | First step of Task 12 (Lifetime elision) |
| D3 Test Coverage | ✅ | 5 new unit tests cover all paths |
| D4 Next-Phase Readiness | ✅ | Foundation for rule 3 (self) + full region checking |
| D5 Design Rationality | ✅ | Follows RFC 141 elision rules |
| D6 Performance | ✅ | One extra pass over param types; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | collect_region_vids + apply_elision_rule_2 fully tested |

**Committee Vote**: GO — Stage 15.90 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 241/241 PASS (was 236, +5 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7601 tests passing, 0 failures, 0 warnings.**

## 8. Next Steps

Stage 15.90 implements elision rule 2. Remaining work for Task 12:

1. **Rule 3 (self)**: If there are multiple input lifetimes but one is
   `&self`/`&mut self`, that lifetime is assigned to all elided output
   lifetimes. Requires tracking which param is self.
2. **Explicit lifetime tracking**: Currently, explicit lifetimes (`'a`)
   each get a fresh vid — references with the same lifetime name should
   share a vid. Requires HIR lifetime name → vid mapping.
3. **Region inference activation**: The region inference infrastructure
   (Stages 7.1-7.5, 15.48-15.52) needs to use the now-correct region
   vids to actually check lifetime constraints.

## 9. Version Policy

v0.214.0 → v0.215.0 (minor bump — Phase 3 Task 12 Lifetime elision rule 2
+ 5 new unit tests).
