# Stage 18.180 — Real String Type (TD-STRING-AS-STR-ALIAS fix)

> **Date**: 2026-08-17
> **Version**: v0.447.0 → v0.448.0
> **Task ID**: stage18.180
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.178 (heap alloc), Stage 18.179 (Box MVP)
> **Blocks**: Stage 18.181 (String intrinsics), Stage 18.182 (format!)

## 1. Scope

Per Stage 18.177 task review: replace the `String = &str` alias (Stage 18.176
MVP compromise) with a REAL owned heap type. This fixes the design violation
documented as TD-STRING-AS-STR-ALIAS.

**Scope of this stage** (deliberately minimal):
- Remove `"String" => PrimTy::Str` from `lookup_prim_ty`
- Add `struct String { ptr: *mut u8, len: i64, cap: i64 }` to prelude
- Update `stdlib_type_resolve` to classify "String" as AllocType
- Update conformance test `std-err-002` (String::new still unimplemented)
- 9 new tests (4 positive + 5 negative)

**Deferred to Stage 18.181** (recorded as TD-STRING-INTRINSICS):
- `String::from_str(s: &str) -> String` intrinsic
- `String::push_str(s: &str)` intrinsic
- `String::push(ch: char)` intrinsic
- `String::len() -> i64` intrinsic
- `String::as_str() -> &str` intrinsic

## 2. Implementation

### 2.1 Remove the &str Alias (src/resolve/primitives.rs)

**Before** (Stage 18.176):
```rust
"str" | "String" => PrimTy::Str,
```

**After** (Stage 18.180):
```rust
"str" => PrimTy::Str,
// "String" is NOT here — it's a prelude struct now.
```

This means `String` is no longer resolved as a primitive type. It falls
through to the normal module tree lookup, where the prelude's
`struct String { ... }` is registered.

Per §2 原則 9 (正确>妥协): the alias compromise is removed.
Per §1.0 原則 6 (通解>特例): one resolution path for String (module tree).

### 2.2 Add Real String Struct (src/stdlib/prelude.rs)

Added to PRELUDE_SOURCE:
```landin
struct String { ptr: *mut u8, len: i64, cap: i64 }
```

This matches the design doc (09-stdlib.md §3.4) intent — String is an
owned heap type. The MVP uses a simpler layout than the design doc's
`struct String { vec: Vec<u8> }` (we don't have Vec yet), but the
semantic is the same: String owns a heap buffer.

Per §1.0 原則 6 (通解>特例): one String type — no per-encoding special cases.
Per §2 原則 9 (正确>妥协): the &str alias compromise is removed.

### 2.3 Update stdlib facade (src/stdlib/mod.rs)

`resolve_stdlib_type("String")` now returns `StdlibTypeKind::AllocType`
(was `StdlibTypeKind::Str`). This aligns the facade with the real type
definition.

### 2.4 Update conformance test

`tests/conformance/06-stdlib/99-error-cases/std-err-002-undefined-string.lin`:
- Was: "use of undefined stdlib type String"
- Now: "String::new() (an unimplemented intrinsic) produces a compile error"

String is no longer undefined — it's a prelude struct. But `String::new()`
is still unimplemented (Stage 18.181 work).

### 2.5 Update test assertion

`tests/v0/stage5/plan/stdlib_type_resolve_tests.rs::test_resolve_alloc_types`:
- Was: `assert_eq!(resolve_stdlib_type("String"), StdlibTypeKind::Str);`
- Now: `assert_eq!(resolve_stdlib_type("String"), StdlibTypeKind::AllocType);`

### 2.6 Tests (tests/v0/stage18/plan/stage18_180_real_string_tests.rs)

9 tests (4 positive + 5 negative):
- Positive: struct literal construct, field access, prelude no-import,
  holds heap bytes (full alloc→store→load cycle)
- Negative: literal assign fails (was OK in 18.176), redefinition fails,
  + 1 SOFT (String::new not yet implemented), missing field fails,
  wrong field type fails

All 9 pass.

## 3. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 2996 passed (was 2987, +9 new)
- **Total**: 3654 tests, 0 failures

## 4. Tech Debt Status

| ID | Status |
|----|--------|
| TD-STRING-AS-STR-ALIAS | ✅ Resolved (Stage 18.180) — String is now a real struct |
| TD-HEAP-ALLOC | ✅ Resolved (Stage 18.178) |
| TD-STRING-INTRINSICS | 🟡 New — Stage 18.181 will add from_str/push_str/len/as_str |
| TD-BOX-AUTO-DROP | 🟡 Active — Box::new + auto-drop still pending |

## 5. Design Doc Sync

`docs/lang-design/09-stdlib.md` §3.4 MVP 偏差说明 (added in Stage 18.177)
is now PARTIALLY obsolete — the alias compromise is removed. The remaining
gap is the layout difference:
- Design doc: `struct String { vec: Vec<u8> }`
- Current: `struct String { ptr: *mut u8, len: i64, cap: i64 }`

Once Vec is implemented (Stage 18.182+), String can be refactored to wrap
Vec<u8>. Until then, the direct ptr/len/cap layout is functionally
equivalent (just less DRY).

## 6. Next Steps

Per user's NEW directive (this stage's user message): before continuing
with heap-allocated Vec/format!, audit the BASE types (str, primitives,
fat pointer) for completeness. This is a task review trigger.

Stage 18.181: Task review — base types completeness audit
- Audit `str` type: is it complete? (lacks &str methods like chars(), bytes())
- Audit primitive types: any gaps?
- Audit fat pointer codegen: any latent bugs?
- Re-plan task graph based on findings
