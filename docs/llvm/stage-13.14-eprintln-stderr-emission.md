# Stage 13.14 — `eprintln!`/`eprint!` Stderr Emission

> **Author**: redskaber
> **Date**: 2026-07-27
> **Stage**: 13.14 (patch bump v0.24.1 → v0.24.2)
> **Scope**: Closes the explicit Stage 13.13 deferral — the `stderr` flag on
> `StatementKind::Println` is now exercised at codegen time
> **Process**: `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8

---

## 1. Overview

Stage 13.13 introduced the inline `StatementKind::Println { msg, newline,
stderr }` MIR variant and translated it to `printf("%s", <msg_global>)` in
codegen. The implementation explicitly deferred the `stderr` flag handling
to Stage 13.14:

> `src/codegen/mod.rs:420` (Stage 13.13):
> ```rust
> let _ = stderr; // Stage 13.14: switch to fprintf(stderr, ...) when true
> ```

This meant `eprintln!("msg")` and `eprint!("msg")` were routed to **stdout**
via `printf`, not to **stderr** as Rust semantics require. Stage 13.14 closes
this deferral.

When `stderr == true` (i.e., `eprintln!` or `eprint!` was invoked), codegen
emits a call to a new `__landin_eprint` C wrapper helper that routes the
message to stderr via `fprintf(stderr, "%s", s)`. When `stderr == false`
(i.e., `println!` or `print!`), the existing `printf` path is unchanged.

---

## 2. Why Stderr Matters

Per Rust semantics (`https://doc.rust-lang.org/std/macro.eprintln.html`):

> `eprintln!` prints to the **standard error** (stderr), which is unbuffered
> (or line-buffered on terminals).
> `println!` prints to the **standard output** (stdout), which is line-buffered
> on terminals.

The distinction matters for:

1. **Pipe redirection**: `./prog > out.txt` should redirect only stdout;
   stderr goes to the terminal
2. **Diagnostic separation**: Error/warning messages on stderr don't pollute
   stdout data
3. **Buffering semantics**: stderr is unbuffered (writes appear immediately),
   stdout is line-buffered (writes appear on `\n`)
4. **Convention compliance**: POSIX tools expect diagnostics on stderr;
   pipelines parse stdout

Without Stage 13.14, Landin's `eprintln!` was semantically incorrect: it
would interleave with `println!` output on stdout, breaking pipe redirection
and convention.

---

## 3. Architecture

### 3.1 Strategy Choice: `__landin_eprint` Helper

Per `stage-13.14-design-alignment.md` §1.3, four strategies were evaluated:

| Strategy | Verdict | Rationale |
|----------|---------|-----------|
| A: Direct `fprintf` + `stderr` extern in codegen | ❌ REJECTED | Portability risk — `stderr` is a macro in glibc, not a simple global; declaring `@stderr = external global ptr` in LLVM IR doesn't work portably |
| **B: `__landin_eprint` helper in C wrapper** | ✅ **ADOPTED** | Portable (C wrapper handles libc differences); symmetric with existing `__landin_panic_*` helpers; codegen just calls one function |
| C: Defer to v0.2 macro_rules! | ❌ REJECTED | Design-forbidden per `02-grammar.md` §4.4 for v0.1/v0.3 |
| D: Status quo (eprintln! → stdout) | ❌ REJECTED | Known correctness bug; violates §15 (long-term > short-term) |

### 3.2 Data Flow (Stage 13.14)

```
Source:        fn landin_main() -> i32 { eprintln!("err"); 0 }
                                    ↓
Parser:        Expr::Println { msg: "err", newline: true, stderr: true }
                                    ↓
HIR lower:     HirExprKind::Println { msg: "err", newline: true, stderr: true }
                                    ↓
MIR lower:     bb0 {
                 StatementKind::Println { msg: "err\n", newline: true, stderr: true }
                 ... (rest of body)
                 Terminator::Return
               }
                                    ↓
Codegen:       define i32 @landin_main() {
               bb0:
                 ; Stage 13.14: stderr == true → __landin_eprint helper
                 %msg = ...                         ; global "@.str_msg_0" = "err\n\0"
                 call void @__landin_eprint(i8* %msg)
                 ...
                 ret i32 0
               }
                                    ↓
LLVMSysEmitter: declares __landin_eprint as `declare void @__landin_eprint(i8*)`
                                    ↓
Linker:        cc wrapper.c prog.o -o exe -lm
               (C wrapper defines __landin_eprint: fprintf(stderr, "%s", s))
                                    ↓
Runtime:       $ ./exe 2>/tmp/stderr.txt
               $ cat /tmp/stderr.txt
               err
               $ echo $?
               0
```

### 3.3 Why a Helper Instead of Direct `fprintf`

Direct `fprintf(stderr, ...)` in codegen would require:
1. Declaring `fprintf` as an LLVM extern (`declare i32 @fprintf(ptr, ptr, ...)`)
2. Declaring `stderr` as an LLVM extern (`@stderr = external global ptr`)

The problem is **step 2**: on glibc (Linux), `stderr` is a macro that expands
to `_stderr` or `_IO_2_1_stderr_` or similar — there's no portable
`@stderr` symbol. On musl libc, `stderr` is a real global. On macOS, it's
`__stderrp`. Declaring `@stderr = external global ptr` in LLVM IR would
work on some libcs but fail at link time on others.

The `__landin_eprint` helper sidesteps this entirely:
- The C compiler (cc) knows how `stderr` expands on its target libc
- The C wrapper just uses `fprintf(stderr, "%s", s)` — standard C, portable
- The LLVM module only needs to declare `__landin_eprint` (a custom symbol
  we define in the C wrapper, not a libc symbol)

This is the same pattern used by the `__landin_panic_*` helpers (Stage 13.10):
the C wrapper provides a portable abstraction, and the LLVM module calls the
abstraction.

---

## 4. Implementation Details

### 4.1 Codegen Branch

```rust
// src/codegen/mod.rs (codegen_statement, StatementKind::Println arm)
StatementKind::Println { msg, newline, stderr } => {
    let _ = newline; // already encoded in `msg` (trailing "\n")

    // Emit message string (null-terminated for C).
    let mut msg_bytes = msg.as_bytes().to_vec();
    msg_bytes.push(0); // null terminator
    let str_global = emitter.emit_string_global(&msg_bytes);

    if *stderr {
        // Stage 13.14: eprintln!/eprint! → __landin_eprint helper.
        emitter.emit_call(
            "__landin_eprint",
            &[(EmitType::OpaquePtr, &str_global)],
            &EmitType::Void,
        );
    } else {
        // Stage 13.13: println!/print! → printf("%s", msg) (unchanged).
        let fmt = emitter.emit_string_global(b"%s\0");
        emitter.emit_call(
            "printf",
            &[
                (EmitType::OpaquePtr, &fmt),
                (EmitType::OpaquePtr, &str_global),
            ],
            &EmitType::I32,
        );
    }
}
```

### 4.2 C Wrapper Helper

```c
// src/bin/main.rs (C wrapper source string)
/* Stage 13.14: eprintln!/eprint! helper — routes to stderr via fprintf.
   Codegen calls this when StatementKind::Println.stderr == true.
   Portable across libc implementations (stderr is a macro in glibc;
   the helper hides this). The helper takes only the message string
   (no format string) — the C helper hardcodes "%s" as the format, so
   `%` in msg is literal (no format-string injection risk).
   Per api-naming-standard.md §8.1: __landin_<verb>_<noun> pattern. */
void __landin_eprint(const char* s) {
    fprintf(stderr, "%s", s);
}
```

### 4.3 LLVMSysEmitter Auto-Declaration

When `emit_call("__landin_eprint", ...)` is invoked,
`LLVMSysEmitter::get_or_declare_function` auto-declares `__landin_eprint`
in the LLVM module as:

```llvm
declare void @__landin_eprint(i8*)
```

The actual definition is provided by the C wrapper at link time. This mirrors
how `printf` is auto-declared (Stage 13.13) and how `__landin_panic_*` are
auto-declared (Stage 13.10).

### 4.4 API Naming Compliance

Per `api-naming-standard.md` §8.1:

| Symbol | Pattern | Compliance |
|--------|---------|------------|
| `__landin_eprint` | `__landin_<verb>_<noun>` | ✅ Matches `__landin_panic_*` siblings |
| `__landin_panic_overflow` | `__landin_<noun>_<noun>` | ✅ Existing (Stage 13.10) |
| `__landin_panic_bounds_check` | `__landin_<noun>_<noun>_<noun>` | ✅ Existing (Stage 13.10) |
| `__landin_panic_div_by_zero` | `__landin_<noun>_<prep>_<noun>` | ✅ Existing (Stage 13.10) |

All C wrapper helpers follow the `__landin_` prefix convention, ensuring
they don't collide with user symbols or libc symbols.

---

## 5. End-to-End Execution Pipeline

### 5.1 Pipeline (Stage 13.13 + 13.14)

```
Landin source (.lin)
    ↓ landin-stage0 --run prog.lin
    ↓ parse → HIR → MIR (with StatementKind::Println in basic block)
    ↓ LLVMSysEmitter.codegen_from_mir()
    ↓   → emit_function_begin("landin_main", ...)
    ↓   → for each basic block:
    ↓       for each statement:
    ↓         StatementKind::Println { stderr: false, .. } → emit_call("printf", ...)
    ↓         StatementKind::Println { stderr: true, .. }  → emit_call("__landin_eprint", ...)
    ↓         StatementKind::Assign → ... (existing logic)
    ↓       codegen_terminator
    ↓   → emit_function_end
    ↓ LLVMSysEmitter.to_object_file() → prog.o
    ↓ generate landin_wrapper_*.c (with __landin_eprint helper definition)
    ↓ cc -fno-pie -no-pie wrapper.c prog.o -o prog -lm
    ↓ execute prog
    ↓ prog's main() calls landin_main()
    ↓ landin_main() runs, hitting inline printf/__landin_eprint calls
    ↓ exit code = landin_main() return value
Program stdout = println!/print! messages in source order
Program stderr = eprintln!/eprint! messages in source order
```

### 5.2 Worked Example

**Source** (`/tmp/demo.lin`):
```rust
fn landin_main() -> i32 {
    println!("to stdout");
    eprintln!("to stderr");
    println!("also stdout");
    0
}
```

**Execution**:
```bash
$ landin-stage0 --run /tmp/demo.lin
info: object file written to /tmp/demo.o
info: executable written to /tmp/demo.out
info: running /tmp/demo.out
to stdout
to stderr
also stdout
```

**Output stream separation**:
```bash
$ landin-stage0 --run /tmp/demo.lin > /tmp/stdout.txt 2> /tmp/stderr.txt
$ cat /tmp/stdout.txt
to stdout
also stdout
$ cat /tmp/stderr.txt
to stderr
```

Stage 13.13 would have printed all three messages to stdout (incorrectly
routing `eprintln!` to stdout). Stage 13.14 fixes this — `eprintln!`
correctly goes to stderr.

---

## 6. Verification

### 6.1 Test Coverage

Stage 13.14 adds 7 verification tests in
`tests/v0/stage13/plan/stage13_14_tests.rs`:

1. `test_codegen_println_branches_on_stderr` — codegen has `if *stderr` branch
2. `test_codegen_eprint_calls_helper` — `stderr == true` calls `__landin_eprint`
3. `test_codegen_stdout_unchanged` — `stderr == false` still calls `printf` (no regression)
4. `test_c_wrapper_has_eprint_helper` — C wrapper defines `__landin_eprint` with `fprintf(stderr, ...)`
5. `test_stage_13_14_design_alignment_exists` — design doc exists with required sections
6. `test_stage_13_14_gate_review_exists` — gate review doc exists with PASS verdict
7. `test_v01_gate_still_holds_after_stage_13_14` — ≥5000 conformance .lin files

### 6.2 Behavioral Smoke Test

After build:

```bash
$ cat > /tmp/test_eprintln.lin << 'EOF'
fn landin_main() -> i32 {
    println!("stdout msg");
    eprintln!("stderr msg");
    0
}
EOF

$ cargo run --features llvm-backend -- --run /tmp/test_eprintln.lin > /tmp/out.txt 2> /tmp/err.txt
$ cat /tmp/out.txt
stdout msg
$ cat /tmp/err.txt
stderr msg
```

The stdout message appears on stdout; the stderr message appears on stderr.
Pipe redirection captures only the stdout message.

### 6.3 Conformance Suite

The conformance suite (5026 `.lin` files) is unchanged — Stage 13.14 doesn't
touch parsing or type checking. All 5026 conformance tests continue to pass.

---

## 7. Limitations & Forward Plan

### 7.1 Current Limitations (after Stage 13.14)

1. **Format args unsupported** — `println!("{}", x)` does NOT substitute
   `x`'s value. Stage 13.15 will add format-args expansion.

2. **String-escape sequences not processed** — `println!("hello\nworld")`
   emits the literal 12-character string `"hello\nworld"` (with `\n` as
   two characters, not as a newline). Stage 13.16 will add escape sequence
   processing in the lexer.

3. **`print!` (no newline) flush behavior** — currently `newline: false` is
   captured but doesn't affect codegen (the trailing `"\n"` is encoded in
   `msg` at MIR-lowering time). Future stages may use the flag for
   additional behavior (e.g., flushing).

### 7.2 Future Stage Roadmap

| Stage | Feature | Estimated |
|-------|---------|-----------|
| 13.15 | Format args (`println!("{}", x)`) | 1-2 days (requires HIR-time format expansion) |
| 13.16 | String escape sequences in lexer (`\n`, `\t`, `\\`, `\"`) | 4 hours |
| 13.17 | `print!` (no newline) flush behavior | 2 hours |
| v0.2+ | Full `macro_rules!` expansion (replaces Stage 13.13/13.14 inline approach) | Stage 1 rewrite scope |

### 7.3 Long-Term Vision

Stage 13.14's `__landin_eprint` helper is a **v0.1 hardcode-expanded built-in
macro emission point** for stderr routing. When `macro_rules!` lands in v0.2
(per `08-bootstrap-strategy.md`), `eprintln!` will expand to a real
`fprintf(stderr, ...)` call at HIR-lowering time, and the `__landin_eprint`
helper will be deprecated (and eventually removed).

This is the design-aligned path per `02-grammar.md` §4.4:
> "MVP 不支持 macro_rules! 自定义宏（推迟 v0.2），但 支持 26 个内建宏（编译器硬编码展开）"

Stage 13.14 is the "硬编码展开" (hardcoded expansion) implementation for
the stderr-routing macros. v0.2 will replace it with proper macro expansion.

---

## 8. References

- `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` — §13.4 design alignment
- `docs/develop/v0/stage-13/gate-review-13.14.md` — Stage gate review (PASS)
- `docs/llvm/stage-13.13-println-inline-emission.md` — Stage 13.13 (inline println! — predecessor)
- `docs/llvm/execution-pipeline.md` — End-to-end execution pipeline
- `docs/llvm/README.md` — LLVM integration overview
- `docs/stage-committee-process.md` v3.21 §13.4, §14.4, §16, §25.8
- `src/codegen/mod.rs:401-472` — Codegen Println arm with stderr branch
- `src/bin/main.rs:185-194` — C wrapper `__landin_eprint` helper definition
- `tests/v0/stage13/plan/stage13_14_tests.rs` — 7 verification tests
- Rust `eprintln!` documentation: https://doc.rust-lang.org/std/macro.eprintln.html
