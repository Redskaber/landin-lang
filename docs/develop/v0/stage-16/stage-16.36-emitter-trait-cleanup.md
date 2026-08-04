# Stage 16.36 — Emitter Trait Cleanup: Remove Dead `emit_output` + Documentation Groups

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.232.0 → v0.232.1
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维"

## 1. Executive Summary

Stage 16.36 continues the codegen architecture refactoring by removing the
dead `emit_output` method from the `Emitter` trait and reorganizing the
trait methods into clear documentation groups.

**What was removed**:
- `emit_output(&self) -> &str` from the `Emitter` trait
- `emit_output` implementation from `TextEmitter` (returned `&self.output`)
- `emit_output` implementation from `LLVMSysEmitter` (returned `""`)

**Why it was dead**:
- `TextEmitter` uses `output_with_globals()` (concrete method) for output
- `LLVMSysEmitter` uses `to_module()` / `to_object_file()` (concrete methods)
- No codegen pipeline code ever called `emit_output`

**What was added**:
- Clear documentation grouping of trait methods into:
  - Module-level (5 methods: header, declare, globals)
  - Function scope (30 methods: instructions, control flow)
  - Local state (4 methods: set/get local pointers and values)

**Test results**: 7804 tests passing, 0 failures, 0 warnings. No behavior change.

## 2. Architecture Decision

### 2.1 Why Not Split the Trait?

The original plan was to split `Emitter` into `ModuleEmitter` + `FunctionEmitter`
super-traits, mirroring LLVM's `ModuleRef` vs `BuilderRef` split. However,
Rust does not allow multiple `impl` blocks for the same trait on the same
type. The current code has module-level and function-scoped methods
interleaved within the impl blocks, so splitting would require physically
moving ~1000 lines of code — a high-risk change with no behavior benefit.

Instead, we:
1. Removed `emit_output` (dead code — the primary goal)
2. Added clear documentation groups within the trait
3. Deferred the physical trait split to a future stage that can do the
   code movement safely

Per §1.0 原則 9 "正确 > 妥协": the trait split is the correct long-term
design, but the code movement risk is too high for this stage. The
documentation groups provide the architectural clarity without the risk.

### 2.2 Future Trait Split (Deferred)

When the code movement is done, the split will be:
- `ModuleEmitter`: `emit_header`, `emit_declare`, `emit_string_global`,
  `emit_vtable_global`, `emit_dyn_trait_const`
- `FunctionEmitter`: `emit_function_begin`, `emit_function_end`, all
  instruction methods, local state methods
- `Emitter: ModuleEmitter + FunctionEmitter`: combined super-trait

This requires moving the 3 module-level global methods from their current
position (after `emit_checked_binop`) to after `emit_declare`, and moving
the 4 local state methods to after `emit_checked_binop`.

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2336/2336 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7804 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.232.0 → v0.232.1 (patch bump — removed dead `emit_output` from trait.
API surface change but no behavior change.)
