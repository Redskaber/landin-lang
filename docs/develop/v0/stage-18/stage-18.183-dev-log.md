# Stage 18.183 — Fat Pointer Index Projection Fix (TD-FAT-PTR-INDEX-PROJ)

> **Date**: 2026-08-17
> **Version**: v0.450.0 → v0.451.0
> **Task ID**: stage18.183
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.182 (array index fix)
> **Blocks**: Stage 18.184 (str methods), Stage 18.185 (String intrinsics)

## 1. Scope

Per Stage 18.181 task review: fix the P1 fat pointer Index projection bug
where `s[0]` on `&str` produced invalid IR ("GEP base pointer is not a vector").

This unblocks str byte indexing — a prerequisite for String intrinsics
(Stage 18.185: from_str needs to read &str bytes).

## 2. Dependency & Infrastructure Audit

Per user's NEW directive ("如果在设计和开发过程中设计的内容需要依赖底层实现和功能时，
应当先做依赖与基础设施完整能力审查"), audited all dependencies BEFORE implementation:

- ✅ MIR layer: Index projection, DCE, const_prop all complete (Stage 18.182 fix)
- ✅ Codegen emit: emit_gep_field, emit_extractvalue, emit_load, emit_gep_index_ptr
- ✅ Type system: TyKind::Ref/Str/Slice all supported
- 🟡 2 codegen bugs found (not infrastructure gaps): unwrap_fat_ptr_for_index + Index codegen
- ✅ Test infrastructure: str::len(), str Field projection all working

**Conclusion**: Dependencies complete, no task re-plan needed. See
`docs/develop/v0/stage-18/stage-18.183-dep-audit.md` for full audit.

## 3. Root Cause Analysis

### 3.1 Bug Reproduction

```landin
let s: &str = "hello";
let b: u8 = s[0];  // Error: "GEP base pointer is not a vector or a vector of pointers"
```

### 3.2 Bug 1: `unwrap_fat_ptr_for_index` missing LOAD

**Location**: `src/codegen/mir_translation/places.rs:430`

The function GEP'd to field 0 (address of data pointer) but didn't LOAD the
data pointer. The caller then tried to GEP into the address (pointer-to-pointer),
producing invalid IR.

**Fix**: Added `emit_load` after `emit_gep_field`:
```rust
let field_addr = emitter.emit_gep_field(&base_ptr_owned, storage_ty, 0);
let data_ptr = emitter.emit_load(&fields[0], &field_addr);  // NEW: load the data pointer
```

### 3.3 Bug 2: Index codegen loaded VALUE for fat pointer Refs

**Location**: `src/codegen/mir_translation/places.rs:695`

For ALL `Ref` types (including `&str`), the code loaded the fat pointer VALUE
(`{ ptr, i64 }`). This value was passed to `unwrap_fat_ptr_for_index` which
expected an ADDRESS (alloca pointer) to GEP into.

**Fix**: Distinguish fat pointer Refs from thin pointer Refs:
- `&str` / `&[T]` (fat): use alloca pointer (ADDRESS) → unwrap handles GEP+load
- `&[T; N]` / `&i32` (thin): load pointer value (unchanged behavior)
- bare `str` / `[T]`: use alloca pointer (same as fat Ref)

Per §1.0 原則 6 (通解>特例): one alloca+GEP+load path for all fat pointer Index.

## 4. Verification

```
s[0] = 104  ('h')  ✅ (was: codegen error)
s[1] = 101  ('e')  ✅
s[2] = 108  ('l')  ✅
s[3] = 108  ('l')  ✅
s[4] = 111  ('o')  ✅
```

## 5. Tests

### 5.1 New Tests (tests/v0/stage18/plan/stage18_183_fat_ptr_index_tests.rs)

8 tests (7 positive + 1 negative):
- Positive: first byte, various positions, multi-index, let-bound index,
  returns u8, empty string (soft), len+index combined
- Negative: i32[0] fails type check

All 8 pass.

## 6. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3012 passed (was 3004, +8 new)
- **Total**: 3670 tests, 0 failures

## 7. Tech Debt Status

| ID | Status |
|----|--------|
| TD-FAT-PTR-INDEX-PROJ | ✅ Resolved (Stage 18.183) |
| TD-ARRAY-INDEX-CODEGEN | ✅ Resolved (Stage 18.182) |
| TD-ARRAY-BOUNDS-CHECK | 🟡 Active — Stage 18.184+: insert LLVM bounds checks |
| TD-STR-METHODS-RUNTIME | 🟡 Active — Stage 18.184 |
| TD-STRING-INTRINSICS | 🟡 Active — Stage 18.185 |

## 8. Next Steps

Stage 18.184: str methods runtime fix (is_empty/as_bytes/to_string)
- These methods compile but segfault at runtime
- Now that s[0] works, we can implement them as MIR intrinsics
- is_empty: s.len() == 0
- as_bytes: return the &str as &[u8] (same fat pointer, different type)
- to_string: alloc + copy bytes + wrap in String struct
