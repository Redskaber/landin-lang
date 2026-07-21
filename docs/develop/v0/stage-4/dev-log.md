# Stage 4 Development Log

> **Author**: redskaber
> **Date**: 2026-07-22
> **Version**: v0.9.1
> **Status**: 🔄 In progress (4.1-4.4 complete, 4.5+ pending)
> **Test count**: 989 total (Stage 4 added 5 tests: 3 nested modules + 2 closure lowering)

## Overview

Stage 4 focuses on: nested module support, visibility enforcement, closure
codegen (L3), macro system + attributes, and PHI optimization (L1 — closed
as design decision). This stage was launched after the Stage 3.69 deep
review (GO-WITH-CONDITIONS) identified the priority list.

## Sub-stages

### Stage 4.1 — Nested Module Support (v0.9.0)

**Priority**: From deep review D4 (next-stage readiness) — unblocks
visibility enforcement.

**Work completed**:
- `build_module_tree` refactored to recursively process inline modules
- New `collect_item_registration` helper handles each item kind, extracts
  `def_kinds` + `def_visibility` + registrations + use_decls + nested_children
- New `build_child_module` recursively builds child `ModuleNode` for
  `HirModKind::Inline(items)` — handles arbitrarily deep nesting
- New `item_def_id` helper extracts `DefId` from any `HirItem` variant
  via `hir_id.owner`
- `ModuleNode.children` is now populated for inline modules
- Previously: all items registered at crate root (ModuleNode.children
  never filled)
- Now: `mod foo { pub fn bar() {} }` registers `bar` in child ModuleNode
  under "foo"

**New tests** (3):
- `nested_module_items_resolve` — `mod inner { pub fn f() {} }` + `inner::f()`
- `nested_module_struct_resolves` — struct inside module
- `deeply_nested_module_resolves` — 2-level nesting (`a::b::deep_fn`)

**What this unblocks**:
- Visibility enforcement (TD-004) — `check_visibility` can now use
  `current_module` context
- Future `use` resolution improvements — glob imports can pull from
  child modules
- Path resolution — `mod::item` paths can walk into child modules

### Stage 4.2 — L1 PHI Optimization: Design Decision (v0.9.0)

**Priority**: From deep review D2 (tech debt) — L1 was listed as "PHI
node optimization".

**Analysis**: L1 is **not a limitation** — it's the **standard design**
used by Clang, rustc, and most LLVM frontends. The `alloca`-based IR is
correct and produces optimal code after `opt -mem2reg` or `lli`.

**Decision**: L1 is **CLOSED** as a design decision. Documentation in
`src/codegen/mod.rs` explicitly explains:
1. `mem2reg` is a well-tested LLVM pass that produces optimal SSA form
2. Implementing PHI emission manually would duplicate `mem2reg` logic
3. The `alloca`-based IR is correct — valid LLVM IR that any toolchain optimizes
4. The IR quality concern is non-blocking — `opt -mem2reg` produces optimal code

**What was considered and rejected**: Emitting PHI nodes directly in
`codegen_function` by tracking SSA values per basic block. This would
require per-block value mapping, PHI insertion at joins, dominance frontier
computation, and handling of partially-defined variables — essentially
reimplementing `mem2reg` in Rust (high effort, high risk, low benefit).

**L1 status**: ✅ CLOSED (design decision)

### Stage 4.3 — Visibility Enforcement Activation (v0.9.1)

**Priority**: From deep review D4 — quick win on top of Stage 4.1 nested
modules.

**Work completed**:
- `check_visibility` implemented (was stub in Stage 3.68)
- `Visibility::Public` → always visible ✅
- `Visibility::Private` → visible from crate root (same crate) ✅
  (cross-module private enforcement deferred — needs `current_module` tracking)
- `Visibility::PubRestricted(_)` → visible within the crate ✅
  (full `pub(crate)`/`pub(super)` discrimination deferred)
- Visibility checked at every `Res::Def` resolution (value + type namespaces)

**What this means**: visibility is now collected and checked at every
`Res::Def` resolution. Currently all same-crate access is allowed (since
there's no `current_module` tracking yet), but the infrastructure is fully
in place — once module context tracking is added, full enforcement activates
automatically.

### Stage 4.4 — L3 Closure Lowering (v0.9.1)

**Priority**: From deep review D4 — high user value.

**Work completed**:
- `HirExprKind::Closure` now creates a proper closure value via
  `AggregateKind::Closure(def_id, substs)`
- The closure type is `TyKind::Closure(def_id, substs)`
- Codegen: `TyKind::Closure` → `EmitType::Struct(vec![])` (empty struct
  for now — captures deferred to Stage 4.5)
- The closure body is still lowered (for type inference), and a closure
  value is assigned to a new local

**What this enables**: Closure expressions now produce proper MIR with
closure-typed values. The closure type flows through typeck and codegen.
When capture analysis is added (Stage 4.5), the empty struct will be
populated with captured environment fields.

**New tests** (2):
- `closure_lowers_to_aggregate` — verifies `|x: i32| x + 1` produces
  `AggregateKind::Closure` in MIR
- `closure_no_crash_on_complex_body` — closure with if-expression body

**Limitations** (deferred to Stage 4.5):
- Capture analysis: no variables captured yet (empty environment)
- Closure call lowering: closure calls still go through regular `Call`
- Closure type inference: return type inferred from body

## Key Design Decisions

### 1. L1 PHI as Design Decision (Stage 4.2)
- Rely on LLVM `mem2reg` rather than emitting PHI directly
- Standard approach used by Clang, rustc, and most LLVM frontends
- Implementing PHI manually would duplicate `mem2reg` (high effort, high risk)
- `alloca`-based IR is correct and produces optimal code after optimization

### 2. Visibility Pragmatic Enforcement (Stage 4.3)
- Same-crate access allowed (no `current_module` tracking yet)
- Infrastructure fully in place — full enforcement activates when module
  context tracking is added
- Matches Stage 1.3 design where module tree was flat

### 3. Closure Empty-Environment Approach (Stage 4.4)
- Closure type + AggregateKind::Closure created immediately
- Empty struct for captures (deferred to Stage 4.5)
- Allows closure expressions to produce valid MIR + codegen now
- Capture analysis can be added incrementally without breaking existing code

## Test Summary

| Test file | Count | Scope |
|-----------|-------|-------|
| `tests/hir_resolution.rs` | +3 | Nested module resolution (Stage 4.1) |
| `tests/mir_lowering.rs` | +2 | Closure lowering (Stage 4.4) |
| **Stage 4 total** | **+5** | |

## Next Stage 4 Priorities (from deep review)

1. **L3 capture analysis** (Stage 4.5) — analyze which variables a closure
   captures; populate the closure struct with captured environment fields
2. **Macro system + attributes** — `Expr::MacroCall` expansion
3. **Performance benchmark suite** — add `benches/` + criterion (QA condition
   from deep review)
4. **Closure call lowering** — closure calls should go through a closure-
   specific call mechanism (not regular `Call`)
5. **Full visibility enforcement** — add `current_module` tracking to
   activate cross-module private access checks

## Verification (cumulative through Stage 4.4)

- `cargo test`: **989 passed, 0 failed, 2 ignored**
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

---

**Last updated**: 2026-07-22 (Stage 4.4)
**Process version**: v3.16
