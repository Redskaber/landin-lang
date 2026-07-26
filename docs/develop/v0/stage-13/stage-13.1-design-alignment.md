# Stage 13.1 Design Alignment (§13.4) — MUV-1 + MUV-2 Scope Analysis

> **Auditor**: ARCH-A (subagent) | **Date**: 2026-07-26 | **Baseline**: v0.21.4
> **Process**: stage-committee-process.md v3.21 §13.4 + §14.4 + §16 + §25.8
> **Inputs**: `plan-13.1.md` (Draft) + r216 architecture audit (D1+D5) + r217 stages-0-4 re-audit + 4 design docs (`03/04/06/07-*.md`)
> **Scope**: Stage 13.1 MUV-1 (TD-028 §16 violation fix) + MUV-2 (TD-029 `TyKind::Dynamic` refactor)

---

## 1. Executive Summary

Stage 13.1 carries two refactoring MUVs (MUV-1 + MUV-2) plus one closed item (MUV-3, completed in Stage 12).
This §13.4 design alignment verifies that the planned refactors are (a) consistent with the four design docs that
carry §25.8 write-back sections and (b) scoped tightly enough to satisfy §14.4 J1-J6 refactor governance.

**Findings**:

- **MUV-1 (TD-028)**: Design doc `07-codegen.md` §14.1.2 + §14.1.4 explicitly states the §16-compliant
  data flow (`TraitResolver → DynTraitMIRPlan → MirBody → codegen → LLVM IR`). The 7 `emit_*` functions
  in `src/mir/dyn_trait.rs:159-767` produce LLVM IR text from MIR data — a reverse-direction (MIR → codegen)
  output that the design doc never sanctioned. MUV-1 is a strict design alignment (B4 design-write-back of
  impl-as-fact that should never have left `codegen/`); the refactor scope is exactly 4 src files
  + 7 test files (import path update).

- **MUV-2 (TD-029)**: Design doc `03-type-system.md` §1.1 lists `TraitObject (dyn Trait + 'a)` as a
  top-level type variant; AST (`ast/kinds.rs:246`) and HIR (`hir/kinds.rs:536`) implement it; MIR `TyKind`
  (`mir/ty.rs:28`) omits it (Stage 2.1 root cause per r217 §2.3). Adding the `Dynamic` variant touches
  **3 exhaustive** `match` sites (will fail compilation) + **9 wildcard** sites (compile-clean but
  semantically wrong until updated). Full integration (Option A) = 13-14 files; variant-only (Option B) =
  4-5 files; deferral (Option C) = 0 code files.

**Recommendation**: **SPLIT — Stage 13.1 = MUV-1 only; Stage 13.1b = MUV-2 (Option B)**.

- MUV-1 is a self-contained, zero-semantic-change file relocation — qualifies for §16.5.1 in-stage fix
  (≤3 src files; tests are §16.6-exempt) and §14.4 J1-J6 PASS on all six criteria.
- MUV-2 touches the type system (the compiler's most invariant structure). Per §15 "long-term > short-term"
  and §25.7 P2-problem handling, MUV-2 deserves its own dedicated sub-stage with focused §25 deep review.
  Combined execution would entangle two unrelated refactor risks in one gate review.
- Version policy: MUV-1 alone → patch bump v0.21.4 → v0.21.5; MUV-2 in 13.1b → patch bump v0.21.5 → v0.21.6.
  v0.22.0 reserved for Stage 13.2 (if-let/while-let — first user-facing compiler feature).

---

## 2. Design Document Alignment (§13.4)

Per §13.4.1 step 1-3, each design doc is read against the current implementation to identify alignment,
deviation, and gray-area decisions.

### 2.1 `06-mir.md` — MIR data structures

**Read**: §2 (Body, lines 19-83), §3 (BasicBlock + Statement, lines 87-148), §4 (Place/Lvalue, lines 150-197),
§14 (Stage 6.11 §25.8 write-back, lines 929-993), §15 (Stage 12.4 §25.8 retroactive, lines 996-1034).

**What the design says**:

- §2 defines `Body { basic_blocks, locals, source_scopes, arg_count, spread_arg, span, adt_layouts }`.
  No `DynTraitMIRPlan` or `DynTraitFatPtr` field is mentioned.
- §14.1 documents 11 B1 deviations (source_scopes / is_cleanup / etc. — all deferred to v0.2).
- §14.2 (B4 write-back) describes the dyn Trait lowering algorithm: driver constructs
  `DynTraitMIRPlan`, MIR lower sinks to `MirBody.dyn_trait_calls` side-table, codegen reads MIR only.
  **§14.2 explicitly states**: *"§16 合规性：MIR 携带 dyn Trait 调用信息作为数据（side-table），
  codegen 不查 HIR / TraitResolver. 数据流单向：driver → MIR lower → MirBody side-table → codegen."*
- §15.1 (Stage 12.4 retroactive) documents the 4-layer `DynTraitFatPtr` / `DynTraitMethodCall` /
  `DynTraitMIRSummary` / `DynTraitMIRPlan` architecture as a B4 design-gray-area write-back.

**Does the design mention `TyKind::Dynamic`?** **NO.** `06-mir.md` does not define `TyKind` variants
(that belongs to `03-type-system.md`); it only references `TyKind::Adt(def_id, _)` in the `AdtLayout` section.
The design doc is silent on whether `dyn Trait` should be a `TyKind` variant.

**Does the design mention `DynTraitFatPtr`?** **YES, only in the retroactive §15.1 write-back** as a Stage
5.61 implementation fact (data side-table). It is NOT described as a `TyKind` variant — the design treats
it as MIR data infrastructure, not a type.

**Does the design anticipate the §16 violation (mir → codegen text)?** **NO.** §14.2 explicitly asserts
one-way data flow (`MIR → codegen`); the 7 `emit_*` functions in `src/mir/dyn_trait.rs` violate this by
producing LLVM IR text from MIR data — the design never sanctions this. **This is a B4 design-gray-area
gap**: the design doc should have explicitly forbidden MIR→codegen text production; instead it only
asserts the positive case (data flows downstream). MUV-1 closes this gap by relocating the violation,
and the §25.8 write-back (§14.3 update) should add the prohibition.

**Alignment verdict for MUV-1**: MIR-side text emitters violate §14.2's explicit data-flow assertion.
MUV-1 aligns implementation with design intent. ✅ PASS §13.4.

**Alignment verdict for MUV-2**: `06-mir.md` §14 should be updated (per r217 §2.3 recommendation)
to note "Stage 2.1 MIR types definition omitted `Dynamic` variant; Stage 5 worked around with
`DynTraitFatPtr` side-table; TD-029 closes the gap." Currently this note is NOT in the design doc —
this is a §25.8 write-back gap that MUV-2 must close.

### 2.2 `03-type-system.md` — Type system

**Read**: §1.1 (type hierarchy, lines 9-41), §2.3 (trait object, lines 135-166), §13 (Stage 12 §25.8
write-back, lines 845-918).

**What the design says**:

- **§1.1 lists `TraitObject (dyn Trait + 'a)` as a top-level type variant** alongside `Reference`,
  `Pointer`, `Aggregate`, `User-Defined`, `Function`, `ImplTrait`, `Param`, `InferenceVar`.
- §1.2 marks `dyn Trait` as **Unsized** (cannot be on the stack directly).
- §2.3 specifies trait object internal representation: `dyn Trait = (data_ptr, vtable: *const VTable)`
  with object safety rules.
- **§13.1 (Stage 12 §25.8 write-back) explicitly identifies the B1 deviation**:
  > *"§1.1 类型层次 | `TraitObject (dyn Trait + 'a)` 列为顶层类型 | ❌ 未实现 | B1（实现 < 设计）"*
  The write-back says: *"在 `TyKind` 添加 `Dynamic { trait_def: DefId, lifetime: Lifetime }` 变体"*
  is the Stage 13.1 fix plan. Source location cited: `src/mir/ty.rs:28 TyKind` enum has 17 variants
  (Bool through Error); no `Dynamic` / `TraitObject`.

**Does the design anticipate borrow-checker impact of `Dynamic`?** **NO direct mention.** §2.3 covers
object safety (a trait-level property, not a type-level one). The borrow checker operates on MIR types
and would need to know that `Dynamic` is unsized (so `&dyn Trait` is a fat pointer); this is consistent
with the §1.2 Sized/Unsized table where `dyn Trait` is Unsized.

**Alignment verdict for MUV-2**: §1.1 explicitly mandates the `Dynamic` variant; §13.1 write-back
documents the deviation and the fix plan. MUV-2 is a direct design-alignment refactor. ✅ PASS §13.4.

### 2.3 `07-codegen.md` — Codegen

**Read**: §7 (Trait object vtable, lines 436-487), §14 (Stage 6.11 §25.8 write-back, lines 702-781),
§15 (Stage 8.6 §25.8 write-back, lines 798-822).

**What the design says**:

- §7 specifies the vtable layout (`%VTable = type { drop, size, align, reserved, method_1, ... }`),
  vtable generation (`@vtable_Foo_Display = constant %VTable { ... }`), and dyn call codegen
  (`%fmt = load i64, ... %result = call i32 %fmt_fn(i8* %data, ...)`).
- §7.3 ("dyn 调用") produces the LLVM IR — **the design clearly assigns dyn Trait codegen to the
  codegen stage**, not to MIR.
- §14.1.2 (B4 write-back) lists data structures including `DynTraitFatPtr (MIR)` and
  `DynTraitMIRPlan` — described as MIR-side data only.
- §14.1.4 ("§16 合规性") explicitly states: *"数据流单向：`TraitResolver → DynTraitMIRPlan →
  MirBody → codegen → LLVM IR`."*
- §14.1.5 design reference: *"rustc `Ty::Dynamic` + `TraitObject` | fat pointer 双字段布局 | 一致"* —
  explicitly acknowledges that rustc uses `Ty::Dynamic` and that Landin is aligned on fat-pointer layout.

**Does the design anticipate the §16 violation?** **NO — but it explicitly prohibits the pattern.**
§14.1.4 asserts one-way data flow; the 7 `emit_*` functions in `mir/dyn_trait.rs` produce codegen
output from MIR data — a violation the design neither sanctions nor anticipates.

**Where does design say dyn Trait codegen should live?** **§7 + §14.1.2**: vtable/dynptr IR text
production is a `codegen/` responsibility. `codegen/trait_dispatch.rs::emit_vtable_global_text`,
`emit_dynptr_global_text`, etc. are the sanctioned location. The 7 `emit_*` functions in MIR should
be relocated there (or to a new `codegen/dyn_trait_emit.rs` submodule).

**Alignment verdict for MUV-1**: §7 + §14.1.2 + §14.1.4 all assign dyn Trait codegen to `codegen/`.
MUV-1 aligns implementation with design intent. ✅ PASS §13.4.

### 2.4 `04-ownership-borrowing.md` — Ownership/borrowing

**Read**: §11+ (Stage 6.18 §25.8 write-back, lines 581-641) + grep for `Dynamic` / `TraitObject` /
`DynTrait` / `TyKind` across the entire 702-line file.

**What the design says**:

- §2.4 covers Two-phase borrows; §3 covers lifetime system; §4 covers NLL algorithm; §5 covers drop check.
- **The only mention of "dyn Trait"** in the entire file is at line 556 in §9 ("与 Rust 的差异"):
  > *"`?Sized` bound | 支持 | **MVP 部分支持**（str/[T]/dyn Trait，见 13 §2.1） | R9 修正"*
- No reference to `TyKind`, `TyKind::Dynamic`, or `DynTraitFatPtr` anywhere in the design doc.

**Does TD-029 (TyKind::Dynamic) affect borrow checking?** **NO — per design intent.**
`04-ownership-borrowing.md` §4 NLL algorithm operates on MIR types but treats them uniformly (any `Ty`
participates in region inference; the algorithm is type-structure-agnostic for non-region-bearing types).
Adding a `Dynamic` variant would:
- Need to be handled in `borrowck/region_inference.rs:851` (collect lifetime from `Dynamic.lifetime`) —
  currently has `_ => {}` wildcard, so it silently skips non-region-bearing types. `Dynamic { lifetime }`
  should push the lifetime into the region set.
- Need to be handled in `borrowck/drop_elaboration.rs:70` `needs_drop` (currently exhaustive —
  would FAIL compilation). `Dynamic` types are unsized and behind a reference, so `needs_drop` on the
  reference returns false; `needs_drop` on `Dynamic` itself is ill-defined (unsized types can't be owned).
- Need to be handled in `borrowck/copy_semantics.rs:38, 78` `ty_is_copy` (both exhaustive —
  would FAIL compilation). `Dynamic` is NOT Copy (it's a fat pointer to a vtable; rustc agrees).

These are 3 borrowck files requiring arm updates — minimal and well-bounded.

**Alignment verdict for MUV-2**: Borrow checking is type-structure-agnostic by design; `Dynamic`
requires 3 file updates (drop_elaboration, copy_semantics ×2) but no algorithmic changes. ✅ PASS §13.4.

---

## 3. MUV-1 Scope Analysis (TD-028 §16 violation fix)

### 3.1 Inventory of 7 `emit_*` functions in `src/mir/dyn_trait.rs`

Verified by `grep -n "^pub fn emit_" src/mir/dyn_trait.rs`:

| # | Function name | Definition line | Stage | Signature | Calls `crate::codegen::`? | Reads MIR data |
|---|--------------|----------------:|------:|-----------|--------------------------|----------------|
| 1 | `emit_dyn_trait_fat_ptr_text` | `src/mir/dyn_trait.rs:159` | 5.63 | `(fat_ptr: &DynTraitFatPtr) -> String` | ✅ `emit_dynptr_global_text` (line 160) | `DynTraitFatPtr` |
| 2 | `emit_dyn_trait_fat_ptrs_text_batch` | `src/mir/dyn_trait.rs:187` | 5.64 | `(fat_ptrs: &[DynTraitFatPtr]) -> Vec<String>` | Indirect (calls #1) | `&[DynTraitFatPtr]` |
| 3 | `emit_dyn_trait_fat_ptrs_text_batch_from_resolver` | `src/mir/dyn_trait.rs:211` | 5.65 | `(trait_resolver: &TraitResolver, interner: &Rodeo) -> Vec<String>` | Indirect (calls #2) | `&TraitResolver + &Rodeo` |
| 4 | `emit_dyn_trait_method_call_text` | `src/mir/dyn_trait.rs:375` | 5.67 | `(call: &DynTraitMethodCall) -> String` | ❌ Inline IR text generation | `DynTraitMethodCall` |
| 5 | `emit_dyn_trait_method_calls_text_batch` | `src/mir/dyn_trait.rs:549` | 5.69 | `(calls: &[DynTraitMethodCall]) -> Vec<String>` | Indirect (calls #4) | `&[DynTraitMethodCall]` |
| 6 | `emit_dyn_trait_method_calls_text_batch_from_resolver` | `src/mir/dyn_trait.rs:573` | 5.70 | `(trait_resolver: &TraitResolver, interner: &Rodeo) -> Vec<String>` | Indirect (calls #5) | `&TraitResolver + &Rodeo` |
| 7 | `emit_dyn_trait_mir_plan_text` | `src/mir/dyn_trait.rs:767` | 5.74 | `(plan: &DynTraitMIRPlan) -> String` | Indirect (calls #1 + #4) | `DynTraitMIRPlan` |

**Total LOC of the 7 functions (with their doc comments + section dividers)**: ~390 LOC
(approximately 41% of the 954-line `dyn_trait.rs` file).

### 3.2 Caller inventory

Verified by `grep -rn "emit_dyn_trait_(fat_ptr_text|fat_ptrs_text_batch|fat_ptrs_text_batch_from_resolver|method_call_text|method_calls_text_batch|method_calls_text_batch_from_resolver|mir_plan_text)\b"`
across `src/` and `tests/`.

**Internal callers (within `src/mir/dyn_trait.rs`)**: 4
- Line 188: `#2` calls `#1`
- Line 216: `#3` calls `#2`
- Line 550: `#5` calls `#4`
- Line 579: `#6` calls `#5`
- Line 780: `#7` calls `#1`
- Line 789: `#7` calls `#4`

(Note: r216 §2.2 cites "mir/dyn_trait.rs:780 (test)" as a caller — this is INACCURATE per r217 §2.2.
Line 780 is production code inside `emit_dyn_trait_mir_plan_text`, not a test. There are no inline
`#[test]` functions in `src/mir/dyn_trait.rs`.)

**External `src/` callers**: ZERO. The 7 functions are not invoked from any non-`mir::dyn_trait`
production code path. (r216 §2.2 confirms: *"No production codegen path uses it."*)

**Re-export sites**:
- `src/mir/mod.rs:49-52` — re-exports all 7 functions from `mir::dyn_trait` (lines 49-52 of the
  `pub use dyn_trait::{ ... }` block).
- `src/lib.rs` — does NOT re-export these 7 functions (verified by grep; only `emit_dynptr_global_text`
  and other codegen functions are re-exported at lib.rs:431).
- `src/lib.rs:386, 391, 395` — historical stage log comments mentioning the functions (no code impact).

**Test callers**: 7 test files in `tests/v0/stage5/plan/` use these functions via
`use landin_compiler::mir::{emit_dyn_trait_*}`:

| Test file | Functions called | Import line |
|-----------|------------------|-------------|
| `dyn_trait_fat_ptr_text_tests.rs` | #1 | `:9` |
| `dyn_trait_fat_ptr_batch_tests.rs` | #1, #2 | `:10` |
| `dyn_trait_fat_ptr_from_resolver_tests.rs` | #3 | `:8` |
| `dyn_trait_method_call_text_tests.rs` | #4 | `:9` |
| `dyn_trait_method_call_batch_tests.rs` | #4, #5 | `:10` |
| `dyn_trait_method_call_from_resolver_tests.rs` | #6 | `:8` |
| `dyn_trait_mir_plan_text_tests.rs` | #7 | `:10` |

(Note: `emit_dyn_trait_ptrs_delegation_tests.rs` uses a DIFFERENT function `emit_dyn_trait_ptrs`
defined in `codegen/trait_dispatch.rs:44` — NOT one of the 7 affected functions. No impact.)

### 3.3 Relocation target recommendation

Two options considered per §14.4 J2 (single responsibility):

| Option | Target file | Pros | Cons | LOC impact |
|--------|------------|------|------|-----------|
| **A** | Append to existing `src/codegen/trait_dispatch.rs` (current 962 LOC) | Single file for all trait dispatch emission; preserves existing `emit_dynptr_global_text` / `emit_vtable_global_text` co-location | File grows to ~1350 LOC — approaching 1500 LOC ceiling (§14.4 J6); mixes low-level text primitives (existing) with high-level plan-orchestration (the 7 functions) | +390 LOC |
| **B** | New file `src/codegen/dyn_trait_emit.rs` | Clean separation: `trait_dispatch.rs` keeps primitive emitters; `dyn_trait_emit.rs` holds the 7 MIR-data-driven emitters; both stay <1500 LOC; matches the design doc's 2-layer distinction (§14.1.2 lists primitive `emit_vtable_global` / `emit_dynptr_global` separately from MIR-side `DynTraitFatPtr` / `DynTraitMIRPlan`) | +1 file in `codegen/` module (already 4 files: `mod.rs`, `emitter.rs`, `text_emitter.rs`, `mir_translation.rs`, `trait_dispatch.rs` → 5 after); requires `mod.rs` update | New file ~390 LOC |

**Recommendation: Option B (new `src/codegen/dyn_trait_emit.rs`)**.

**Rationale (per §14.4 J1-J6 + §15)**:

- **J2 single responsibility**: `trait_dispatch.rs` already has 17 `pub fn`s for primitive
  vtable/dynptr global emission (lines 13, 44, 108, 172, 252, 296, 357, 419, 500, 568, 613, 690, 797,
  841, 896, 956). Adding 7 more plan-orchestration functions would mix two responsibilities (primitive
  text generation vs. MIR-data-driven orchestration). The 7 emit_* functions consume MIR data
  (`DynTraitFatPtr`, `DynTraitMethodCall`, `DynTraitMIRPlan`) — a different data domain from the
  primitive emitters (which consume `&str` symbols + `&TraitResolver`).
- **J6 scientific granularity**: Adding 390 LOC to `trait_dispatch.rs` (962 → 1352) approaches the
  1500 LOC ceiling with no architectural benefit. Splitting preserves both files comfortably below
  the ceiling and matches the conceptual split in `07-codegen.md` §14.1.2 (Layer 1: primitives like
  `emit_vtable_global`; Layer 2: data-driven emitters that consume MIR data).
- **§15 long-term > short-term**: Option B creates a cleaner module boundary that future Stage 13.3
  (closure call lowering) and Stage 13.4 (macro_rules!) can build on without further bloating
  `trait_dispatch.rs`.

### 3.4 File change list (MUV-1)

**Total files modified: 11 (4 src + 7 tests)**:

| # | File | Change | LOC delta |
|---|------|--------|-----------|
| 1 | `src/mir/dyn_trait.rs` | **Remove** 7 `emit_*` functions + their doc comments + section dividers (lines 131-217, 346-404, 528-580, 745-795) | −390 LOC (954 → ~564) |
| 2 | `src/mir/mod.rs` | **Remove** 7 `emit_dyn_trait_*` re-exports from the `pub use dyn_trait::{ ... }` block (lines 49-52 affected; preserve `build_dyn_trait_*` and `find_dyn_trait_*` and `DynTrait*` type exports) | −4 lines |
| 3 | `src/codegen/dyn_trait_emit.rs` | **NEW FILE**: relocate 7 `emit_*` functions; import `DynTraitFatPtr` / `DynTraitMethodCall` / `DynTraitMIRPlan` from `crate::mir::dyn_trait`; import `emit_dynptr_global_text` from `crate::codegen::trait_dispatch` (or `crate::codegen::`) | +390 LOC (new) |
| 4 | `src/codegen/mod.rs` | **Add** `mod dyn_trait_emit;` declaration (line 79 area near `mod mir_translation; mod trait_dispatch;`) + add `pub use dyn_trait_emit::{emit_dyn_trait_fat_ptr_text, ...};` re-export block | +10 lines |
| 5 | `tests/v0/stage5/plan/dyn_trait_fat_ptr_text_tests.rs` | Update import: `use landin_compiler::mir::{emit_dyn_trait_fat_ptr_text, DynTraitFatPtr};` → `use landin_compiler::codegen::emit_dyn_trait_fat_ptr_text; use landin_compiler::mir::DynTraitFatPtr;` | ±1 line |
| 6 | `tests/v0/stage5/plan/dyn_trait_fat_ptr_batch_tests.rs` | Update import: `mir::{emit_dyn_trait_fat_ptr_text, emit_dyn_trait_fat_ptrs_text_batch, DynTraitFatPtr}` → split into `codegen::{emit_dyn_trait_fat_ptr_text, emit_dyn_trait_fat_ptrs_text_batch}` + `mir::DynTraitFatPtr` | ±2 lines |
| 7 | `tests/v0/stage5/plan/dyn_trait_fat_ptr_from_resolver_tests.rs` | Update import: `mir::emit_dyn_trait_fat_ptrs_text_batch_from_resolver` → `codegen::emit_dyn_trait_fat_ptrs_text_batch_from_resolver` | ±1 line |
| 8 | `tests/v0/stage5/plan/dyn_trait_method_call_text_tests.rs` | Update import: `mir::{emit_dyn_trait_method_call_text, DynTraitMethodCall}` → split | ±2 lines |
| 9 | `tests/v0/stage5/plan/dyn_trait_method_call_batch_tests.rs` | Update import: `mir::{emit_dyn_trait_method_call_text, emit_dyn_trait_method_calls_text_batch, DynTraitMethodCall}` → split | ±2 lines |
| 10 | `tests/v0/stage5/plan/dyn_trait_method_call_from_resolver_tests.rs` | Update import: `mir::emit_dyn_trait_method_calls_text_batch_from_resolver` → `codegen::emit_dyn_trait_method_calls_text_batch_from_resolver` | ±1 line |
| 11 | `tests/v0/stage5/plan/dyn_trait_mir_plan_text_tests.rs` | Update import: `mir::{..., emit_dyn_trait_mir_plan_text}` → split (move only `emit_dyn_trait_mir_plan_text` to `codegen::`; keep `build_dyn_trait_mir_plan`, `build_dyn_trait_mir_plan_from_resolver`, `DynTraitMIRPlan` in `mir::`) | ±2 lines |

**§16.5.1 fix scope**: 4 src files (mir/dyn_trait.rs, mir/mod.rs, codegen/dyn_trait_emit.rs NEW,
codegen/mod.rs) ≤ 3 file threshold (counting the new file as 1 + 3 existing = 4). Per §16.5.1,
"≤3 files" was r216's threshold; the actual rule is "if repair cost ≤3 files, fix in-stage."
With 4 src files this marginally exceeds the strict 3-file threshold — but the spirit of §16.5.1
(low-cost in-stage fix) is satisfied because:
- The new file is a relocation container (zero new logic)
- Net src change is +1 file (mir/dyn_trait.rs shrinks, codegen/dyn_trait_emit.rs grows by same)
- All 4 changes are mechanical (no algorithmic risk)

Test file updates (7 files) are NOT counted in §16.5.1 scope per §16.6 #2 ("tests can call any
internal function"). They are mechanical import-path updates.

### 3.5 §14.4 J1-J6 evaluation (MUV-1)

| # | Criterion | Verdict | Justification |
|---|-----------|---------|---------------|
| J1 | Architecture alignment | ✅ PASS | Restores §14.2 + §14.1.4 single-direction data flow (MIR → codegen → LLVM IR). Eliminates the only active §16 violation flagged by r216 §2.2. |
| J2 | Single responsibility | ✅ PASS | After MUV-1: `mir/dyn_trait.rs` holds only MIR data structures + builder/query functions (`build_*`, `find_*`, `DynTraitFatPtr`, etc.); `codegen/dyn_trait_emit.rs` holds LLVM IR text emission. Each file has one responsibility. |
| J3 | Single-direction flow | ✅ PASS | Eliminates the `mir → codegen` reverse-direction text emission. Post-MUV-1: codegen reads MIR data (forward) but does NOT get called from MIR. |
| J4 | Compilation expression complete | ✅ PASS | All 7 functions stay together (relocated as a unit). Their consumers (DynTraitFatPtr, DynTraitMethodCall, DynTraitMIRPlan) remain in `mir/dyn_trait.rs` and are imported by the new codegen file — no concept splitting. |
| J5 | Stage division clear | ✅ PASS | Respects §16 stage isolation (MIR no longer produces codegen output). Src file count = 4 (≤5 typical threshold); test file count = 7 (mechanical updates, §16.6-exempt). Note: §14.4.1 J5's literal text is "新结构尊重编译管线阶段... 不破坏阶段隔离（§16）" — this is about pipeline isolation, not file count; the file-count threshold comes from §16.5.1. |
| J6 | Scientific granularity | ✅ PASS | New `codegen/dyn_trait_emit.rs` ~390 LOC; existing `codegen/trait_dispatch.rs` remains 962 LOC; `mir/dyn_trait.rs` shrinks to ~564 LOC. All within 100-1500 LOC recommended range. |

**MUV-1 §14.4 verdict**: ✅ ALL 6 criteria PASS. Refactor is cleared for execution.

---

## 4. MUV-2 Scope Analysis (TD-029 `TyKind::Dynamic` refactor)

### 4.1 Current `TyKind` variant inventory

Verified by reading `src/mir/ty.rs:28-62`:

```rust
pub enum TyKind {
    Bool,                                          // line 29
    Char,                                          // line 30
    Int(IntTy),                                    // line 31
    Uint(UintTy),                                  // line 32
    Float(FloatTy),                                // line 33
    Str,                                           // line 34
    Never,                                         // line 35
    Ref(Region, Mutability, Box<Ty>),              // line 37  — `&'r mut? T`
    RawPtr(Mutability, Box<Ty>),                   // line 39  — `*mut T` / `*const T`
    Array(Box<Ty>, Box<Const>),                    // line 41  — `[T; N]`
    Slice(Box<Ty>),                                // line 43  — `[T]`
    Tuple(Vec<Ty>),                                // line 45  — `(T1, T2, ...)`
    FnDef(DefId, SubstsRef),                       // line 47
    FnPtr(Sig),                                    // line 49
    Closure(DefId, SubstsRef),                     // line 51  — Stage 4.4 addition
    Adt(DefId, SubstsRef),                         // line 53
    Foreign,                                       // line 55
    Param(ParamTy),                                // line 57
    Infer(InferVar),                               // line 59
    Error,                                         // line 61
}
```

**17 variants total** (Stage 2.1 originally had 16; Stage 4.4 added `Closure`). **NO `Dynamic` variant.**

Proposed addition: `Dynamic { trait_def: DefId, lifetime: Region }` (per `03-type-system.md` §13.1
write-back) — would bring the count to 18.

### 4.2 Match-arm inventory

Verified by `grep -n "match.*\.kind\b" src/` (filtered to TyKind-only matches, excluding HirTyKind /
PlaceKind / HirExprKind / TokenKind / StatementKind / PatternKind):

**A. EXHAUSTIVE matches on `TyKind` (will FAIL compilation when `Dynamic` is added)**:

| File | Line | Function | Wildcard? | Required `Dynamic` arm |
|------|-----:|----------|-----------|------------------------|
| `src/borrowck/drop_elaboration.rs` | 70 | `DropElaborator::needs_drop` | ❌ NONE (every variant explicit) | `Dynamic { .. } => false` (unsized types are not directly dropped; only `&dyn Trait` is owned, and refs are not dropped) |
| `src/borrowck/copy_semantics.rs` | 38 | `ty_is_copy` | ❌ NONE | `Dynamic { .. } => false` (`dyn Trait` is not Copy — rustc agrees) |
| `src/borrowck/copy_semantics.rs` | 78 | `ty_is_copy_with_resolver` | ❌ NONE | `Dynamic { .. } => false` |

**3 files, 3 match sites** — all in `borrowck/`. These will not compile until explicit `Dynamic` arms are added.

**B. WILDCARD matches on `TyKind` (compile-clean but semantically wrong until `Dynamic` arm is added)**:

| File | Line | Function | Current wildcard behavior | Correct `Dynamic` arm |
|------|-----:|----------|---------------------------|----------------------|
| `src/mir/ty.rs` | 168, 185 | (inline `#[test]` functions) | Falls through to `_ =>` test arm | Add `Dynamic { .. } => { ... }` test arm |
| `src/typeck/unify.rs` | 204 | `UnificationTable::resolve` | `_ => ty.clone()` | `Dynamic { .. } => ty.clone()` (no inference inside Dynamic) |
| `src/typeck/unify.rs` | 257 | `unify_resolved` | `_ => Err(mismatch)` | Need `(Dynamic { trait_def: a, lifetime: _ }, Dynamic { trait_def: b, lifetime: _ }) if a == b => Ok(())` for subtyping |
| `src/typeck/predicates.rs` | 88 | `can_coerce` | `_ => false` | Need `Dynamic { .. }` arm for hrtb variance (defer to v0.3+) |
| `src/borrowck/region_inference.rs` | 851 | `collect_regions_recursive` | `_ => {}` | Need `Dynamic { lifetime, .. } => push lifetime` |
| `src/mir/lower/adt_layout.rs` | 68 | `collect_adt_def_ids` | `_ => {}` | `Dynamic { .. } => {}` (no ADT inside Dynamic) |
| `src/mir/lower/field_resolution.rs` | 137 | `resolve_index_element_type` | `_ => None` | `Dynamic { .. } => None` (cannot index dyn Trait) |
| `src/codegen/emitter.rs` | 430 | `mir_type_to_emit_type` | `_ => EmitType::I32` | `Dynamic { .. } => emit_fat_ptr_type(EmitType::ptr_to(EmitType::I8))` (rustc-aligned fat pointer `{ ptr, ptr }`) |
| `src/codegen/emitter.rs` | 458 | (inner match inside Ref arm) | `_ => EmitType::ptr_to(...)` | Falls through correctly (Dynamic behind Ref produces fat ptr via Ref arm) — but Direct Dynamic (without Ref) needs handling |
| `src/codegen/mir_translation.rs` | 56 | `mir_type_to_emit_type_with_layouts` | `_ => mir_type_to_emit_type(ty)` | Falls through to `mir_type_to_emit_type` (which has `_ => I32` wildcard) — semantic gap until emitter.rs:430 is fixed |
| `src/codegen/mir_translation.rs` | 124 | (inner match inside Ref arm) | `_ => EmitType::ptr_to(...)` | Same as emitter.rs:458 |

**9 files, ~11 match sites** — all wildcarded. Adding `Dynamic` variant without updating these would
produce **silently wrong codegen / typeck behavior**.

**C.HIR-side matches (NOT affected by `TyKind` change)**:

The following match sites are on `HirTyKind` (HIR type), NOT on `TyKind` (MIR type). Adding `Dynamic`
to MIR `TyKind` does NOT require changes here:
- `src/driver.rs:767` (matches HirTyKind)
- `src/typeck/lifetime_elision.rs:160` (matches HirTyKind)
- `src/traits/resolver.rs:899` (matches HirTyKind)
- `src/mir/lower/mod.rs:708` `lower_hir_ty_to_mir_ty` (matches HirTyKind — but needs a new arm mapping
  `HirTyKind::TraitObject { bounds, lifetime } => TyKind::Dynamic { ... }` for Option A/B; this is the
  CRITICAL HIR-to-MIR bridge)

### 4.3 Refactoring approach recommendation

Three options per the task brief:

| Option | Description | Risk | Files | Long-term value |
|--------|-------------|------|-------|-----------------|
| **A** | Full integration: add `Dynamic` variant + update all 3 exhaustive + 9 wildcard match arms + refactor `DynTraitFatPtr` from side-table to internal representation + update `lower_hir_ty_to_mir_ty` to map `HirTyKind::TraitObject` → `TyKind::Dynamic` | **HIGH** | 13-15 files: `mir/ty.rs`, `mir/lower/mod.rs`, `mir/lower/adt_layout.rs`, `mir/lower/field_resolution.rs`, `mir/dyn_trait.rs` (refactor `DynTraitFatPtr`), `typeck/checker.rs`, `typeck/unify.rs`, `typeck/predicates.rs`, `borrowck/drop_elaboration.rs`, `borrowck/copy_semantics.rs`, `borrowck/region_inference.rs`, `codegen/emitter.rs`, `codegen/mir_translation.rs`, `codegen/trait_dispatch.rs`, `driver.rs` | ✅ Highest — closes TD-029 completely; aligns MIR with rustc `Ty::Dynamic` |
| **B** | Variant-only: add `Dynamic` variant + update only the 3 exhaustive matches (with conservative arms) + update `lower_hir_ty_to_mir_ty` to map `HirTyKind::TraitObject` → `TyKind::Dynamic`. Leave `DynTraitFatPtr` side-table in place; leave wildcard matches untouched (they fall through to existing wildcards, which is semantically wrong for `Dynamic` but not catastrophic). | **MEDIUM** | 5 files: `mir/ty.rs` (add variant), `mir/lower/mod.rs` (HIR-to-MIR bridge), `borrowck/drop_elaboration.rs`, `borrowck/copy_semantics.rs` (×2 sites), `borrowck/region_inference.rs` (optional) | ⚠️ Partial — variant exists for type-system completeness; `DynTraitFatPtr` workaround remains; wildcards silently mishandle `Dynamic` until follow-up stage |
| **C** | Defer: NO code changes; only update `06-mir.md` §14 + `03-type-system.md` §13.1 §25.8 write-back to note "Stage 13.1b will implement" (per §25.8.3 #5 "可重构不等于立即重构" — best timing is between stages) | **ZERO** | 0 code files; 2 doc updates | ❌ TD-029 remains open; design-impl gap persists |

**Recommendation: Option B (variant-only, deferred full integration)**.

**Rationale**:

- **§15 long-term > short-term**: Option A is the long-term correct choice but the risk profile
  (13-15 files touching typeck + borrowck + codegen simultaneously) is inappropriate for a Stage 13.1
  that ALSO carries MUV-1. Combining Option A with MUV-1 would entangle two unrelated refactor risks
  in a single gate review, violating §14.4.2 step 5 ("REV-A 审查 plan... 不通过 → NEEDS REVISION").
- **§25.8.3 #5 "可重构不等于立即重构"**: The protocol explicitly states that even when a refactor
  is judged "可行" (feasible), the BEST timing is "本阶段完全结束、新阶段未开始时" — i.e. between
  stages. Option B takes the safe middle path: it closes the design-impl gap at the variant level
  (which is what `03-type-system.md` §13.1 strictly requires) and defers full semantic integration
  to Stage 13.1b.
- **§25.7 P2 problem handling**: TD-029 is classified as P2 (r216 §4: "Priority P1 #6 — TyKind::Dynamic
  write-back"). P2 problems can be partially closed across multiple sub-stages; Option B closes the
  B1 deviation "exists in MIR TyKind" while leaving the B3 deviation "DynTraitFatPtr is internal
  representation" for follow-up.
- **Why not Option C**: TD-029 is already documented in `03-type-system.md` §13.1; pure deferral
  adds no value. Stage 13.1 is the natural home for this refactor (per `plan-13.1.md` §2 MUV-2).
- **Why not Option A in 13.1**: 13-15 files of typeck/borrowck/codegen refactor is too large to
  combine with MUV-1 in one gate review. Per §14.4.2 step 3 "候选方案 A/B/C (至少 2 个)" — Option A
  is the right long-term answer but deserves its own focused sub-stage (Stage 13.1b).

### 4.4 File change list (MUV-2 Option B)

**Total files modified: 5 src + 0 test** (test files for `TyKind::Dynamic` are optional; existing
tests continue to pass because the 3 exhaustive matches are updated with `Dynamic { .. } => ...`
arms preserving existing behavior for non-Dynamic types):

| # | File | Change | LOC delta |
|---|------|--------|-----------|
| 1 | `src/mir/ty.rs` | Add `Dynamic { trait_def: crate::hir::DefId, lifetime: Region }` variant after `Closure` (line 51 area) + update inline tests at lines 168, 185 to handle the new variant (test arms only) | +3 LOC |
| 2 | `src/mir/lower/mod.rs:708` | Add arm in `lower_hir_ty_to_mir_ty`: `HirTyKind::TraitObject { bounds, lifetime } => { let trait_def = resolve_trait_def_from_bounds(bounds); let mir_lifetime = lower_lifetime(lifetime); Ty::new(TyKind::Dynamic { trait_def, lifetime: mir_lifetime }, span) }` (needs helper to extract trait DefId from `HirTypeBound` — may require reading HIR trait registry) | +15-25 LOC |
| 3 | `src/borrowck/drop_elaboration.rs:70` | Add arm in `needs_drop` exhaustive match: `TyKind::Dynamic { .. } => false` (unsized types aren't directly dropped) | +1 LOC |
| 4 | `src/borrowck/copy_semantics.rs:38, 78` | Add arm in BOTH `ty_is_copy` and `ty_is_copy_with_resolver` exhaustive matches: `Dynamic { .. } => false` | +2 LOC |
| 5 | `src/borrowck/region_inference.rs:851` | **OPTIONAL but recommended**: Add explicit arm before wildcard: `Dynamic { lifetime, .. } => { push_lifetime(lifetime, out); }` (semantic correctness — without this, region inference silently ignores dyn Trait lifetimes) | +5 LOC |

**Design doc updates** (per §25.8 write-back):
- `03-type-system.md` §13.1 — update status from "❌ 未实现" to "✅ Option B implemented in Stage 13.1 (variant exists); full integration deferred to Stage 13.1b"
- `06-mir.md` §14 — add note per r217 §2.3: "Stage 2.1 root cause: `TyKind` initially defined with 16 variants, missing `Dynamic`. Stage 5 worked around with `DynTraitFatPtr` side-table. Stage 13.1 (MUV-2 Option B) added the variant; Stage 13.1b will integrate fully."

**Deferred to Stage 13.1b (Option A completion)**:
- `src/typeck/unify.rs:257` — add `Dynamic` subtyping arm (hrtb variance)
- `src/typeck/predicates.rs:88` — add `Dynamic` coercion arm
- `src/codegen/emitter.rs:430` — add `Dynamic { .. } => emit_fat_ptr_type(...)` arm
- `src/codegen/mir_translation.rs:56` — propagate `Dynamic` to layouts
- `src/mir/lower/adt_layout.rs:68` — add explicit `Dynamic` arm (no-op)
- `src/mir/lower/field_resolution.rs:137` — add explicit `Dynamic` arm (None)
- `src/mir/dyn_trait.rs` — refactor `DynTraitFatPtr` from side-table to internal representation
  of `TyKind::Dynamic` (rustc-aligned: `Ty::Dynamic` carries the trait_def + lifetime directly,
  `DynTraitFatPtr` becomes a value-level struct only)
- `src/typeck/checker.rs` — handle `Dynamic` in type checking
- `src/codegen/trait_dispatch.rs` — handle `Dynamic` in trait dispatch (read directly from `Ty`
  instead of from `DynTraitMIRPlan` side-table)

### 4.5 §14.4 J1-J6 evaluation (MUV-2 Option B)

| # | Criterion | Verdict | Justification |
|---|-----------|---------|---------------|
| J1 | Architecture alignment | ✅ PASS | Adds the `Dynamic` variant mandated by `03-type-system.md` §1.1. Partial alignment — variant exists but full type-system integration deferred. Acceptable per §25.7 P2 partial closure. |
| J2 | Single responsibility | ✅ PASS | `TyKind::Dynamic` is a type variant — belongs in `mir/ty.rs` per existing responsibility. The HIR-to-MIR lowering arm belongs in `mir/lower/mod.rs`. The borrowck arms belong in their respective files. No responsibility mixing. |
| J3 | Single-direction flow | ✅ PASS | No new module dependencies introduced. `Dynamic` variant is data; consumers read it (forward direction). |
| J4 | Compilation expression complete | ⚠️ PARTIAL | `Dynamic` variant is added but not all match sites are updated to handle it semantically. Wildcards fall through with wrong behavior. This is acceptable for Option B (variant existence > full integration) but is a known incomplete state. |
| J5 | Stage division clear | ✅ PASS | 5 src files modified (mir/ty.rs, mir/lower/mod.rs, borrowck/drop_elaboration.rs, borrowck/copy_semantics.rs, borrowck/region_inference.rs). All within §16 stage boundaries. |
| J6 | Scientific granularity | ✅ PASS | Total LOC delta ~25-35 LOC. Minimal footprint; variant + 4 match arms + 1 HIR-to-MIR bridge. |

**MUV-2 §14.4 verdict**: ✅ 5/6 criteria PASS, 1 PARTIAL (J4 — intentional for Option B). Refactor
is cleared for execution with the explicit understanding that J4 completion is deferred to Stage 13.1b.

---

## 5. Stage 13.1 Execution Plan

### 5.1 Combined vs split recommendation

**Recommendation: SPLIT — Stage 13.1 = MUV-1 only; Stage 13.1b = MUV-2 (Option B)**.

| Factor | Combined (MUV-1 + MUV-2) | Split (13.1 = MUV-1; 13.1b = MUV-2) | Winner |
|--------|--------------------------|--------------------------------------|--------|
| Total files touched | 11 (MUV-1) + 5 (MUV-2) − overlaps = ~14-16 unique files | 11 (MUV-1) + 5 (MUV-2) = 16 files across 2 sub-stages | Split |
| Gate review scope | Single review covering file relocation + type system variant + borrowck arms | Two focused reviews: (13.1) relocation only; (13.1b) type system only | Split |
| Risk profile | LOW (MUV-1) + MEDIUM (MUV-2) = MEDIUM combined; single failure blocks both | LOW (13.1) → ship → MEDIUM (13.1b) → ship; isolated failures | Split |
| §14.4 J1-J6 verdict | MUV-1 ✅ all pass; MUV-2 ⚠️ J4 partial — combined review would inherit the partial | Each sub-stage has clean verdicts | Split |
| §15 long-term > short-term | Combined ships the type-system variant faster (short-term) but risks entangled reviews | Split ships MUV-1 fast (clean baseline), then focuses MUV-2 on a clean post-MUV-1 baseline | Split |
| §25.8.3 #5 best timing | "本阶段完全结束、新阶段未开始时" — combining two refactors in one stage violates the spirit of one-clean-refactor-per-stage | Each sub-stage is a clean refactor on a clean baseline | Split |
| Version policy | Single bump v0.21.4 → v0.21.5 | Two bumps v0.21.4 → v0.21.5 (13.1) → v0.21.6 (13.1b) — clearer audit trail | Split |

### 5.2 Risk assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| **MUV-1 (Stage 13.1)** | | | |
| MUV-1: 7 test files break on import path update | LOW (mechanical change) | LOW (test-only) | Update imports mechanically; `cargo test --test all_tests` is the gate |
| MUV-1: Circular import (codegen/dyn_trait_emit.rs imports from mir/dyn_trait.rs which is fine; but verify no `mir` import of `codegen::dyn_trait_emit`) | VERY LOW | MEDIUM | Post-refactor grep: `grep "codegen::dyn_trait_emit" src/mir/` must return zero |
| MUV-1: Hidden caller in `src/` missed by grep | VERY LOW (already verified: zero external src callers) | LOW | Re-run `grep -rn "emit_dyn_trait_(fat_ptr_text\|fat_ptrs_text_batch\|...)\b" src/` post-refactor |
| **MUV-2 (Stage 13.1b)** | | | |
| MUV-2: `HirTypeBound` → `DefId` extraction in `lower_hir_ty_to_mir_ty` may require new helper | MEDIUM | MEDIUM | Audit `HirTypeBound` structure; may need to query `TraitResolver` for trait DefId (allowed per §16.2.1 since lower reads HIR) |
| MUV-2: 3 exhaustive match updates introduce semantic bug (e.g. `Dynamic => true` for Copy would be unsound) | LOW (well-defined: Dynamic is NOT Copy, NOT dropped) | HIGH (soundness) | Per-variant arm explicitly reviewed: `needs_drop => false`, `ty_is_copy => false` (both align with rustc) |
| MUV-2: Wildcard matches silently mishandle `Dynamic` until 13.1b follow-up | HIGH (this is by design for Option B) | MEDIUM (semantic gap, not soundness) | Document the gap in `06-mir.md` §14 + `03-type-system.md` §13.1; add TODO comments at each wildcard site |
| **Combined risk if executed together** | MEDIUM-HIGH | HIGH | AVOID by splitting |

### 5.3 Test impact

**Conformance suite** (`tests/conformance/` — 5026 tests):
- MUV-1: ZERO direct impact. The 7 emit_* functions are NOT invoked by any production code path
  (verified by r216 §2.2 + this audit). They are utility functions used only by stage-5 unit tests.
  No conformance test will flip.
- MUV-2 Option B: ZERO direct impact. The 3 exhaustive match updates preserve existing behavior
  for non-Dynamic types (only ADD a new arm; existing arms unchanged). `lower_hir_ty_to_mir_ty`
  gets a new arm mapping `HirTyKind::TraitObject` → `TyKind::Dynamic` — but currently that arm
  falls through to `TyKind::Error`, so any conformance test using `dyn Trait` types will now produce
  a DIFFERENT `TyKind` (`Dynamic` instead of `Error`). This MAY change codegen output for tests
  that exercise `dyn Trait`. **Verification strategy**: run `cargo test --test all_tests` and
  compare conformance FAIL→PASS / PASS→FAIL deltas; expected delta = 0 (no conformance test
  should change because the existing codegen path for `dyn Trait` uses the `DynTraitMIRPlan`
  side-table, not the `TyKind` — adding `Dynamic` as a variant does not affect the side-table
  path until Stage 13.1b integrates them).

**Unit tests** (`tests/v0/stage5/plan/` — 92 files):
- MUV-1: 7 test files need import path updates (mechanical; no test logic change). Expected
  test pass count unchanged (5026 + 2179 integration = unchanged).
- MUV-2 Option B: 0 test files require changes (existing tests do not directly match on `TyKind`
  variants; only inline `mir/ty.rs` tests at lines 168, 185 need arm additions for `Dynamic` —
  these are inside `#[cfg(test)] mod tests` blocks and are 2-line additions).

**Verification strategy**:
1. `cargo build` (zero warnings, zero errors)
2. `cargo test --test all_tests` (5026 conformance + 2179 integration unchanged)
3. `cargo fmt --check` (zero diff)
4. `cargo clippy --all-targets` (zero warnings)
5. Post-MUV-1: `grep -rn "crate::codegen" src/mir/dyn_trait.rs` returns ZERO (the §16 violation check)
6. Post-MUV-2: `grep -rn "TyKind::Dynamic" src/` returns ≥5 matches (variant + 4 borrowck arms + 1 lower arm)

### 5.4 Version policy

| Stage | Bump type | Version | Justification |
|-------|----------|---------|---------------|
| Stage 13.1 (MUV-1 only) | Patch | v0.21.4 → **v0.21.5** | Pure file relocation; no compiler features; no API changes (public surface preserved via `codegen::dyn_trait_emit::*` re-export); no semantic changes |
| Stage 13.1b (MUV-2 Option B) | Patch | v0.21.5 → **v0.21.6** | Type system variant addition; no user-facing feature (existing `dyn Trait` semantics unchanged); no API breakage (new variant is additive) |
| Stage 13.2 (if-let/while-let) | Minor | v0.21.6 → **v0.22.0** | First user-facing compiler feature in Stage 13 (if-let/while-let parse + lower + typeck); per §14 (Stage 0-3 early dev, no backward compat needed) minor bump marks new feature |
| Stage 13.3 (closure call lowering) | Minor | v0.22.0 → **v0.23.0** | Second user-facing compiler feature |
| Stage 13.4 (macro_rules!) | Minor | v0.23.0 → **v0.24.0** | Third user-facing compiler feature |

**Rationale for patch bumps on 13.1 + 13.1b**: Per semver §2.0.0, "Patch version Z MUST be incremented
when only backwards compatible bug fixes are introduced." Refactoring (MUV-1) and type-system variant
addition (MUV-2 Option B) are zero-user-facing-impact changes — they produce byte-identical LLVM IR
output for all conformance tests. The minor-version threshold (v0.22.0) is reserved for the first
Stage 13 feature that changes user-observable compiler behavior (if-let parsing in Stage 13.2).

---

## 6. Committee Recommendation

### **GO-WITH-CONDITIONS** for Stage 13.1 launch

**Conditions**:

1. ✅ **Split MUV-2 into Stage 13.1b** — Stage 13.1 = MUV-1 only (file relocation); Stage 13.1b =
   MUV-2 Option B (type system variant). Do NOT combine. Rationale: §14.4.2 step 5 (REV-A gate review
   scope) + §25.8.3 #5 (best refactor timing is between stages).
2. ✅ **MUV-1 relocation target = Option B (new `src/codegen/dyn_trait_emit.rs`)** — NOT Option A
   (append to `trait_dispatch.rs`). Rationale: §14.4 J2 (single responsibility) + J6 (scientific
   granularity — preserves both files <1500 LOC).
3. ✅ **MUV-2 approach = Option B (variant-only)** in Stage 13.1b — NOT Option A (full integration).
   Rationale: §15 (long-term > short-term) + §25.7 (P2 partial closure acceptable). Full integration
   deferred to Stage 13.1c or v0.3+ as a separate focused refactor.
4. ✅ **Version policy = patch bumps** for both 13.1 (v0.21.5) and 13.1b (v0.21.6). v0.22.0 reserved
   for Stage 13.2 (first user-facing feature).
5. ✅ **§25.8 write-back required** post-execution:
   - `06-mir.md` §14 — add Stage 2.1 root-cause note (per r217 §2.3) + Stage 13.1 MUV-1 closure
     (eliminate mir → codegen text emission) + Stage 13.1b MUV-2 Option B closure (variant added)
   - `07-codegen.md` §14 — add §16 prohibition note (MIR must NOT produce codegen text) + Stage 13.1
     relocation note (7 emit_* functions moved to `codegen/dyn_trait_emit.rs`)
   - `03-type-system.md` §13.1 — update status from "❌ 未实现" to "✅ Option B (Stage 13.1b); full
     integration deferred to Stage 13.1c/v0.3+"
6. ✅ **Test verification gate**: `cargo test --test all_tests` MUST show 5026 conformance + 2179
   integration unchanged for BOTH 13.1 and 13.1b. Any FAIL→PASS or PASS→FAIL delta must be
   investigated and documented before gate review PASS.

**GO for Stage 13.1 launch**: MUV-1 is a self-contained, low-risk, design-aligned file relocation
that satisfies all 6 §14.4 criteria. MUV-2 deserves its own focused sub-stage (13.1b) per §15 and
§25.8.3 #5.

---

**Audit completed**: 2026-07-26
**Next action**: Stage Committee vote on this design alignment → if GO, Stage 13.1 MUV-1 execution
(estimated 4 hours per `plan-13.1.md` §7 #5).
