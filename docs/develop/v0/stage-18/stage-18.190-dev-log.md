# Stage 18.190 — Box::new Type Coercion Fix (TD-BOX-NEW-TYPE-COERCE)

> **Date**: 2026-08-17
> **Version**: v0.457.0 → v0.458.0
> **Task ID**: stage18.190
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A

## 1. Scope

Fix TD-BOX-NEW-TYPE-COERCE: `Box::new(x)` stores through `*mut u8`, truncating
larger types (i64 stored as i8 → only 1 byte written).

## 2. Root Cause

`Box::new(x)` calls `__landin_alloc(size)` which returns `*mut u8`. The store
`*alloc_dest = x` uses `ProjectionElem::Deref` on a `*mut u8` local. In
`emit_store`, the value was cast to the target type but the **pointer** was not
cast — LLVM stored the i64 value through an i8* pointer, writing only 1 byte.

## 3. Fix (src/codegen/llvm/memory.rs::emit_store)

Added pointer type bitcast before store:

```rust
let ptr_ty = LLVMTypeOf(p);
let expected_ptr_ty = LLVMPointerType(target_llvm_ty, 0);
let final_ptr = if ptr_ty == expected_ptr_ty {
    p
} else {
    LLVMBuildBitCast(self.builder, p, expected_ptr_ty, name_c.as_ptr())
};
LLVMBuildStore(self.builder, stored, final_ptr);
```

Per §1.0 原則 9 (正确>妥协): fix root cause (cast pointer type), not symptom.
Per §1.0 原則 6 (通解>特例): one bitcast for all type mismatches.

## 4. Verification

```
Box::new(42).0 = 42 (i32)     ✅
Box::new(255).0 = 255 (u8)    ✅
Box::new(42).0 = 42 (i64)    ✅ (was: truncated to i8)
```

Note: Large i64 values (> i32 max) still fail due to pre-existing
TD-INT-UINT-VAR (literals default to i32, truncated before Box::new).

## 5. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3049 passed
- **Total**: 3707 tests, 0 failures

## 6. Tech Debt Status

| ID | Status |
|----|--------|
| TD-BOX-NEW-TYPE-COERCE | ✅ Resolved (Stage 18.190) |
| TD-INT-UINT-VAR | 🟡 Pre-existing — i64 literals > i32 max truncated |
