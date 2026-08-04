# Stage 16.22 — Task 10 Steps 3+4: Closure Switch SUCCESS (No-Capture Closures) 🎉

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.8 → v0.229.0 (**MINOR BUMP** — behavior change: no-capture closures use synthesized `call` function)
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

**CLOSURE SWITCH SUCCEEDED!** No-capture closures now use the synthesized
`call` function (Strategy A) instead of inline lowering. This is a major
milestone for Task 10.

**Verified**: `let f = |x| x + 1; f(10)` returns `11` ✅
**All 7709 tests pass** (244 lib + 2241 integration + 5224 conformance).

**Key changes**:
1. **Empty struct alloca fix**: `{}` → `i8` (size 1, not 0) — fixes LLVM UB
2. **No-capture Closure is Copy**: `Closure(_, substs) if substs.is_empty() => true`
3. **Switch enabled for no-capture closures**: `lower_closure_call_to_synthesized`
4. **Capture closures still use inline path**: Needs GEP-from-pointer fix
5. **Operand::Copy for self**: No-capture closures are Copy → allows chained calls

## 2. The Breakthrough

### 2.1 Empty Struct Alloca Fix (emit_type_to_llvm_str + llvm_type)

**Problem**: `EmitType::Struct(vec![])` produces `{}` in LLVM (size 0).
`alloca {}` creates an invalid pointer → undefined behavior.

**Fix**: Empty structs use `i8` (size 1) instead of `{}`:
```rust
// Text emitter (emit_type_to_llvm_str):
if fields.is_empty() { "i8".into() }

// LLVM backend (llvm_type):
if fields.is_empty() { return LLVMInt8TypeInContext(self.ctx); }
```

### 2.2 No-Capture Closure is Copy (ty_is_copy_with_resolver)

**Problem**: `Closure(_, _) => false` — all closures treated as non-Copy.
Chained calls `f(f(f(0)))` fail with "use of moved value".

**Fix**: Closures with no captures (empty substs) are Copy:
```rust
Closure(_, substs) if substs.is_empty() => true,
```

### 2.3 Switch Enabled for No-Capture Closures

**Implementation**: At the call site, check if the closure has captures:
```rust
if !has_captures {
    return lower_closure_call_to_synthesized(cx, func_local, &arg_locals, expr);
} else {
    // Closures with captures: use inline path for now.
    let info = cx.closure_bodies.get(&func_local).cloned().unwrap();
    return lower_closure_call_inline(cx, info, func_local, &arg_locals, expr);
}
```

## 3. What Works

- ✅ `let f = |x| x + 1; f(10)` → returns 11
- ✅ `let f = |x: i32| x + 1; let y = f(f(f(0)));` → chained calls work
- ✅ Multiple closures in same function
- ✅ Closures in different functions
- ✅ All 5224 conformance tests pass
- ✅ All 2241 integration tests pass
- ✅ All 244 lib tests pass

## 4. What Doesn't Work Yet (Capture Closures)

- 🔧 `let x = 10; let f = |y| x + y; f(5)` → capture extraction needs GEP-from-pointer
- 🔧 Nested closures with captures
- 🔧 Capture chain closures

**Root cause**: The synthesized function body extracts captures via
`Projection(self_local, Field(i, cap_ty))`, which generates field access
on the struct value. But `self` is now a pointer (OpaquePtr), so the
codegen needs to emit GEP from the pointer, not direct field access.

**Fix plan**: In `build_synthesized_closure_mir_body`, change capture
extraction to use `Deref` projection before `Field` projection, so
codegen emits `GEP` from the `self` pointer.

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2241/2241 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7709 tests passing, 0 failures, 0 warnings.**
- **Runtime verified**: `f(10)` returns 11 ✅

## 6. Version Policy

v0.228.8 → v0.229.0 (**minor bump** — behavior change: no-capture closures
now use synthesized `call` function instead of inline lowering. This is a
significant architectural change, even though all tests pass.)

## 7. Task 10 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (16.13) | Infrastructure |
| Step 2 | ✅ COMPLETE (16.14) | MIR body synthesis |
| Step 3 | ✅ **COMPLETE for no-capture** (16.22) | Call site migration |
| Step 4 | ✅ **COMPLETE for no-capture** (16.22) | Codegen |
| Step 5 | 🔧 Pending | Cleanup (remove inline path for no-capture) |

**Next**: Fix capture extraction (GEP-from-pointer) to enable capture closures.
