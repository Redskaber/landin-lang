# Stage 15.92 — Explicit Lifetime Tracking

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.216.0 → v0.217.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.92 implements **explicit lifetime tracking**: references with
the same explicit lifetime name (e.g., `'a`) now share the same
`RegionVid` in MIR, instead of each getting a fresh vid.

**What changed**:
- Added `lower_hir_ty_to_mir_ty_with_lifetimes` function in
  `src/mir/lower/mod.rs` — a variant of `lower_hir_ty_to_mir_ty_with_regions`
  that takes a `lifetime_map: HashMap<Symbol, RegionVid>` for deduplication.
- When an explicit lifetime is encountered, the function looks up the
  lifetime name in `lifetime_map`. If found, reuses the existing vid; if
  not found, creates a fresh vid and records it.
- Updated `lower_hir_body_to_mir_full` to create a `lifetime_map` and
  pass it to the new function for param and return type lowering.
- Non-Ref types (Int, Bool, Adt, etc.) delegate to the existing
  `lower_hir_ty_to_mir_ty_with_regions` (no lifetime tracking needed).

**Before** (each `&'a` gets a fresh vid):
```
fn foo<'a>(x: &'a i32, y: &'a i32) -> &'a i32 { x }
// x: Ref(Var(0), _, i32)   — vid 0
// y: Ref(Var(1), _, i32)   — vid 1 (different!)
// return: Ref(Var(2), _, i32) — vid 2 (different!)
```

**After** (same name → same vid):
```
fn foo<'a>(x: &'a i32, y: &'a i32) -> &'a i32 { x }
// x: Ref(Var(0), _, i32)   — vid 0
// y: Ref(Var(0), _, i32)   — vid 0 (same! deduplicated)
// return: Ref(Var(0), _, i32) — vid 0 (same! deduplicated)
```

**Test impact**:
- 2 new unit tests (deduplication + no-deduplication for elided)
- 0 conformance test changes
- **Total: 7604 tests passing** (244 lib [was 242, +2 new] + 2144
  integration + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": explicit lifetimes are tracked by name.
Per §23: `lower_hir_ty_to_mir_ty_with_lifetimes` follows
`<verb>_<noun>_<noun>_<prep>_<noun>` pattern.

## 2. Why This Matters

Before Stage 15.92, each reference with an explicit lifetime got a
fresh `RegionVid`, even if they shared the same lifetime name. This
meant the region inference couldn't enforce that `&'a T` and `&'a U`
have the same lifetime — they were treated as unrelated.

This is the foundation for **region inference activation** (the next
step): with explicit lifetimes correctly deduplicated, the region
inference can now verify that lifetime constraints are satisfied (e.g.,
that a returned `&'a T` doesn't outlive its input `&'a` reference).

## 3. The Implementation

### 3.1 `lower_hir_ty_to_mir_ty_with_lifetimes` function

```rust
pub(crate) fn lower_hir_ty_to_mir_ty_with_lifetimes(
    ty: &HirTy,
    region_counter: &mut u32,
    lifetime_map: &mut HashMap<Symbol, RegionVid>,
) -> Ty {
    match &ty.kind {
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(lt) => {
                    let name = lt.ident.name;
                    if let Some(&existing_vid) = lifetime_map.get(&name) {
                        Region::Var(existing_vid)  // reuse
                    } else {
                        let vid = *region_counter;
                        *region_counter += 1;
                        let rvid = RegionVid(vid);
                        lifetime_map.insert(name, rvid);  // create + record
                        Region::Var(rvid)
                    }
                }
                None => { /* fresh vid for elided */ }
            };
            // ... recursively lower inner type
        }
        // Non-Ref types delegate to lower_hir_ty_to_mir_ty_with_regions
        _ => lower_hir_ty_to_mir_ty_with_regions(ty, region_counter),
    }
}
```

### 3.2 Integration in `lower_hir_body_to_mir_full`

```rust
let mut lifetime_map: HashMap<Symbol, RegionVid> = HashMap::new();

// Param lowering:
let mir_ty = lower_hir_ty_to_mir_ty_with_lifetimes(t, &mut region_counter, &mut lifetime_map);

// Return type lowering:
let raw_return_ty = lower_hir_ty_to_mir_ty_with_lifetimes(t, &mut region_counter, &mut lifetime_map);
```

The `lifetime_map` is created once per body and shared across all param
and return type lowering, ensuring all references with the same lifetime
name get the same vid.

## 4. API Naming Compliance (§23)

**New function**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `lower_hir_ty_to_mir_ty_with_lifetimes(ty, region_counter, lifetime_map)` | `mir::lower::mod` | ✅ `<verb>_<noun>_<noun>_<prep>_<noun>` with `_with_lifetimes` suffix |

Per §23 rule 4 (Re-export): the function is `pub(crate)` — not
re-exported. Callers use the full path.

## 5. §16 Interface Isolation

The new function is entirely within `mir::lower::mod`. It reads only
`HirTy`/`Lifetime`/`Symbol` data (HIR types). The `lifetime_map` is
local to the lowering function — not shared with other stages.

Non-Ref types delegate to `lower_hir_ty_to_mir_ty_with_regions` (no
lifetime tracking needed), maintaining backward compatibility.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | New function wraps existing; non-Ref types delegate |
| D2 Tech Debt | ✅ | Explicit lifetime tracking complete; foundation for region inference |
| D3 Test Coverage | ✅ | 2 new unit tests (dedup + no-dedup) |
| D4 Next-Phase Readiness | ✅ | Region inference can now use correct vids |
| D5 Design Rationality | ✅ | HashMap dedup is the standard approach |
| D6 Performance | ✅ | One HashMap lookup per explicit lifetime; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Dedup + no-dedup paths tested |

**Committee Vote**: GO — Stage 15.92 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS (was 242, +2 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7604 tests passing, 0 failures, 0 warnings.**

## 8. Next Steps for Task 12

Stage 15.92 completes explicit lifetime tracking. Remaining work:

1. **Region inference activation**: The region inference infrastructure
   (Stages 7.1-7.5, 15.48-15.52) needs to use the now-correct region
   vids (deduplicated explicit + elision rules 2/3) to actually check
   lifetime constraints and report errors.

## 9. Version Policy

v0.216.0 → v0.217.0 (minor bump — Phase 3 Task 12 explicit lifetime
tracking + 2 new unit tests).
