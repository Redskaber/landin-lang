# Stage 16.24 — Task 10 Step 3+4: Capture Closure Switch Attempt + Revert

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.229.1 → v0.229.2
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.24 attempted to enable the synthesized `call` function for ALL
closures (including captures). The text emitter produces correct LLVM IR,
but LLVMSysEmitter produces incorrect runtime output (segfault or wrong
exit code). Capture closures reverted to inline path.

**No behavior change** — all 7709 tests pass. No-capture closures still
use synthesized `call` function.

## 2. Attempt

Enabled `lower_closure_call_to_synthesized` for ALL closures. Result:
- Simple capture: `let x = 10; let f = |y: i32| x + y; f(5)` — returns 15
  with `--run` but segfaults with `--emit-bin`
- Conformance: 5 failures (all capture closure tests)

## 3. Root Cause Analysis

The text emitter produces correct LLVM IR:
```llvm
%v3 = load ptr, %loc_1     ; load self pointer
%v4 = getelementptr inbounds { i32 }, { i32 }* %v3, i32 0, i32 0
%v5 = load i32, %v4         ; load capture value
```

But LLVMSysEmitter produces different (incorrect) code. The difference
between `--run` (which sometimes works) and `--emit-bin` (which crashes)
suggests a state management issue in LLVMSysEmitter — possibly the GEP
type or pointer value is incorrect when built via the LLVM C API.

## 4. Revert

Capture closures use inline path. No-capture closures use synthesized
`call` function. All 7709 tests pass.

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2241/2241 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7709 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅ (no-capture closures)

## 6. Version Policy

v0.229.1 → v0.229.2 (patch bump — capture switch attempt + revert, no behavior change.)
