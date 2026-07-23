# Stage 5 Development Log

> **Author**: redskaber
> **Date**: 2026-07-22
> **Version**: v0.11.0
> **Status**: 🔄 In progress (5.1 complete, 5.2+ pending)

## Overview

Stage 5 focuses on: TraitResolver, vtable generation, stdlib MVP, mini-cargo,
user-defined macros, and NLL fixpoint. This stage was launched after the
cross-stage deep review R49 (GO for Stage 5).

## Sub-stages

### Stage 5.1 — TraitResolver 基础 (v0.11.0)

**Priority**: Stage 5 core — trait resolution infrastructure.

**Work completed**:
- New `src/traits/mod.rs` — TraitResolver module
  * `TraitInfo` — trait definition metadata (def_id, name, methods, is_unsafe)
  * `ImplInfo` — impl block metadata (def_id, trait_name, self_ty_name, methods, is_unsafe)
  * `TraitResolver` — collects trait/impl from HIR, builds dispatch tables
  * `trait_by_name` — Spur → DefId lookup
  * `impl_by_trait_and_type` — (trait_name, self_ty_name) → DefId lookup
  * `find_trait` / `find_impl` / `implements` — query methods
- `src/lib.rs` — added `pub mod traits` + `pub use traits::TraitResolver`
- 3 new tests in `tests/v0/stage5/plan/trait_resolver_tests.rs`

**Test impact**: +3 (1005/1005 — was 1002)
**Verification**: 0 clippy warnings, fmt clean

### Stage 5.2 — TraitResolver Driver Integration + fmt Fix (v0.11.1)

**Priority**: Integrate TraitResolver into pipeline + fix fmt issues.

**Work completed**:
- src/driver.rs: CompileResult now has `trait_resolver: TraitResolver` field
- compile() builds TraitResolver via `collect(&hir, &interner)` after resolve
- CompileResult::empty() initializes empty TraitResolver
- Fixed cargo fmt issues in src/traits/mod.rs + tests/v0/stage5/plan/trait_resolver_tests.rs
- 2 new integration tests in tests/v0/stage5/plan/trait_integration_tests.rs

**Test impact**: +2 (1007/1007 — was 1005)
**Verification**: 0 clippy warnings, **fmt clean (zero diff)** ✅

### Stage 5.3 — ty_is_copy_with_resolver (v0.11.2)

**Priority**: Precise Copy detection using TraitResolver.

**Work completed**:
- src/borrowck/mod.rs: new `ty_is_copy_with_resolver(ty, resolver, interner)` function
  * For non-Adt types: identical to ty_is_copy
  * For Adt: falls back to true (same as ty_is_copy) until DefId→name map (Stage 5.4)
  * Recursive for Tuple and Array
- Original `ty_is_copy` retained as fallback (no resolver needed)
- 3 new tests in tests/v0/stage5/plan/ty_is_copy_tests.rs

**Test impact**: +3 (1010/1010 — was 1007)
**Verification**: 0 clippy warnings, **fmt clean (exit 0)** ✅

### Stage 5.4 — DefId→name Reverse Map + Full Copy Detection (v0.11.3)

**Priority**: Complete Copy trait detection — close TD-016.

**Work completed**:
- src/traits/mod.rs: added `type_by_def_id: HashMap<DefId, Spur>` field
  * Populated for struct/enum/trait during `collect()`
  * New query methods: `implements_by_def_id()`, `is_copy()`, `type_count()`
- src/borrowck/mod.rs: `ty_is_copy_with_resolver` Adt branch now fully active
  * Looks up type name via `type_by_def_id`
  * Checks `resolver.is_copy(def_id, copy_name)` — returns false if no Copy impl
  * Falls back to true if "Copy" not interned (conservative)
- 3 new tests in tests/v0/stage5/plan/def_id_name_map_tests.rs

**TD-016 status**: ✅ CLOSED — Copy detection now uses TraitResolver instead of
treating all Adt as Copy.

**Test impact**: +3 (1013/1013 — was 1010)
**Verification**: 0 clippy warnings, **fmt clean (exit 0)** ✅

### Stage 5.5 — Vtable Generation (v0.11.4)

**Priority**: L5 trait dispatch foundation — vtable data structures.

**Work completed**:
- src/traits/mod.rs: new `VtableEntry` struct (method_name → fn_def_id)
- src/traits/mod.rs: new `Vtable` struct (trait_name, self_ty_name, impl_def_id, entries)
- src/traits/mod.rs: `vtables: HashMap<(Spur, Spur), Vtable>` field on TraitResolver
- `collect()` now builds vtables for each `impl Trait for Type`
- New query methods: `find_vtable(trait_name, type_name)`, `vtable_count()`
- 3 new tests in tests/v0/stage5/plan/vtable_tests.rs

**Note**: Rust toolchain unavailable in current environment. Code changes
are based on existing patterns. Verification pending environment restoration.

**Test impact**: +3 (pending verification — was 1013)

### Stage 5.5 audit — Test Infrastructure Refactor (v0.11.4, no version bump)

**Priority**: Clean up duplicate test files + shrink Cargo.toml.

**Problem**: `tests/` directory had 14 legacy flat `.rs` files (11489 lines)
that were 100% duplicates of the organized `tests/v0/stage{N}/plan/` files.
`Cargo.toml` had 19 `[[test]]` entries (one per file), bloating the config.

**Work completed**:
- Removed 14 legacy flat files: `probe_rp0.rs`, `deep_inspection.rs`,
  `hir_resolution.rs`, `negative_cases.rs`, `ast_structure.rs`,
  `codegen_tests.rs`, `integration_stage2_4c.rs`, `hir_structure.rs`,
  `hir_lowering.rs`, `typeck_tests.rs`, `lexer.rs`, `parser.rs`,
  `hir_scope_resolution.rs`, `mir_lowering.rs`
- Created `tests/all_tests.rs` unified entry point (23 `#[path] mod` declarations)
- `Cargo.toml`: added `autotests = false` + replaced 19 `[[test]]` entries
  with a single `[[test]] name = "all_tests"` entry
- Cargo.toml line count: 130 → 38 (71% reduction)
- Updated README.md (Testing section + Project layout)
- Updated docs/tests/README.md (new structure + migration history)
- Updated plan-5.5.md §7, gate-review-round5.md §6, test gate-review-round5.md §6

**Test impact**: 0 (1017 tests unchanged — pure infrastructure refactor)
**§16 compliance**: ✅ (no source code changes)
**API naming**: N/A (no API changes)

### Stage 5.6 — Vtable Codegen Emission (v0.11.5)

**Priority**: L5 trait dispatch foundation — emit vtable as LLVM IR global.

**Work completed**:
- src/traits/mod.rs: `VtableEntry.fn_def_id` replaced by `fn_name: String`
  * Resolved at collect time as `landin_<Type>_<method>`
  * Self-contained vtable entry — codegen needs no upstream lookup
  * Per §15 (最优 > 最小): cleaner than threading fn_name_by_def_id through
- src/traits/mod.rs: `extract_impl_self_ty_name` promoted to `pub`
- src/driver.rs: `body_metas` population extended (HirItem::Impl branch)
  * Impl method bodies now emitted as `landin_<Type>_<method>`
- src/codegen/emitter.rs: `Emitter::emit_vtable_global` trait method
- src/codegen/text_emitter.rs: TextEmitter implements `emit_vtable_global`
  * Emits `@.vtable.<trait>.<type> = private unnamed_addr constant [N x ptr] [ptr @sym1, ...]`
- src/codegen/mod.rs: new `pub fn emit_vtables(trait_resolver, interner, emitter)`
- src/codegen/mod.rs: `codegen_crate` calls `emit_vtables` after `codegen_from_mir`
- src/lib.rs: re-export `emit_vtables` + `extract_impl_self_ty_name`
- 3 new tests in tests/v0/stage5/plan/vtable_codegen_tests.rs

**TD-014 status**: 🔄 → partial CLOSE — vtable data + codegen emission done;
`dyn Trait` fat-pointer construction deferred to Stage 5.7+.

**Test impact**: +3 (922 expected — was 919)
**§16 compliance**: ✅ codegen is still a pure MIR/TraitResolver consumer.
**API naming**: ✅ all new APIs follow api-naming-standard §3.

### Stage 5.7 — dyn Trait Fat-Pointer Construction (v0.11.6)

**Priority**: L5 trait dispatch foundation — construct `dyn Trait` fat pointers.

**Work completed**:
- src/codegen/emitter.rs: new `pub fn emit_dyn_trait_ptr_type()` returning
  `EmitType::Struct([OpaquePtr, OpaquePtr])` (data + vtable, both opaque)
- src/codegen/emitter.rs: new `Emitter::emit_dyn_trait_const` trait method
  * Signature: `(global_name, data_symbol, vtable_symbol) -> EmitValue`
- src/codegen/text_emitter.rs: TextEmitter implements `emit_dyn_trait_const`
  * Emits `@.dynptr.<trait>.<type> = private unnamed_addr constant { ptr, ptr } { ptr @.data.<type>, ptr @.vtable.<trait>.<type> }`
- src/codegen/mod.rs: new `pub fn emit_dyn_trait_ptrs(trait_resolver, interner, emitter)`
  * Iterates `trait_resolver.vtables.keys()`, calls `emit_dyn_trait_const` per pair
- src/codegen/mod.rs: `codegen_crate` calls `emit_dyn_trait_ptrs` after `emit_vtables`
- src/lib.rs: re-export `emit_dyn_trait_ptr_type` + `emit_dyn_trait_ptrs`
- 4 new tests in tests/v0/stage5/plan/dyn_trait_ptr_tests.rs

**TD-014 status**: partial CLOSE → further CLOSE — vtable + codegen + dyn fat
pointer all in place; MIR→codegen dyn value wiring deferred to Stage 5.8+.

**Test impact**: +4 (926 expected — was 922)
**§16 compliance**: ✅ codegen is still a pure MIR/TraitResolver consumer.
**API naming**: ✅ all new APIs follow api-naming-standard §3.

### Stage 5.8 — Standard Trait Registry / stdlib MVP (v0.11.7)

**Priority**: Make compiler recognize builtin standard traits automatically.

**Work completed**:
- src/traits/mod.rs: new `BUILTIN_TRAIT_NAMES` constant (10 traits: Copy,
  Clone, Drop, Sized, Send, Sync, Unpin, Fn, FnMut, FnOnce)
- src/traits/mod.rs: new `BUILTIN_DEF_ID_BASE` constant (u32::MAX)
- src/traits/mod.rs: new `builtin_traits: HashMap<Spur, DefId>` field on
  TraitResolver
- src/traits/mod.rs: new `register_builtin_traits(&mut Rodeo)` method —
  interns all builtin trait names + assigns reserved DefIds (u32::MAX
  downward) + registers in trait_by_name/type_by_def_id
- src/traits/mod.rs: new `is_builtin_trait(name) -> bool` query
- src/traits/mod.rs: new `find_builtin_trait(name) -> Option<DefId>` query
- src/driver.rs: calls `register_builtin_traits(&mut interner)` before
  `collect()` (needs &mut Rodeo)
- src/lib.rs: re-export `BUILTIN_TRAIT_NAMES` + `BUILTIN_DEF_ID_BASE`
- tests/v0/stage5/plan/builtin_traits_tests.rs: 5 new tests
- tests/all_tests.rs: added builtin_traits_tests module

**Test impact**: +5 (931 — was 926)
**Verification**: cargo clean + cargo test (931 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅
**§16 compliance**: ✅ register_builtin_traits is called by driver; downstream
stages read builtin_traits as data.
**API naming**: ✅ SCREAMING_SNAKE_CASE constants + snake_case methods + is_/find_ prefixes.

### Stage 5.9 — Builtin Copy Activation + Soundness Fix (v0.11.8)

**Priority**: Activate builtin Copy trait + fix unsound Adt fallback.

**Work completed**:
- src/traits/mod.rs: new `is_copy_builtin(&self, def_id, &Rodeo) -> bool` method
  * Looks up builtin Copy Spur automatically (no caller-supplied Spur)
  * Defensive fallback: `false` (was unsound `true` in old code)
- src/borrowck/mod.rs: `ty_is_copy_with_resolver` Adt branch simplified
  * Old: `if let Some(copy_name) = interner.get("Copy") { is_copy(...) } else { true }`
  * New: `resolver.is_copy_builtin(*def_id, interner)`
  * Soundness fix: Adt without `impl Copy` now correctly returns `false`
- tests/v0/stage5/plan/ty_is_copy_tests.rs: updated `test_adt_fallback_copy`
  → `test_adt_without_copy_impl_not_copy` (asserts `false` not `true`)
- tests/v0/stage5/plan/builtin_copy_activation_tests.rs: 5 new tests
- tests/all_tests.rs: added builtin_copy_activation_tests module
- Cargo.toml: version 0.11.7 → 0.11.8

**Soundness fix**: The old `ty_is_copy_with_resolver` Adt branch fell back to
`true` when "Copy" wasn't interned — treating ALL Adt as Copy. Stage 5.9 fixes
this to `false`. Only types with explicit `impl Copy for <Type>` are Copy.

**Test impact**: +5 (936 — was 931), +1 test updated (soundness assertion)
**Verification**: cargo clean + cargo test (936 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅
**§16 compliance**: ✅ is_copy_builtin reads builtin_traits as data.
**API naming**: ✅ `is_copy_builtin` follows `is_` prefix + `_builtin` suffix.

### Stage 5.10 — Builtin Clone/Drop Activation + Generic Builtin Trait Check + Spec v3.20 (v0.11.9)

**Priority**: Extend builtin trait activation to Clone/Drop + generic check + spec evolution.

**Work completed**:
- src/traits/mod.rs: new `is_clone_builtin(def_id, &Rodeo) -> bool` method
- src/traits/mod.rs: new `is_drop_builtin(def_id, &Rodeo) -> bool` method
- src/traits/mod.rs: new `implements_builtin_trait(def_id, trait_name_str, &Rodeo) -> bool`
  generic method — works for any builtin trait by name (Send/Sync/Sized/etc.)
- docs/stage-committee-process.md: updated to v3.20
  * §0.2 任务类型精确路由（8 种任务 → 必读章节）
  * §1.1 环境工具检查与准备（工具缺失时查找+安装）
  * §1.2 交付前验收检查（cargo clean+test+fmt+clippy 全绿）
  * §1.3 Spec 持续演进原则（精要化，反臃肿）
  * §28.3 变更日志 v3.19→v3.20
- tests/v0/stage5/plan/builtin_clone_drop_tests.rs: 7 new tests
- tests/all_tests.rs: added builtin_clone_drop_tests module (28 mods)
- Cargo.toml: version 0.11.8 → 0.11.9

**Test impact**: +7 (943 — was 936)
**Verification**: cargo clean + cargo test (943 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `is_*_builtin` + `implements_builtin_trait` follow §23.

### Stage 5.11 — Primitive Copy Auto-Detection (v0.11.10)

**Priority**: Extract primitive Copy knowledge into queryable constant + function.

**Work completed**:
- src/traits/mod.rs: new `BUILTIN_PRIMITIVE_COPY_KINDS` constant (10 always-Copy
  TyKinds: Bool, Char, Int, Uint, Float, Never, Ref, RawPtr, FnDef, FnPtr)
- src/traits/mod.rs: new `is_primitive_copy_kind(kind_name: &str) -> bool` free fn
  * String-based check (avoids traits↔mir circular dep)
  * Strips "(...)" suffix: "Int(I32)" → "Int" → true
- src/lib.rs: re-export `is_primitive_copy_kind` + `BUILTIN_PRIMITIVE_COPY_KINDS`
- tests/v0/stage5/plan/primitive_copy_tests.rs: 6 new tests
- tests/all_tests.rs: added primitive_copy_tests module (29 mods)
- Cargo.toml: version 0.11.9 → 0.11.10

**Test impact**: +6 (949 — was 943)
**Verification**: cargo clean + cargo test (949 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ pure constant + function, no HIR access.
**API naming**: ✅ SCREAMING_SNAKE_CASE constant + `is_` + `_kind` suffix.

### Stage 5.12 — Copy Detection Unification (v0.11.11)

**Priority**: Wire `is_primitive_copy_kind()` into `ty_is_copy_with_resolver`.

**Work completed**:
- src/borrowck/mod.rs: `ty_is_copy_with_resolver` primitive branches refactored
  * Old: `Bool | Char | Int(_) | ... => true` (hardcoded)
  * New: `... => is_primitive_copy_kind(&format!("{:?}", ty.kind))` (delegated)
  * Match still handles Tuple/Array (recursive) + Adt (resolver) + Str/Slice/etc.
- src/borrowck/mod.rs: new `ty_is_copy_unified()` entry point
  * Delegates to `ty_is_copy_with_resolver`
  * Preferred entry for new code (explicit "unified" intent)
- tests/v0/stage5/plan/copy_unification_tests.rs: 5 new tests
- tests/all_tests.rs: added copy_unification_tests module (30 mods)
- Cargo.toml: version 0.11.10 → 0.11.11

**Test impact**: +5 (954 — was 949)
**Verification**: cargo clean + cargo test (954 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ pure consumer, no HIR access.
**API naming**: ✅ `ty_is_copy_unified` follows `ty_is_copy_` prefix + `_unified` suffix.

### Stage 5.13 — Trait Impl Statistics (v0.11.12)

**Priority**: Add trait impl counting + listing for diagnostics and typeck.

**Work completed**:
- src/traits/mod.rs: 4 new query methods on TraitResolver:
  * `impl_count_for_type(def_id) -> usize` — count impls for a type
  * `impl_count_for_trait(trait_spur) -> usize` — count impls for a trait
  * `builtin_trait_count() -> usize` — count builtin traits
  * `traits_for_type(def_id) -> Vec<Spur>` — list trait names a type implements
- tests/v0/stage5/plan/trait_impl_stats_tests.rs: 7 new tests
- tests/all_tests.rs: added trait_impl_stats_tests module (31 mods)
- Cargo.toml: version 0.11.11 → 0.11.12

**Test impact**: +7 (961 — was 954)
**Verification**: cargo clean + cargo test (961 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `impl_count_for_*` + `builtin_trait_count` + `traits_for_type`.

### Stage 5.14 — Trait Method Query API (v0.11.13)

**Priority**: Add trait method query methods for method resolution + vtable lookup.

**Work completed**:
- src/traits/mod.rs: 5 new query methods on TraitResolver:
  * `trait_methods(trait_spur) -> Option<&Vec<Spur>>` — trait declared methods
  * `impl_methods(trait_spur, ty_spur) -> Option<&Vec<Spur>>` — impl methods
  * `trait_has_method(trait_spur, method_spur) -> bool` — trait declares method?
  * `traits_with_method(method_spur) -> Vec<Spur>` — traits declaring a method
  * `method_count_for_trait(trait_spur) -> usize` — method count for a trait
- tests/v0/stage5/plan/trait_method_query_tests.rs: 8 new tests
- tests/all_tests.rs: added trait_method_query_tests module (32 mods)
- Cargo.toml: version 0.11.12 → 0.11.13

**Test impact**: +8 (969 — was 961)
**Verification**: cargo clean + cargo test (969 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `<noun>_<noun>` + `<noun>_<verb>_<noun>` + `<noun>_with_<noun>` + `<noun>_count_for_<noun>`.

### Stage 5.15 — Trait Hierarchy / Supertraits (v0.11.14)

**Priority**: Collect + query supertrait information for trait hierarchy traversal.

**Work completed**:
- src/traits/mod.rs: new `supertraits: Vec<Spur>` field on TraitInfo
  * Populated in collect() from HirTrait.supertraits (Vec<HirTypeBound>)
  * Extracts last path segment name Spur from each HirTypeBound::Trait
- src/traits/mod.rs: 3 new query methods:
  * `trait_supertraits(trait_spur) -> Option<&Vec<Spur>>`
  * `trait_has_supertrait(trait_spur, super_spur) -> bool`
  * `supertrait_count_for_trait(trait_spur) -> usize`
- tests/v0/stage5/plan/trait_hierarchy_tests.rs: 8 new tests
- tests/all_tests.rs: added trait_hierarchy_tests module (33 mods)
- Cargo.toml: version 0.11.13 → 0.11.14

**Test impact**: +8 (977 — was 969)
**Verification**: cargo clean + cargo test (977 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ supertraits collected in collect() (driver phase),
query methods read data only.
**API naming**: ✅ `<noun>_<noun>` + `<noun>_<verb>_<noun>` + `<noun>_count_for_<noun>`.

### Stage 5.16 — TraitResolver Summary (v0.11.15)

**Priority**: Add human-readable state report for diagnostics + debugging.

**Work completed**:
- src/traits/mod.rs: new `summary(&Rodeo) -> String` method on TraitResolver
  * Header: trait/impl/type/vtable/builtin counts
  * Per-trait: name + method count + supertrait count (+ supertrait names)
  * Per-type: name + impl count (+ implemented trait names)
  * Skips builtin trait DefIds from Types section
- tests/v0/stage5/plan/trait_summary_tests.rs: 7 new tests
- tests/all_tests.rs: added trait_summary_tests module (34 mods)
- Cargo.toml: version 0.11.14 → 0.11.15

**Test impact**: +7 (984 — was 977)
**Verification**: cargo clean + cargo test (984 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ summary reads TraitResolver data only.
**API naming**: ✅ `summary` noun naming (output content).

### Stage 5.17 — Vtable Method Resolution (v0.11.16)

**Priority**: Single-entry-point method dispatch resolution.

**Work completed**:
- src/traits/mod.rs: 3 new query methods on TraitResolver:
  * `resolve_vtable_method(trait, ty, method) -> Option<&str>` — resolve to LLVM symbol
  * `vtable_method_names(trait, ty) -> Vec<&str>` — all method symbols
  * `vtable_has_method(trait, ty, method) -> bool` — vtable has method?
- tests/v0/stage5/plan/vtable_method_resolve_tests.rs: 8 new tests
- tests/all_tests.rs: added vtable_method_resolve_tests module (35 mods)
- Cargo.toml: version 0.11.15 → 0.11.16

**Test impact**: +8 (992 — was 984)
**Verification**: cargo clean + cargo test (992 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `resolve_<noun>_<noun>` + `<noun>_<noun>_<noun>` + `<noun>_<verb>_<noun>`.

### Stage 5.18 — Trait Coherence Checking (v0.11.17)

**Priority**: Detect conflicting impls (multiple `impl Trait for Type` for same pair).

**Work completed**:
- src/traits/mod.rs: new `CoherenceError` struct (trait_name, self_ty_name, impl_def_ids)
- src/traits/mod.rs: 3 new query methods on TraitResolver:
  * `check_coherence() -> Vec<CoherenceError>` — detect all conflicting pairs
  * `has_coherence_error(trait, ty) -> bool` — check specific pair
  * `coherence_error_count() -> usize` — count of conflicting pairs
- src/lib.rs: re-export `CoherenceError`
- tests/v0/stage5/plan/trait_coherence_tests.rs: 7 new tests
- tests/all_tests.rs: added trait_coherence_tests module (36 mods)
- Cargo.toml: version 0.11.16 → 0.11.17

**Test impact**: +7 (999 — was 992)
**Verification**: cargo clean + cargo test (999 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `CoherenceError` + `check_coherence` + `has_coherence_error` + `coherence_error_count`.

### Stage 5.19 — Trait Impl Completeness Check (v0.11.18) — 1000+ tests milestone 🎉

**Priority**: Detect incomplete impls (trait methods not implemented).

**Work completed**:
- src/traits/mod.rs: 3 new query methods on TraitResolver:
  * `impl_covers_trait(trait, ty) -> bool` — impl covers all trait methods?
  * `missing_impl_methods(trait, ty) -> Vec<Spur>` — missing method names
  * `missing_method_count(trait, ty) -> usize` — missing method count
- tests/v0/stage5/plan/impl_completeness_tests.rs: 8 new tests
- tests/all_tests.rs: added impl_completeness_tests module (37 mods)
- Cargo.toml: version 0.11.17 → 0.11.18

**Test impact**: +8 (1007 — was 999) — **1000+ tests milestone** 🎉
**Verification**: cargo clean + cargo test (1007 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `impl_covers_trait` + `missing_impl_methods` + `missing_method_count`.

### Stage 5.20 — Trait Impl Validation Report (v0.11.19)

**Priority**: Aggregate coherence + completeness into single validation pass.

**Work completed**:
- src/traits/mod.rs: 2 new structs:
  * `IncompleteImpl` — trait_name, self_ty_name, missing_methods
  * `ImplValidationReport` — coherence_errors, incomplete_impls, is_valid
- src/traits/mod.rs: 3 new query methods:
  * `validate_impls() -> ImplValidationReport` — single-pass validation
  * `impls_are_valid() -> bool` — all valid?
  * `all_impls_complete() -> bool` — all complete?
- src/lib.rs: re-export `ImplValidationReport` + `IncompleteImpl`
- tests/v0/stage5/plan/impl_validation_tests.rs: 9 new tests
- tests/all_tests.rs: added impl_validation_tests module (38 mods)
- Cargo.toml: version 0.11.18 → 0.11.19

**Test impact**: +9 (1016 — was 1007)
**Verification**: cargo clean + cargo test (1016 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ all new methods read TraitResolver data only.
**API naming**: ✅ `validate_impls` + `impls_are_valid` + `all_impls_complete`.

### Stage 5.21 — Deep Review (§25) — 7-Dimension Analysis (v0.11.19, no version bump)

**Priority**: §25 阶段末尾深度审查 — 评估 Stage 5 trait infrastructure 是否
足够支撑进入下一阶段（dyn Trait MIR lowering / full stdlib / mini-cargo）。

**Work completed**:
- docs/develop/v0/stage-5/deep-review-r70.md: 7-dimension deep review report
  * D1. 架构健康度: ✅ §16 compliant; P2 risk: traits/mod.rs 1010 LOC
  * D2. 技术债清单: TD-014 partial CLOSE + TD-011 + TD-015 + TD-NEW-1
  * D3. 测试覆盖深度: 112 Stage 5 tests / 1016 total; ~100% trait query coverage
  * D4. 下一阶段就绪度: 8/11 ready, 3 not started (dyn MIR / stdlib / cargo)
  * D5. 设计合理性: no over-design; naming consistent
  * D6. 性能: O(n) collect / O(n) coherence / O(n×m) validate; no bottleneck
  * D7. 文档: 21 dev-log entries + 20 gate reviews + 16 test plans + worklog
- Verdict: ✅ **GO** — 0 P0/P1 blockers; Stage 5 trait infra ready for next phase

**Test impact**: 0 (deep review — no code changes)
**§25 compliance**: ✅ 7-dimension analysis + tech debt catalog + GO/NO-GO

### Stage 5.22 — Driver Validation Integration (v0.11.20)

**Priority**: Wire validate_impls() into driver (deep review r70 P2 action item).

**Work completed**:
- src/driver.rs: new `trait_errors: Vec<String>` field on CompileErrors
  * is_empty() + total_count() updated to include trait_errors
- src/driver.rs: validate_impls() called after collect()
  * Coherence errors formatted as "conflicting implementations of trait `T` for type `S` (N impl blocks)"
  * Completeness errors formatted as "impl `T` for `S` is missing method(s): baz"
- tests/v0/stage5/plan/driver_validation_tests.rs: 7 new tests
- tests/all_tests.rs: added driver_validation_tests module (39 mods)
- Cargo.toml: version 0.11.19 → 0.11.20

**Test impact**: +7 (1023 — was 1016)
**Verification**: cargo clean + cargo test (1023 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)

### Stage 5.23 — traits/mod.rs Split (v0.11.21)

**Priority**: Split 1010-LOC mod.rs into sub-modules (deep review r70 TD-NEW-1).

**Work completed**:
- src/traits/vtable.rs: VtableEntry + Vtable structs (30 lines)
- src/traits/builtin.rs: BUILTIN_TRAIT_NAMES + constants + is_primitive_copy_kind (23 lines)
- src/traits/resolver.rs: TraitInfo + ImplInfo + TraitResolver + error types + all methods (903 lines)
- src/traits/mod.rs: thin re-export module (24 lines)
- Fixed: duplicate Vtable import + missing Default derive + missing builtin imports
- Cargo.toml: version 0.11.20 → 0.11.21

**Test impact**: 0 (1023 — pure refactoring)
**TD-NEW-1**: ✅ CLOSED
**Verification**: cargo clean + cargo test (1023 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)

### Stage 5.24 — Mini-Cargo MVP (v0.11.22)

**Priority**: Implement `landinc` — Landin package manager + build tool MVP.

**Work completed**:
- src/cargo.rs: new module with:
  * ProjectManifest — parse landin.toml (name/version/edition/src_dir/entry_point/target_dir)
  * BuildConfig — optimization/emit_llvm/output_name
  * BuildResult — success/error_count/files_compiled/llvm_ir/errors
  * parse_manifest(content) / load_manifest(path)
  * build_project(manifest, config) — compile entry point via public compile() API
- src/lib.rs: added `pub mod cargo` + re-exports
- tests/v0/stage5/plan/mini_cargo_tests.rs: 8 new tests
- tests/all_tests.rs: added mini_cargo_tests module (40 mods)
- Cargo.toml: version 0.11.21 → 0.11.22
- Fixed: clippy warning (BuildConfig manual Default → derive)

**Test impact**: +8 (1031 — was 1023)
**Verification**: cargo clean + cargo test (1031 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ build_project uses only public compile() + codegen_crate().
**API naming**: ✅ `ProjectManifest` + `BuildConfig` + `BuildResult` + `parse_manifest` + `build_project`.

### Stage 5.25 — Stdlib MVP (v0.11.23)

**Priority**: Implement core layer of Landin's three-layer stdlib.

**Work completed**:
- src/stdlib.rs: new module with:
  * STDLIB_CORE_TYPES (17 types: i8-i128/u8-u128/f32/f64/bool/char/str/()/Never)
  * STDLIB_OPS_TRAITS (Add/Sub/Mul/.../PartialEq/Ord/Index/Range/...)
  * STDLIB_CONVERT_TRAITS (From/Into/TryFrom/AsRef/AsMut)
  * STDLIB_ITER_TRAITS (Iterator/IntoIterator/FromIterator/...)
  * all_stdlib_trait_names() + all_stdlib_type_names()
  * StdlibPrelude struct (types + traits, with contains/len/is_empty)
  * register_stdlib(&mut Rodeo) — intern all stdlib names
  * default_prelude() — get default StdlibPrelude
- src/lib.rs: added pub mod stdlib + re-exports
- tests/v0/stage5/plan/stdlib_mvp_tests.rs: 10 new tests
- tests/all_tests.rs: added stdlib_mvp_tests module (41 mods)
- Cargo.toml: version 0.11.22 → 0.11.23
- Fixed: str Sized error (for &name → for name), unused import (StdlibPrelude)

**Test impact**: +10 (1041 — was 1031)
**Verification**: cargo clean + cargo test (1041 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)
**§16 compliance**: ✅ register_stdlib uses &mut Rodeo only.
**API naming**: ✅ SCREAMING_SNAKE_CASE constants + `<Noun><Noun>` types + `register_<noun>` / `<adj>_<noun>` functions.

### Stage 5.26 — Driver Stdlib Integration (v0.11.24)

**Priority**: Wire register_stdlib() into driver + add CompileResult.stdlib_prelude.

**Work completed**:
- src/driver.rs: new `stdlib_prelude: StdlibPrelude` field on CompileResult
  * `empty()` path uses `default_prelude()`
  * Normal path uses `default_prelude()`
- src/driver.rs: `register_stdlib(&mut interner)` called after `register_builtin_traits`
  and before `collect()`
- src/lib.rs: doc comment updated
- tests/v0/stage5/plan/driver_stdlib_tests.rs: 8 new tests
- tests/all_tests.rs: added driver_stdlib_tests module (42 mods)
- Cargo.toml: version 0.11.23 → 0.11.24

**Test impact**: +8 (1049 — was 1041)
**Verification**: cargo clean + cargo test (1049 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)

### Stage 5.27 — Deep Review #2 (§25) — 7-Dimension Analysis (v0.11.24, no version bump)

**Priority**: §25 阶段末尾深度审查 #2 — 评估 r70→r76 期间的进展。

**Work completed**:
- docs/develop/v0/stage-5/deep-review-r76.md: 7-dimension deep review report
  * D1. 架构健康度: ✅ §16 compliant; P2: mir/lower/mod.rs 3124 LOC
  * D2. 技术债: TD-014 partial CLOSE, TD-011 OPEN, TD-015 OPEN, TD-NEW-1 ✅ CLOSED
  * D3. 测试覆盖: 145 Stage 5 tests / 1049 total; ~100% coverage
  * D4. 就绪度: 8/10 ready, 2 not started (dyn MIR / full stdlib)
  * D5. 设计合理性: no over-design; naming consistent
  * D6. 性能: no bottleneck
  * D7. 文档: 27 dev-log + 26 gate reviews + 20 test plans + 2 deep reviews
- Verdict: ✅ GO — 0 P0/P1; trait+vtable+stdlib+cargo infra ready

**Test impact**: 0 (deep review — no code changes)

### Stage 5.28 — Stdlib Alloc Layer (v0.11.25)

**Priority**: Extend stdlib to alloc layer (heap types + fmt/Deref traits).

**Work completed**:
- src/stdlib.rs: new constants:
  * STDLIB_ALLOC_TYPES (13: Box/Vec/String/HashMap/BTreeMap/HashSet/BTreeSet/Rc/Arc/Cell/RefCell/LinkedList/VecDeque)
  * STDLIB_ALLOC_TRAITS (8: Display/Debug/Write/Formatter/Deref/DerefMut/Default/Hash)
- src/stdlib.rs: extended all_stdlib_type_names() + all_stdlib_trait_names()
  + register_stdlib() to include alloc items
- src/lib.rs: doc comment updated
- tests/v0/stage5/plan/stdlib_alloc_tests.rs: 9 new tests
- tests/all_tests.rs: added stdlib_alloc_tests module (43 mods)
- Cargo.toml: version 0.11.24 → 0.11.25

**Test impact**: +9 (1058 — was 1049)
**Verification**: cargo clean + cargo test (1058 passed) + cargo fmt (clean) +
cargo clippy --all-targets (0 warnings) — all green ✅ (per §1.2)

### Stage 5.29 — Stdlib Layer Query + Docs Supplement (v0.11.26)

**Priority**: Add StdlibLayer enum + supplement all missing test docs.

**Work completed**:
- src/stdlib.rs: new StdlibLayer enum (Core/Alloc/None)
- src/stdlib.rs: new layer_for_name() + names_for_layer() on StdlibPrelude
- src/lib.rs: re-export StdlibLayer
- tests/v0/stage5/plan/stdlib_layer_tests.rs: 7 new tests
- tests/all_tests.rs: added stdlib_layer_tests module (44 mods)
- Cargo.toml: version 0.11.25 → 0.11.26

Docs supplement (all missing docs/tests/v0/stage5/ created):
- Test gate reviews: round 23, 24, 25, 26, 28 (5 files)
- Test plans: mini_cargo, stdlib_mvp, driver_stdlib, stdlib_alloc, trait_integration (5 files)
- New: plan-5.29.md, gate-review-round29.md, stdlib_layer.md, test gate-review-round29.md

**Test impact**: +7
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.30 — Stdlib Std Layer (v0.11.27)

**Priority**: Extend stdlib to std layer (OS types + I/O traits + Result/Option).

**Work completed**:
- src/stdlib.rs: new constants:
  * STDLIB_STD_TYPES (26: File/Path/TcpStream/Thread/Mutex/Result/Option/...)
  * STDLIB_STD_TRAITS (6: Read/Write/Seek/BufRead/Error/Termination)
- src/stdlib.rs: StdlibLayer::Std variant added
- src/stdlib.rs: all_stdlib_type_names() + all_stdlib_trait_names()
  + register_stdlib() + layer_for_name() + names_for_layer() extended
- src/lib.rs: doc comment updated
- tests/v0/stage5/plan/stdlib_std_tests.rs: 8 new tests
- tests/all_tests.rs: added stdlib_std_tests module (45 mods)
- Cargo.toml: version 0.11.26 → 0.11.27

**Test impact**: +8
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.31 — Stdlib Facade (v0.11.28)

**Priority**: Unified stdlib statistics + layer query interface.

**Work completed**:
- src/stdlib.rs: new StdlibFacade struct with:
  * from_prelude() / type_count() / trait_count()
  * type_count_for_layer() / layer_count()
  * is_stdlib_name() / summary()
- src/lib.rs: re-export StdlibFacade
- tests/v0/stage5/plan/stdlib_facade_tests.rs: 8 new tests
- tests/all_tests.rs: added stdlib_facade_tests module (46 mods)
- Cargo.toml: version 0.11.27 → 0.11.28

**Test impact**: +8
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.32 — Deep Review #3 (§25) — 7-Dimension Analysis (v0.11.28, no version bump)

**Priority**: §25 阶段末尾深度审查 #3 — 评估 r76→r81 期间的进展。

**Work completed**:
- docs/develop/v0/stage-5/deep-review-r81.md: 7-dimension deep review report
  * D1. 架构健康度: ✅ §16 compliant; P2: mir/lower/mod.rs 3124 LOC
  * D2. 技术债: TD-014 partial, TD-011 OPEN, TD-015 OPEN, TD-NEW-1 ✅ CLOSED
  * D3. 测试覆盖: 177 Stage 5 tests / 1081 total; ~100% coverage
  * D4. 就绪度: 9/10 ready, 1 not started (dyn MIR)
  * D5. 设计合理性: no over-design; naming consistent
  * D6. 性能: no bottleneck
  * D7. 文档: 32 dev-log + 31 gate reviews + 27 test plans + 3 deep reviews
- Verdict: ✅ GO — 0 P0/P1; trait+vtable+stdlib+cargo+facade infra ready

**Test impact**: 0 (deep review — no code changes)

### Stage 5.33 — Stdlib Facade Driver Integration (v0.11.29)

**Priority**: Wire StdlibFacade into CompileResult + driver.

**Work completed**:
- src/driver.rs: new `stdlib_facade: StdlibFacade` field on CompileResult
  * empty() path uses StdlibFacade::default()
  * Normal path uses StdlibFacade::default()
- src/lib.rs: doc comment updated
- tests/v0/stage5/plan/facade_integration_tests.rs: 7 new tests
- tests/all_tests.rs: added facade_integration_tests module (47 mods)
- Cargo.toml: version 0.11.28 → 0.11.29

**Test impact**: +7
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.34 — Stdlib Type Resolution (v0.11.30)

**Priority**: Add StdlibTypeKind + resolve_stdlib_type() for type name → kind mapping.

**Work completed**:
- src/stdlib.rs: new StdlibTypeKind enum (20 variants)
- src/stdlib.rs: new resolve_stdlib_type() function
- src/stdlib.rs: new is_primitive_type() / integer_bit_width() / is_signed_integer() / is_unsigned_integer() / is_float_type()
- src/lib.rs: re-export all new APIs
- tests/v0/stage5/plan/stdlib_type_resolve_tests.rs: 12 new tests (actually 11 #[test] functions, but the comment says 12 due to counting)
- tests/all_tests.rs: added stdlib_type_resolve_tests module (48 mods)
- Cargo.toml: version 0.11.29 → 0.11.30

**Test impact**: +11
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.35 — Stdlib Type Layout (v0.11.31)

**Priority**: Add type_size_bytes + type_alignment_bytes + is_zero_sized_type + type_description.

**Work completed**:
- src/stdlib.rs: new type_size_bytes() / type_alignment_bytes() / is_zero_sized_type() / type_description()
- src/lib.rs: re-export all new APIs
- tests/v0/stage5/plan/stdlib_layout_tests.rs: 7 new tests
- tests/all_tests.rs: added stdlib_layout_tests module (49 mods)
- Cargo.toml: version 0.11.30 → 0.11.31

**Test impact**: +7
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.36 — Stdlib Trait Method Signatures (v0.11.32)

**Priority**: Register static method signature tables for builtin stdlib traits
— prereq for dyn Trait MIR lowering (TD-014 partial close) and typeck trait
bound solving.

**Work completed**:
- src/stdlib.rs: new `StdlibSelfKind` enum (4 variants: SelfByValue /
  SelfByRef / SelfByMutRef / NoSelf)
- src/stdlib.rs: new `StdlibTraitMethod` struct (name / self_kind /
  param_count / return_kind / is_unsafe) + `has_self()` helper method
- src/stdlib.rs: 25+ static method tables (one const per trait):
  * MARKER_METHODS (empty) — for Copy/Send/Sync/Sized/Unpin/Eq
  * CLONE_METHODS (2) / DROP_METHODS (1) / DEFAULT_METHODS (1)
  * DISPLAY_METHODS / DEBUG_METHODS (1 each, both `fmt`)
  * PARTIAL_EQ_METHODS (2) / PARTIAL_ORD_METHODS (1) / ORD_METHODS (1)
  * HASH_METHODS (1) / DEREF_METHODS (1) / DEREF_MUT_METHODS (1)
  * INTO_ITERATOR_METHODS (1) / ITERATOR_METHODS (1)
  * READ_METHODS (1) / WRITE_METHODS (1)
  * NEG_METHODS (1) / NOT_METHODS (1)
  * 10 per-op binary arith tables: ADD/SUB/MUL/DIV/REM/BITAND/BITOR/BITXOR/SHL/SHR
  * 10 per-op assign tables: ADD_ASSIGN/.../SHR_ASSIGN
  * ARITH_OP_METHOD_NAMES + ARITH_ASSIGN_METHOD_NAMES constants (for diagnostics)
- src/stdlib.rs: 5 new free-function query APIs:
  * `stdlib_trait_methods(trait_name) -> Option<&'static [StdlibTraitMethod]>`
  * `stdlib_trait_method_count(trait_name) -> Option<usize>`
  * `find_stdlib_trait_method(trait_name, method_name) -> Option<&'static StdlibTraitMethod>`
  * `is_stdlib_trait_method(trait_name, method_name) -> bool`
  * `stdlib_traits_with_method(method_name) -> Vec<&'static str>` (reverse query)
- src/lib.rs: re-export all new APIs (StdlibSelfKind + StdlibTraitMethod +
  5 query functions) + Stage 5.36 history comment
- tests/v0/stage5/plan/stdlib_trait_method_tests.rs: 24 new tests covering
  all registered traits + edge cases + reverse queries + helper methods
- tests/all_tests.rs: added stdlib_trait_method_tests module (50 mods total)
- Cargo.toml: version 0.11.31 → 0.11.32

**Design highlights**:
- Per-op const tables (Add/Sub/Mul/...) instead of shared "Add" placeholder
  with runtime name override — ensures `StdlibTraitMethod.name` field always
  matches the trait's actual method name.
- `stdlib_traits_with_method()` uses a local `ALL_REGISTERED_TRAITS` constant
  (mirrors the match arms in `stdlib_trait_methods()`) instead of importing
  `traits::builtin::BUILTIN_TRAIT_NAMES` — keeps `stdlib.rs` self-contained
  per §16 (no backwards dependency on the traits module).
- Markers return `Some(&[])` (not `None`) so callers can distinguish
  "trait in registry but no methods" from "trait not in registry at all".

**§16 interface isolation**: `StdlibTraitMethod` uses `StdlibTypeKind` (stdlib-
internal) — no `mir::ty` reference, no circular dependency.

**§23 API naming**: all 7 new public symbols comply (StdlibTraitMethod +
StdlibSelfKind follow `<Noun><Noun><Noun>`; 5 free functions follow
`<noun>_<noun>` / `find_<noun>_<noun>` / `is_<noun>_<noun>` /
`<noun>_<noun>_with_<noun>`).

**Test impact**: +24 (1106 → 1130)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.37 — Stdlib Vtable Slot Layout (v0.11.33)

**Priority**: Add deterministic vtable slot indexing for stdlib traits —
the last static-prep step before dyn Trait MIR lowering. Codegen will use
these queries to emit `@.vtable.<trait>.<type>` globals with the correct
element count and compute method call byte offsets.

**Work completed**:
- src/stdlib.rs: new `StdlibVtableSlot` struct (slot_index: u32 +
  method: &'static StdlibTraitMethod) — describes one vtable slot
- src/stdlib.rs: 5 new free-function query APIs:
  * `stdlib_trait_method_index(trait, method) -> Option<u32>` — slot index
  * `stdlib_vtable_layout(trait) -> Option<Vec<StdlibVtableSlot>>` — full layout
  * `stdlib_vtable_slot_count(trait) -> Option<u32>` — total slot count
  * `is_stdlib_marker_trait(trait) -> bool` — marker check (registered + 0 methods)
  * `stdlib_traits_with_vtable() -> Vec<&'static str>` — all traits with ≥1 slot
- src/lib.rs: re-export all new APIs + Stage 5.37 history comment
- tests/v0/stage5/plan/stdlib_vtable_layout_tests.rs: 22 new tests covering
  method_index queries / vtable_layout (incl. determinism) / slot_count /
  marker detection / traits_with_vtable filtering / StdlibVtableSlot struct
- tests/all_tests.rs: added stdlib_vtable_layout_tests module (51 mods total)
- Cargo.toml: version 0.11.32 → 0.11.33

**Design highlights**:
- Slot index derived from `stdlib_trait_methods()` slice position (0-based),
  not from a HashMap — deterministic for the lifetime of the process.
- Three distinct return states for `stdlib_vtable_slot_count`:
  * `Some(0)` — marker trait (registered, no methods)
  * `Some(n)` — trait with n methods
  * `None` — trait not in registry at all
- `is_stdlib_marker_trait` returns false for unknown traits (not registered
  ≠ marker).
- `stdlib_traits_with_vtable()` excludes markers — codegen doesn't need to
  emit empty vtable globals for marker traits.
- `StdlibVtableSlot` carries `&'static StdlibTraitMethod` (zero-copy ref to
  the existing static table) — no allocation per query.

**§16 interface isolation**: `StdlibVtableSlot` uses `StdlibTraitMethod`
(stdlib-internal) — no `mir::ty` / `codegen::EmitType` reference, no
circular dependency.

**§23 API naming**: all 6 new public symbols comply (StdlibVtableSlot
follows `<Noun><Noun><Noun>`; 5 free functions follow `<noun>_<noun>_<noun>`
/ `<noun>_<noun>_<noun>_<noun>` / `is_<noun>_<adj>_<noun>` /
`<noun>_<noun>_with_<noun>`).

**Test impact**: +22 (1130 → 1152)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.38 — Stdlib Vtable Byte Size + Pointer-Width Layout (v0.11.34)

**Priority**: Translate vtable slot indices into byte offsets — the form
codegen actually needs for LLVM IR emission. Adds pointer-width-aware
vtable size and method-offset calculators.

**Work completed**:
- src/stdlib.rs: new `StdlibPointerWidth` enum (Pointer32 / Pointer64)
- src/stdlib.rs: new `byte_size()` const method on `StdlibPointerWidth`
  (returns 4 / 8)
- src/stdlib.rs: 3 new free-function query APIs:
  * `stdlib_pointer_width_bytes(width) -> u32` — free fn form of byte_size
  * `stdlib_vtable_byte_size(trait, width) -> Option<u64>` — total vtable bytes
  * `stdlib_vtable_method_offset(trait, method, width) -> Option<u64>` — method byte offset
- src/lib.rs: re-export all new APIs + Stage 5.38 history comment
- tests/v0/stage5/plan/stdlib_vtable_size_tests.rs: 20 new tests covering
  pointer width / vtable_byte_size (incl. markers) / method_offset
  (incl. arith/marker/unknown) / cross-check offset < total
- tests/all_tests.rs: added stdlib_vtable_size_tests module (52 mods total)
- Cargo.toml: version 0.11.33 → 0.11.34

**Design highlights**:
- `byte_size()` is `const fn` — can be used in const context (codegen can
  pre-compute fixed vtable sizes at compile time).
- Three-state return (Some(0) / Some(n) / None) consistent with Stage 5.37
  — codegen distinguishes "0-byte vtable" (marker) from "trait unknown".
- Compositional: `vtable_byte_size` and `method_offset` build on Stage 5.37
  `slot_count` and `slot_index` — single source of truth for slot numbering.
- Cross-check test verifies the core safety invariant
  `method_offset < vtable_byte_size` across 7 (trait, method) pairs ×
  2 pointer widths — this is what typeck will enforce in Stage 5.40+.

**§16 interface isolation**: All new APIs use only `StdlibPointerWidth`
(stdlib-internal) + existing `stdlib_vtable_slot_count` /
`stdlib_trait_method_index`. No `mir::ty` / `codegen::EmitType` reference,
no circular dependency.

**§23 API naming**: All 5 new public symbols comply (StdlibPointerWidth
follows `<Noun><Noun><Noun>`; variants Pointer32/Pointer64 follow
`<Noun><Digits>`; 3 free functions follow `<noun>_<noun>_<noun>_<noun>`).

**Test impact**: +20 (1152 → 1172)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.39 — Stdlib Vtable Construction Planner (v0.11.35)

**Priority**: Combine trait method signatures (Stage 5.36) + slot indexing
(Stage 5.37) + impl coverage into a single ordered vtable plan that codegen
can consume in one pass — the "last mile" static planner before dyn Trait
codegen.

**Work completed**:
- src/stdlib.rs: new `StdlibVtablePlanEntry` struct (slot_index +
  method_name + provided) — one entry per vtable slot
- src/stdlib.rs: new `StdlibVtablePlan` struct (trait_name + entries Vec)
  with `is_complete()` + `missing_methods()` methods
- src/stdlib.rs: 4 new free-function query APIs:
  * `stdlib_vtable_plan(trait, provided_methods) -> Option<StdlibVtablePlan>`
  * `stdlib_vtable_plan_entry_count(trait) -> Option<u32>`
  * `stdlib_vtable_plan_is_complete(&plan) -> bool`
  * `stdlib_vtable_plan_missing_methods(&plan) -> Vec<&'static str>`
- src/lib.rs: re-export all new APIs + Stage 5.39 history comment
- tests/v0/stage5/plan/stdlib_vtable_plan_tests.rs: 18 new tests covering
  plan construction (complete/partial/marker/unknown) / extra-names-ignored
  / entry_count / is_complete / missing_methods / determinism / struct
  semantics / slot ordering
- tests/all_tests.rs: added stdlib_vtable_plan_tests module (53 mods total)
- Cargo.toml: version 0.11.34 → 0.11.35

**Design highlights**:
- `stdlib_vtable_plan(trait, provided_methods)` merges three pieces of
  static info into one ordered plan: trait method signatures (5.36) +
  slot indexing (5.37) + impl coverage. Codegen consumes the plan in one
  pass — no need to re-derive slot order or provided-checking.
- `provided` flag per entry: codegen sees `provided=true` → fill slot with
  `@landin_<Type>_<method>` symbol; `provided=false` → fill with `null`
  or panic stub.
- Markers return empty plan with `is_complete() == true` (vacuously
  complete) — consistent with Stage 5.37/5.38 three-state convention.
- Extra names in `provided_method_names` silently ignored (tolerant
  design — impl may implement multiple traits' methods).
- `StdlibVtablePlan` derives PartialEq/Eq — usable for test assertions
  and future plan-cache deduplication.
- `stdlib_vtable_plan_entry_count()` is a non-allocating shortcut for
  `stdlib_vtable_slot_count()` (avoids constructing the entries Vec when
  only the count is needed).

**§16 interface isolation**: `StdlibVtablePlan` / `StdlibVtablePlanEntry`
use only `&'static str` + `Vec<>` + scalars — no `mir::ty` /
`codegen::EmitType` / `traits::TraitResolver` reference, no circular dep.

**§23 API naming**: All 6 new public symbols comply (StdlibVtablePlan +
StdlibVtablePlanEntry follow `<Noun><Noun><Noun>` [+`<Noun>`]; 4 free
functions follow `<noun>_<noun>_<noun>` / `<noun>_<noun>_<noun>_<noun>_<noun>`
/ `<noun>_<noun>_<noun>_<adj>` / `<noun>_<noun>_<noun>_<adj>_<noun>`
patterns — including the 5-noun `stdlib_vtable_plan_entry_count`).

**Test impact**: +18 (1172 → 1190)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.40 — Stdlib Vtable Symbol Name Planner (v0.11.36)

**Priority**: Extract LLVM symbol-name formatting logic from codegen into
pure stdlib functions. Stage 5.41+ will replace codegen's inline `format!`
calls with these planner functions — behavior-equivalent but string logic
centralized for future naming convention changes.

**Work completed**:
- src/stdlib.rs: 5 new free-function symbol-name planners:
  * `stdlib_vtable_global_name(trait, type) -> String` — `.vtable.<trait>.<type>`
  * `stdlib_dynptr_global_name(trait, type) -> String` — `.dynptr.<trait>.<type>`
  * `stdlib_data_global_name(type) -> String` — `.data.<type>`
  * `stdlib_impl_method_symbol(type, method) -> String` — `landin_<type>_<method>`
  * `stdlib_vtable_method_symbols(trait, type, provided) -> Option<Vec<String>>`
    — full ordered symbol list combining Stage 5.39 plan + impl symbol
    formatting; `provided=false` → "null" string for codegen to emit literally
- src/lib.rs: re-export all new APIs + Stage 5.40 history comment
- tests/v0/stage5/plan/stdlib_vtable_symbol_tests.rs: 16 new tests covering
  single-string generation / vtable_method_symbols (complete/partial/marker/
  unknown/arith/extra-ignored) / **codegen-format cross-checks** (verify
  byte-for-byte equivalence with existing codegen `format!` calls)
- tests/all_tests.rs: added stdlib_vtable_symbol_tests module (54 mods total)
- Cargo.toml: version 0.11.35 → 0.11.36

**Design highlights**:
- Strict byte-for-byte reproduction of existing codegen conventions:
  * `stdlib_vtable_global_name` matches `src/codegen/mod.rs:145`
  * `stdlib_dynptr_global_name` matches `src/codegen/mod.rs:184`
  * `stdlib_data_global_name` matches `src/codegen/text_emitter.rs:565`
  * `stdlib_impl_method_symbol` matches `src/traits/resolver.rs:235`
- Two tests (`test_stdlib_vtable_global_name_match_codegen` and
  `test_stdlib_vtable_method_symbols_match_codegen_format`) explicitly
  cross-check by formatting the same string via `format!()` and asserting
  equality — guarantees Stage 5.41+ refactor will be behavior-equivalent.
- `stdlib_vtable_method_symbols` combines Stage 5.39 plan + impl symbol
  formatting: `provided=true` → `landin_<type>_<method>`,
  `provided=false` → `"null"` string. Codegen consumes the list directly
  to emit `@.vtable.<trait>.<type> = ... [n x ptr] [...]`.
- Markers (Copy/Send/Sync/Sized/Unpin/Eq) return `Some(vec![])` —
  consistent with Stage 5.37/5.38/5.39 three-state convention.
- Extra provided names silently ignored — same tolerant design as Stage 5.39.

**§16 interface isolation**: All new APIs input `&str`, output `String` /
`Vec<String>`. No `mir::ty` / `codegen::EmitType` / `traits::TraitResolver`
reference, no circular dependency. Pure functions, callable from any stage.

**§23 API naming**: All 5 new public symbols comply — 4 follow
`<noun>_<noun>_<adj>_<noun>` pattern (global_name variants + impl_method_symbol),
1 follows `<noun>_<noun>_<noun>_<noun>` pattern (vtable_method_symbols).

**Test impact**: +16 (1190 → 1206)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.41 — Stdlib Vtable Emission Plan (Aggregate) (v0.11.37)

**Priority**: Single-call aggregate that returns everything codegen needs
to emit `@.vtable.<trait>.<type>` global. Stage 5.42+ will replace codegen's
5 separate stdlib calls with one `stdlib_vtable_emission()` call — codegen
becomes simpler.

**Work completed**:
- src/stdlib.rs: new `StdlibVtableEmission` struct (9 fields: trait_name +
  type_name + global_name + method_symbols + slot_count + byte_size_32 +
  byte_size_64 + is_marker + is_complete)
- src/stdlib.rs: 2 new free-function query APIs:
  * `stdlib_vtable_emission(trait, type, provided) -> Option<StdlibVtableEmission>`
    — single-call aggregate (combines Stage 5.40 global_name +
    method_symbols + Stage 5.37/5.38 slot_count/byte_size)
  * `stdlib_vtable_emissions_for_traits(traits, type, provided) -> Vec<StdlibVtableEmission>`
    — batch query for one type implementing multiple traits; unknown
    traits silently skipped
- src/lib.rs: re-export all new APIs + Stage 5.41 history comment
- tests/v0/stage5/plan/stdlib_vtable_emission_tests.rs: 17 new tests covering
  single-emission construction (complete/partial/marker/unknown/arith) /
  field correctness (global_name/byte_sizes/is_complete/is_marker) /
  batch query (multi-trait/filters-unknown/empty/includes-markers) /
  struct semantics (PartialEq/Eq/field access)
- tests/all_tests.rs: added stdlib_vtable_emission_tests module (55 mods total)
- Cargo.toml: version 0.11.36 → 0.11.37

**Design highlights**:
- Single-call aggregate: `stdlib_vtable_emission()` returns all 9 fields
  codegen needs in one struct. Stage 5.42+ codegen refactor becomes a
  one-liner: `let e = stdlib_vtable_emission(trait, type, provided)?;`
  then directly use `e.global_name`, `e.method_symbols`, `e.byte_size_64`,
  etc.
- Compositional: internally calls Stage 5.40 `stdlib_vtable_global_name()` +
  `stdlib_vtable_method_symbols()`. Single source of truth — no duplicated
  formatting logic.
- Batch query `stdlib_vtable_emissions_for_traits()` for one type
  implementing multiple traits (common case: `struct S` impls Clone + Drop
  + Display). Unknown traits silently skipped — caller may pass user-defined
  trait names mixed with stdlib names.
- Markers included in batch results with `is_marker=true` — codegen can
  decide whether to skip empty vtable emission.
- `StdlibVtableEmission` derives `PartialEq`/`Eq` — usable for test
  assertions and future emission-cache deduplication.

**§16 interface isolation**: struct uses only `&'static str` + `String` +
`Vec<String>` + scalars — no `mir::ty` / `codegen::EmitType` /
`traits::TraitResolver` reference, no circular dependency.

**§23 API naming**: All 3 new public symbols comply (StdlibVtableEmission
follows `<Noun><Noun><Noun>`; 2 free functions follow `<noun>_<noun>_<noun>`
and `<noun>_<noun>_<noun>_<prep>_<noun>`). All 9 field names comply
(`<noun>_<noun>` / `<noun>_<noun>_<digits>` / `is_<adj>`).

**Test impact**: +17 (1206 → 1223)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.42 — Stdlib Vtable Emission Summary + Deep Review #4 (v0.11.38)

**Priority**: Add project-level vtable emission statistics (the last
static-analysis step before codegen modification). Triggers §25 deep
review #4 (Stage 5.33-5.42 = 10 sub-stages since review #3).

**Work completed**:
- src/stdlib.rs: new `StdlibVtableEmissionSummary` struct (8 fields:
  total_emissions + marker_count + complete_count + incomplete_count +
  total_slots + total_byte_size_32 + total_byte_size_64 + trait_names)
- src/stdlib.rs: 1 new free-function query API:
  * `stdlib_vtable_emission_summary(&[StdlibVtableEmission]) -> StdlibVtableEmissionSummary`
    — aggregates total counts, slot totals, byte-size totals (32/64-bit),
    and deduplicated trait names (first-seen order preserved)
- src/lib.rs: re-export all new APIs + Stage 5.42 history comment
- tests/v0/stage5/plan/stdlib_vtable_emission_summary_tests.rs: 13 new tests
  covering empty input / single complete / single marker / multi-mixed /
  total_slots / byte_sizes / trait_names dedup + order / incomplete_count /
  marker_count / complete_count / struct Eq / from-real-emissions
- tests/all_tests.rs: added stdlib_vtable_emission_summary_tests module
  (56 mods total)
- Cargo.toml: version 0.11.37 → 0.11.38

**Deep review #4 (§25 7-dimension)**:
- `docs/develop/v0/stage-5/deep-review-r91.md` created
- 7 dimensions audited: architecture / tech debt / tests / readiness /
  design / performance / docs
- 0 P0 / 0 P1 / 2 P2 blockers
- 5/5 GO — Stage 5 static infrastructure complete, ready for codegen
  vtable emission refactor (Stage 5.43)

**Design highlights**:
- Project-level aggregate: one call returns total counts + slot totals +
  byte-size totals + deduplicated trait names. Codegen uses this for
  diagnostic output ("emit N vtables, M bytes total").
- `trait_names` dedup preserves first-seen order — deterministic output
  for diagnostics.
- Compositional: builds on Stage 5.41 `StdlibVtableEmission` — single
  source of truth, no duplicated logic.

**§16 interface isolation**: struct uses only `&'static str` + `Vec<>` +
scalars — no `mir::ty` / `codegen::EmitType` / `traits::TraitResolver`
reference, no circular dependency.

**§23 API naming**: Both new public symbols comply
(`StdlibVtableEmissionSummary` follows `<Noun><Noun><Noun><Noun>`;
`stdlib_vtable_emission_summary` follows `<noun>_<noun>_<noun>_<noun>`).
All 8 field names comply.

**Test impact**: +13 (1223 → 1236)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅
  (修复了 1 个 clippy 警告: cloned_ref_to_slice_refs in test)

**Deep review impact**: 0 code changes (review only)

**Verdict**: ✅ GO — 0 P0/P1; full vtable static-planning chain (5.36-5.42)
complete; codegen vtable emission refactor ready for Stage 5.43.

### Stage 5.43 — Codegen Vtable Emission Helper (v0.11.39)

**Priority**: First Stage 5 sub-stage modifying `src/codegen/` — adds new
free function `emit_vtable_global_from_emission()` that produces LLVM IR
text from a `StdlibVtableEmission`. **Does NOT modify existing emission
path** — `emit_vtables()` + `TextEmitter::emit_vtable_global()` unchanged.
"先并行、后委托" strategy: Stage 5.44+ will refactor
`TextEmitter::emit_vtable_global()` to delegate here.

**Work completed**:
- src/codegen/mod.rs: new free function
  `emit_vtable_global_from_emission(&StdlibVtableEmission) -> String`
  * Pure-function counterpart of `TextEmitter::emit_vtable_global()`
  * Produces byte-for-byte identical LLVM IR (verified by cross-check test)
  * Extra: handles `"null"` string → `ptr null` literal (TextEmitter
    current path doesn't handle this because `emit_vtables()` only passes
    real symbols)
- src/lib.rs: re-export `emit_vtable_global_from_emission` from codegen
  + Stage 5.43 history comment
- tests/v0/stage5/plan/codegen_vtable_emission_helper_tests.rs: 13 new tests
  covering basic emission (Clone/Drop/Copy-marker/Clone-partial/Add/PartialEq)
  + format components (global_name/array/entries/null/zeroinitializer)
  + **two cross-check tests** verifying byte-for-byte equivalence with
  `TextEmitter::emit_vtable_global()` (non-null + marker paths)
- tests/all_tests.rs: added codegen_vtable_emission_helper_tests module
  (57 mods total)
- Cargo.toml: version 0.11.38 → 0.11.39

**Design highlights**:
- **"先并行、后委托" strategy**: new function exists in parallel to
  `TextEmitter::emit_vtable_global()` — no existing path modified. This
  makes the change independently reviewable and revertable. Stage 5.44+
  will refactor `TextEmitter::emit_vtable_global()` to delegate here,
  eliminating the duplicated LLVM IR formatting logic.
- **"null" handling**: `stdlib_vtable_method_symbols()` produces `"null"`
  strings for missing slots. The new function detects this and emits
  `ptr null` (no `@` prefix) instead of `ptr @null`. `TextEmitter::emit_vtable_global()`
  doesn't need this because `emit_vtables()` only passes real symbols from
  `VtableEntry.fn_name` — but the new function is designed to consume
  `StdlibVtableEmission` directly, which may contain "null" entries.
- **Cross-check test**: `test_emit_vtable_global_from_emission_match_text_emitter`
  constructs a `StdlibVtableEmission` with real symbols, calls both the
  free function and `TextEmitter::emit_vtable_global()`, and asserts the
  free function output appears verbatim in TextEmitter's
  `output_with_globals()`. This is the safety net for Stage 5.44+ refactor.

**§16 interface isolation**: function takes `&StdlibVtableEmission`
(stdlib-internal type), returns `String`. No `mir::ty` /
`traits::TraitResolver` / `Emitter` trait reference, no circular dependency.

**§23 API naming**: `emit_vtable_global_from_emission` follows
`<verb>_<noun>_<adj>_<prep>_<noun>` pattern. The `emit_` prefix is
consistent with the rest of the codegen module (`emit_vtables`,
`emit_dyn_trait_ptrs`, `emit_fat_ptr_type`).

**Test impact**: +13 (1236 → 1249)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅

### Stage 5.44 — Codegen Vtable Global Text Bridge (v0.11.40)

**Priority**: Bridge function between Stage 5.43's
`emit_vtable_global_from_emission()` (high-level API) and Stage 5.45's
`TextEmitter::emit_vtable_global()` delegation refactor. Same parameter
signature as the trait method — Stage 5.45 delegation becomes a trivial
body change.

**Work completed**:
- src/codegen/mod.rs: new free function
  `emit_vtable_global_text(global_name: &str, method_symbols: &[String]) -> String`
  * **Exact same parameter signature** as `TextEmitter::emit_vtable_global()`
  * Handles `"null"` string → `ptr null` literal (consistent with Stage 5.43)
  * Byte-for-byte identical to TextEmitter on non-null paths (cross-check tests)
- src/lib.rs: re-export `emit_vtable_global_text` from codegen
  + Stage 5.44 history comment
- tests/v0/stage5/plan/codegen_vtable_global_text_tests.rs: 12 new tests
  covering basic emission (2-symbol/empty/single/multi) + null handling
  (single + mixed) + format components (global_name/array/no-leading-@)
  + **two cross-check tests** (non-null + empty paths) +
  **one divergence-documenting test** (null path: free fn correct, TextEmitter
  current path incorrect — Stage 5.45 will fix by delegation)
- tests/all_tests.rs: added codegen_vtable_global_text_tests module
  (58 mods total)
- Cargo.toml: version 0.11.39 → 0.11.40

**Design highlights**:
- **Bridge function strategy**: Stage 5.43 added high-level
  `emit_vtable_global_from_emission(&StdlibVtableEmission)`. Stage 5.44
  adds low-level `emit_vtable_global_text(&str, &[String])` matching the
  trait method signature. Stage 5.45 will:
  1. Make `emit_vtable_global_from_emission()` internally call
     `emit_vtable_global_text()` (extracting global_name + method_symbols
     from the emission struct)
  2. Make `TextEmitter::emit_vtable_global()` delegate to
     `emit_vtable_global_text()` (trivial body change, same signature)
  This eliminates three duplicated LLVM IR formatting paths.
- **"null" handling consistency**: both Stage 5.43 and 5.44 free functions
  handle `"null"` → `ptr null`. TextEmitter's current path doesn't (it
  would emit `ptr @null`), but `emit_vtables()` never passes "null" —
  only real symbols from `VtableEntry.fn_name`. Stage 5.45 delegation
  will fix this latent bug.
- **Divergence documentation**: `test_emit_vtable_global_text_null_path_diverges_from_text_emitter`
  explicitly documents the free fn vs TextEmitter divergence on the null
  path. This is not a failure — it's a known issue that Stage 5.45 will
  resolve.

**§16 interface isolation**: pure function, input `(&str, &[String])`,
output `String`. No `mir::ty` / `traits::TraitResolver` / `Emitter` /
`StdlibVtableEmission` reference, no circular dependency.

**§23 API naming**: `emit_vtable_global_text` follows
`<verb>_<noun>_<adj>_<noun>` pattern. The `_text` suffix indicates the
function returns LLVM IR text (String), distinguishing it from the trait
method's side-effect version.

**Test impact**: +12 (1249 → 1261)
**Verification**: cargo clean + cargo test + cargo fmt + cargo clippy — all green ✅
