# Stage 14.64 — Integer Type Coercion in Codegen (Bool Store + i64 Constants)

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.80.0
> **Process**: stage-committee-process.md v3.22 §11.3 (LLVM doc sync)

## 1. Problem: Silent Type Mismatches in `emit_store`

The `LLVMSysEmitter::emit_store` function had a critical bug: it ignored the
`ty: &EmitType` parameter and just called `LLVMBuildStore(builder, v, p)`,
which uses the value's actual LLVM type — not the target alloca's type.

This caused two distinct silent miscompilation patterns:

### 1.1 Bool Store Mismatch (i32 → i1)

`codegen_rvalue` for comparison ops (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge`) always
zexts the i1 comparison result to i32:

```rust
BinOp::Gt => {
    let cmp = emitter.emit_icmp("sgt", &ty, &a_val, &b_val);
    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)  // i1 → i32
}
```

When this i32 value is stored to a Bool (i1) local's alloca:
- TextEmitter produces `store i1 %v25, %loc_11` — but `%v25` is i32 (invalid IR)
- LLVMSysEmitter calls `LLVMBuildStore(builder, i32_val, i1_ptr)` — stores 4
  bytes to a 1-byte alloca, corrupting adjacent stack memory

**Symptom**: `bubble_sort_pass([5, 3, 1, 4, 2])` returned `0 0 1 2 4` instead
of `3 1 4 2 5`. The `if result[i] > result[i+1]` comparison was reading
corrupted bool values from the stack.

### 1.2 i64 Constant Width Mismatch

`LLVMSysEmitter::emit_const` always creates i32 constants for `ConstVal::Int`:

```rust
ConstVal::Int(n) => {
    let ty = LLVMInt32TypeInContext(self.ctx);  // ALWAYS i32!
    LLVMConstInt(ty, *n as u64, 1)
}
```

When the constant's actual type is i64 (from `c.ty`), storing the i32 value
to an i64 alloca only writes 4 bytes. The upper 4 bytes are uninitialized
garbage. Loading as i64 produces wrong values.

**Symptom**: `big_sum(1_000_000_000, 2_000_000_000)` returned
`180228417674752` instead of `3000000000`. The assembly showed:
```asm
movl $0x3b9aca00, 0x50(%rsp)   ; 32-bit store (4 bytes)
mov  0x50(%rsp), %rdi          ; 64-bit load (8 bytes — upper 4 are garbage)
```

## 2. Fix

### 2.1 Statement-Level Bool Truncation (`src/codegen/statement.rs`)

When storing to an i1 local AND the rvalue is a comparison (`BinaryOp` with
`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge`), trunc the i32 value to i1:

```rust
if ty == EmitType::I1 {
    if let Rvalue::BinaryOp(op, _, _) = rvalue {
        if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
            val = emitter.emit_cast(&EmitType::I32, &EmitType::I1, &val);
        }
    }
}
```

`emit_cast(I32, I1, val)` calls `LLVMBuildTrunc` which extracts the low bit
of the i32 value, producing a proper i1.

### 2.2 Constant Type Cast (`src/codegen/operand.rs`)

After `emit_const`, cast the value to the constant's declared type (`c.ty`)
— but ONLY for integer types. For non-integer types (struct, enum), the
constant's value is a placeholder and the actual value is constructed
elsewhere via `insertvalue`.

```rust
let raw = emitter.emit_const(&c.val);
let target_ty = mir_type_to_emit_type_with_layouts(&c.ty, layouts);
let src_ty = match &c.val {
    ConstVal::Int(_) | ConstVal::Uint(_) | ConstVal::Char(_) => EmitType::I32,
    ConstVal::Bool(_) => EmitType::I1,
    ConstVal::Float(_) => EmitType::F64,
    _ => return raw,
};
// Only cast if BOTH src and target are integer types
let is_int_cast = matches!((src_ty, target_ty), /* integer pairs */);
if src_ty == target_ty || !is_int_cast {
    raw
} else {
    emitter.emit_cast(&src_ty, &target_ty, &raw)
}
```

### 2.3 Store-Level Integer Coercion (`src/codegen/llvm/mod.rs`)

`emit_store` now checks the value's actual LLVM type via `LLVMTypeOf`. If it
doesn't match the target type AND both are integers, cast via
`LLVMBuildIntCast2` (which handles zext/sext/trunc automatically):

```rust
let val_ty = LLVMTypeOf(v);
let target_llvm_ty = self.llvm_type(ty);
let val_kind = LLVMGetTypeKind(val_ty);
let target_kind = LLVMGetTypeKind(target_llvm_ty);
let stored = if val_ty == target_llvm_ty {
    v
} else if val_kind == LLVMIntegerTypeKind && target_kind == LLVMIntegerTypeKind {
    let name_c = CString::new("cast").unwrap();
    LLVMBuildIntCast2(self.builder, v, target_llvm_ty, 1, name_c.as_ptr())
} else {
    v  // Non-integer mismatch — store directly, let LLVM verification catch it
};
LLVMBuildStore(self.builder, stored, p);
```

The `1` parameter to `LLVMBuildIntCast2` means "signed" — sign-extend for
wider types, trunc for narrower. This matches Landin's default i32 (signed)
semantics.

## 3. Why Three Fixes Instead of One?

Each fix addresses a different layer of the problem:

1. **Statement-level Bool trunc** (2.1): Surgical fix for the specific
   comparison→Bool case. Catches the issue at the source.

2. **Constant type cast** (2.2): General fix for integer constants with
   declared types wider than i32. Catches the issue at the operand level.

3. **Store-level coercion** (2.3): Defense-in-depth at the store instruction.
   Catches any remaining mismatches that slip through (1) and (2).

Per §1.0 原则 6 "通用 > 特例": fix (2.3) is the general mechanism, while (2.1)
and (2.2) are specific applications. Having all three ensures robustness.

## 4. Verification

### Before fix
```
$ ./landin-stage0 --run bubble_sort.lin
0 0 1 2 4                    # WRONG (expected: 3 1 4 2 5)

$ ./landin-stage0 --run i64_test.lin
180228417674752              # WRONG (expected: 3000000000)
```

### After fix
```
$ ./landin-stage0 --run bubble_sort.lin
3 1 4 2 5                    # CORRECT

$ ./landin-stage0 --run i64_test.lin
3000000000                   # CORRECT
```

### Assembly (after fix)

For i64 constants:
```asm
movabs $0x3b9aca00, %rax     ; 64-bit immediate load
mov    %rax, 0x50(%rsp)      ; 64-bit store (8 bytes)
mov    0x50(%rsp), %rdi      ; 64-bit load (8 bytes — correct)
```

## 5. References

- LLVM API: `LLVMBuildStore`, `LLVMBuildIntCast2`, `LLVMTypeOf`, `LLVMGetTypeKind`
- LLVM Language Reference: [Integer Cast Instructions](https://llvm.org/docs/LangRef.html#cast-instructions)
- Landin process: `docs/stage-committee-process.md` v3.22 §11.3 (LLVM doc sync)
- Related: `docs/llvm/stage-14.63-forward-decl-dedup-and-zst.md` (ZST representation)
