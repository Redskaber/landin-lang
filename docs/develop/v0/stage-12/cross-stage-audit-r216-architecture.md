# Cross-Stage Architecture Audit (r216) — D1 + D5

> **Auditor**: ARCH-A | **Date**: 2026-07-26 | **Baseline**: v0.21.0
> **Scope**: §16 interface isolation, §21 cross-stage data flow, §25.8 design deviation
> **Coverage**: D1 (architecture health) + D5 (design rationality) + §21 cross-stage audit

---

## 1. Executive Summary

The Landin Stage 0 compiler exhibits a **healthy, well-disciplined pipeline architecture**.
The data flow is strictly one-directional (`source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen → .ll`),
the §16 §21.3 grep checklist passes 4/5 items cleanly, and the design docs (`06-mir.md`, `07-codegen.md`, `03-type-system.md`, `04-ownership-borrowing.md`)
all carry current §25.8 write-back sections (§14/§15 §11/§12/§13 §10/§11/§12). All 7 large files (≥1000 LOC) are cohesive single-responsibility units;
none exceed the 1500 LOC ceiling.

**One new §16 violation** was uncovered that escaped prior audits: `src/mir/dyn_trait.rs:160` calls `crate::codegen::emit_dynptr_global_text()` —
a reverse-direction dependency (MIR → codegen) that should be relocated to `codegen/`. Additionally, `typeck`/`borrowck` retain
`#[deprecated]` `check_crate(hir: &HirCrate, …)` legacy entry points that read HIR directly (properly marked, not active in `driver::compile`).

For Stage 1 self-hosting readiness, **Stage 0 has critical B1 gaps**: closure call lowering is incomplete
(closures parse + capture but cannot be called), `if let`/`while let`/HRTB/`macro_rules!`/`impl Trait` return position are not yet implemented.
These are documented as FAIL tests in the conformance suite but block the v0.3 bootstrap target.

- **Count of §16 violations**: 1 active (mir::dyn_trait → codegen) + 2 deprecated entry points (properly marked, not active)
- **Count of design deviations**:
  - B1 (impl < design): 18 (most documented as "v0.2/v0.3+" in design docs' §25.8 sections; 1 newly uncovered — `TyKind::Dynamic`/`TraitObject` missing)
  - B2 (impl > design): 0
  - B3 (impl ≠ design, accepted): 7 (all documented + accepted in design docs)
  - B4 (design gray area, written back): 3 (all §25.8 write-back complete)
- **Recommendation**: **GO-WITH-CONDITIONS** for v0.1 release (already shipped); the §16 violation should be filed as P2 tech-debt (≤3 files to fix). For v0.3 self-hosting target: NO-GO until Stage 0 closes the 5 critical B1 gaps (closures-callable, if-let, while-let, HRTB, macro_rules!).

---

## 2. D1 — Architecture Health

### 2.1 §16 Interface Isolation Compliance

| Check | Verification | Result | Notes |
|-------|--------------|--------|-------|
| codegen → `crate::mir::lower` | `grep -rn "crate::mir::lower" src/codegen/` | ✅ ZERO (matches are comment-only, see `src/codegen/mod.rs:7`) | Active code path is clean |
| codegen → `crate::typeck` | `grep -rn "crate::typeck" src/codegen/` | ✅ ZERO (matches are comment-only, see `src/codegen/mod.rs:8`) | Active code path is clean |
| codegen → `crate::driver` | `grep -rn "crate::driver" src/codegen/` | ✅ ZERO (data-type refs only: `crate::driver::CompileResult`, `crate::driver::BodyMeta`) | Per §16.2 #1, type-only references are allowed data contracts |
| Glob exports `pub use X::*` | `grep -n "pub use .*::\*" src/hir/mod.rs src/mir/mod.rs` | ✅ ZERO (matches are comments confirming explicit re-exports) | `src/hir/mod.rs:17` + `src/mir/mod.rs:15` document Stage 3.57 P0-3 fix |
| driver is sole HIR reader (active path) | `grep -rn "HirCrate" src/` filtered by active entry points | ✅ PASS for active path | `driver::compile` (line 284) is the only active caller that reads `HirCrate`; `resolve::resolve_crate`, `traits::TraitResolver::collect`, `mir::lower::*` all receive `&HirCrate` as driver-passed data (allowed per §16.2.1 — upstream→downstream data flow). |

**Deprecated legacy entry points** (technical debt, properly marked):

| Location | Status | Note |
|----------|--------|------|
| `src/typeck/checker.rs:902` `pub fn check_crate(hir: &HirCrate, …)` | `#[deprecated(note = "Use TypeChecker::check_mir_body_with_tables (§16-compliant) or driver::compile instead")]` | Per §23.6, properly deprecated |
| `src/typeck/checker.rs:392` `check_mir_body_with_hir(_, _hir: Option<&HirCrate>)` | `#[deprecated(note = "Use check_mir_body_with_tables instead")]` | Param is `_hir` (unused) |
| `src/typeck/checker.rs:85` `populate_fn_sigs(&mut self, _hir: &HirCrate)` | `#[deprecated(note = "Set fn_sigs directly from FnSigTable instead")]` | Body is `{}` |
| `src/borrowck/mod.rs:591` `pub fn check_crate(hir: &crate::hir::HirCrate, …)` | `#[deprecated(note = "Use BorrowChecker::check_mir_body (§16-compliant) or driver::compile instead")]` | Per §23.6, properly deprecated |
| `src/typeck/mod.rs:32-33` | `#[allow(deprecated)] pub use checker::{check_crate, check_mir_body, TypeChecker};` | Re-export shims for backward compatibility |

These four legacy entry points are technical debt: they pre-date the §16-compliant `check_mir_body_with_tables` API (Stage 3.60+).
They are properly deprecated per §23.6 with explicit pointers to §16-compliant alternatives, and are **not invoked by `driver::compile`**.
Recommendation: schedule for removal in v0.3+ once external test code (Stage 5.48+ conformance expansions) is migrated.

### 2.2 Data Flow Integrity

The intended pipeline (per §21.4 D1-D8) is:

```
source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen → .ll
```

**Reverse-direction checks** (must be zero in `src/`):

| Reverse direction | `grep` result | Status |
|-------------------|---------------|--------|
| `parser → codegen` | zero matches | ✅ |
| `typeck → codegen` | zero matches | ✅ |
| `borrowck → codegen` | zero matches | ✅ |
| `mir → codegen` (forward!) | **1 match** in `src/mir/dyn_trait.rs:160` | ❌ §16 violation |
| `hir → mir::lower` | zero matches | ✅ |
| `lexer/parser/ast → anything downstream` | zero matches | ✅ |

**§16 violation detail** — `src/mir/dyn_trait.rs:159-165`:

```rust
/// Stage 5.63: Convert a `DynTraitFatPtr` to LLVM IR text.
pub fn emit_dyn_trait_fat_ptr_text(fat_ptr: &DynTraitFatPtr) -> String {
    crate::codegen::emit_dynptr_global_text(
        &fat_ptr.dynptr_symbol,
        &fat_ptr.data_symbol,
        &fat_ptr.vtable_symbol,
    )
}
```

The module-level comment (line 142-144) tries to justify this as "one-way: mir → codegen, no circular dependency".
But per §16, MIR is upstream of codegen — MIR producing LLVM IR text is itself a §16 violation
(upstream stage producing downstream stage's output). The function should be relocated to `src/codegen/trait_dispatch.rs`
next to its callee `emit_dynptr_global_text`, and `src/mir/mod.rs:49` re-export removed. **Fix scope: ≤3 files** (mir/dyn_trait.rs, mir/mod.rs, codegen/trait_dispatch.rs), qualifying for §16.5.1 in-stage fix.

Caller inventory (from `grep`): `emit_dyn_trait_fat_ptr_text` is called by `emit_dyn_trait_fat_ptrs_text_batch` (same file, line 188),
by `mir/dyn_trait.rs:780` (test), and by `tests/v0/stage5/plan/dyn_trait_fat_ptr_batch_tests.rs`. No production codegen path uses it.

**D1-D8 verification** (per §21.4 data flow checkpoints):

| Checkpoint | Verification | Status |
|-----------|--------------|--------|
| D1 tokenize → `Vec<Token>` | `src/driver.rs:289` `let (tokens, lex_errors) = tokenize(src, &mut interner);` | ✅ |
| D2 parse_crate → `Crate<ast::Item>` | `src/driver.rs:297` `let krate = parser.parse_crate();` | ✅ |
| D3 lower_crate → `HirCrate` | `src/driver.rs:304` `let mut hir = lower_crate(&krate, &interner);` | ✅ |
| D4 resolve_crate → mutates HIR | `src/driver.rs:307` + `scan_for_unresolved_paths(&hir, …)` (line 313) | ✅ |
| D5 lower_hir_body_to_mir_full → `MirBody` + `UnificationTable` | `src/driver.rs:411` `lower_hir_body_to_mir_full_with_dyn_trait_plan(…)` | ✅ |
| D6 TypeChecker::check_mir_body_with_tables → resolved types | pre-computed `FieldTyTable` + `FnSigTable` (driver.rs:318-362) | ✅ |
| D7 BorrowChecker::check_mir_body → borrow errors | (in driver per-body loop, post-MIR-lower) | ✅ |
| D8 codegen_crate → LLVM IR String | `src/driver.rs` calls `codegen::codegen_crate(&result)` indirectly via `compile_expect_ok`/`compile_expect_errors` test helpers | ✅ |

**Error-path coverage**: `driver::CompileResult::has_errors()` (line 252) is checked at `driver.rs:796` and `driver.rs:819`
(in `compile_expect_ok` / `compile_expect_errors` test helpers). The `compile()` entry point (line 284) returns early on lex/parse errors
(lines 291-293, 299-301) but does NOT short-circuit on type/borrow errors — by design, MIR is still produced for partial-result analysis
(comment lines 280-283). `codegen_crate` does not have a `gen_ll_unchecked` variant; there are zero `gen_ll_unchecked` references anywhere.

### 2.3 Large File Analysis

LOC count from `wc -l` (90 source files total):

| File | LOC | Cohesive? | Single Responsibility? | Refactor Recommendation |
|------|-----|-----------|------------------------|------------------------|
| `src/borrowck/region_inference.rs` | 1462 | ✅ Yes — single algorithm (region inference data structures + constraint collection + fixed-point iteration) | Yes — TD-015 (Stage 7.1) explicitly extracted this from `borrowck/mod.rs` for §14.4 single-responsibility | **No split**. 66 fns all relate to region inference. Near 1500 LOC ceiling — monitor but don't preemptively split. |
| `src/mir/lower/expr_operand.rs` | 1279 | ✅ Yes — expression lowering algorithm only (5 entry fns: `lower_expr_to_place`, `lower_expr_to_operand`, `build_dyn_trait_call_terminator`, `resolve_enum_variant`, etc.) | Yes — TD-011 (Stage 6.10) explicitly extracted algorithm from `mir/lower/mod.rs` infrastructure | **No split**. The 5 pub fns + 1274 lines of internal helpers form one cohesive algorithm. |
| `src/borrowck/mod.rs` | 1205 | ✅ Yes — BorrowChecker struct + `check_mir_body` entry point + tests | Yes — TD-024 (Stage 6.14) already extracted `liveness.rs`, `copy_semantics.rs`, `place_path.rs` | **No split**. ~50% is inline tests; production logic is ~600 LOC. |
| `src/typeck/checker.rs` | 1156 | ✅ Yes — TypeChecker struct + `check_mir_body_with_tables` entry | Yes — TD-025 (Stage 6.15) already extracted `tables.rs`, `predicates.rs` | **No split**. ~40% is inline tests; production logic is ~700 LOC. |
| `src/stdlib/trait_methods.rs` | 1103 | ✅ Yes — static registry of stdlib trait method signatures + query API (29 fns all query-related) | Yes — TD-stdlib-split extraction from `stdlib/mod.rs` | **No split**. All 29 fns are queries over the same static registry. |
| `src/codegen/mod.rs` | 1058 | ✅ Yes — codegen entry points (`codegen_crate`, `codegen_from_mir`, `codegen_dyn_trait_call`) + re-exports | Yes — Stage 6.7 already extracted `trait_dispatch.rs` (962 LOC); `text_emitter.rs` (650 LOC); `mir_translation.rs` (487 LOC) | **No split**. 8 pub fns + re-exports. |
| `src/parser/expr.rs` | 1047 | ✅ Yes — Pratt expression parser (23 fns all `parse_*_expr`) | Yes — TD-022 (Stage 6.12) extracted from `parser.rs` | **No split**. Pratt parser requires all precedence levels in one file. |

**All 7 large files are cohesive and below the 1500 LOC ceiling.** None require immediate refactoring.
The Stage 6.10-6.15 extractions (TD-011/017/022/024/025) have already moved algorithm bodies out of the `mod.rs` infrastructure files,
leaving each large file as a single algorithm or single data structure.

### 2.4 Module Boundary Issues

**New coupling that emerged since Stage 3.63 §21 audit**:

1. **`mir::dyn_trait` → `codegen::emit_dynptr_global_text`** (the §16 violation flagged in §2.2 above).
   The `mir/dyn_trait.rs` module (954 LOC, 9 sub-sections from Stage 5.61-5.80) accumulates many functions
   that bridge MIR data structures and codegen text emission: `emit_dyn_trait_fat_ptr_text`,
   `emit_dyn_trait_fat_ptrs_text_batch`, `emit_dyn_trait_method_call_text`, `emit_dyn_trait_mir_plan_text`,
   `emit_dyn_trait_method_calls_text_batch`, `emit_dyn_trait_method_calls_text_batch_from_resolver`,
   `emit_dyn_trait_fat_ptrs_text_batch_from_resolver` (7 `emit_*` functions in MIR!).
   These functions belong in `codegen/`, not `mir/`. **Recommendation**: relocate the 7 `emit_*` functions
   from `mir/dyn_trait.rs` to `codegen/trait_dispatch.rs` (or a new `codegen/dyn_trait_emit.rs` submodule),
   then update `mir/mod.rs` re-exports and `lib.rs` re-exports.

2. **Driver calls `mir::lower::lower_hir_ty_to_mir_ty` directly** (`src/driver.rs:324, 340, 347`) to pre-compute
   `FieldTyTable` and `FnSigTable`. This is `driver → mir::lower` (allowed per §16.6 driver exception), but it
   calls an internal `pub(crate) fn lower_hir_ty_to_mir_ty` (defined at `src/mir/lower/mod.rs:706`).
   This is a known architectural compromise: driver does the metadata pre-computation that typeck used to do,
   so typeck no longer needs to read HIR. Acceptable as-is; no action needed.

3. **`typeck::checker::TypeChecker::fn_sigs: HashMap<crate::hir::DefId, Sig>`** (line 53) — typeck stores
   a `DefId → Sig` map populated from the pre-computed `FnSigTable` (which driver builds). This is §16-compliant
   (DefId is a shared ID type per §16.2.3), but the typeck struct still has the legacy `hir_to_local: HashMap<HirId, LocalId>`
   (line 49) and `populate_fn_sigs` deprecated method (line 85). These are deprecated remnants and should be removed
   when the deprecated `check_crate` / `check_mir_body_with_hir` paths are removed.

---

## 3. D5 — Design vs Implementation Deviation (§25.8)

The four design docs (`03-type-system.md`, `04-ownership-borrowing.md`, `06-mir.md`, `07-codegen.md`) all carry
current §25.8 write-back sections. This audit confirms the documented deviations and identifies one previously-uncovered
B1 deviation (`TyKind::Dynamic`/`TraitObject` not modeled in `mir/ty.rs`).

### 3.1 `06-mir.md` Deviations

Verified against `src/mir/body.rs`, `src/mir/mod.rs`, `src/mir/place.rs`, `src/mir/ty.rs`, `src/mir/lower/*`, `src/mir/dyn_trait.rs`.

| Design Section | Deviation Type | Description | Optimal | Refactor? | Action |
|---------------|---------------|-------------|---------|-----------|--------|
| §2 `Body.source_scopes: IndexVec<SourceScope, SourceScopeData>` | B1 | Not implemented (per §14.1) — impl uses `LocalDecl.source_info: Span` simplification | design | no (deferred to v0.2 unwind stage) | Already documented in §14.1 |
| §2 `Body.arg_count: usize` | B1 | Not implemented (per §14.1) — convention: params are local 1..N | impl (simpler) | no | Already documented in §14.1 |
| §2 `Body.spread_arg: Option<Local>` | B1 | Not implemented (per §14.1) — v0.2 extern ABI | design | no (deferred to v0.2) | Already documented in §14.1 |
| §2 `BasicBlockData.is_cleanup: bool` | B1 | Not implemented (per §14.1) — v0.2 unwind | design | no (deferred to v0.2) | Already documented in §14.1 |
| §2 `BasicBlockData.terminator: Option<Terminator>` | B3 | Impl uses `Terminator::Unreachable` default (per §14.1) | impl | no | Already documented in §14.1 (accepted as permanent) |
| §2 `LocalDecl.is_temp` / `is_arg: bool` | B1 | Not implemented (per §14.1) — convention-based | impl (simpler) | no | Already documented in §14.1 |
| §2 `Body.adt_layouts: HashMap<DefId, AdtLayout>` | equal | ✅ Implemented (Stage 3.47, `src/mir/body.rs:59`) | equal | no | None |
| **NEW**: `MirBody.dyn_trait_calls: Vec<DynTraitMethodCall>` side-table | B4 | Not in design §2; added in Stage 5.78 | impl | no | **Already written back** in §14.2 (dyn Trait lowering algorithm) |
| **NEW**: `src/mir/dyn_trait.rs::emit_dyn_trait_fat_ptr_text` calls `crate::codegen::emit_dynptr_global_text` | B4 + §16 violation | 7 `emit_*` functions in `mir::dyn_trait` produce LLVM IR text — should live in `codegen` | design (MIR should not produce codegen output) | **yes (≤3 files)** | **NEW finding — needs §25.8 write-back** to `06-mir.md` §14 + relocation refactor |

### 3.2 `07-codegen.md` Deviations

Verified against `src/codegen/mod.rs`, `src/codegen/emitter.rs`, `src/codegen/text_emitter.rs`, `src/codegen/trait_dispatch.rs`, `src/codegen/mir_translation.rs`.

| Design Section | Deviation Type | Description | Optimal | Refactor? | Action |
|---------------|---------------|-------------|---------|-----------|--------|
| §4.1 Local mapping (`alloca` + `load`/`store`) | B3 (accepted) | Impl emits `alloca` for all locals; relies on LLVM `mem2reg` for SSA | impl (industry standard) | no | Already documented (L1 closure in `src/codegen/mod.rs:24-27`) |
| §6 Drop glue | B1 (now ✅ Stage 8.4) | Was not implemented through Stage 7; ✅ implemented in Stage 8.4 via `borrowck::drop_elaboration::DropElaborator` | equal | no | Already documented in §15.1 |
| §7 vtable layout | B4 (written back) | Not in original design §7; added in Stage 5.6-5.7 | impl | no | **Already written back** in §14.1 (Trait dispatch codegen subsystem) |
| §8 Closure codegen | B1 (partial) | Closure type lowering + capture analysis Stage 4.7-4.9; full call lowering deferred (per `src/mir/lower/expr_operand.rs:876` comment) | design | yes (Stage 4.10 / v0.3) | **CRITICAL for Stage 1** — see §3.5 below |
| §10 Linking (static/dynamic library) | B1 | Not implemented — MVP produces `.ll` only | design | no (v0.3+) | Already documented in §15.1 |
| §11 DWARF debug info | B1 | Not implemented | design | no (v0.3+) | Already documented in §15.1 |
| §13 C++/Rust interop | B1 | Not implemented | design | no (v0.2+) | Already documented in §15.1 |
| §1 Monomorphization collection | B1 | Not implemented — MVP generates one IR per HIR fn, no mono-item collection | design | no (v0.3+) for self-hosting | **Needs Stage 13 planning** — Stage 1 generics require monomorphization |
| §9.2 Mangling (`_LND3foo...`) | B1 | Not implemented — MVP uses `landin_<Type>_<method>` simple naming | design | no (v0.3+) | Documented limitation |

### 3.3 `03-type-system.md` Deviations

Verified against `src/ast/kinds.rs`, `src/hir/kinds.rs`, `src/mir/ty.rs`, `src/typeck/*`, `src/traits/*`.

| Design Section | Deviation Type | Description | Optimal | Refactor? | Action |
|---------------|---------------|-------------|---------|-----------|--------|
| §1.1 Type hierarchy — `TraitObject (dyn Trait + 'a)` | **B1 (NEW finding)** | `src/mir/ty.rs:28` `TyKind` enum has **no** `Dynamic` / `TraitObject` variant. `dyn Trait` is represented only as `DynTraitFatPtr` data in `mir/dyn_trait.rs` (a side-table), not as a first-class `Ty`. | design (proper Ty variant needed for type-system completeness) | yes (v0.3+) | **NEW finding — needs §25.8 write-back** to `03-type-system.md` §10/§11/§12 |
| §1.1 `ImplTrait (impl Trait)` | B1 | Not implemented at type level (`TyKind` has no `ImplTrait` variant) | design | no (v0.2 per §1.1) | Documented (param position only is parsed; return position is `❌ v0.2`) |
| §1.2 `?Sized` bound | B1 (partial) | Parser may accept syntax; full `?Sized` enforcement not implemented (per `04 §9` "MVP 部分支持") | design | no (v0.3+) | Already documented in §11.2 / §12 |
| §5.6 Orphan rule | B1 | Not implemented | design | no (v0.2+) | Already documented in §10.2 |
| §5.8 Canonical query / depth limit | B1 (B3 simplified) | Impl uses direct query, no canonical form | design | no (v0.2+) | Already documented in §10.2 |
| §5.9 `?` operator + From uniqueness | B1 | Not implemented | design | no (v0.2+) | Already documented in §10.2 |
| §5.10 Deferred trait constraint | B1 | Not implemented | design | no (v0.2+) | Already documented in §10.2 |
| §7 Associated type normalization | B1 | Not implemented (algorithm in §7.1 with depth limit + cycle detection not coded) | design | yes (v0.3+) — **critical for trait dispatch** | Already documented in §10.3 |
| §8 Subtyping rules | B3 (simplified) | Impl uses `typeck::predicates::can_coerce` coercion matrix instead of full variance lattice | impl (simpler, MVP-sufficient) | no | Already documented in §10.3 |
| §4.6 Integer fallback (i32 default) | equal | ✅ Implemented (`typeck/checker.rs` end of `check_mir_body`) | equal | no | None |
| §2.3 Object safety rules | B1 → ✅ | Stage 8.2 implemented (`traits/object_safety.rs`) | equal | no | Already documented in §12.1 |
| §3.2 Lifetime elision rules | B1 → ✅ | Stage 8.1 implemented (`typeck/lifetime_elision.rs`) | equal | no | Already documented in §12.1 |

### 3.4 `04-ownership-borrowing.md` Deviations

Verified against `src/borrowck/mod.rs`, `src/borrowck/region_inference.rs`, `src/borrowck/drop_elaboration.rs`, `src/borrowck/move_tracker.rs`, `src/borrowck/liveness.rs`, `src/borrowck/borrow_set.rs`.

| Design Section | Deviation Type | Description | Optimal | Refactor? | Action |
|---------------|---------------|-------------|---------|-----------|--------|
| §2.4 Two-phase borrows (method-call subset) | B1 | Not implemented (per §11.1) — MVP not supported, but R6 report says MVP **must** support method-call subset | design | yes (v0.3+) — **blocker for `vec.push(vec.len())` pattern** | Already documented in §11.7; **needs Stage 13 prioritization** |
| §3.1 Lifetime parameter annotation | B3 (simplified) | Parser parses, typeck uses `Region::Erased` (Stage 7 activates region inference as no-op) | impl (MVP) | no | Already documented in §11.2 |
| §3.2 Lifetime elision (RFC #141) | B1 → ✅ | Stage 8.1 implemented (`LifetimeElisionCtxt`) | equal | no | Already documented in §13.1 |
| §3.4 Lifetime bound | B1 → ✅ | Stage 7 region inference implemented | equal | no | Already documented in §12.1 |
| §4.1 NLL data structures (BorrowSet / MoveTracker) | equal | ✅ Implemented (`borrowck/borrow_set.rs`, `borrowck/move_tracker.rs`) | equal | no | None |
| §4.2 Algorithm 3-phase | B3 (simplified) | Impl merges liveness + maybe-init + borrow analysis (no separate constraint-collection phase) | impl (MVP) | no | Already documented in §11.3 |
| §4.3 Liveness analysis | equal | ✅ Implemented (`borrowck/liveness.rs::compute_last_use_map`) | equal | no | None |
| §4.4 Maybe-initialized places | B3 (simplified) | Tracked implicitly via `StorageLive`/`StorageDead` statements | impl (MVP) | no | Already documented in §11.3 |
| §4.5 Move tracking | equal | ✅ Implemented (`borrowck/move_tracker.rs::MoveTracker`) | equal | no | None |
| §4.6 NLL full spec (universal region / implied bounds / universe / type tests / SCC) | B1 → ✅ | Stage 7.1-7.5 fully implemented (`borrowck/region_inference.rs`, 1462 LOC) | equal | no | Already documented in §12.1 |
| §5.1-5.3 Drop check | B1 → ✅ | Stage 8.4 implemented (`borrowck/drop_elaboration.rs::DropElaborator` + `needs_drop`) | equal | no | Already documented in §13.1 |
| §5.4 Drop order (reverse declaration) | B3 (simplified) | Impl uses scope-order drop, not strict reverse-declaration | impl (MVP) | no | Already documented in §11.4 |
| §6 Borrow error diagnostics | B3 (simplified) | Basic error info, no suggested fix | impl (MVP) | no (v0.3+) | Already documented in §11.5 |
| §8 Disjoint closure captures (RFC 2229) | B1 | Not implemented (per §11.6) — R6 report says MVP **must** implement | design | yes (v0.3+) — **critical for self-hosting** | Already documented in §11.7; **needs Stage 13 prioritization** |
| §5.3 `#[may_dangle]` attribute | B1 | Not implemented (per §13.1) | design | no (v0.3+) | Already documented in §13.1 |

### 3.5 `13-stage1-feature-whitelist.md` vs Stage 0 Capabilities

The whitelist defines the **contract** between Stage 0 (must-implement) and Stage 1 (allowed-to-use).
Cross-referenced against conformance FAIL tests (820 total across categories: 79 parse + 221 typeck + 268 borrowck + 10 codegen + 27 e2e + 163 soundness + 17 stdlib + 32 integration = 817 unique FAIL markers).

| Stage 1 Need (§13 ref) | Stage 0 Status | Evidence | Deviation | Action |
|------------------------|---------------|----------|-----------|--------|
| §2.5 Closure call (Fn/FnMut/FnOnce) | ❌ NOT CALLABLE | v0.1-release.md §5: "Closures not callable in compile pipeline"; `src/mir/lower/expr_operand.rs:876`: "Closure call lowering: closure calls still go through regular Call" (deferred Stage 4.8+); 41 FAIL tests in `01-typecheck/03-closures/`, `02-borrowck/03-closure-capture/`, `04-e2e/03-closures/` | **B1 — CRITICAL BLOCKER** | **Stage 13 P0**: implement closure call lowering |
| §2.3 `if let` expression | ❌ NOT in AST/HIR | `grep "IfLet" src/` returns zero; 6 FAIL tests with "if let" descriptions in `02-borrowck/01-nll-advanced/` | B1 | Stage 13 P0 |
| §2.3 `while let` expression | ❌ NOT in AST/HIR | `grep "WhileLet" src/` returns zero; 5 FAIL tests with "while let" descriptions | B1 | Stage 13 P0 |
| §2.3 `for x in iter` | needs further investigation | No `ForLoop` AST variant found in quick check; needs deeper audit | B1 (likely) | Stage 13 P1 |
| §2.5 `move` closure | ❌ Parser captures `is_move: bool` (`src/ast/kinds.rs:461`, `src/hir/kinds.rs:757`); MIR/codegen do not use it | `grep "is_move" src/mir/ src/codegen/` returns zero usage | B1 | Stage 13 P1 |
| §2.1 HRTB `for<'a>` | ❌ NOT implemented | `grep "HRTB\|for<" src/typeck/ src/ast/ src/hir/` returns zero; 1 FAIL test | B1 | Stage 13 P1 |
| §2.2 Associated type normalization | ❌ NOT implemented | Per `03-type-system.md` §10.3 B1; `grep` confirms no normalization code | B1 | Stage 13 P1 (needed for `Iterator::Item`) |
| §2.4 `?` operator | ❌ NOT implemented | Per `03-type-system.md` §10.2 B1 | B1 | Stage 13 P2 |
| §2.2 `Send`/`Sync` auto trait | ❌ NOT implemented | Per `03-type-system.md` §10.2 deferred | B1 | Stage 13 P2 (v0.3+) |
| §2.6 `macro_rules!` | ❌ NOT implemented | v0.1-release.md §5: "macro_rules! is Stage 1 feature"; 0 FAIL tests (gated out) | B1 | Stage 13 P1 — **critical for Stage 1** (Rust compiler source uses macros heavily) |
| §2.1 `impl Trait` in return position | ❌ NOT implemented | Per `03-type-system.md` §1.1 + §2.4 (v0.2); `src/ast/kinds.rs:255` has `ImplTrait(Vec<TypeBound>, Span)` AST variant but typeck doesn't lower it | B1 | Stage 13 P2 (v0.3+) |
| §2.10 `extern "Rust"` ABI | ❌ NOT implemented | Per `07-codegen.md` §15.1 | B1 | Stage 13 P2 (v0.3+) |
| §2.8 `#[may_dangle]` attribute | ❌ NOT implemented | Per `04-ownership-borrowing.md` §13.1 | B1 | Stage 13 P2 (v0.3+) |
| §2.9 Negative impl `impl !Trait for Type` | ❌ NOT implemented | Per `03-type-system.md` §10.2 deferred | B1 | Stage 13 P2 (v0.2+) |
| §2.5 `async` / `await` | ✅ partial (Stage 8.5) | AST `Expr::Await`/`Expr::Async` exists; MIR lowering is MVP synchronous | B3 | No action |
| §2.10 `extern "C"` ABI | ✅ Stage 8.3 | `BodyMeta.abi` + `codegen_function` abi parameter | equal | No action |
| §2.7 `let` / `let mut` / `fn` / `const` / `static` / `struct` / `enum` / `trait` / `impl` / `type` / `extern` / `use` / `mod` / `pub` | ✅ All implemented | `src/parser/items.rs` (777 LOC) | equal | No action |
| §2.7 `let-else` | ❌ NOT implemented | Per `13-stage1-feature-whitelist.md` §2.7 (`❌ v0.2`) | B1 | Stage 13 P2 (v0.3+) |
| §2.4 Or-patterns `1 \| 2 \| 3` | needs further investigation | Parser may support; no FAIL test pattern matched | B1 (likely) | Stage 13 P2 audit |
| §2.4 `box` pattern / Deref pattern | ❌ NOT implemented | Per `13-stage1-feature-whitelist.md` §2.4 (`❌ v0.2`) | B1 | Stage 13 P2 (v0.3+) |

**Critical path for v0.3 self-hosting (Stage 13 P0)**:

1. **Closure call lowering** — without this, Stage 1 cannot write any iterator-based code, callback-based code, or combinator-style code. This is the #1 blocker.
2. **`if let` / `while let`** — Stage 1 source code uses these pervasively (e.g., `if let Some(x) = opt { … }`).
3. **`macro_rules!`** — Stage 1 source needs `vec![]`, `println!`, `assert_eq!`, etc. (26 built-in macros per §2.6); without `macro_rules!`, all macros must be hard-coded in compiler.

**Stage 13 P1**: `for` loop, `move` closure, HRTB, associated type normalization.
**Stage 13 P2**: `?` operator, `Send`/`Sync`, `impl Trait` return, `extern "Rust"`, `#[may_dangle]`, negative impls, `let-else`.

---

## 4. Recommendations for Stage 13 Planning

**Priority P0 (blockers for v0.3 self-hosting)**:

1. **Implement closure call lowering** — In `src/mir/lower/expr_operand.rs`, the `HirExprKind::Call` arm needs to detect when the callee is a closure local and emit the proper `Terminator::Call` to the closure's synthesized `call` function. Estimate: 200-400 LOC, ≤5 files (mir/lower/expr_operand.rs, mir/lower/closure_capture.rs, codegen/mir_translation.rs, typeck/checker.rs, driver.rs).
2. **Implement `if let` / `while let`** — Add `IfLet`/`WhileLet` variants to `ast::Expr` + `hir::HirExprKind`, parser support, MIR lower (desugar to `match`), typeck. Estimate: 300-500 LOC.
3. **Implement `macro_rules!` + 26 built-in macros** — New `src/macro_expand/` module. Estimate: 1500-2500 LOC, major new subsystem.

**Priority P1 (Stage 13 architectural cleanups)**:

4. **§16 violation fix**: relocate 7 `emit_*` functions from `src/mir/dyn_trait.rs` to `src/codegen/trait_dispatch.rs` (or new `src/codegen/dyn_trait_emit.rs`). ≤3 files.
5. **Remove deprecated `check_crate` legacy entry points** in `typeck/checker.rs:902` and `borrowck/mod.rs:591` after external test code migration. Per §23.6, these have `#[deprecated]` markers; removal scheduled for v0.3+.
6. **`TyKind::Dynamic` write-back**: add `Dynamic` variant to `src/mir/ty.rs::TyKind` to model `dyn Trait` as a first-class type (currently only modeled as side-table `DynTraitFatPtr`). Update `03-type-system.md` §10/§11/§12 §25.8 write-back.
7. **Implement two-phase borrows (method-call subset)** — needed for `vec.push(vec.len())` pattern (R6 report MVP requirement). Estimate: 200-400 LOC in `borrowck/`.
8. **Implement disjoint closure captures (RFC 2229)** — needed to avoid borrow-checker false positives in Stage 1 source (R6 report MVP requirement). Estimate: 300-500 LOC in `hir/lower/`.
9. **Implement HRTB** (`for<'a>`) — needed for higher-ranked function pointers. Estimate: 500-800 LOC across typeck.

**Priority P2 (v0.3+ feature backfill, non-blocking for self-hosting)**:

10. Associated type normalization (algorithm in `03-type-system.md` §7.1 with depth limit + cycle detection)
11. `?` operator (desugar to `match` + `From::from`)
12. `Send`/`Sync` auto traits
13. `impl Trait` in return position (existential types)
14. `extern "Rust"` ABI
15. `#[may_dangle]` attribute
16. Negative impls `impl !Trait for Type`
17. `let-else` and `for x in iter` (verify)
18. Monomorphization collection (`07-codegen.md` §9.1) — needed for proper generic codegen
19. Name mangling (`_LND3foo...` per §9.2)

**Priority P3 (v0.4+)**:

20. Full LLVM linking pipeline (`.ll` → `.o` → executable)
21. DWARF debug info
22. Static/dynamic library output
23. C++/Rust interop

---

## 5. Committee Vote (ARCH-A)

### **GO-WITH-CONDITIONS**

**Reasoning**:

The v0.1 release gate (5026/5000 conformance, §25 deep review PASS) is met and the release has already shipped.
For the v0.1 release artifact, this audit confirms:

- ✅ All 4 §16 §21.3 grep checks pass cleanly (codegen makes zero calls to `mir::lower`/`typeck`/`driver` beyond type-only refs)
- ✅ Zero glob exports in `src/hir/mod.rs` and `src/mir/mod.rs`
- ✅ Driver is the sole active HIR reader; deprecated legacy entry points are properly marked per §23.6
- ✅ D1-D8 data flow checkpoints all verified in `driver::compile`
- ✅ All 7 large files (≥1000 LOC) are cohesive single-responsibility units below the 1500 LOC ceiling; none require immediate refactoring
- ✅ All 4 design docs carry current §25.8 write-back sections (no missing write-back)
- ✅ All documented B1/B3 deviations are accepted and have explicit "v0.2/v0.3+" target stages

For the **v0.3 self-hosting target** (Stage 13), this audit identifies:

- ❌ 1 active §16 violation (`mir::dyn_trait` → `codegen::emit_dynptr_global_text`) — easy fix (≤3 files)
- ❌ 2 deprecated legacy entry points (`typeck::check_crate`, `borrowck::check_crate`) — scheduled for removal
- ❌ 1 newly-uncovered B1 deviation (`TyKind::Dynamic` not modeled) — needs §25.8 write-back
- ❌ 3 P0 blocker B1 deviations for self-hosting (closure call lowering, `if let`/`while let`, `macro_rules!`)
- ❌ 6 P1 blocker B1 deviations for self-hosting (`for` loop, `move` closure, HRTB, associated type normalization, two-phase borrows method-call subset, disjoint closure captures)

**Conditional GO**: v0.1 release stands as shipped. v0.3 self-hosting target requires Stage 13 to close the 3 P0 blockers + 1 §16 violation before Stage 1 source writing begins.
Recommended Stage 13 sequencing: (a) fix §16 violation + write-back `TyKind::Dynamic` first (1-2 days, clean architectural baseline); (b) implement P0 blockers (closures, if-let/while-let, macro_rules!) — estimated 4-8 weeks; (c) implement P1 blockers concurrently with Stage 1 source drafting.

**Vote**: ARCH-A ✅ GO-WITH-CONDITIONS (v0.1 release ratified; v0.3 self-hosting contingent on P0 closure).
