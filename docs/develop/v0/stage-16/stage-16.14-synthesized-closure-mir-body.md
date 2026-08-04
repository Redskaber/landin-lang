# Stage 16.14 — Task 10 Step 2: Synthesized Closure MIR Body Synthesis

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.0 → v0.228.1
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §13.4 (数据结构选型) + §16 (接口隔离) + §23 API 命名标准化

## 1. Executive Summary

Stage 16.14 is **Step 2 of Task 10** (Closure Redesign). It builds
independent MIR bodies for each synthesized closure `call` function,
stored in `CompileResult.synthesized_closure_mir_bodies`.

**Key changes**:
1. Added `synthesized_closure_mir_bodies: Vec<MirBody>` field to `CompileResult`.
2. Added `build_synthesized_closure_mir_body()` function to `mir::lower`.
3. Updated driver to build MIR bodies for each synthesized closure function.
4. Updated `CompileResult::empty()` to include the new field.
5. +8 integration tests.

**No behavior change** — the inline approach (Stage 13.3a) is still used
for closure calls. The synthesized MIR bodies are built but NOT yet used
by codegen (Step 4) or call sites (Step 3).

## 2. Implementation

### 2.1 `build_synthesized_closure_mir_body()` Function

```rust
pub fn build_synthesized_closure_mir_body(
    func: &SynthesizedClosureFunction,
    interner: &Rodeo,
    hir: &HirCrate,
) -> MirBody
```

This function builds a MirBody representing the closure's `call` function:
1. Creates a fresh `MirLowerCtxt`.
2. Sets up `LocalId(0)` as the return local (fresh Infer type).
3. Sets up `LocalId(1)` as `self` (the closure struct type).
4. Sets up `LocalId(2)`, `(3)`, ... as closure parameters.
5. Extracts captures from `self` via field projections.
6. Lowers the closure body expression.
7. Assigns the body result to the return local.
8. Terminates with `Return`.

### 2.2 Capture Extraction

For each capture `(hir_id, field_idx, cap_ty)`:
```rust
let extract_local = cx.mir.new_local(cap_ty, ...);
cx.push_assign(
    Place::local(extract_local),
    Rvalue::Use(Operand::Copy(Place::Projection(
        Place::local(self_local),
        ProjectionElem::Field(FieldId(field_idx), cap_ty),
    ))),
);
cx.local_map.insert(cap_hir_id, extract_local);
```

This maps the captured binding's HirId to the extract local, so when the
closure body references the captured variable, it resolves to the
extracted value from `self`.

### 2.3 Driver Integration

The driver iterates over `synthesized_closures` (returned from MIR
lowering) and builds a MirBody for each:

```rust
for func in synthesized_closures.values() {
    let closure_mir = build_synthesized_closure_mir_body(func, &interner, &hir);
    synthesized_closure_mir_bodies.push(closure_mir);
}
```

The `synthesized_closure_mir_bodies` Vec is stored in `CompileResult`.

## 3. API Naming Standard Compliance (§23)

| Item | Pattern | Status |
|------|---------|--------|
| `synthesized_closure_mir_bodies` | `<adj>_<noun>_<noun>_<noun>` | ✅ |
| `build_synthesized_closure_mir_body` | `<verb>_<adj>_<noun>_<noun>_<noun>` | ✅ |

## 4. §16 Interface Isolation Compliance

- `build_synthesized_closure_mir_body` reads HIR (closure body) — allowed during MIR lowering
- Driver calls it in the per-body loop (after main MIR lowering)
- Codegen will read the MIR bodies from `CompileResult` in Step 4 (future)
- No new HIR access from borrowck/codegen

## 5. Tests

Added `tests/v0/stage16/plan/stage16_14_synthesized_closure_mir_body_tests.rs`
with 8 tests:
1. Closure literal produces a synthesized MIR body
2. Multiple closures produce multiple MIR bodies
3. Synthesized MIR body has basic blocks
4. Synthesized MIR body has a Return terminator
5. Synthesized MIR body has local declarations (return, self, params)
6. Closure with captures has capture extraction (extra locals)
7. No closures means no synthesized bodies
8. Closures in different functions produce separate bodies

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2219/2219 PASS (+8 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7687 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.228.0 → v0.228.1 (patch bump — new field + function, no behavior change.
The `synthesized_closure_mir_bodies` field is additive; existing code
doesn't use it yet.)

## 8. Task 10 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.13) | Infrastructure: struct, side-table, DefId allocation |
| Step 2 | ✅ **COMPLETE (Stage 16.14)** | MIR body synthesis |
| Step 3 | 🔧 Pending | Call site migration (inline → `TerminatorKind::Call`) |
| Step 4 | 🔧 Pending | Codegen: emit LLVM function |
| Step 5 | 🔧 Pending | Cleanup: remove `ClosureBodyInfo`, inline path |

**Next**: Step 3 (Call site migration) — change `lower_closure_call_inline`
to emit `TerminatorKind::Call` to the synthesized function instead of
inlining the body.
