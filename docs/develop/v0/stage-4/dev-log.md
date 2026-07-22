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
| `tests/v0/stage1/plan/hir_resolution_tests.rs` | +3 | Nested module resolution (Stage 4.1) |
| `tests/v0/stage2/plan/mir_lowering_tests.rs` | +2 | Closure lowering (Stage 4.4) |
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

---

### Stage 4.6 — Process v3.17 + 三阶段文档协议执行 (v0.9.3)

**Priority**: User requested process doc update + standardized tests/ directory.

**Work completed**:
- Process doc `docs/stage-committee-process.md` updated v3.16 → v3.17:
  - §17 refactored: "测试目录标准化与三阶段文档协议" (was "测试矩阵全覆盖原则")
  - §17.1: standardized `tests/` directory structure (forced `tests/v0/stage-N/plan/` + `tests/v0/stage-N/gate/`)
  - §17.2: standardized `docs/tests/` directory structure
  - §17.3: three-phase documentation protocol (开发轮/审查轮/深度审查轮)
  - §17.4-§17.6: coverage requirements + migration strategy + doc format standard
  - §18 refactored: "轮次文档同步执行规则" (integrated with §17.3)
  - §27: changelog v3.16→v3.17
- Stage 4.6 三阶段文档协议执行:
  - 时期 1 (开发轮): `plan-4.md` + `tests/v0/stage4/plan/stage4_features.md`
  - 时期 2 (审查轮): `gate-review-round1.md` + `tests/v0/stage4/gate/gate-review-round1.md`
  - 目录标准化: `tests/v0/stage4/plan/` + `tests/v0/stage4/gate/` + `docs/tests/v0/stage4/plan/` + `docs/tests/v0/stage4/gate/`

**Test impact**: 0 (pure process/doc work)
**Verification**: 989 tests pass, 0 clippy warnings, fmt clean

---

### Stage 4.7 — L3 Closure Capture Analysis (v0.9.4)

**Priority**: From deep review D4 — L3 closure codegen (high user value).

**Work completed**:
- New `collect_captured_locals` function — walks closure body's `HirExpr` tree
  to find all `HirExprKind::Path` with `Res::Local(hir_id)`, filters out closure
  params, collects remaining external variable references
- New `collect_pat_hir_ids` helper — extracts all HirIds from closure parameter
  patterns (identifies which locals are params, not captures)
- New `collect_block_captured` helper — walks block statements + final expr
- Modified closure lowering:
  - Capture field types → `TyKind::Closure(def_id, capture_tys)` substs
  - Capture values → `Aggregate(Closure, capture_operands)` operands
- Modified codegen emitter:
  - `TyKind::Closure(_, substs)` → `EmitType::Struct(fields)` where fields are
    capture types (was empty struct in Stage 4.4)

**What this means**: Closures now properly "close over" their environment.
`let y = 10; let f = |x: i32| x + y;` produces a closure struct with one
field (the captured `y`), and the `Aggregate` value carries `y`'s value.

**New tests** (4) — in standardized `tests/v0/stage4/plan/` directory:
- `test_closure_no_captures` — `|x: i32| x + 1` → empty env
- `test_closure_captures_one_var` — `let y = 10; |x: i32| x + y` → 1 capture
- `test_closure_captures_multiple_vars` — 2 captures
- `test_closure_params_not_captured` — params excluded from captures

**Test impact**: +4 (993/993 tests pass — was 989)
**Verification**: 0 clippy warnings, fmt clean, §16 compliance maintained

---

### Stage 4.9 — L3 Closure Call Lowering (v0.9.6)

**Priority**: L3 closure codegen continuation.

**Work completed**:
- `src/mir/lower/mod.rs`: `Call` lowering now checks `TyKind::Closure` after
  the `TyKind::Adt` check. Closure calls produce a simplified placeholder
  (unit type local) instead of an incorrect `Terminator::Call`.
- `src/codegen/mod.rs`: L3 documentation updated to "IN PROGRESS (Stage 4.9)"
- Full closure call lowering (extract captures + invoke body) deferred to Stage 4.10

**New tests** (2):
- `tests/v0/stage4/plan/closure_call_tests.rs`
  * `test_closure_call_no_crash` — `let f = |x: i32| x; f(42);`
  * `test_closure_call_with_capture` — `let y = 10; let f = |x: i32| x + y; f(1);`

**Test impact**: +2 (995/995 tests pass — was 993)
**Verification**: 0 clippy warnings, fmt clean

---

### Stage 4.10 — Macro System (v0.9.7)

**Priority**: Macro system + attributes from deep review priority list.

**Work completed**:
- `src/mir/lower/mod.rs`: MacroCall lowering now checks macro name and expands
  built-in macros:
  * `println!`/`print!`/`eprintln!`/`eprint!` → unit expression
  * `stringify!` → `&str` typed local
  * `assert!`/`debug_assert!` → unit expression
  * Unknown macros → Error placeholder (fallback)
- Previously ALL macros produced TyKind::Error

**New tests** (3):
- `tests/v0/stage4/plan/macro_system_tests.rs`
  * test_macro_println_no_crash
  * test_macro_stringify
  * test_macro_assert_no_crash

**Test impact**: +3 (998/998 tests pass — was 995)
**Verification**: 0 clippy warnings, fmt clean

---

### Stage 4.11 — Benchmark Suite + ADR Docs (v0.9.8)

**Priority**: Closes deep review R37 GO-WITH-CONDITIONS conditions.

**Work completed**:
- `benches/compile_bench.rs` — 5 lightweight benchmarks (small/medium/closure/macros/nested_modules)
  using std::time::Instant (no external dependencies)
- `docs/develop/v0/architecture-decisions.md` — 7 ADRs:
  * ADR-001: HirParam duplication (accepted)
  * ADR-002: Emitter trait 36 methods (decompose later)
  * ADR-003: L1 PHI — rely on LLVM mem2reg (CLOSED)
  * ADR-004: Visibility — same-crate access (deferred)
  * ADR-005: Closure capture — Copy mode (deferred)
  * ADR-006: Closure call — simplified placeholder (deferred)
  * ADR-007: Built-in macro expansion — MIR lowering stage (deferred)

**R37 conditions status**: All 3 conditions CLOSED ✅

**Test impact**: +5 benchmarks (998 tests + 5 benchmarks)
**Verification**: 0 clippy warnings, fmt clean

---

### Stage 4.12 — Process v3.18 + Visibility Tracking + 1000 Tests (v0.9.9)

**Work completed**:
- Process v3.18: §18.4.0 worklog snapshot sync to `docs/worklog/`
- 5 historical worklog snapshots created (R42-R46)
- `current_module: Option<Spur>` field on Resolver (Stage 4.12)
- `check_visibility` updated to reference `current_module`
- `current_module()` public accessor for testing
- 2 new visibility tests → **1000 tests milestone** 🎉

**Test impact**: +2 (1000/1000 — was 998)
**Verification**: 0 clippy warnings, fmt clean
