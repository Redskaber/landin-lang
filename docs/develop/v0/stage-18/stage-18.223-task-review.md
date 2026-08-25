# Stage 18.223 — Task Review: TD-C-WRAPPER-OVERUSE Dependency Audit + MIR Intrinsic Ops Design Plan

> **Date**: 2026-08-23
> **Version**: v0.475.0 (no bump — audit)
> **Task ID**: stage18.223
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A

## 1. 触发场景

Per Stage 18.219 v0.2 Phase 2 task re-plan → v0.2.5: TD-C-WRAPPER-OVERUSE.
Per §17.7 (任务审查): before starting a large architectural task, audit dependencies and timing.

## 2. TD-C-WRAPPER-OVERUSE Description

**Current state**: 4 compound C helpers (`__landin_vec_push`, `__landin_vec_get`,
`__landin_string_push_str`, `__landin_format_variadic`) bypass MIR-level intrinsic
expansion by calling C runtime functions directly from codegen.

**Migration plan** (from Stage 18.203 audit):
1. v0.2: Add MIR-level intrinsic ops (Alloc, Copy, BinOp, Branch)
2. v0.3: Replace C helpers with MIR intrinsics for stage-1 self-hosting

## 3. 依赖与基础设施完整能力审查 (per user directive)

### 3.1 Current MIR Rvalue Variants

```rust
pub enum Rvalue {
    Use(Operand),           // copy/move
    BinaryOp(BinOp, ...),   // arithmetic
    UnaryOp(UnOp, ...),     // negation/not
    Ref(Region, BorrowKind, Place),  // borrow
    Cast(CastKind, ...),    // type cast
    Aggregate(AggregateKind, Vec<Operand>),  // struct/tuple/array construction
    BinaryOp2(RangeOp, ...), // range expressions
}
```

### 3.2 What's Missing for MIR Intrinsic Ops

| Required MIR Op | Status | Description |
|-----------------|--------|-------------|
| `Rvalue::Load(ptr) -> val` | ❌ Missing | Load value from raw pointer |
| `Rvalue::Store(ptr, val)` | ❌ Missing | Store value to raw pointer |
| `Rvalue::Alloc(size) -> ptr` | ❌ Missing | Heap allocation (currently via `__landin_alloc` C call) |
| `TerminatorKind::Branch(cond, then, else)` | ✅ Exists (via `SwitchInt`) | Conditional branch |
| `Rvalue::GetElementPtr(base, indices)` | ❌ Missing (via `emit_gep_field` in codegen) | Pointer arithmetic |

### 3.3 Dependencies

| Dependency | Status |
|-----------|--------|
| MIR Rvalue enum | ✅ Exists (extensible) |
| Codegen infrastructure | ✅ Exists (LLVMSysEmitter) |
| `compute_type_size` | ✅ Stage 18.203 |
| `extract_vec_element_type` | ✅ Stage 18.208 |
| `build_adt_layout` with generics | ✅ Stage 18.212 |
| AdtLayouts (crate-level shared) | ✅ Stage 15.8 |
| Design document for MIR intrinsic ops | ❌ Missing |
| Test infrastructure for MIR ops | ❌ Missing |

### 3.4 Migration Complexity Assessment

| C Helper | Migration Complexity | Estimated LOC |
|----------|---------------------|---------------|
| `__landin_vec_push` | Medium — needs Load+Store+BinOp+Branch (growth logic) | ~80 MIR ops |
| `__landin_vec_get` | Low — needs Load+BinOp (bounds check) + Load (element) | ~30 MIR ops |
| `__landin_string_push_str` | Medium — needs Load+Store+BinOp (growth+copy) | ~60 MIR ops |
| `__landin_format_variadic` | High — needs va_list equivalent in MIR | ~150+ MIR ops |

## 4. 任务审查结论 (per §17.8)

### 4.1 Is this the best time?

**No** — TD-C-WRAPPER-OVERUSE migration is a **v0.2/v0.3 architectural task**:
- Requires new MIR Rvalue variants (Load, Store, Alloc, GEP)
- Requires codegen support for new MIR ops
- Requires migrating 4 compound C helpers
- Requires design document before implementation
- format_variadic migration is especially complex (va_list semantics)

### 4.2 Should we re-plan?

**Yes** — defer to v0.2 Phase 2 design stage:

```
v0.2 Phase 2 (design):
  v0.2.5a: MIR intrinsic ops design document (docs/lang-design/06-mir.md §X)
  v0.2.5b: Add MIR Rvalue::Load, Rvalue::Store, Rvalue::Alloc
  v0.2.5c: Add codegen support for new MIR ops

v0.2 Phase 2 (implementation):
  v0.2.5d: Migrate __landin_vec_get → MIR intrinsic (simplest)
  v0.2.5e: Migrate __landin_vec_push → MIR intrinsic
  v0.2.5f: Migrate __landin_string_push_str → MIR intrinsic
  v0.2.5g: Migrate __landin_format_variadic → MIR intrinsic (most complex)

v0.3 (self-hosting):
  v0.3.x: Remove compound C helpers from runtime.rs
```

### 4.3 Current v0.1 status

**v0.1 is complete and stable**:
- 8/11 TDs resolved (TD-FUNCTION-REDEFINE-PARAMS, TD-VEC-GET-TYPE-INFERENCE,
  TD-TUPLE-CTOR-TYPECK, TD-BOX-AUTO-DROP, TD-TUPLE-FIELD-CHECK,
  TD-INT-UINT-VAR, TD-GENERIC-PARAM-CHECK, TD-VEC-PUSH-SHARED-BORROW)
- 3772 tests, 0 failures
- Full validation flow (LLVM 22.1)
- All v0.1 features working (Box, Vec, String, format!, typeck)

**Remaining TDs**:
- TD-METHOD-RESOLVE-STRICT (partial — needs resolver tracking through typeck defaulting)
- TD-DROP-MOVED-LOCALS (v0.3+ — move tracking in drop elaboration)
- TD-C-WRAPPER-OVERUSE (v0.2/v0.3 — MIR intrinsic ops design + migration)

## 5. Recommendation

**Defer TD-C-WRAPPER-OVERUSE to v0.2 Phase 2 design stage.**

Current v0.1 is complete with 8/11 TDs resolved. The remaining 3 TDs all require
v0.2+ infrastructure:
- TD-C-WRAPPER-OVERUSE: MIR intrinsic ops design (v0.2.5a-g)
- TD-DROP-MOVED-LOCALS: Move tracking (v0.3+)
- TD-METHOD-RESOLVE-STRICT: Resolver tracking (v0.2.3, needs design)

**Next action**: Write v0.2 Phase 2 design document for MIR intrinsic ops,
then begin implementation. This is a design-first task (per §13.1 设计对齐).

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE is recorded with full migration plan
in Stage 18.203 audit doc. No information is lost.

Per §17.8 (审查结论): NEEDS REVISION — task is deferred to v0.2 design stage.
