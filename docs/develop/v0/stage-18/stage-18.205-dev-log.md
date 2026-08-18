# Stage 18.205 — TD-FUNCTION-REDEFINE-PARAMS Fix (format! method calls)

> **Date**: 2026-08-17
> **Version**: v0.469.0 → v0.470.0
> **Task ID**: stage18.205
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6

## 1. Scope

Per Stage 18.204 deep review §5.2 action plan: fix TD-FUNCTION-REDEFINE-PARAMS
— the segfault when calling methods on `format!` results (e.g., `s.len()`).

Per §17.6 (缺陷纳入): this TD was identified in Stage 18.202 dev-log as
"pre-existing TD-FUNCTION-REDEFINE (forward declaration param type mismatch
for prelude methods)". Stage 18.204 deep review confirmed it as P2 priority
(item #3 in §5.2 action plan).

## 2. Dependency Audit (per §13.1 + user directive "依赖与基础设施完整能力审查")

| 依赖项 | 状态 |
|--------|------|
| LLVM C API (LLVMBuildStore, LLVMConstNull, etc.) | ✅ |
| EmitType::OpaquePtr → LLVM ptr type | ✅ |
| ArithmeticEmitter trait (emit_const, emit_cast) | ✅ |
| MemoryEmitter trait (emit_store, emit_load) | ✅ |
| Stage 18.188 fix (return type mismatch) | ✅ (precedent for same-root-cause family) |

**结论**: 依赖完整, 可立即实施.

## 3. Root Cause Analysis

### 3.1 Symptom

`format!("x={}", 42).len()` segfaults (exit 139). But `format!("x={}", 42).len`
(field access) returns `4` correctly.

### 3.2 Root Cause

The bug is in `src/codegen/operand.rs` + `src/codegen/llvm/memory.rs`:

1. `ConstVal::Int(0)` with target type `*mut u8` (pointer) is lowered to:
   - `emit_const` creates `i32 0` (4 bytes, per Stage 18.191 optimization)
   - `operand.rs` returns `i32 0` without casting (OpaquePtr not in int-cast list)

2. `emit_store` stores `i32 0` to an `alloca ptr` (8-byte slot):
   - LLVM IR: `store ptr 0, %loc_10` (looks correct — 8 bytes)
   - BUT LLVM's `-O2` backend optimizes `store ptr null` → `store i32 0` (4 bytes)
   - This is because LLVM sees the null constant (value 0) and uses a 32-bit store

3. Later, `load ptr, %loc_10` reads 8 bytes:
   - Lower 4 bytes: 0 (from the 4-byte store)
   - Upper 4 bytes: stack garbage (NOT zeroed by the 4-byte store)

4. The garbage pointer is passed to `__landin_format_variadic` as `arg_types`:
   - C function checks `if (arg_types == 0)` — fails (garbage ≠ 0)
   - Takes `else` branch: `effective_types = arg_types` (garbage pointer)
   - Dereferences `effective_types[arg_idx]` → SEGFAULT

### 3.3 Same-Root-Cause Family

This bug is the same family as Stage 18.188 (TD-FUNCTION-REDEFINE return type
mismatch) — both are codegen bugs where the LLVM declaration/store doesn't
match the actual function/slot type. Stage 18.188 fixed the return type case;
Stage 18.205 fixes the param/store case.

Per §17.6 (同类型整体修复): this is an integrated fix for the "function
redefine" family — both return type (18.188) and param/store (18.205) now
handle type mismatches correctly.

## 4. Implementation

### 4.1 New `emit_null_ptr` method (src/codegen/emitter/arithmetic.rs)

Added `fn emit_null_ptr(&mut self) -> EmitValue` to `ArithmeticEmitter` trait.
Returns a `ptr null` constant for pointer-typed contexts.

### 4.2 LLVM backend implementation (src/codegen/llvm/arithmetic.rs)

```rust
fn emit_null_ptr(&mut self) -> EmitValue {
    unsafe {
        // Use i64 0 + inttoptr to force 8-byte representation.
        // LLVM folds this to `ptr null` at IR level, but the store
        // optimization is then handled by emit_store (see 4.3).
        let i64_ty = LLVMInt64TypeInContext(self.ctx);
        let i64_zero = LLVMConstInt(i64_ty, 0, 0);
        let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
        let null_val = LLVMConstIntToPtr(i64_zero, ptr_ty);
        self.fresh_named(null_val)
    }
}
```

### 4.3 `emit_store` pointer-type fix (src/codegen/llvm/memory.rs)

Added a branch at the top of `emit_store` that handles pointer-typed targets:

```rust
if target_kind == LLVMPointerTypeKind {
    // Force 8-byte store by casting through i64.
    // This works around LLVM's -O2 optimization that collapses
    // `store ptr null` → `store i32 0` (4 bytes).
    let i64_ptr = LLVMBuildBitCast(builder, p, i64_ptr_ty, ...);
    let val_i64 = if val_kind == PointerTypeKind {
        LLVMBuildPtrToInt(builder, v, i64_ty, ...)  // ptr → i64
    } else if val_kind == IntegerTypeKind {
        LLVMBuildIntCast2(builder, v, i64_ty, ...)  // int → i64
    } else { v };
    LLVMBuildStore(builder, val_i64, i64_ptr);
    return;
}
```

This forces ALL pointer-typed stores to go through `i64` (8 bytes), preventing
the 4-byte `movl` optimization.

### 4.4 Text backend implementation (src/codegen/text/arithmetic.rs)

```rust
fn emit_null_ptr(&mut self) -> EmitValue {
    "ptr null".to_string()
}
```

### 4.5 `operand.rs` constant handling (src/codegen/operand.rs)

Added a branch before the int-cast check:

```rust
let is_ptr_target = matches!(target_ty, EmitType::OpaquePtr | EmitType::Ptr(_));
if is_ptr_target {
    return emitter.emit_null_ptr();
}
```

When a constant is used in a pointer-typed context, emit `ptr null` directly
instead of `i32 0` + cast.

### 4.6 `emit_cast` int→ptr fix (src/codegen/llvm/arithmetic.rs)

Added a branch for integer→pointer casts:

```rust
} else if src_kind == LLVMIntegerTypeKind
    && dst_kind == LLVMPointerTypeKind
{
    LLVMBuildIntToPtr(self.builder, v, dst_ty, name_c.as_ptr())
}
```

Previously, int→ptr fell through to `LLVMBuildBitCast`, which is invalid for
int→ptr on x86-64 (produces wrong codegen).

## 5. Verification

```
format!("x={}", 42).len()           → 4          ✅ (was: segfault)
format!("x={}", 42).len              → 4          ✅ (already worked)
format!("{}+{}={}", 1, 2, 3).len()   → 5          ✅ (was: segfault)
String::from_str("hello").len()      → 5          ✅ (no regression)
String::new().len()                  → 0          ✅ (no regression)
Box::new(42) + *b.0                   → 42         ✅ (no regression)
```

## 6. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 664 passed
- ✅ cargo test --features llvm-backend --tests: 3089 passed (was 3081, +8 new)
- ✅ cargo clippy: 5 warnings (all pre-existing, 0 new)
- **Total**: 3753 tests, 0 failures, zero regression

## 7. Tech Debt

| ID | Status |
|----|--------|
| TD-FUNCTION-REDEFINE-PARAMS | ✅ Resolved — format! method calls now work (s.len() returns 4) |
| TD-C-WRAPPER-OVERUSE | 🟡 Active — the underlying C helper (`__landin_format_variadic`) is still used; this fix addresses the codegen bug, not the C wrapper pattern (migration plan in audit doc) |

## 8. Design Principles Applied

- §10 (DRY): `emit_null_ptr` is the single source for null pointer constants
- §12 (最优 > 最小): force 8-byte store via `i64` cast, not symptom workaround
- §1.0 原則 4 (报错>静默): old code silently stored 4 bytes, producing garbage
- §1.0 原則 6 (通解>特例): one `emit_store` branch handles all pointer stores
- §1.0 原則 9 (正确>妥协): fix root cause (force 8-byte), not symptom (zero-init upper)
- §17.6 (同类型整体修复): same family as Stage 18.188 (return type) — both fixed
