# Stage 14.110 — Gate Review: Data Structure Audit + O(1) HirCrate Lookup + Dead Field Removal

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.123.0 → v0.124.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.110 launches a deep data structure and pipeline architecture review,
then implements the top 2 quick-win optimizations from the audit:

1. **HirCrate owner/body lookup: O(n) → O(1)** via cached OnceCell index
2. **Remove dead `println_messages` field** from MirBody

## 2. Data Structure Audit Results

A general-purpose subagent conducted an exhaustive audit of all core data
structures. Results:

- **11 ✅ OPTIMAL** (DefId, HirId, MirBody.basic_blocks, etc.)
- **24 ⚠️ SUBOPTIMAL** (OwnerId triple-indirection, Ty not Copy, etc.)
- **16 ❌ WRONG** (HirCrate Vec lookup, dyn_trait_calls magic marker, EmitValue=String, etc.)

### Top 10 Recommendations (prioritized)

1. Intern `Ty` to `Ty<'tcx>` (Copy, 8-byte pointer) — 3-4 weeks
2. Switch `SubstsRef` to `&'tcx [GenericArg<'tcx>]` — 2-3 weeks
3. Redesign `TraitResolver` keys to `(DefId, SubstsRef)` — 2 weeks
4. **Convert `HirCrate.owners/bodies` to indexed lookup** — 1 day ✅ DONE
5. Convert `UnificationTable` HashMap → Vec — 1 day
6. Refactor `Terminator` to `struct { kind, span }` — 1 week
7. Consolidate 8 writeback passes → 2 — 1-2 weeks
8. Move `dyn_trait_calls` into `Terminator::Call` — 1 week
9. Add `CrateNum` to `DefId` — 3-4 weeks
10. Replace `EmitValue = String` with typed handle — 4-6 weeks

### Refactoring Optimality Verdict

**⚠️ SUBOPTIMAL — correct in spirit, minimal in execution.**
- P0 bug fixes: ✅ right approach (errors > silent)
- Dead code removal: ✅ mostly right
- HP-1 infrastructure: ⚠️ Option<&ref> design suboptimal
- HP-19/21 spans: ❌ shortcut (fields on BasicBlock vs proper Terminator struct)
- 9 refactors were skipped that should have been done

## 3. What Was Implemented

### Fix #4: HirCrate O(1) Lookup (src/hir/kinds.rs)

**Before**: `owner()` and `body()` did O(n) linear scans of `owners`/`bodies` Vecs.
Called ~50+ times per compile (driver, MIR lower, typeck, codegen all query HIR).

**After**: Added `OnceCell<HashMap<u32, usize>>` cached indexes. First lookup
builds the index in O(n); all subsequent lookups are O(1).

Per Phase 2 audit: "Convert HirCrate.owners/bodies Vec → indexed Vec/FxHashMap —
1 day, O(n²)→O(1), zero API change."

### Fix: Remove Dead `println_messages` Field (src/mir/body.rs)

**Before**: `MirBody.println_messages: Vec<String>` was declared and initialized
but never populated or read. Dead field from Stage 13.12 that was superseded
by `StatementKind::Println` (Stage 13.13).

**After**: Field removed. Cleaner MirBody struct.

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 5. Stage Verdict

**PASS** — Data structure audit complete. 2 quick-win optimizations implemented.
All tests pass. No regressions. Foundation for deeper v0.2 refactoring.

v0.124.0: minor bump (data structure audit + O(1) HirCrate lookup + dead field removal)
