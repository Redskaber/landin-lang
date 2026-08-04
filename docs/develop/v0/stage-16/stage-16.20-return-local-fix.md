# Stage 16.20 — Task 10 Step 3+4: Return Local Fix + Codegen Analysis

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.6 → v0.228.7
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.20 fixed a critical bug in `build_synthesized_closure_mir_body()`:
LocalId(0) was incorrectly assigned the closure struct type (self) instead
of the return type. The fix ensures LocalId(0) is the return local,
LocalId(1) is self, LocalId(2+) are closure params — matching the
convention used by regular function MIR bodies.

**Key changes**:
1. Fixed `build_synthesized_closure_mir_body()` — explicitly create
   LocalId(0) as return local before creating self/params.
2. Attempted switch — codegen still fails (Closure struct passed as `{}`
   instead of pointer).
3. **Reverted** to inline path.
4. Return local fix is a **permanent improvement** — correct MIR structure.

**No behavior change** — all 7709 tests pass with the inline path.

## 2. The Bug

### Before Fix
```
MirBody::new() creates empty local_decls.
First new_local(closure_struct_ty) → LocalId(0) = Closure type (WRONG!)
```

LocalId(0) should be the return local, but it was the self parameter.

### After Fix
```
LocalId(0) = Infer(TyVar) — return local (correct)
LocalId(1) = Closure(def_id, substs) — self parameter
LocalId(2) = Infer(TyVar) — closure param x
LocalId(3) = Infer(IntVar) — body temp
```

## 3. Remaining Codegen Issue

Even with the correct MIR structure, codegen produces incorrect LLVM IR:
- `self` parameter (LocalId(1), type `Closure(_, [])`) is emitted as `{}`
  (empty struct) because a closure with no captures has `substs = []`.
- The call site passes `{} 0` instead of a pointer to the closure struct.
- The synthesized function signature is `define i32 @closure_call_fn_0({} %arg0, i32 %arg1)`
  instead of `define i32 @closure_call_fn_0(ptr %self, i32 %x)`.

**Root cause**: Codegen passes Closure-typed values by value (as structs),
but for function parameters, they should be passed by pointer (reference).

**Fix needed**: Codegen should emit `OpaquePtr` for Closure-typed function
parameters, and the call site should pass the closure struct's address.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2241/2241 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7709 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.228.6 → v0.228.7 (patch bump — return local fix, no behavior change.)
