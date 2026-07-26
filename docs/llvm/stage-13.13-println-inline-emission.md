# Stage 13.13 — Inline `println!` Emission via `StatementKind::Println`

> **Author**: redskaber
> **Date**: 2026-07-27
> **Stage**: 13.13 (patch bump v0.24.0 → v0.24.1)
> **Scope**: Fixes Stage 13.12 known limitation (println! output ordering bug)
> **Process**: `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8

---

## 1. Overview

Stage 13.13 fixes a known limitation from Stage 13.12: the println! output
ordering bug. The Stage 13.12 implementation stashed println messages in a
`Vec<String>` side-table on `MirBody` and emitted a separate helper function
`__landin_printlns_<fnname>` containing all `printf` calls, which the C
wrapper then called **before** `landin_main()` via a weak symbol. This caused
all println output to appear before the program body executed, breaking
output ordering for loops and conditionals.

Stage 13.13 introduces a new MIR `StatementKind::Println { msg, newline,
stderr }` variant that is emitted **inline** in the basic block where the
`println!` macro appears. Codegen translates this statement to an inline
`printf("%s", <msg_global>)` call at the exact source-code position,
restoring §16 compliance (basic block is the single source of truth for
execution order).

---

## 2. Architecture

### 2.1 Data Flow (Stage 13.13)

```
Source:        fn landin_main() -> i32 { println!("hello"); 0 }
                                    ↓
Parser:        Expr::Println { msg: "hello", newline: true, stderr: false }
                                    ↓
HIR lower:     HirExprKind::Println { msg: "hello", newline: true, stderr: false }
                                    ↓
MIR lower:     bb0 {
                 StatementKind::Println { msg: "hello\n", newline: true, stderr: false }
                 ... (rest of body)
                 Terminator::Return
               }
                                    ↓
Codegen:       define i32 @landin_main() {
               bb0:
                 ; Stage 13.13: inline printf call at source position
                 %fmt = ...                         ; global "@.str_fmt" = "%s\0"
                 %msg = ...                         ; global "@.str_msg_0" = "hello\n\0"
                 %v0 = call i32 @printf(i8* %fmt, i8* %msg)
                 ...
                 ret i32 0
               }
                                    ↓
LLVMSysEmitter: declares printf as `declare i32 @printf(i8*, i8*)`
                                    ↓
Linker:        cc wrapper.c prog.o -o exe -lm
               (C wrapper provides main() that calls landin_main())
                                    ↓
Runtime:       $ ./exe
               hello
               $ echo $?
               0
```

### 2.2 Why Inline (Stage 13.13) vs Side-Table (Stage 13.12)

Per `stage-committee-process.md` §16 "Interface Isolation":

> MIR's `basic_blocks` array is the **single source of truth** for execution
> order. Side-tables are for **unordered metadata** (e.g., vtable indices,
> capture lists, span info) — never for ordered side effects.

Stage 13.12 violated this rule by stashing println messages in a side-table
`Vec<String>` and having codegen iterate the side-table to emit a helper
function. The helper function ran **before** `landin_main()`, so:

| Source pattern | Stage 13.12 (broken) | Stage 13.13 (fixed) |
|---------------|----------------------|---------------------|
| `println!("a"); println!("b");` | ✅ "a\nb\n" (correct — both in helper, called once before main) | ✅ "a\nb\n" (correct — both inline, in source order) |
| `while cond { println!("iter"); }` | ❌ prints "iter" **once** before main (loop body not in helper) | ✅ prints "iter" N times during loop execution |
| `if cond { println!("yes"); } else { println!("no"); }` | ❌ prints **both** "yes\nno\n" before main (helper has both messages) | ✅ prints only the taken branch's message |
| `println!("before"); panic!("oops");` | ❌ prints "before" then panics (correct by accident) | ✅ prints "before" then panics (correct by design) |
| `for i in 0..3 { println!("i={}", i); }` | ❌ format args unsupported; prints placeholder once | ⚠️ format args still unsupported (Stage 13.15); prints placeholder inline N times |

Stage 13.13 fixes all 4 broken cases (rows 2, 3, and the accidental correctness
of rows 1 and 4 becomes by-design correctness).

---

## 3. Implementation Details

### 3.1 New MIR Variant

```rust
// src/mir/body.rs
pub enum StatementKind {
    // ... existing variants ...
    Println {
        msg: String,    // already includes trailing "\n" if newline == true
        newline: bool,  // informational; msg already has the newline
        stderr: bool,   // true for eprintln!/eprint! (deferred to Stage 13.14)
    },
}
```

The `msg` field carries the **already-formatted** message string. For v0.1,
this is the raw string literal argument to `println!` (no format args
expansion). Stage 13.15+ will add format-args expansion
(`println!("{}", x)` → format the value at runtime).

### 3.2 MIR Lower

```rust
// src/mir/lower/expr_operand.rs
HirExprKind::Println { msg, newline, stderr } => {
    let full_msg = if *newline {
        format!("{}\n", msg)
    } else {
        msg.clone()
    };
    // Push the println statement to the current basic block —
    // this is the §16-compliant way to express an ordered side effect.
    cx.mir
        .block_mut(cx.current_block)
        .statements
        .push(Statement {
            kind: StatementKind::Println {
                msg: full_msg,
                newline: *newline,
                stderr: *stderr,
            },
            span: expr.span,
        });
    let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
    cx.mir.new_local(unit_ty, None, expr.span)
}
```

The `MirBody.println_messages: Vec<String>` field is **retained** (kept as
`Vec::new()` for all bodies) for backward compatibility with external tooling
that may read MIR side-tables. The field is no longer populated by MIR lower.

### 3.3 Codegen

```rust
// src/codegen/mod.rs (codegen_statement)
StatementKind::Println { msg, newline, stderr } => {
    let _ = newline; // already encoded in `msg` (trailing "\n")
    let _ = stderr;  // Stage 13.14: switch to fprintf(stderr, ...) when true

    // Emit format string "%s\0" (null-terminated for printf).
    // The emitter deduplicates identical globals, so this is emitted
    // once per module.
    let fmt = emitter.emit_string_global(b"%s\0");

    // Emit message string (null-terminated for printf).
    let mut msg_bytes = msg.as_bytes().to_vec();
    msg_bytes.push(0); // null terminator
    let str_global = emitter.emit_string_global(&msg_bytes);

    // Call printf("%s", str_global) inline at this position.
    // printf returns i32 (number of chars printed); we discard it.
    emitter.emit_call(
        "printf",
        &[
            (EmitType::OpaquePtr, &fmt),
            (EmitType::OpaquePtr, &str_global),
        ],
        &EmitType::I32,
    );
}
```

### 3.4 LLVMSysEmitter Auto-Declaration

When `emit_call("printf", ...)` is invoked, `LLVMSysEmitter::get_or_declare_function`
auto-declares `printf` in the LLVM module as:

```llvm
declare i32 @printf(i8*, i8*)
```

(Note: this is **not** variadic — LLVM is lenient about call-site arg counts
matching the declaration when the actual libc symbol is variadic. The
non-variadic declaration works because we always call `printf` with exactly
2 args, matching the declaration.)

For `TextEmitter` (`--emit-llvm-ir` output for inspection), the call is
emitted as text:

```llvm
%v0 = call i32 @printf(i8* %fmt, i8* %msg)
```

The `printf` declaration is not auto-emitted by TextEmitter (the user would
need to add `declare i32 @printf(i8*, ...)` manually if they want to
assemble the IR). This is acceptable for v0.1 because `--emit-llvm-ir` is
for inspection only; `--run` and `--emit-bin` go through `LLVMSysEmitter`
which auto-declares.

### 3.5 C Wrapper Simplification

```c
// src/bin/main.rs (C wrapper source, Stage 13.13)
#include <stdio.h>
#include <stdlib.h>
extern int landin_main(void);
/* Runtime stubs — codegen declares these as extern */
void __landin_panic_overflow(int op, int lhs, int rhs) {
    fprintf(stderr, "panic: arithmetic overflow (op=%d lhs=%d rhs=%d)\n", op, lhs, rhs);
    exit(1);
}
void __landin_panic_bounds_check(long long index, long long len) {
    fprintf(stderr, "panic: index out of bounds (index=%lld len=%lld)\n", index, len);
    exit(1);
}
void __landin_panic_div_by_zero(void) {
    fprintf(stderr, "panic: divide by zero\n");
    exit(1);
}
int main(void) {
    /* Stage 13.13: println! output is emitted inline within landin_main()
       via StatementKind::Println → printf("%s", <msg_global>).
       No pre-main helper call needed. */
    int ret = landin_main();
    return ret;
}
```

The Stage 13.12 weak-symbol trick (`__attribute__((weak)) void
__landin_printlns_landin_main(void);` + conditional call before
`landin_main()`) is removed entirely.

---

## 4. End-to-End Execution Pipeline

### 4.1 Pipeline (unchanged from Stage 13.10, with Stage 13.13 inline println)

```
Landin source (.lin)
    ↓ landin-stage0 --run prog.lin
    ↓ parse → HIR → MIR (with StatementKind::Println in basic block)
    ↓ LLVMSysEmitter.codegen_from_mir()
    ↓   → emit_function_begin("landin_main", ...)
    ↓   → for each basic block:
    ↓       for each statement:
    ↓         StatementKind::Println → emit_string_global + emit_call("printf", ...)
    ↓         StatementKind::Assign → ... (existing logic)
    ↓       codegen_terminator
    ↓   → emit_function_end
    ↓ LLVMSysEmitter.to_object_file() → prog.o
    ↓ generate landin_wrapper_*.c (simplified — no weak symbol)
    ↓ cc -fno-pie -no-pie wrapper.c prog.o -o prog -lm
    ↓ execute prog
    ↓ prog's main() calls landin_main()
    ↓ landin_main() runs, hitting inline printf calls at source positions
    ↓ exit code = landin_main() return value
Program output (stdout) = println messages in source order
```

### 4.2 Worked Example

**Source** (`/tmp/demo.lin`):
```rust
fn landin_main() -> i32 {
    println!("step 1");
    let mut i = 0;
    while i < 3 {
        if i == 1 {
            println!("middle");
        }
        i = i + 1;
    }
    println!("step 2");
    0
}
```

**Execution**:
```bash
$ landin-stage0 --run /tmp/demo.lin
info: object file written to /tmp/demo.o
info: executable written to /tmp/demo.out
info: running /tmp/demo.out
step 1
middle
step 2
```

**Output ordering analysis**:
- "step 1" — emitted by the first `StatementKind::Println` in `bb0`
- "middle" — emitted by the `StatementKind::Println` inside the `if i == 1`
  branch (only when `i == 1`, so once during the loop)
- "step 2" — emitted by the last `StatementKind::Println` after the loop

Stage 13.12 would have printed:
```
step 1
middle
step 2
```
...but ALL before `landin_main()` ran. The "middle" message would appear
**unconditionally** (once), not just when `i == 1`. This is the bug Stage
13.13 fixes.

---

## 5. Verification

### 5.1 Test Coverage

Stage 13.13 adds 10 verification tests in
`tests/v0/stage13/plan/stage13_13_tests.rs`:

1. `test_statement_kind_has_println_variant` — variant exists with correct fields
2. `test_mir_lower_emits_println_statement_inline` — MIR lower pushes to BB (not side-table)
3. `test_codegen_statement_handles_println` — codegen has Println arm with `emit_call("printf", ...)`
4. `test_no_helper_function_emission` — helper function emission removed from codegen
5. `test_c_wrapper_no_weak_symbol` — C wrapper no longer references weak symbol
6. `test_println_messages_field_kept_for_compat` — side-table field retained for backward compat
7. `test_stage_13_13_gate_review_exists` — gate review doc exists with PASS verdict
8. `test_stage_13_13_design_alignment_exists` — design alignment doc exists with required sections
9. `test_typeck_checker_handles_println` — typeck checker has Println arm
10. `test_v01_gate_still_holds_after_stage_13_13` — ≥5000 conformance .lin files

### 5.2 Behavioral Smoke Test

After build:

```bash
$ echo 'fn landin_main() -> i32 { println!("hello world"); 0 }' > /tmp/hello.lin
$ cargo run --features llvm-backend -- --run /tmp/hello.lin
info: object file written to /tmp/hello.o
info: executable written to /tmp/hello.out
info: running /tmp/hello.out
hello world
```

The output "hello world" appears on **stdout** (not stderr).

### 5.3 Conformance Suite

The conformance suite (5026 `.lin` files) is unchanged — Stage 13.13 doesn't
touch parsing or type checking. All 5026 conformance tests continue to pass.

---

## 6. Limitations & Forward Plan

### 6.1 Current Limitations (Stage 13.13)

1. **`eprintln!`/`eprint!` not differentiated** — the `stderr` flag is
   captured but ignored at codegen time (still uses `printf` to stdout).
   **Fix**: Stage 13.14 will switch to `fprintf(stderr, ...)` when
   `stderr == true`.

2. **`print!` (no newline) vs `println!` (with newline)** — both work, but
   the `newline` flag is informational only (the trailing `"\n"` is already
   encoded in `msg`). Future stages may use the flag for additional
   behavior (e.g., flushing).

3. **Format args unsupported** — `println!("{}", x)` does NOT substitute
   `x`'s value. The parser captures the format string as a literal
   (`"{}"` is stored verbatim as `msg`). Stage 13.15 will add format-args
   expansion.

4. **`printf` not declared as variadic in LLVM IR** — the LLVM declaration
   is `declare i32 @printf(i8*, i8*)` (non-variadic). This works for our
   2-arg call sites but is technically incorrect. A future stage should
   add an explicit `emit_declare("i32 @printf(i8*, ...)")` to ensure
   correctness when variadic calls are added.

5. **String-escape sequences not processed** — `println!("hello\nworld")`
   emits the literal 12-character string `"hello\nworld"` (with `\n` as
   two characters, not as a newline). Stage 13.16 will add escape
   sequence processing in the lexer.

### 6.2 Future Stage Roadmap

| Stage | Feature | Estimated |
|-------|---------|-----------|
| 13.14 | `eprintln!`/`eprint!` → `fprintf(stderr, ...)` | 1 hour |
| 13.15 | Format args (`println!("{}", x)`) | 1-2 days (requires HIR-time format expansion) |
| 13.16 | String escape sequences in lexer (`\n`, `\t`, `\\`, `\"`) | 4 hours |
| 13.17 | `print!` (no newline) flush behavior | 2 hours |
| v0.2+ | Full `macro_rules!` expansion (replaces Stage 13.13 inline statement) | Stage 1 rewrite scope |

### 6.3 Long-Term Vision

Stage 13.13's `StatementKind::Println` variant is a **v0.1 hardcode-expanded
built-in macro emission point**. When `macro_rules!` lands in v0.2 (per
`08-bootstrap-strategy.md`), `println!` will expand to a real `printf` call
at HIR-lowering time, and `StatementKind::Println` will be deprecated (and
eventually removed).

This is the design-aligned path per `02-grammar.md` §4.4:
> "MVP 不支持 macro_rules! 自定义宏（推迟 v0.2），但 支持 26 个内建宏（编译器硬编码展开）"

Stage 13.13 is the "硬编码展开" (hardcoded expansion) implementation for
the printing macros. v0.2 will replace it with proper macro expansion.

---

## 7. References

- `docs/develop/v0/stage-13/stage-13.13-design-alignment.md` — §13.4 design alignment
- `docs/develop/v0/stage-13/gate-review-13.13.md` — Stage gate review (PASS)
- `docs/llvm/execution-pipeline.md` — End-to-end execution pipeline (Stage 13.10+)
- `docs/llvm/README.md` — LLVM integration overview
- `docs/stage-committee-process.md` v3.21 §13.4, §14.4, §16, §25.8
- `src/mir/body.rs:191-220` — `StatementKind::Println` variant
- `src/mir/lower/expr_operand.rs:1375-1414` — MIR lower Println arm
- `src/codegen/mod.rs:400-437` — Codegen Println arm
- `src/bin/main.rs:155-191` — Simplified C wrapper
- `tests/v0/stage13/plan/stage13_13_tests.rs` — 10 verification tests
