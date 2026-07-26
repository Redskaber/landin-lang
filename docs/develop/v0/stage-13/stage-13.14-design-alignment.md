# Stage 13.14 — §13.4 Design Alignment: eprintln!/eprint! Stderr Emission

> **Author**: redskaber
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25.8)
> **Baseline**: v0.24.1 / 2310 rust tests + 5026 conformance (Stage 13.13 ✅ inline println!)
> **Version policy**: v0.24.1 → v0.24.2 (patch bump — completes Stage 13.13 deferral)
> **Status**: 🔄 Active — implements the `stderr` flag handling that Stage 13.13 explicitly deferred

---

## 1. Background & Problem Statement

### 1.1 Stage 13.13 Recap & Explicit Deferral

Stage 13.13 introduced the inline `StatementKind::Println { msg, newline, stderr }` MIR variant and translated it to `printf("%s", <msg_global>)` in codegen. The implementation explicitly deferred the `stderr` flag handling:

> `src/codegen/mod.rs:420` (Stage 13.13):
> ```rust
> let _ = stderr; // Stage 13.14: switch to fprintf(stderr, ...) when true
> ```

This means `eprintln!("msg")` and `eprint!("msg")` currently route to **stdout** via `printf`, not to **stderr** as the Rust semantics require. Stage 13.14 closes this deferral.

### 1.2 Why Stderr Matters

Per Rust semantics (`https://doc.rust-lang.org/std/macro.eprintln.html`):

> `eprintln!` prints to the **standard error** (stderr), which is unbuffered (or line-buffered on terminals).
> `println!` prints to the **standard output** (stdout), which is line-buffered on terminals.

The distinction matters for:
1. **Pipe redirection**: `./prog > out.txt` should redirect only stdout; stderr goes to the terminal
2. **Diagnostic separation**: Error/warning messages on stderr don't pollute stdout data
3. **Buffering semantics**: stderr is unbuffered (writes appear immediately), stdout is line-buffered (writes appear on `\n`)
4. **Convention compliance**: POSIX tools expect diagnostics on stderr; pipelines parse stdout

Without Stage 13.14, Landin's `eprintln!` is semantically incorrect: it would interleave with `println!` output on stdout, breaking pipe redirection and convention.

### 1.3 §15 Long-Term vs Short-Term Analysis

| Option | Long-term value | Short-term cost | Decision |
|--------|----------------|----------------|----------|
| A: Use `fprintf(stderr, ...)` directly in codegen | MEDIUM — direct libc call; matches C idiom | HIGH — requires declaring `stderr` (a macro in glibc, not a simple global) + `fprintf` as LLVM externs; non-portable across libc implementations | ❌ REJECTED (portability risk) |
| **B: Add `__landin_eprint` helper in C wrapper** | **HIGH** — portable (C wrapper handles libc differences); symmetric with existing `__landin_panic_*` helpers; codegen just calls one function | **LOW** — 1 line in C wrapper + 1 branch in codegen | ✅ **ADOPTED** |
| C: Defer to v0.2 macro_rules! | LOW — would also fix the issue but is design-forbidden for v0.1/v0.3 | HIGH — Stage 13.4a explicitly REJECTED macro_rules! for v0.1/v0.3 per design docs | ❌ REJECTED (design-forbidden per 02-grammar.md §4.4) |
| D: Keep status quo (eprintln! → stdout) | ZERO — leaves a known correctness bug | ZERO | ❌ REJECTED (per §15: long-term > short-term) |

**Conclusion**: Strategy B (`__landin_eprint` helper) is the right call:
- Minimal codegen change (1 branch on existing `stderr` flag)
- Portable (C wrapper handles libc differences)
- Symmetric with existing `__landin_panic_*` helpers (consistent API surface)
- Forward-compatible: when v0.2 macro_rules! lands, the helper can be deprecated

---

## 2. §13.4 Design Alignment Verification

Per `stage-committee-process.md` v3.21 §13.4 "阶段开始时的设计对齐", the following design docs were consulted:

### 2.1 Design Doc Survey

| Design doc | Relevant section | Alignment verdict |
|------------|------------------|-------------------|
| `02-grammar.md` §4.4 (line 421) | "MVP 不支持 macro_rules! 自定义宏（推迟 v0.2），但 支持 26 个内建宏（编译器硬编码展开）" | ✅ ALIGNED — Stage 13.14 keeps "硬编码展开" approach; doesn't introduce macro_rules! |
| `09-stdlib.md` | Mentions `eprintln!`/`eprint!` as built-in macros for I/O | ✅ ALIGNED — Stage 13.14 implements the stderr routing that 09-stdlib.md implies |
| `07-codegen.md` §8.1 | "Codegen translates MIR statements in source order" | ✅ ALIGNED — Stage 13.14 keeps inline emission, just branches on `stderr` flag |
| `13-stage1-feature-whitelist.md` §2.6 (line 152) | "禁止使用：macro_rules! 自定义宏（v0.2 才支持）" | ✅ ALIGNED — Stage 13.14 doesn't introduce macro_rules! |
| `06-mir.md` (StatementKind section) | Stage 13.13 already added `StatementKind::Println` (B4 gray-area, write-back pending) | ✅ ALIGNED — Stage 13.14 doesn't add new MIR variants; just exercises the existing `stderr` field |

### 2.2 Design-Deviation Classification

Per `stage-committee-process.md` §25.8 design-deviation taxonomy:

- **B1 (impl missing design field)**: NONE — Stage 13.14 doesn't touch any struct field
- **B2 (impl has non-design field)**: NONE
- **B3 (impl accepts design-forbidden input)**: NONE
- **B4 (impl introduces design-gray-area)**: NONE — Stage 13.14 uses the existing `stderr: bool` field (added in Stage 13.13); no new MIR surface

**Net deviation**: ZERO. Stage 13.14 is a pure implementation refinement of Stage 13.13's already-documented variant.

### 2.3 §14.4 Six Refactoring Criteria (J1-J6)

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Restores Rust semantics for `eprintln!`/`eprint!` (stderr, not stdout) |
| J2 Single responsibility | ✅ PASS | `__landin_eprint` helper carries one job: print to stderr |
| J3 Unidirectional data flow | ✅ PASS | MIR lower → MIR body → codegen → C wrapper helper → libc; all forward |
| J4 Compile-time expressiveness | ✅ PASS | No new types; just a branch on existing `bool` field |
| J5 Stage partition (≤5 src files) | ✅ PASS | 2 src files: codegen/mod.rs (1 branch), bin/main.rs (1 helper function) |
| J6 Scientific granularity | ✅ PASS | One bug fix, one branch, one helper — minimum viable change |

**§14.4 verdict**: 6/6 PASS. No file-count exception needed.

---

## 3. Implementation Blueprint (Strategy B)

### 3.1 Source Files Touched (2 src + 1 test + 1 wiring = 4 files)

| File | Change type | Lines (est.) |
|------|------------|--------------|
| `src/codegen/mod.rs` | Modify `StatementKind::Println` arm: branch on `stderr` flag — `false` → `printf` (unchanged), `true` → `__landin_eprint` (new) | +15 / -3 |
| `src/bin/main.rs` | Add `__landin_eprint(const char* s)` helper to C wrapper source | +3 |
| `tests/v0/stage13/plan/stage13_14_tests.rs` | NEW — 6 verification tests | +180 |
| `tests/all_tests.rs` | Wire `stage13_14_tests` module | +2 |

### 3.2 API Surface — New Public Symbols

**LLVM module level** (auto-declared by `LLVMSysEmitter::get_or_declare_function`):
- `declare void @__landin_eprint(i8*)` — auto-declared on first call (Stage 13.13 pattern; mirrors how `printf` is auto-declared)

**C wrapper level** (in `src/bin/main.rs` C source string):
- `void __landin_eprint(const char* s) { fprintf(stderr, "%s", s); }` — defined once, called from Landin codegen

**API naming compliance** (per `api-naming-standard.md` §3 + §8):
- `__landin_eprint` — `<prefix>_<verb>_<noun>` pattern (`__landin_` prefix per §8.1 codegen convention, matches `__landin_panic_*` siblings)
- Symmetric with `printf` (Stage 13.13) and `__landin_panic_overflow` (Stage 13.10)

### 3.3 Codegen Behavior

For each `StatementKind::Println { msg, newline, stderr }`:

```rust
let fmt = emitter.emit_string_global(b"%s\0");
let mut msg_bytes = msg.as_bytes().to_vec();
msg_bytes.push(0);
let str_global = emitter.emit_string_global(&msg_bytes);

if stderr {
    // Stage 13.14: eprintln!/eprint! → __landin_eprint helper (C wrapper
    // calls fprintf(stderr, "%s", s)). Portable across libc implementations
    // (stderr is a macro in glibc; the helper hides this).
    emitter.emit_call(
        "__landin_eprint",
        &[(EmitType::OpaquePtr, &str_global)],
        &EmitType::Void,
    );
} else {
    // Stage 13.13: println!/print! → printf("%s", msg) (unchanged)
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

Note: `__landin_eprint` takes only the message string (no format string) — the C helper hardcodes `"%s"` as the format. This:
- Reduces LLVM module globals (1 string per println instead of 2)
- Matches the `__landin_panic_*` helper convention (single string arg)
- Avoids format-string injection risk (msg can contain `%` without breaking)

### 3.4 C Wrapper Addition

**Before** (Stage 13.13):
```c
void __landin_panic_overflow(int op, int lhs, int rhs) { ... }
void __landin_panic_bounds_check(long long index, long long len) { ... }
void __landin_panic_div_by_zero(void) { ... }
```

**After** (Stage 13.14):
```c
void __landin_panic_overflow(int op, int lhs, int rhs) { ... }
void __landin_panic_bounds_check(long long index, long long len) { ... }
void __landin_panic_div_by_zero(void) { ... }
/* Stage 13.14: eprintln!/eprint! helper — routes to stderr via fprintf.
   Codegen calls this when StatementKind::Println.stderr == true. */
void __landin_eprint(const char* s) {
    fprintf(stderr, "%s", s);
}
```

### 3.5 §16 Interface Isolation Check

- `src/codegen/mod.rs`: Adds a branch on existing `bool` field; calls existing `emitter.emit_call` (already public); no new codegen → MIR back-edge
- `src/bin/main.rs`: Adds a new C helper function to the wrapper source string; no new Rust-side dependency
- No new module boundaries crossed
- No new pub API on the Rust side (the helper lives in C, called via `emit_call`)

**Verdict**: §16 compliant. No new module boundaries crossed.

---

## 4. Verification Plan

### 4.1 Existing Test Suite (must not regress)

| Suite | Baseline | Expected after Stage 13.14 |
|-------|----------|-----------------------------|
| `cargo test --test all_tests` | 2310 passed | 2310 + 6 (Stage 13.14 tests) = 2316 passed |
| `python3 tests/conformance/run_all.py` | 5026 passed | 5026 passed (no conformance change) |
| `cargo fmt --check` | clean | clean |
| `cargo clippy --all-targets` | 0 warnings | 0 warnings |

### 4.2 New Stage 13.14 Verification Tests (6 tests)

1. `test_codegen_println_branches_on_stderr` — `StatementKind::Println` arm in codegen has a branch on `stderr` flag
2. `test_codegen_eprint_calls_helper` — when `stderr == true`, codegen calls `__landin_eprint` (not `printf`)
3. `test_codegen_stdout_unchanged` — when `stderr == false`, codegen still calls `printf` (no regression)
4. `test_c_wrapper_has_eprint_helper` — C wrapper source defines `__landin_eprint` with `fprintf(stderr, ...)`
5. `test_stage_13_14_design_alignment_exists` — design alignment doc exists with required sections
6. `test_stage_13_14_gate_review_exists` — gate review doc exists with PASS verdict
7. `test_v01_gate_still_holds_after_stage_13_14` — ≥5000 conformance .lin files

### 4.3 Behavioral Smoke Test (manual, post-build)

After implementation:

```bash
# Test eprintln! goes to stderr (not stdout)
echo 'fn landin_main() -> i32 { eprintln!("err msg"); println!("out msg"); 0 }' > /tmp/test.lin
cargo run --features llvm-backend -- --run /tmp/test.lin 2>/tmp/stderr.txt
# Expected (combined output): err msg, out msg
# Expected (stdout only): out msg
# Expected (stderr only): err msg
```

The stderr message should appear on **stderr**, the stdout message on **stdout**. Pipe redirection (`> out.txt`) should capture only the stdout message.

---

## 5. §25.8 Design Write-Back Plan

Per `stage-committee-process.md` v3.21 §25.8, Stage 13.14 introduces **zero** new design deviations (the `stderr` field was already added in Stage 13.13; Stage 13.14 just exercises it correctly). Therefore:

| Design doc | Write-back content | Priority |
|------------|-------------------|----------|
| `docs/lang-design/06-mir.md` | NONE (StatementKind::Println already documented in Stage 13.13 write-back) | — |
| `docs/lang-design/07-codegen.md` | NONE (codegen behavior for stderr is an implementation detail, not a design-level concern) | — |
| `docs/lang-design/09-stdlib.md` | NONE (eprintln!/eprint! semantics are standard Rust; no Landin-specific design) | — |

**Net write-back**: ZERO. Stage 13.14 is a pure implementation refinement.

---

## 6. Version Policy

Per `stage-13.1-design-alignment.md` §5.4 version policy framework:

| Stage | Version bump | Rationale |
|-------|-------------|-----------|
| Stage 13.14 | v0.24.1 → v0.24.2 | **Patch bump** — bug fix (stderr routing); no new user-facing feature; no API removal; no design-doc change |

Patch bump justification:
- Bug fix (eprintln! routing to wrong stream)
- No new language feature (eprintln! was already "working" in 13.13, just to wrong stream)
- No new CLI flag
- No new conformance test (5026 unchanged)
- No design-doc write-back (zero new deviations)

---

## 7. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| `__landin_eprint` symbol not declared in LLVMSysEmitter module | LOW (auto-declared via `get_or_declare_function` on first `emit_call`) | LOW | Verify `emit_call("__landin_eprint", ...)` triggers auto-declaration (same pattern as `printf` in Stage 13.13) |
| C wrapper missing `#include <stdio.h>` | ZERO (already included for `printf`/`fprintf` in panic helpers) | — | No action needed |
| Stage 13.13 tests break (regression) | LOW (Stage 13.14 only adds a branch; existing stdout path unchanged) | LOW | Run Stage 13.13 tests after change |
| `eprintln!` not actually invoked in any conformance test | MEDIUM (no .lin file tests eprintln! behavior at runtime) | LOW (smoke test covers it; conformance is compile-only) | Manual smoke test post-build |
| Format string injection (msg contains `%`) | LOW (`__landin_eprint` uses `"%s"` format, so `%` in msg is literal) | ZERO | No action needed — Stage 13.14 design avoids this risk by using `"%s"` format in helper |

**Overall risk**: LOW. The change is a 1-branch addition to existing codegen + 1 helper function in C wrapper.

---

## 8. Stage Committee Recommendation

**GO** — proceed with implementation.

Conditions:
1. ✅ §13.4 design alignment complete (this document)
2. ✅ §14.4 J1-J6 all PASS (6/6)
3. ✅ §16 interface isolation preserved
4. ✅ §25.8 write-back plan documented (zero new deviations)
5. ✅ Version policy: v0.24.1 → v0.24.2 (patch bump, justified)
6. ✅ Test plan: 6 new verification tests + 2310 existing tests + 5026 conformance

No conditions blocking implementation. Proceed to gate-review-13.14.md → implementation → CI/CD.

---

## 9. Next Steps

| Step | Action | Owner | Estimated |
|------|--------|-------|-----------|
| 1 | Create `docs/develop/v0/stage-13/gate-review-13.14.md` | REV-A | 20 min |
| 2 | Implement Strategy B (2 src files) | DEV-A | 30 min |
| 3 | Create `tests/v0/stage13/plan/stage13_14_tests.rs` (6 tests) | QA-A | 30 min |
| 4 | Wire `stage13_14_tests` into `tests/all_tests.rs` | DEV-A | 2 min |
| 5 | Bump `Cargo.toml` v0.24.1 → v0.24.2 | DEV-A | 1 min |
| 6 | Run full CI/CD (cargo clean + build + fmt + clippy + test) | QA-A | 30 min |
| 7 | Create `docs/llvm/stage-13.14-eprintln-stderr-emission.md` (new) + update README + execution-pipeline | REC-A | 30 min |
| 8 | Update `RELEASE_NOTES.md`, `api-naming-standard.md`, `docs/tests/matrix.md`, `docs/worklog.md` | REC-A | 30 min |
| 9 | Rewrite `README.md` | REC-A | 20 min |
| 10 | Create zip package | DEV-A | 5 min |

**Total estimated**: ~3.5 hours.

---

## 10. Lessons Applied

From Stage 13.13 retrospective:
- **Lesson**: When adding a new codegen arm, capture all relevant semantic flags as fields on the MIR variant — even if not yet exercised. This makes future refinements (like Stage 13.14) trivially additive.
- **Applied**: Stage 13.13 captured `stderr: bool` on `StatementKind::Println` even though it was unused; Stage 13.14 just exercises the existing field with zero new MIR surface.

From Stage 13.10 retrospective (C wrapper helpers):
- **Lesson**: When C wrapper helpers are needed, follow the `__landin_<verb>_<noun>` naming pattern and keep them minimal (1-line bodies). This makes the helper API surface auditable.
- **Applied**: `__landin_eprint` follows the pattern; 1-line body (`fprintf(stderr, "%s", s)`).

---

## 11. References

- `stage-committee-process.md` v3.21 §13.4, §14.4, §15, §16, §25.8
- `docs/develop/v0/stage-13/stage-13.13-design-alignment.md` (Stage 13.13 — added `StatementKind::Println { msg, newline, stderr }` variant)
- `docs/develop/v0/stage-13/gate-review-13.13.md` (Stage 13.13 gate review — PASS)
- `docs/llvm/stage-13.13-println-inline-emission.md` (Stage 13.13 LLVM doc — explicitly defers stderr to Stage 13.14)
- `src/codegen/mod.rs:413-437` (Stage 13.13 codegen Println arm — modification target)
- `src/bin/main.rs:155-191` (Stage 13.13 C wrapper — helper addition target)
- Rust `eprintln!` documentation: https://doc.rust-lang.org/std/macro.eprintln.html
