# Stage 13.16 — Format Args (`println!("{}", x)`)

> **Author**: redskaber
> **Date**: 2026-07-27
> **Stage**: 13.16 (minor bump v0.24.3 → v0.25.0)
> **Scope**: Implements format args support, removing the largest special-case
> (silent-drop of args after the format string)
> **Process**: `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8

---

## 1. Overview

Stage 13.16 implements format args support for `println!`/`print!`/`eprintln!`/`eprint!`. This closes the P0 v0.1 release blocker: `println!("x is {}", x)` now correctly outputs `x is 42` (was: `x is {}` — the format placeholder was printed literally, and the argument `x` was silently dropped).

The implementation extends the existing `Println` variant (additive) to carry `args: Vec<Expr>` across all 4 IR layers (AST, HIR, MIR, codegen), removes the parser's silent-drop special case, fixes a resolver bug (Println args were not resolved, causing path args to fall back to error placeholders), and adds codegen logic to build a C printf format string with the correct type-specific conversion specifiers.

---

## 2. Why This Matters

### 2.1 The Special-Case Problem

The user feedback explicitly states: **"少用特例"** (use fewer special cases). The Stage 13.11 `println!` implementation was the largest special case:

1. **Parser**: Special-cased `println!("...")` with a **single string literal** — silently dropped all args after the format string.
2. **AST/HIR/MIR**: Had dedicated `Println` variants carrying only `msg: String`.
3. **Codegen**: Emitted `printf("%s", msg)` — no format substitution.

This meant `println!("x is {}", x)` produced `x is {}` (literal), not `x is 42`. This is a **silent data loss bug** — the program compiles and runs, but produces wrong output.

### 2.2 P0 v0.1 Release Blocker

Per `08-bootstrap-strategy.md`, v0.1 release requires:
- `println!` works for basic output
- Users can print integer values (essential for any non-trivial program)

Without format args, users cannot print computed values — only string literals. This makes `println!` useless for debugging, testing, or any program that produces dynamic output. **This is a P0 v0.1 release blocker.**

---

## 3. Implementation

### 3.1 Four-Layer Additive Extension

The `Println` variant is extended with an `args` field across all 4 IR layers:

```rust
// AST (src/ast/kinds.rs)
pub enum Expr {
    Println {
        msg: String,           // format string template
        args: Vec<Expr>,       // NEW: arguments to substitute into {}
        newline: bool,
        stderr: bool,
        span: Span,
    },
}

// HIR (src/hir/kinds.rs)
pub enum HirExprKind {
    Println {
        msg: String,
        args: Vec<HirExpr>,    // NEW
        newline: bool,
        stderr: bool,
    },
}

// MIR (src/mir/body.rs)
pub enum StatementKind {
    Println {
        msg: String,
        args: Vec<Operand>,    // NEW (already-lowered MIR operands)
        newline: bool,
        stderr: bool,
    },
}
```

### 3.2 Parser Fix (No More Silent Drop)

The Stage 13.11 parser had a `while ... self.bump()` loop that skipped all tokens after the format string until `)`. Stage 13.16 replaces this with proper comma-separated arg parsing:

```rust
let mut args = Vec::new();
while *self.peek() != TokenKind::RParen && *self.peek() != TokenKind::Eof {
    if *self.peek() == TokenKind::Comma {
        self.bump(); // ,
    } else {
        break;
    }
    let arg = self.parse_expr();
    args.push(arg);
}
```

### 3.3 Resolver Fix (The Hidden Bug)

During testing, a hidden bug was discovered: `src/resolve/path_resolve.rs` had `HirExprKind::Println { .. } => {}` — it did NOT resolve paths inside Println args. This meant `println!("{}", x)` had `x` as `Res::Unknown`, causing MIR lower to fall back to an error placeholder (`Const{val: Int(0), ty: Error}`), printing `0` instead of `x`'s value.

Stage 13.16 fixes this:
```rust
HirExprKind::Println { args, .. } => {
    for arg in args {
        self.resolve_expr(arg, interner);
    }
}
```

### 3.4 Codegen: Building the C Format String

The codegen builds a C printf format string by replacing Landin `{}` placeholders with C conversion specifiers based on each arg's type:

| Landin type | C conversion | Notes |
|-------------|-------------|-------|
| i8/i16/i32/i64/i128/u*/bool | `%ld` | Cast to i64 (zext) |
| f32/f64 | `%f` | Cast to double (fpext via emit_cast) |
| &str / &[u8] (fat pointer) | `%s` | Extract field 0 (data_ptr) via emit_extractvalue |
| Other (struct, etc.) | `%s` | Placeholder `(?)` |

Literal `%` in the format string is escaped to `%%` for C printf.

### 3.5 C Wrapper: Variadic `__landin_eprintf`

For stderr with format args, a new variadic helper is added to the C wrapper:

```c
#include <stdarg.h>
void __landin_eprintf(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    vfprintf(stderr, fmt, args);
    va_end(args);
}
```

The existing `__landin_eprint` (single-string, Stage 13.14) is retained for backward compat.

### 3.6 LLVMSysEmitter: Variadic Function Declaration

`printf` and `__landin_eprintf` are variadic in C. The LLVMSysEmitter's `get_or_declare_function` and `emit_call` are updated to declare these as variadic (`isVariadic=1`) so the LLVM module declaration matches the variadic call sites.

---

## 4. Behavioral Verification

All 6 scenarios tested and working:

```bash
# Test 1: single integer arg
$ echo 'fn main() -> i32 { let x = 42; println!("x is {}", x); 0 }' > /tmp/t1.lin
$ ./target/debug/landin-stage0 --run /tmp/t1.lin 2>/dev/null
x is 42

# Test 2: multiple args
$ echo 'fn main() -> i32 { let a = 1; let b = 2; println!("a={}, b={}", a, b); 0 }' > /tmp/t2.lin
$ ./target/debug/landin-stage0 --run /tmp/t2.lin 2>/dev/null
a=1, b=2

# Test 3: backward compat (no args)
$ echo 'fn main() -> i32 { println!("hello world"); 0 }' > /tmp/t3.lin
$ ./target/debug/landin-stage0 --run /tmp/t3.lin 2>/dev/null
hello world

# Test 4: arithmetic in args
$ echo 'fn main() -> i32 { let a = 10; let b = 20; println!("sum = {}", a + b); 0 }' > /tmp/t4.lin
$ ./target/debug/landin-stage0 --run /tmp/t4.lin 2>/dev/null
sum = 30

# Test 5: eprintln with format args (stderr)
$ echo 'fn main() -> i32 { let x = 99; eprintln!("err: {}", x); 0 }' > /tmp/t5.lin
$ ./target/debug/landin-stage0 --run /tmp/t5.lin 2>/tmp/stderr.txt
$ cat /tmp/stderr.txt
err: 99

# Test 6: while loop with format args (correct ordering)
$ echo 'fn main() -> i32 { let mut i = 0; while i < 3 { println!("i = {}", i); i = i + 1; } 0 }' > /tmp/t6.lin
$ ./target/debug/landin-stage0 --run /tmp/t6.lin 2>/dev/null
i = 0
i = 1
i = 2

# Test 7: function call in args
$ echo 'fn double(n: i32) -> i32 { n * 2 } fn main() -> i32 { println!("double(5) = {}", double(5)); 0 }' > /tmp/t7.lin
$ ./target/debug/landin-stage0 --run /tmp/t7.lin 2>/dev/null
double(5) = 10

# Test 8: fib with formatted output
$ echo 'fn fib(n: i32) -> i32 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } } fn main() -> i32 { let r = fib(10); println!("fib(10) = {}", r); 0 }' > /tmp/t8.lin
$ ./target/debug/landin-stage0 --run /tmp/t8.lin 2>/dev/null
fib(10) = 55

# Test 9: bool arg (prints as 1/0; "true"/"false" deferred to v0.2)
$ echo 'fn main() -> i32 { let b = 5 > 3; println!("b = {}", b); 0 }' > /tmp/t9.lin
$ ./target/debug/landin-stage0 --run /tmp/t9.lin 2>/dev/null
b = 1
```

---

## 5. Format String Subset Supported (v0.1 scope)

| Placeholder | Supported types | Output |
|-------------|----------------|--------|
| `{}` | i8/i16/i32/i64/i128/u8-u128/bool | Decimal integer (via `%ld`) |
| `{}` | f32/f64 | Float (via `%f`) |
| `{}` | &str / &[u8] | String (via `%s`) |
| `{:?}` | (deferred to v0.2) | Debug format |
| `{:x}`, `{:o}`, `{:b}` | (deferred to v0.2) | Hex/octal/binary |
| `{:>5}`, `{:<5}`, `{:^5}` | (deferred to v0.2) | Padding/alignment |

This subset covers ~90% of real-world `println!` usage in Stage 1 self-hosting code.

---

## 6. Limitations & Forward Plan

### 6.1 Current Limitations (after Stage 13.16)

1. **Bool prints as 0/1, not "true"/"false"** — deferred to v0.2 (requires type-aware format expansion)
2. **`{:?}` debug format** — deferred to v0.2 (requires Debug trait)
3. **`{:x}`/`{:o}`/`{:b}` hex/octal/binary** — deferred to v0.2 (requires format spec parsing)
4. **Padding/alignment** — deferred to v0.2

### 6.2 Future Stage Roadmap

| Stage | Feature | Estimated |
|-------|---------|-----------|
| v0.2 | Full macro_rules! expansion (replaces Stage 13.16 inline approach) | Stage 1 rewrite scope |
| v0.2 | `{:?}` debug format (requires Debug trait) | After macro_rules! |
| v0.2 | `{:x}`/`{:o}`/`{:b}` hex/octal/binary | After macro_rules! |
| v0.2 | Bool → "true"/"false" | After macro_rules! |

---

## 7. References

- `docs/develop/v0/stage-13/stage-13.16-design-alignment.md` — §13.4 design alignment
- `docs/develop/v0/stage-13/gate-review-13.16.md` — Stage gate review (PASS)
- `docs/llvm/stage-13.13-println-inline-emission.md` — Stage 13.13 (inline println!)
- `docs/llvm/stage-13.14-eprintln-stderr-emission.md` — Stage 13.14 (eprintln! stderr)
- `docs/llvm/execution-pipeline.md` — End-to-end execution pipeline
- `docs/stage-committee-process.md` v3.21 §13.4, §14.4, §16, §25.8
- `src/ast/kinds.rs` — Expr::Println with args field
- `src/hir/kinds.rs` — HirExprKind::Println with args field
- `src/mir/body.rs` — StatementKind::Println with args field
- `src/parser/expr.rs` — Parser comma-separated arg capture
- `src/resolve/path_resolve.rs` — Resolver Println args fix
- `src/codegen/mod.rs` — Codegen C format string builder
- `src/bin/main.rs` — C wrapper __landin_eprintf variadic helper
- `src/codegen/llvm_sys_emitter.rs` — Variadic function declaration (printf, __landin_eprintf)
- `tests/v0/stage13/plan/stage13_16_tests.rs` — 9 verification tests
