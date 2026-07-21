# Stage 1 Development Log

> **Author**: redskaber
> **Date**: 2026-07-22
> **Version**: v0.9.1 (Stage 3.63-3.68 + Stage 4.1-4.4 retroactive updates)
> **Status**: ✅ Complete
> **Test count**: 117 tests (Stage 1 scope)

## Overview

Stage 1 covers HIR (High-level Intermediate Representation) data structures,
AST→HIR lowering, and name resolution (module-level + scope-based). The
Stage 1 work was originally completed across sub-stages 1.1-1.4, then
received retroactive improvements during Stage 3.63-3.68 (cross-stage
naming standardization + P2 fixes) and Stage 4.1/4.3 (nested modules +
visibility enforcement).

## Sub-stages

### Stage 1.1 — HIR Data Structures + Deferred AST Schema Fixes

**Version**: v0.2.0 (9-round Stage Committee cycle)

**Work completed**:
- 3 AST schema fixes: `SelfKind` enum + `Param.self_kind` field;
  `BindingMode::ByValue(Mutability)`; type-position-only generic args
  heuristic + turbofish support
- HIR data structures complete (~810 lines): `HirId`/`DefId`/`ItemLocalId`/
  `OwnerId` + `HirIdMap`/`HirIdSet`/`DefIdMap`/`DefIdSet`
- 11 `HirItem` + 4 `OwnerNode` + 16 `HirTyKind` + 12 `HirPatKind` +
  28 `HirExprKind` + `HirStmt`/`HirLocal`/`HirArm`/`HirBlock`
- `Res` enum (name resolution placeholder) + `InferTy` + `InferTyCounter`
- 20 HIR structure tests + 12 inline unit tests
- Method-call turbofish support + `HirQSelf` type

**Stage Committee**: 5/0/0 APPROVED (unanimous) after 9 rounds

### Stage 1.2 — HIR Lowering (AST → HIR)

**Version**: v0.2.1

**Work completed**:
- `lower_crate` entry point + `LowerCtxt` (later renamed `HirLowerCtxt`
  in Stage 3.63 for parity with `MirLowerCtxt`)
- All 11 item kinds lowered: fn/const/static/struct/enum/trait/impl/
  type-alias/extern-block/mod/use
- Body/expr/stmt/pat/ty/path/generics/use-tree lowering
- `LowerError` type
- 36 lowering tests + 2 inline

### Stage 1.3 — Name Resolution (Module-Level)

**Version**: v0.2.2 + retroactive updates through v0.9.1

**Original work (v0.2.2)**:
- `resolve_crate` entry point + `Resolver` struct
- Module tree construction: `ModuleNode` with value_ns + type_ns + children
- Path resolution: single-segment (primitive/local/Self/value/type) +
  multi-segment
- Primitive type recognition (all 16 primitives)
- Self type resolution
- Duplicate definition detection

**Retroactive updates**:
- **Stage 3.63**: `LowerCtxt` → `HirLowerCtxt`; `DefKind` moved from
  `resolve::module_tree` to `hir::kinds` (architectural home)
- **Stage 3.64**: `use` declaration resolution implemented (was no-op stub) —
  leaf/glob/path-prefix/alias imports; `UseImport` struct + `use_imports`
  table on `ModuleNode`; `resolve_path` consults table as fallback
- **Stage 3.65**: `Res::SelfTy` now carries `HirSelfKind` (Trait/Impl);
  `unsafe impl`/`unsafe trait` `is_unsafe` field propagated from AST to HIR
- **Stage 3.66**: Resolver owner context threading — `current_self_kind`
  field set when resolving Trait/Impl item paths
- **Stage 3.67**: Body owner context threading — `resolve_all_paths` builds
  `HashMap<DefId, HirSelfKind>` and sets `current_self_kind` before each
  `resolve_body` call (body-level `Self` now accurate)
- **Stage 3.67**: `&mut Rodeo` → `&Rodeo` in `resolve_crate` (lexer now
  interns keywords at tokenization time)
- **Stage 3.68**: Visibility metadata collection — `def_visibility` map
  populated during `build_module_tree`
- **Stage 4.1**: Nested module support — `build_module_tree` recursively
  processes `HirModKind::Inline(items)`; `ModuleNode.children` populated;
  `build_child_module` + `collect_item_registration` + `item_def_id` helpers
- **Stage 4.3**: Visibility enforcement activation — `check_visibility`
  implements real checks (was stub); `pub`/`private`/`pub-restricted`
  discrimination (same-crate access; cross-module deferred)

**Tests**: 17 resolution tests + 5 use-resolution tests + 1 visibility
test + 3 nested module tests = 26 tests

### Stage 1.4 — Scope-Based Resolution (Locals)

**Version**: v0.2.3

**Work completed**:
- `Scope` / `ScopeKind` / `ScopeStack` (5 ScopeKind variants: Fn/Block/
  Closure/MatchArm/Loop; linked-list parent chain)
- `collect_pat_bindings` — handles Ident/Struct/TupleStruct/Tuple/Slice/
  Or/Ref/Path patterns
- `resolve_body` / `resolve_expr` / `resolve_block` — scope tracking for
  Fn/Block/Closure/MatchArm/Loop
- Forward reference prevention (resolve_block resolves init BEFORE
  registering binding)
- 17 scope resolution tests + 4 inline

## Key Design Decisions

### 1. HIR ID System (Stage 1.1)
- `HirId` = (owner: DefId, local_id: ItemLocalId) — mirrors rustc
- `DefId` uniquely identifies top-level items
- `OwnerId` is a typed wrapper around `DefId` for body ownership
- `HirIdMap`/`HirIdSet` use std HashMap (no fxhash dependency)

### 2. `HirParam` Duplication (accepted design choice)
- `HirFnSig.inputs: Vec<HirParam>` and `Body.params: Vec<HirParam>` both
  carry the same data (clone)
- This matches rustc's design — declaration params vs implementation params
- Stage 3.65 deep review accepted this as a design choice, not a defect

### 3. `HirSelfKind` Discrimination (Stage 3.65-3.67)
- `Res::SelfTy(HirSelfKind)` distinguishes trait-Self (abstract) from
  impl-Self (concrete)
- Named `HirSelfKind` (not `SelfKind`) to avoid collision with
  `ast::SelfKind` (method receiver kinds — different concept)
- Resolver threads owner context (`current_self_kind`) for accurate
  discrimination at both owner and body levels

### 4. `DefKind` Architectural Home (Stage 3.63)
- `DefKind` defined in `hir::kinds` (not `resolve::module_tree`)
- Rationale: `DefKind` is consumed by `Res::Def(DefId, DefKind)` — a HIR type
- Aligns dependency direction: `resolve` depends on `hir`, not vice versa
- `resolve::module_tree` imports + re-exports for backwards compatibility

### 5. Nested Module Support (Stage 4.1)
- `build_module_tree` recursively processes `HirModKind::Inline(items)`
- `ModuleNode.children` populated for inline modules
- Handles arbitrarily deep nesting (verified with 2-level test)
- Unblocks visibility enforcement + improved use resolution

## Test Summary

| Test file | Count | Scope |
|-----------|-------|-------|
| `tests/v0/stage1/plan/hir_structure_tests.rs` | 20 | HIR node construction + ID system |
| `tests/v0/stage1/plan/hir_lowering_tests.rs` | 36 | AST → HIR lowering |
| `tests/v0/stage1/plan/hir_resolution_tests.rs` | 26 | Module-level + use + visibility + nested modules |
| `tests/v0/stage1/plan/hir_scope_resolution_tests.rs` | 17 | Scope-based local resolution |
| Inline lib tests | 18 | id.rs + kinds.rs + map.rs + scope.rs |
| **Total** | **117** | |

## Known Limitations (deferred to Stage 4+)

- **Full visibility enforcement**: cross-module private access needs
  `current_module` tracking (infrastructure in place, activation deferred)
- **3+ segment use paths**: `use a::b::c::d;` not yet supported
- **Cross-crate imports**: Stage 5+
- **Prelude injection**: Stage 5 stdlib MVP

---

**Last updated**: 2026-07-22 (Stage 4.4)
**Process version**: v3.16
