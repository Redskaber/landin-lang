# Stage 15.80 — Error System Cleanup: Human-Readable Type Names + Remove Debug Enum Leaks

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.204.0 → v0.205.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.80 cleans up the user-facing error system by:

1. **Adding `type_to_string` / `type_kind_to_string`** helpers in
   `src/mir/ty.rs` that format `Ty` / `TyKind` as human-readable type
   names (e.g., `i32`, `&mut bool`, `[i32; 10]`, `(i32, bool)`) instead
   of Rust Debug format (e.g., `Int(I32)`, `Ref(Erased, Mutable, Bool)`,
   `Array(Int(I32), Const { ... })`).
2. **Replacing `{:?}` Debug formatting** in user-facing error messages
   with the new helpers:
   - `typeck::error::TypeError::mismatch` — `expected {:?}, found {:?}` →
     `expected i32, found bool`
   - `typeck::checker` (3 sites) — `expected function, found {:?}` →
     `expected function, found i32`
   - `typeck::checker` (1 site) — `assert condition must be bool, found {:?}` →
     `assert condition must be bool, found i32`
   - `driver.rs::to_diagnostics` typeck notes — `expected: {:?}` /
     `found: {:?}` → `expected: i32` / `found: bool`
3. **Removing `({:?})` enum variant name leak** in borrowck errors:
   - `driver.rs::format_for_user` — `[borrowck] {} ({:?})` → `[borrowck] {}`
     (removes `(AssignImmutable)`, `(BorrowImmutable)`, etc.)
   - `driver.rs::to_diagnostics` — same removal for diagnostic-rendered
     borrowck errors

**Before** (typical error output):
```
error[E500]: cannot assign twice to immutable variable (AssignImmutable)
error[E400]: expected function, found Int(I32)
note: expected: Infer(IntVar(IntVid(0)))
note: found: Bool
```

**After**:
```
error[E500]: cannot assign twice to immutable variable
error[E400]: expected function, found i32
note: expected: {integer}
note: found: bool
```

**Test impact**:
- 8 new Rust unit tests for `type_to_string` (primitives, references,
  raw pointers, arrays, tuples, inference vars, special types, nested)
- 0 conformance test changes (all `ERROR_PATTERN` matches still work —
  they check for substring patterns like "immutable", "cannot borrow",
  not Debug format)
- **Total: 7580 tests passing** (232 lib + 2132 integration + 5216
  conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
Per §1.0 原則 4 "报错 > 静默": error messages are clear, not cryptic.
Per §23 (API Naming): `type_to_string` follows `<noun>_<verb>_<noun>`
pattern (matches Rust convention `ty::type_to_string`).

## 2. Why This Matters

The previous error messages leaked internal compiler implementation
details:

1. **`Int(I32)`** — the user doesn't know (or care) that the compiler
   represents integer types as `TyKind::Int(IntTy::I32)`. They want to
   see `i32`.
2. **`Infer(IntVar(IntVid(0)))`** — this is meaningless to users. The
   Rust convention is `{integer}` for integer inference variables.
3. **`(AssignImmutable)`** — this is the Debug enum variant name of
   `BorrowErrorKind::AssignImmutable`. It adds noise without value; the
   message "cannot assign twice to immutable variable" is already
   descriptive.

These leaks are particularly bad for new users who don't have the
mental model to decode Debug output. They make the compiler look
unpolished and unprofessional.

## 3. The Fix

### 3.1 `type_to_string` / `type_kind_to_string` (src/mir/ty.rs)

New public functions:

```rust
pub fn type_to_string(ty: &Ty) -> String
pub fn type_kind_to_string(kind: &TyKind) -> String
```

Both produce the same output. The `type_kind_to_string` variant is for
callers that have a `TyKind` directly (e.g., `expected.kind`, `found.kind`)
without wrapping it in a `Ty`.

Format conventions (match Rust):

| TyKind | Output |
|--------|--------|
| `Bool` | `bool` |
| `Char` | `char` |
| `Int(I32)` | `i32` |
| `Int(Isize)` | `isize` |
| `Uint(U8)` | `u8` |
| `Uint(Usize)` | `usize` |
| `Float(F64)` | `f64` |
| `Str` | `str` |
| `Never` | `!` |
| `Ref(_, Immutable, T)` | `&T` |
| `Ref(_, Mutable, T)` | `&mut T` |
| `RawPtr(Immutable, T)` | `*const T` |
| `RawPtr(Mutable, T)` | `*mut T` |
| `Array(T, n)` | `[T; n]` |
| `Slice(T)` | `[T]` |
| `Tuple([])` | `()` |
| `Tuple([A])` | `(A,)` |
| `Tuple([A, B])` | `(A, B)` |
| `FnDef(_, _)` | `fn` |
| `FnPtr(sig)` | `fn(...) -> ...` |
| `Closure(_, _)` | `{closure}` |
| `Adt(_, _)` | `<adt>` |
| `Foreign` | `<foreign type>` |
| `Param(_)` | `<type param>` |
| `Infer(TyVar(_))` | `_` |
| `Infer(IntVar(_))` | `{integer}` |
| `Infer(FloatVar(_))` | `{float}` |
| `Error` | `<type error>` |

### 3.2 Error message replacements

#### 3.2.1 `typeck::error::TypeError::mismatch`

```rust
// Before
message: format!("mismatched types: expected {:?}, found {:?}", expected.kind, found.kind)

// After
message: format!(
    "mismatched types: expected {}, found {}",
    type_kind_to_string(&expected.kind),
    type_kind_to_string(&found.kind),
)
```

#### 3.2.2 `typeck::checker` (3 sites)

Two sites in `check_terminator` for "expected function, found {:?}":
```rust
// Before
format!("expected function, found {:?}", func_ty.kind)

// After
format!("expected function, found {}", type_kind_to_string(&func_ty.kind))
```

One site in `post_check_terminator` for "assert condition must be bool, found {:?}":
```rust
// Before
format!("assert condition must be bool, found {:?}", cond_ty.kind)

// After
format!("assert condition must be bool, found {}", type_kind_to_string(&cond_ty.kind))
```

#### 3.2.3 `driver.rs::to_diagnostics` typeck notes

```rust
// Before
builder = builder.with_note(format!("expected: {:?}", expected.kind), e.span);
builder = builder.with_note(format!("found: {:?}", found.kind), e.span);

// After
builder = builder.with_note(format!("expected: {}", type_kind_to_string(&expected.kind)), e.span);
builder = builder.with_note(format!("found: {}", type_kind_to_string(&found.kind)), e.span);
```

#### 3.2.4 `driver.rs::format_for_user` borrowck messages

```rust
// Before
out.push_str(&format!("  [borrowck] {} ({:?})\n", e.message, e.kind));

// After
out.push_str(&format!("  [borrowck] {}\n", e.message));
```

#### 3.2.5 `driver.rs::to_diagnostics` borrowck messages

```rust
// Before
DiagnosticBuilder::error(format!("{} ({:?})", e.message, e.kind), e.span)

// After
DiagnosticBuilder::error(&e.message, e.span)
```

## 4. API Naming Compliance (§23)

**New public functions**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `type_to_string(ty: &Ty) -> String` | `mir::ty` | ✅ `<noun>_<verb>_<noun>` (matches Rust convention) |
| `type_kind_to_string(kind: &TyKind) -> String` | `mir::ty` | ✅ `<noun>_<verb>_<noun>` |

Private helpers (`int_ty_to_string`, `uint_ty_to_string`,
`float_ty_to_string`, `infer_var_to_string`, `fn_sig_to_string`) are
not public — no §23 concern.

## 5. §16 Interface Isolation

The new `type_to_string` / `type_kind_to_string` functions live in
`mir::ty` — the same module that defines `Ty` and `TyKind`. They read
only `Ty` / `TyKind` data (no resolver, no HIR, no borrowck).

Callers (`typeck::error`, `typeck::checker`, `driver`) import the
helpers via `crate::mir::ty::type_kind_to_string`. No new cross-stage
dependencies.

The `Adt(_, _)` arm returns `<adt>` rather than the type name because
resolving `DefId` → name requires resolver access (lives in `driver`).
This keeps `type_to_string` pure (no resolver dependency). A future
stage can add `type_to_string_with_resolver` if richer type display is
needed.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Helpers in `mir::ty` (same module as types); callers import explicitly |
| D2 Tech Debt | ✅ | 6 `{:?}` leaks fixed; error system is now consistent |
| D3 Test Coverage | ✅ | 8 new unit tests cover all `TyKind` variants + nested |
| D4 Next-Phase Readiness | ✅ | No regressions; error messages are user-friendly |
| D5 Design Rationality | ✅ | Matches Rust convention (`{integer}`, `_`, `()`, `(A,)`) |
| D6 Performance | ✅ | `String` allocation per error message; negligible (errors are rare) |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | All `TyKind` variants have at least one test case |

**Committee Vote**: GO — Stage 15.80 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 232/232 PASS (was 224, +8 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2132/2132 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7580 tests passing, 0 failures, 0 warnings.**

## 8. Audit Update (per user directive)

Per the user's directive ("将修正 tests/conformance/ 下的 (compile, run)
error 测试分析当前阶段是否具备修复能力纳入计划") and the new constraint
("前期内容(尤其是在开发期)不要单一的简写语法"):

- The Stage 15.78 audit identified 4 e2e tests with potential shorthand-
  syntax issues (`Vec{T}`, generic struct literal construction, etc.).
  Per the new constraint, these are **NOT** pursued in this stage —
  shorthand syntax work belongs in the stable phase, not development.
- Instead, this stage focuses on **error system quality**, which is a
  cross-cutting concern that improves all error paths without changing
  language syntax.
- The 412 remaining `EXPECTED: compile_error` tests in conformance are
  correctly classified (verified in Stage 15.78 audit). No flips needed
  in this stage.

## 9. Next Steps

### 9.1 Error system follow-ups (optional, low priority)

- `if 42` Span points to 1:1 (file start) instead of the `42` literal.
  Fixing this requires threading the condition expression's span
  through `check_terminator`. Not done in this stage — separate work.
- `Adt(_, _)` displays as `<adt>` instead of the type name. Adding
  resolver-backed type display would require passing the resolver to
  `type_to_string`. Defer until needed.
- Some borrowck error messages could be more descriptive (e.g.,
  "cannot borrow as mutable" could mention the borrow location). Not
  done in this stage.

### 9.2 Recommended next stage

The fresh_infer_ty reduction pattern (Stages 15.73-15.77) has one more
target: `HirExprKind::Loop` result type (currently `fresh_infer_ty`,
could be resolved from the first `break` operand's type). This is a
small type-resolution improvement, symmetric with the previous stages.

Alternatively, start **Task 12 (Lifetime elision)** — the next major
v0.2 task (2-3 weeks, P1, ready now).

## 10. Version Policy

v0.204.0 → v0.205.0 (minor bump — Phase 2 error system cleanup + 8 new
unit tests).
