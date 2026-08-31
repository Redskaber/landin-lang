# Integer Type Responsibility Boundaries — Design Document

> **Stage**: 31.6d (v0.19)
> **Date**: 2026-08-31
> **Version**: v0.565.0
> **Author**: ARCH-A (PM-A)
> **Status**: Design — added to tech-debt repair queue

## 1. Problem Statement

Landin's integer type system has a structural design flaw: the `IntTy` enum
conflates signed and unsigned integers. It contains both `I64` (signed) and
`U64` (unsigned) as variants, while a separate `UintTy` enum exists but is
not used at the lexer token level.

This causes:
- **Type safety loss**: signed/unsigned distinction is lost at token level
- **ABI mismatch risk**: C ABI functions expect `i64` but Landin may pass `u64`
- **Semantic confusion**: `IntTy::U64` is not an "int type" — it's unsigned

## 2. Rust Reference Design

Rust separates signed and unsigned integers into two distinct enums:

```rust
// Rust's IntTy — signed integers ONLY
pub enum IntTy {
    Isize,  // pointer-sized signed (target-dependent)
    I8, I16, I32, I64, I128,
}

// Rust's UintTy — unsigned integers ONLY
pub enum UintTy {
    Usize,  // pointer-sized unsigned (target-dependent)
    U8, U16, U32, U64, U128,
}
```

Key Rust principles:
1. **Signed/Unsigned are distinct types** — never mixed without explicit cast
2. **`usize` is for sizes/indices** — array indexing, container sizes, `sizeof`
3. **`isize` is for pointer offsets** — rarely used in user code
4. **Default integer literal type is `i32`** — matches C's `int`
5. **`sizeof` returns `usize`** — standard convention

## 3. Landin Current State

### 3.1 Type Definitions

```rust
// ast/kinds.rs — Landin's IntTy (BOTH signed AND unsigned)
pub enum IntTy {
    I8, I16, I32, I64, I128, Isize,  // signed
    // ... but lexer also maps u8/u16/u32/u64/u128/usize here:
    U8, U16, U32, U64, U128, Usize,  // unsigned (WRONG: should be UintTy)
}

// ast/kinds.rs — Landin's UintTy (CORRECT separation, but UNUSED in lexer)
pub enum UintTy {
    U8, U16, U32, U64, U128, Usize,  // unsigned
}
```

### 3.2 Token Level

```rust
// lexer/token.rs — IntLit uses IntTy for ALL integers (including unsigned)
TokenKind::IntLit(u128, Option<IntTy>)
// "42u64" → IntLit(42, Some(IntTy::U64))  ← WRONG: U64 is unsigned, not IntTy
// "42i64" → IntLit(42, Some(IntTy::I64))  ← correct
```

### 3.3 MIR Level

```rust
// mir/ty.rs — Correctly separates Int and Uint
pub enum TyKind {
    Int(IntTy),    // signed
    Uint(UintTy),  // unsigned
    ...
}
```

### 3.4 ConstVal

```rust
// mir/ty.rs — Both Int and Uint stored as u128
pub enum ConstVal {
    Int(u128),   // signed value stored as u128
    Uint(u128),  // unsigned value stored as u128
    ...
}
```

### 3.5 Codegen

```rust
// codegen/emitter/mod.rs — I64/U64 both map to EmitType::I64 (same LLVM type)
TyKind::Int(IntTy::I64) | TyKind::Uint(UintTy::U64) => EmitType::I64,
TyKind::Int(IntTy::Isize) | TyKind::Uint(UintTy::Usize) => EmitType::I64,
```

### 3.6 Default Type

```rust
// typeck/unify.rs — Default unresolved int vars to I32
self.int_vars[root.0 as usize] = IntVarBinding::Bound(IntTy::I32);
```

### 3.7 Type Size

```rust
// mir/lower/adt_layout.rs — isize/usize hardcoded to 8 bytes
IntTy::Isize => 8,  // WRONG on 32-bit targets (should be 4)
UintTy::Usize => 8, // WRONG on 32-bit targets (should be 4)
```

## 4. Responsibility Boundary Design

### 4.1 Type Hierarchy

| Category | Types | Signed | Width (64-bit target) | Primary Use |
|----------|-------|--------|-----------------------|-------------|
| **Signed** | `i8` | Yes | 1 byte | Small signed values |
| | `i16` | Yes | 2 bytes | Short integers |
| | `i32` | Yes | 4 bytes | Default integer type (literal `42`) |
| | `i64` | Yes | 8 bytes | C ABI `long long`, large values |
| | `i128` | Yes | 16 bytes | BigInt operations |
| | `isize` | Yes | 8 bytes (64-bit) / 4 bytes (32-bit) | Pointer offset arithmetic |
| **Unsigned** | `u8` | No | 1 byte | Byte buffers, raw memory |
| | `u16` | No | 2 bytes | UTF-16, ports |
| | `u32` | No | 4 bytes | Unicode codepoints, hash values |
| | `u64` | No | 8 bytes | Large unsigned values |
| | `u128` | No | 16 bytes | Uuid, large hashes |
| | `usize` | No | 8 bytes (64-bit) / 4 bytes (32-bit) | **Sizes, indices, sizeof** |

### 4.2 Responsibility Boundaries

| Type | Responsibility | Capability Boundary | Prohibited |
|------|---------------|---------------------|-----------|
| **`i32`** | Default integer literal type | General arithmetic, function return values | Array indexing (use `usize`), raw memory (use `u8`) |
| **`i64`** | C ABI signed integer (`long long`) | Extern "C" function parameters, large signed values | Array indexing, pointer arithmetic base |
| **`isize`** | Pointer offset arithmetic | `ptr + isize` (signed offset, can be negative) | Array indexing (use `usize`), general arithmetic |
| **`usize`** | **Sizes, indices, lengths** | Array/slice indexing, `len`, `cap`, `sizeof` result | General arithmetic (use `i32`/`i64`), C ABI (use `i64`) |
| **`u8`** | Raw byte data | `*mut u8`, `*const u8`, byte buffers | Arithmetic (use `i32`), indexing (use `usize`) |
| **`u64`** | Large unsigned values | Hash values, timestamps | Array indexing, C ABI (use `i64`) |
| **`u128`** | Uuid, large hashes | 128-bit unsigned arithmetic | General use (performance cost) |
| **`i128`** | 128-bit signed arithmetic | BigInt, crypto | General use (performance cost) |

### 4.3 Key Design Rules (from Rust)

1. **`usize` is NOT a general-purpose integer** — it's for sizes/indices ONLY
2. **`isize` is NOT a general-purpose integer** — it's for pointer offsets ONLY
3. **Default integer literal type is `i32`** — matches C's `int` and Rust's default
4. **C ABI functions use `i64` for 64-bit integers** — NOT `usize` (which is target-dependent)
5. **`sizeof(T)` returns `usize`** — standard convention
6. **Array indexing uses `usize`** — standard convention
7. **Signed/Unsigned never mix without explicit cast** — type safety

### 4.4 Landin-Specific Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Default int literal | `i32` | Matches Rust + C convention |
| C ABI 64-bit integer | `i64` | Matches C `long long` (not `usize` which is target-dependent) |
| Array/slice index | `usize` | Matches Rust convention |
| String length | `usize` | Matches Rust convention |
| sizeof(T) result | `usize` | Matches Rust convention |
| Pointer offset | `isize` (from `ptr + isize`) | Matches Rust convention; allows negative offsets |
| Extern "C" alloc size | `i64` | C runtime uses `long long` (not `size_t`); matches existing tests |

## 5. Current Issues & Repair Queue

### 5.1 P1 Issues (Type Safety — Must Fix)

| ID | Issue | Fix Plan |
|----|-------|----------|
| **TD-INT-SIGN-CONFUSION** | `IntTy` enum conflates signed/unsigned; `TokenKind::IntLit` uses `IntTy` for unsigned literals | Refactor: split `IntTy` into signed-only; use `UintTy` for unsigned; change `IntLit` to `IntLit(u128, Option<SignedIntTy>)` or unify to a single `IntType` enum with sign info |

### 5.2 P2 Issues (Correctness — Should Fix)

| ID | Issue | Fix Plan |
|----|-------|----------|
| **TD-CONST-INT-UINT-U128** | `ConstVal::Int(u128)` and `ConstVal::Uint(u128)` both use `u128` storage — overflow semantics differ but storage is same | Acceptable for MVP (Rust also uses `u128` for both); document the design decision |
| **TD-ISIZE-USIZE-HARDCODED** | `isize`/`usize` hardcoded to 8 bytes in `compute_type_size` | Acceptable for MVP (Landin targets 64-bit only); add `#[cfg(target_pointer_width)]` in future |

### 5.3 P3 Issues (Design — Should Document)

| ID | Issue | Fix Plan |
|----|-------|----------|
| **TD-DEFAULT-INT-I32** | Default unresolved int vars to `I32`, but prelude uses `i64` for C ABI | This is correct: default literal type is `i32` (matches Rust); C ABI explicitly uses `i64` suffix (`42i64`); no change needed |
| **TD-EMIT-I64-SAME-LLVM** | `i64` and `u64` both map to `EmitType::I64` (same LLVM type `i64`) | This is correct: LLVM doesn't distinguish signed/unsigned at IR level (sign is in instruction semantics, e.g., `sdiv` vs `udiv`); no change needed |

## 6. Implementation Plan (Future Stages)

### Stage 31.6d: sizeof(T) Language Feature
- Add `sizeof::<T>()` intrinsic or `T::size()` to prelude
- Returns `usize`
- Unblocks Vec::push/get/Box::new migration

### Stage 31.7: IntTy/UintTy Separation (TD-INT-SIGN-CONFUSION)
- Refactor `IntTy` to signed-only: `I8, I16, I32, I64, I128, Isize`
- Use existing `UintTy` for unsigned: `U8, U16, U32, U64, U128, Usize`
- Change `TokenKind::IntLit(u128, Option<IntTy>)` to `IntLit(u128, Option<IntSuffix>)` where `IntSuffix` is a unified enum
- Update all downstream code (typeck, codegen, MIR)

### Stage 31.8: sizeof(T) Migration
- Migrate `Box::new`, `Vec::push`, `Vec::get` to prelude impl using `sizeof(T)`
- Remove remaining intrinsic dispatch

Per §1.0 原則 1 (内存安全决不能妥协): signed/unsigned confusion is a memory safety risk.
Per §1.0 原則 3 (显式 > 隐式): type boundaries must be explicit.
Per §12 (最优 > 最小): root-cause fix via proper type separation, not workarounds.
