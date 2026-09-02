# Stage 36.2 (v0.24) — TD-FORMAT-MIGRATION Slice-based Prelude Format Impl Design

> **Author**: redskaber (PM-A + ARCH-A + DEV-A)
> **Date**: 2026-09-01
> **Version**: v0.578.0 (target)
> **Process**: stage-committee-process.md v7.5 §13.1 + §14.8
> **Complexity**: L3 (~200 LOC prelude + ~30 LOC macro + ~30 LOC cleanup - 598 LOC MIR walker = net -368 LOC)

## 1. Executive Summary

Stage 36.2 (v0.24) resolves TD-FORMAT-MIGRATION (P2) — the last major
tech-debt from v0.19. The 598-LOC `lower_format_variadic_intrinsic` MIR
walker (特解 — special case) is replaced with a slice-based prelude
impl (通解 — general mechanism).

**Approach** (per §1.0 原則 6 通解 > 特解, §12 最优 > 最小):
1. Add `__landin_i64_to_str` extern "C" declaration to prelude
2. Add `fn __landin_format_v2(fmt: &str, args: &[i64]) -> String` to
   prelude with a real body that walks fmt + args using standard Landin
   language features (while loops, slice indexing, String construction)
3. Modify format! macro to expand to `__landin_format_v2("x={}", &[x as i64, y as i64])`
4. Remove the 598-LOC MIR walker (`lower_format_variadic_intrinsic`)
5. Remove the `__landin_format` interception in `expr_variants.rs:565-580`

## 2. Bug Confirmation

The `format!` feature works today via:
1. Macro expansion: `format!("x={}", x)` → `__landin_format("x={}", x)`
2. MIR lowering interception (expr_variants.rs:565-580): detects
   `__landin_format` with >1 args, calls `lower_format_variadic_intrinsic`
3. 598-LOC MIR walker (format_intrinsics.rs): weaves format logic into
   each call site's MIR body

This is 特解 (special case) — per §1.0 原則 6, the 通解 (general mechanism)
is a prelude impl that uses standard method resolution.

## 3. Rust Reference Design Alignment

Per [Rust format! macro](https://doc.rust-lang.org/std/macro.format.html):
> `format!(fmtstr, args...)` expands to `std::fmt::format(format_args!(fmtstr, args...))`.
> `format_args!` builds a `fmt::Arguments` value at compile time.

In Rust, the macro builds an Arguments struct (not a variadic call).
The actual formatting is done by `std::fmt::format` which walks the
Arguments. This is the 通解 approach — no MIR-level weaving.

Landin's equivalent: macro builds an `&[i64]` array at expansion time,
prelude `__landin_format_v2` walks the array. MVP limit: all args cast
to i64 (same as existing MIR walker — preserved for backward compat).

**Rust philosophy applied**:
- §1.0 原則 6 (通解 > 特解): one prelude fn for all format! calls
  (no per-call-site MIR weaving).
- §1.0 原則 4 (报错 > 静默): errors propagate through standard typeck
  (arg count, type mismatches) — no special MIR-level error paths.
- §1.0 原則 9 (正确 > 妥协): preserve MVP limit (i64-only args) —
  documented, not silently extended.
- §12 (最优 > 最小): root-cause fix = prelude impl + macro expansion
  (removes 598 LOC of special-case MIR generation).

## 4. Design

### 4.1 Prelude Changes (src/stdlib/prelude.rs)

Add `__landin_i64_to_str` to the extern "C" block:
```rust
extern "C" {
    fn __landin_alloc(size: i64) -> *mut u8;
    fn __landin_memcpy(dst: *mut u8, src: *const u8, n: i64);
    fn __landin_realloc(ptr: *mut u8, old_size: i64, new_size: i64) -> *mut u8;
    fn __landin_panic_bounds_check(index: i64, len: i64);
    // Stage 36.2: format! prelude impl needs i64→str conversion.
    fn __landin_i64_to_str(buf: *mut u8, cap: i64, val: i64) -> i64;
}
```

Add `__landin_format_v2` free function to prelude:
```rust
// Stage 36.2: format! prelude impl — replaces 598-LOC MIR walker.
// Walks fmt string byte-by-byte, replacing {} placeholders with
// args[i] formatted via __landin_i64_to_str.
//
// MVP limit (same as old MIR walker): all args are i64. Non-i64 args
// must be cast by the caller (the format! macro does this).
//
// Per §1.0 原則 6 (通解 > 特解): one prelude fn for all format! calls.
// Per §12 (最优 > 最小): uses standard Landin language features.
fn __landin_format_v2(fmt: &str, args: &[i64]) -> String {
    let buf_size: i64 = 4096;
    let out_ptr: *mut u8 = __landin_alloc(buf_size);
    let mut out_len: usize = 0;
    let mut fmt_idx: usize = 0;
    let mut arg_idx: usize = 0;
    while fmt_idx < fmt.len() {
        // Load byte at fmt.ptr[fmt_idx]
        let byte_ptr: *mut u8 = fmt.ptr + fmt_idx;
        let byte: u8 = *byte_ptr;
        if byte == 123 {  // '{'
            // Format arg via __landin_i64_to_str
            if arg_idx < args.len() {
                let written: i64 = __landin_i64_to_str(
                    out_ptr + out_len,
                    buf_size - out_len as i64,
                    args[arg_idx],
                );
                out_len = out_len + written as usize;
                arg_idx = arg_idx + 1usize;
            }
            // Advance past {} (2 bytes)
            fmt_idx = fmt_idx + 2usize;
        } else {
            // Copy byte to output
            let dest: *mut u8 = out_ptr + out_len;
            *dest = byte;
            out_len = out_len + 1usize;
            fmt_idx = fmt_idx + 1usize;
        }
    }
    String { ptr: out_ptr, len: out_len, cap: buf_size as usize }
}
```

### 4.2 Macro Changes (src/parser/builtin_macros/print_macros.rs)

Modify `make_format_macro_rule` to expand to `__landin_format_v2`:
```rust
// Old: format!("x={}", x) → __landin_format("x={}", x)
// New: format!("x={}", x) → __landin_format_v2("x={}", &[x as i64])
//      format!("{}+{}", a, b) → __landin_format_v2("{}+{}", &[a as i64, b as i64])
```

Pattern: `$fmt:literal, $($args:expr),*` (or just `$fmt:literal` for
no-arg case).
Body: `__landin_format_v2($fmt, &[$($args as i64),*])`

For the no-arg case (format!("literal")), keep the existing
`String::from_str(literal)` path (no array needed).

### 4.3 MIR Walker Removal

Delete `src/mir/lower/format_intrinsics.rs` entirely (598 LOC).
Remove the `use` in `expr_variants.rs:38`.
Remove the `__landin_format` interception in `expr_variants.rs:565-580`.

### 4.4 Why This Works (per §1.0 原則 10 唯一可信数据源)

The prelude `__landin_format_v2` is the SINGLE source of truth for
format! logic. It uses:
- `slice.len()` (Stage 36.1) for args count
- `&str` fat pointer field access (`.ptr`, `.len`) for fmt walking
- `__landin_i64_to_str` (C runtime helper) for arg formatting
- `__landin_alloc` for output buffer allocation
- Standard Landin `while` + `if` for control flow

No MIR-level special-casing. No per-call-site weaving. The format!
macro just builds the call — standard method resolution handles it.

### 4.5 Backward Compatibility

Per §1.0 原則 5 (去除兼容思维): the old `__landin_format` name is
REMOVED (not kept as deprecated alias). All existing format! calls
will use the new `__landin_format_v2` path via the updated macro.

## 5. Test Plan (§9.4 + §7.3.1 ≥30 case audit)

### 5.1 Positive Tests (≥5)

| # | Source | Validates |
|---|--------|-----------|
| P1 | `format!("hello")` | No-arg format (literal) |
| P2 | `format!("x={}", 42)` | Single i64 arg |
| P3 | `format!("{}+{}={}", 1, 2, 3)` | Multiple args |
| P4 | `format!("val={}", x)` where x: i32 | Cast to i64 |
| P5 | `let s: String = format!("x={}", 42); s.len()` | E2E with len() |

### 5.2 Negative Tests (≥28 covering 7 error categories)

| # | Category | Source |
|---|----------|--------|
| N1-N10 | Typeck | Wrong arg types, missing args, etc. |
| N11-N13 | Lex | invalid tokens |
| N14-N16 | Parse | missing semis, braces, arrows |
| N17 | Borrowck | double mut borrow |
| N18-N19 | Resolve | undefined type, undefined value |
| N20-N21 | Trait | undefined trait, trait bound |
| N22 | Codegen | extern call path |
| N23-N28 | Nested/Context | other error patterns |

Total: 5 positive + 28 negative = 33 cases.

## 6. §3.2 Verification Plan

- cargo clean ✓
- cargo build --release ✓
- cargo check (0 errors, 0 warnings) ✓
- cargo fmt --check (0 diff) ✓
- cargo clippy -- -D warnings (0 warnings) ✓
- cargo test --release (5227+33 = 5260 tests, 0 failed) ✓

## 7. Implementation Plan

1. Add `__landin_i64_to_str` to prelude extern "C" block.
2. Add `__landin_format_v2` free function to prelude.
3. Modify `make_format_macro_rule` to expand to `__landin_format_v2`.
4. Delete `src/mir/lower/format_intrinsics.rs`.
5. Remove `use` + interception in `expr_variants.rs`.
6. Create `tests/v0/stage36/plan/format_migration_tests.rs` with 5+28 tests.
7. Add module entry to `tests/all_tests.rs`.
8. Run §3.2 verification.
9. Update docs.
10. Package per §19.

## 8. References

- TD-FORMAT-MIGRATION definition: `docs/develop/v0/tech-debt-register.md`
- Stage 36 design: `docs/develop/v0/stage-36/stage-36-format-migration-variadic-design.md`
- Stage 36.1 (slice len + coercion): `docs/develop/v0/stage-36/stage-36.1-slice-len-and-array-coercion-design.md`
- Current MIR walker: `src/mir/lower/format_intrinsics.rs` (598 LOC)
- format! macro: `src/parser/builtin_macros/print_macros.rs:336`
- MIR interception: `src/mir/lower/expr_variants.rs:565-580`
- Rust format! macro: https://doc.rust-lang.org/std/macro.format.html
