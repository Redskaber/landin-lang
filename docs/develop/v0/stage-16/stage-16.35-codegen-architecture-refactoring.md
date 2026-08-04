# Stage 16.35 — Codegen Architecture Refactoring: Abstract Codegen, Organize LLVM/Text

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.231.0 → v0.232.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5, 6 + §23 rule 5 (DRY)

## 1. Executive Summary

Stage 16.35 is a **systemic codegen architecture refactoring** — the first
major refactoring of the codegen module since Stage 6.7-6.8. The goal is
to truly abstract the codegen pipeline and properly organize the LLVM and
text backends.

**Key achievements**:
1. **Fixed compile bug** — `codegen_synthesized_closure_functions` was incorrectly `#[cfg]`-gated, breaking text-only builds
2. **Moved text-backend utilities** — `emit_type_to_llvm_str`, `binop_to_llvm_str` moved from shared `emitter.rs` to `text/mod.rs`
3. **Removed dead code** — `emit_dyn_trait_ptr_type`, `llvm_ptr_str`, `to_context`, `predeclare_function`
4. **Created docs/graph/** — standardized pipeline data flow diagrams
5. **Clean API surface** — no dead code, no `#[allow(dead_code)]` (except deprecated trait method)

**Test results**: 7792 tests passing (244 lib + 2324 integration + 5224
conformance), 0 failures, 0 warnings. No behavior change.

## 2. Architecture Analysis (Pre-Refactoring)

### 2.1 Issues Found

| Issue | Severity | Description |
|-------|----------|-------------|
| Compile bug | P0 | `codegen_synthesized_closure_functions` was `#[cfg(feature = "llvm-backend")]`-gated but called from ungated `codegen_crate` |
| Misplaced utilities | P1 | `emit_type_to_llvm_str`, `binop_to_llvm_str` in shared `emitter.rs` but only used by `TextEmitter` |
| Dead code | P2 | `emit_dyn_trait_ptr_type`, `llvm_ptr_str`, `to_context`, `predeclare_function` |
| Trait bloat | P3 | `Emitter` trait has 40 methods mixing 4 concerns (future refactoring target) |
| `EmitValue = String` | P3 | Leaks text-IR assumptions into LLVM backend (future refactoring target) |

### 2.2 Root Cause

The codegen module grew organically from Stage 3 through Stage 16. The
`Emitter` trait was originally designed for `TextEmitter` only (Stage 3.57).
When `LLVMSysEmitter` was added (Stage 13.5), it inherited the `String`-based
API, forcing it to maintain a `HashMap<String, LLVMValueRef>` and parse
strings back into `LLVMValueRef`s.

The text-backend utilities (`emit_type_to_llvm_str`, `binop_to_llvm_str`)
were placed in `emitter.rs` because that was the only emitter at the time.
They should have moved to `text/mod.rs` when `LLVMSysEmitter` was added.

## 3. The 通解 Fix

### 3.1 Fix Compile Bug (Priority 1)

Removed `#[cfg(feature = "llvm-backend")]` from `codegen_synthesized_closure_functions`.
The function is fully backend-agnostic (operates on `&mut dyn Emitter`), so
it must be available for the text-only build.

### 3.2 Move Text-Backend Utilities (Priority 5)

Moved from `emitter.rs` to `text/mod.rs`:
- `emit_type_to_llvm_str` — renders `EmitType` as LLVM IR type string
- `binop_to_llvm_str` — renders `BinOp` as LLVM IR instruction string

These are only used by `TextEmitter`. The LLVM C-API backend uses its own
`llvm_type()` method (returns `LLVMTypeRef`) and `LLVMBuildAdd` etc. directly.

### 3.3 Remove Dead Code (Priority 5)

| Symbol | Location | Status |
|--------|----------|--------|
| `emit_dyn_trait_ptr_type` | `emitter.rs` | Removed (never called) |
| `llvm_ptr_str` | `emitter.rs` | Removed (never called) |
| `to_context` | `llvm/mod.rs` | Removed (never called) |
| `predeclare_function` | `llvm/mod.rs` | Removed (never called, `#[allow(dead_code)]`) |

### 3.4 Create docs/graph/ (New)

Created standardized pipeline data flow diagrams:
- `docs/graph/README.md` — Index
- `docs/graph/codegen/architecture.md` — Codegen module architecture
- `docs/graph/pipeline/overview.md` — End-to-end compiler pipeline
- `docs/graph/closure/data-flow.md` — Closure data flow (HIR → MIR → Codegen)

## 4. Architecture (Post-Refactoring)

```
src/codegen/
├── mod.rs           — Entry points + shared pipeline (backend-agnostic)
├── emitter.rs       — Emitter trait + shared type helpers (emit_fat_ptr_type, mir_type_to_emit_type)
├── mir_translation.rs — MIR → EmitType translation (shared, pure data)
├── operand.rs       — Operand codegen (shared)
├── rvalue.rs        — Rvalue codegen (shared)
├── statement.rs     — Statement codegen (shared)
├── terminator.rs    — Terminator codegen (shared)
├── text/
│   └── mod.rs       — TextEmitter + text-backend utilities (emit_type_to_llvm_str, binop_to_llvm_str)
├── llvm/
│   └── mod.rs       — LLVMSysEmitter (LLVM C-API, own type rendering)
└── trait_dispatch/  — Vtable + dynptr orchestrators
```

### Design Principles Applied

- **§1.0 原則 5 "去除兼容思维"**: Dead code removed, no `#[allow(dead_code)]`
- **§1.0 原則 6 "通用 > 特例"**: Each backend owns its own rendering logic
- **§23 rule 5 (DRY)**: No duplicate type-rendering logic in shared module
- **§16**: Codegen reads MIR data (no HIR access); backend-specific code isolated

## 5. Future Refactoring Targets (Deferred)

### 5.1 Split Emitter Trait (Priority 2 — Future)

The 40-method `Emitter` trait mixes 4 concerns:
- Module-level (header, declares, globals)
- Function-scoped (instructions, control flow)
- Local state (set_local, get_local)
- Output (emit_output — deprecated)

Split into `ModuleEmitter` + `FunctionEmitter` mirroring LLVM's
`ModuleRef` vs `BuilderRef`.

### 5.2 Replace `EmitValue = String` (Priority 3 — Future)

The `EmitValue = String` choice forces `LLVMSysEmitter` to maintain
`HashMap<String, LLVMValueRef>` + `interpret_adhoc` string parser.
Replace with opaque associated type (`type Value: Clone`).

### 5.3 Unify Codegen Pipeline (Priority 4 — Future)

Text and LLVM backends have inverted emission orders (text emits globals
last, LLVM emits globals first). Unify by adopting LLVM ordering for both.

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2324/2324 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7792 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.231.0 → v0.232.0 (minor bump — API surface change: removed `emit_dyn_trait_ptr_type`
from public re-exports, moved `emit_type_to_llvm_str`/`binop_to_llvm_str` to
`text/mod.rs`. No behavior change, but downstream code referencing the removed
functions would break.)

## 8. References

- Codegen architecture diagram: `docs/graph/codegen/architecture.md`
- Pipeline overview: `docs/graph/pipeline/overview.md`
- Closure data flow: `docs/graph/closure/data-flow.md`
- Stage committee process: `docs/stage-committee-process.md` §1.0 原則 5, 6, §23
