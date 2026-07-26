# Stage 13.13 — §13.4 Design Alignment: Inline println! Emission

> **Author**: redskaber
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25.8)
> **Baseline**: v0.24.0 / 2279 rust tests + 5026 conformance (Stage 13.4a ✅ TD-032 P0 CLOSED)
> **Version policy**: v0.24.0 → v0.24.1 (patch bump — println! ordering bug fix)
> **Status**: 🔄 Active — fixes Stage 13.12 known limitation (helper function ordering bug)

---

## 1. Background & Problem Statement

### 1.1 Stage 13.12 Implementation Recap

Stage 13.12 (preceding session) implemented println! end-to-end output through:

1. **Parser** captures `println!("msg")` → `Expr::Println { msg, newline, stderr }` (src/parser/expr.rs)
2. **HIR** carries `HirExprKind::Println { msg, newline, stderr }` (src/hir/kinds.rs)
3. **MIR lower** pushes message into `MirBody.println_messages: Vec<String>` side-table (src/mir/lower/expr_operand.rs)
4. **Codegen** iterates the side-table and emits a **separate helper function** `__landin_printlns_<fnname>` containing all `puts()` calls (src/codegen/mod.rs)
5. **C wrapper** (src/bin/main.rs) declares `__landin_printlns_landin_main` as a **weak symbol** and calls it **BEFORE** `landin_main()` — relying on linker to resolve to null if no println! exists

### 1.2 Known Limitation: Output Ordering Bug

The Stage 13.12 approach has a **fundamental ordering defect**:

```
Expected program output for:
    fn landin_main() -> i32 {
        let x = 1;
        println!("step 1");
        x + 1
        println!("step 2");
        0
    }

Actual output (Stage 13.12):
    step 1
    step 2
    ← (program runs but produces no further output; return value 0 correct)

Correct output (post-Stage 13.13):
    step 1
    step 2
    ← (program runs after prints; same return value)
```

Wait — at first glance the outputs above look identical. They differ only when the program has runtime side effects that interleave with println! output, e.g.:

```rust
fn landin_main() -> i32 {
    println!("before loop");
    let mut i = 0;
    while i < 3 {
        println!("iter {}", i);  // hypothetical format support
        i = i + 1;
    }
    println!("after loop");
    0
}
```

Stage 13.12 produces ALL prints **before** `landin_main()` runs, which means the program body executes **after** the prints — but since the program body is what triggered the prints in source-code order, the runtime behavior is non-intuitive: any runtime panics (e.g., overflow) in `landin_main` would happen AFTER all prints, even though prints conceptually happen at the point of invocation.

For loops, this means the iteration count is invisible to the print side-table (since the side-table is filled once at MIR-lowering time, not at runtime). Concretely:

```rust
// Stage 13.12 emits: __landin_printlns_landin_main() { puts("iter"); puts("iter"); }
//                    ↑ but only ONE iter print is in source — the side-table
//                      has 1 entry; loop body would not re-emit the print
```

The print statement's position in the basic block is **lost** when stored in a side-table — there's no way to express "emit this print N times during the loop".

### 1.3 Root Cause: Architectural Mismatch

Per `07-codegen.md` §8.1 ("Codegen Conventions") and §16 ("Interface Isolation"):

- MIR's job is to express **ordered computation** as basic-block statements.
- Codegen's job is to translate MIR statements into LLVM instructions **in the same order**.
- Side-tables are for **out-of-band metadata** (e.g., vtable index resolution) — not for **ordered side effects**.

Using a side-table for println! messages violates §16's data-flow rule: "MIR's basic_blocks array is the **single source of truth** for execution order". Stashing println! messages in a separate `Vec<String>` bypasses the basic block ordering, so codegen cannot place the `puts()` call at the source-code position.

### 1.4 §15 Long-Term vs Short-Term Analysis

| Option | Long-term value | Short-term cost | Decision |
|--------|----------------|----------------|----------|
| A: Keep side-table + helper fn (Stage 13.12 status quo) | LOW — broken ordering for loops, conditionals | LOW — already implemented | ❌ REJECTED (per §15: long-term > short-term) |
| B: Inline `StatementKind::Println` in basic block (proposed) | HIGH — correct ordering for all control flow; design-aligned with §16; extends naturally to format strings, file I/O | MEDIUM — 4 src files touched (~150-250 LOC); 1 new MIR variant; 1 codegen arm | ✅ **ADOPTED** |
| C: Defer to v0.2 macro_rules! full expansion | MEDIUM — would also fix the issue | HIGH — Stage 13.4a explicitly REJECTED macro_rules! for v0.1/v0.3 per design docs | ❌ REJECTED (design-forbidden per 02-grammar.md §4.4) |
| D: HIR-time expansion to `printf(format, args)` call | HIGH — proper rustc-aligned macro expansion | HIGH — requires HIR-time macro expansion infrastructure (~500+ LOC); deferred to v0.3 bootstrap | ❌ DEFERRED (proper macro expansion is Stage 1 rewrite scope per 08-bootstrap-strategy.md) |

**Conclusion**: Strategy B (inline `StatementKind::Println`) is the right call:
- Correctly fixes the ordering bug
- Minimal architectural change (additive — new StatementKind variant)
- Design-aligned with `06-mir.md` (StatementKind extension)
- Forward-compatible: when Stage 1 macro expansion lands, the variant can be deprecated in favor of a real `printf` call

---

## 2. §13.4 Design Alignment Verification

Per `stage-committee-process.md` v3.21 §13.4 "阶段开始时的设计对齐", the following design docs were consulted:

### 2.1 Design Doc Survey

| Design doc | Relevant section | Alignment verdict |
|------------|------------------|-------------------|
| `02-grammar.md` §4.4 (line 421) | "MVP 不支持 macro_rules! 自定义宏（推迟 v0.2），但 支持 26 个内建宏（编译器硬编码展开）" | ✅ ALIGNED — Stage 13.13 keeps "硬编码展开" approach; doesn't introduce macro_rules! |
| `05-ast.md` §8 (line 501-505) | `MacroCall { mac: Path, args: Vec<TokenTree>, span: Span }` (B1 deviation: impl lacks `args`) | ⚠️ NOT TOUCHED — Stage 13.13 uses `HirExprKind::Println` (already exists from Stage 13.12), bypasses MacroCall.args |
| `06-mir.md` (StatementKind section) | Lists `Assign`, `Nop`, `StorageLive`, `StorageDead`, `Deinit` — no `Println` variant | ⚠️ B4 design-gray-area — Stage 13.13 introduces new variant; §25.8 write-back required |
| `07-codegen.md` §8.1 | "Codegen translates MIR statements in source order" | ✅ ALIGNED — Stage 13.13 places Println statement in source position |
| `13-stage1-feature-whitelist.md` §2.6 (line 152) | "禁止使用：macro_rules! 自定义宏（v0.2 才支持）" | ✅ ALIGNED — Stage 13.13 doesn't introduce macro_rules! |
| `08-bootstrap-strategy.md` line 206 | "Proc macro：永久不做（v0.2 仅 macro_rules!）" | ✅ ALIGNED — Stage 13.13 doesn't introduce proc macros |
| `09-stdlib.md` | Mentions `println!` as built-in macro for I/O | ✅ ALIGNED — Stage 13.13 keeps println! as built-in |

### 2.2 Design-Deviation Classification

Per `stage-committee-process.md` §25.8 design-deviation taxonomy:

- **B1 (impl missing design field)**: NOT TOUCHED — `MacroCall.args` deviation remains from Stage 13.4a; Stage 13.13 doesn't extend MacroCall
- **B2 (impl has non-design field)**: NONE — `StatementKind::Println` is a new variant, not a stray field on an existing type
- **B3 (impl accepts design-forbidden input)**: NONE — Stage 13.13 doesn't add new accepted input
- **B4 (impl introduces design-gray-area)**: ONE — `StatementKind::Println { msg, newline, stderr }` is a new variant not in `06-mir.md`. §25.8 write-back to `06-mir.md` is required (see §5 below)

### 2.3 §14.4 Six Refactoring Criteria (J1-J6)

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Adds inline statement to basic block, restoring §16 single-source-of-truth for ordering |
| J2 Single responsibility | ✅ PASS | `StatementKind::Println` carries one job: emit a print side-effect in order |
| J3 Unidirectional data flow | ✅ PASS | MIR lower → MIR body → codegen, all forward; no codegen → MIR back-edge |
| J4 Compile-time expressiveness | ✅ PASS | New variant is `Debug + Clone`, fits existing derive regime |
| J5 Stage partition (≤5 src files) | ✅ PASS | 4 src files: mir/body.rs, mir/lower/expr_operand.rs, codegen/mod.rs, bin/main.rs (wrapper simplification) |
| J6 Scientific granularity | ✅ PASS | One bug fix, one variant, one codegen arm — minimum viable change |

**§14.4 verdict**: 6/6 PASS. No file-count exception needed.

---

## 3. Implementation Blueprint (Strategy B)

### 3.1 Source Files Touched (4 src + 1 test + 1 conformance-marker audit = 6 files)

| File | Change type | Lines (est.) |
|------|------------|--------------|
| `src/mir/body.rs` | Add `StatementKind::Println { msg, newline, stderr }` variant | +6 |
| `src/mir/lower/expr_operand.rs` | Modify `HirExprKind::Println` arm: push `StatementKind::Println` to current BB; remove side-table push (keep field for backward compat — empty) | +12 / -3 |
| `src/codegen/mod.rs` | Add `StatementKind::Println` arm to `codegen_statement`: emit `printf("%s", msg_global)` inline; remove `__landin_printlns_<fnname>` helper emission | +35 / -30 |
| `src/bin/main.rs` | Simplify C wrapper: remove `__landin_printlns_landin_main` weak-symbol call (no longer needed) | -7 |
| `tests/v0/stage13/plan/stage13_13_tests.rs` | New — 8 verification tests | +180 |
| `tests/all_tests.rs` | Wire `stage13_13_tests` module | +2 |

### 3.2 API Surface — New Public Types/Functions

```rust
// src/mir/body.rs
pub enum StatementKind {
    // ... existing variants ...
    /// Stage 13.13: Inline println! statement.
    /// Carries the message string (already formatted with \n if `newline == true`),
    /// the `newline` flag (for future format support), and the `stderr` flag
    /// (for `eprintln!` — currently ignored at codegen time, deferred to Stage 13.14).
    ///
    /// Per §16: this variant is the source-of-truth for print ordering.
    /// Codegen emits `printf("%s", <msg_global>)` inline at this statement's
    /// position in the basic block.
    Println {
        msg: String,
        newline: bool,
        stderr: bool,
    },
}
```

API naming follows `api-naming-standard.md` §3 + §8 conventions:
- `StatementKind::Println` — variant matches `HirExprKind::Println` (consistent cross-IR naming)
- Field names `msg`, `newline`, `stderr` — match `HirExprKind::Println` field names

### 3.3 Codegen Behavior

For each `StatementKind::Println { msg, newline, stderr }`:

1. Emit a global string constant `@.println_msg_<n>` containing `msg` bytes + null terminator
2. Emit a global format string `@.println_fmt` = `"%s\0"` (deduplicated across all println! calls in the module)
3. Call `printf(@.println_fmt, @.println_msg_<n>)` via `emitter.emit_call`
4. (`stderr` flag: deferred to Stage 13.14 — would emit `fprintf(stderr, ...)`)

The `printf` declaration is added to the module on first use (via `emitter.emit_declare`).

### 3.4 C Wrapper Simplification

**Before** (Stage 13.12):
```c
__attribute__((weak)) void __landin_printlns_landin_main(void);
int main(void) {
    if (__landin_printlns_landin_main) {
        __landin_printlns_landin_main();
    }
    int ret = landin_main();
    return ret;
}
```

**After** (Stage 13.13):
```c
int main(void) {
    int ret = landin_main();
    return ret;
}
```

The weak-symbol trick is no longer needed because println! output is emitted inline within `landin_main()` itself. The runtime stubs (`__landin_panic_overflow`, etc.) remain in the wrapper.

### 3.5 Backward Compatibility

- `MirBody.println_messages: Vec<String>` field is **kept** (not removed) for backward compatibility with any external tooling that reads MIR side-tables. The field is no longer populated by MIR lower; it remains `Vec::new()` for all bodies.
- The `HirExprKind::Println` variant in `src/hir/kinds.rs` is unchanged.
- The `Expr::Println` variant in `src/ast/kinds.rs` is unchanged.
- No conformance `.lin` file behavior changes (Stage 13.13 doesn't touch parsing or type checking).

### 3.6 §16 Interface Isolation Check

- `src/mir/body.rs`: Adds a new variant to a `pub enum` — additive, no existing API broken
- `src/mir/lower/expr_operand.rs`: Modifies one match arm; no new module-level dependency
- `src/codegen/mod.rs`: Adds a new match arm in `codegen_statement`; calls existing `emitter.emit_call` + `emitter.emit_string_global` (both already public); no new codegen → MIR back-edge introduced
- `src/bin/main.rs`: Removes weak-symbol call from C wrapper; no new dependency

**Verdict**: §16 compliant. No new module boundaries crossed.

---

## 4. Verification Plan

### 4.1 Existing Test Suite (must not regress)

| Suite | Baseline | Expected after Stage 13.13 |
|-------|----------|-----------------------------|
| `cargo test --test all_tests` | 2279 passed | 2279 + 8 (Stage 13.13 tests) = 2287 passed |
| `python3 tests/conformance/run_all.py` | 5026 passed | 5026 passed (no conformance change) |
| `cargo fmt --check` | clean | clean |
| `cargo clippy --all-targets` | 0 warnings | 0 warnings |

### 4.2 New Stage 13.13 Verification Tests (8 tests)

1. `test_statement_kind_has_println_variant` — `StatementKind::Println { msg, newline, stderr }` exists in `src/mir/body.rs`
2. `test_mir_lower_emits_println_statement_inline` — `HirExprKind::Println` arm pushes to current BB (not to side-table)
3. `test_codegen_statement_handles_println` — `codegen_statement` has `StatementKind::Println` arm calling `emit_call("printf", ...)`
4. `test_no_helper_function_emission` — `codegen_from_mir` no longer emits `__landin_printlns_*` helper function
5. `test_c_wrapper_no_weak_symbol` — `src/bin/main.rs` C wrapper source has no `__landin_printlns_landin_main` reference
6. `test_println_messages_field_kept_for_compat` — `MirBody.println_messages` field still exists (backward compat)
7. `test_stage_13_13_gate_review_exists` — `docs/develop/v0/stage-13/gate-review-13.13.md` exists with PASS verdict
8. `test_v01_gate_still_holds_after_stage_13_13` — `cargo test` passes ≥5000 conformance gate

### 4.3 Behavioral Smoke Test (manual, post-build)

After implementation:

```bash
echo 'fn landin_main() -> i32 { println!("hello world"); 0 }' > /tmp/hello.lin
cargo run --features llvm-backend -- --run /tmp/hello.lin
# Expected stderr: info: running /tmp/hello.out
# Expected stdout: hello world
# Expected exit: 0
```

The output "hello world" should appear on **stdout** (not stderr) — Stage 13.13 uses `printf` to stdout by default.

---

## 5. §25.8 Design Write-Back Plan

Per `stage-committee-process.md` v3.21 §25.8, the following design docs require retroactive write-back after Stage 13.13 implementation:

| Design doc | Write-back content | Priority |
|------------|-------------------|----------|
| `docs/lang-design/06-mir.md` | Add `StatementKind::Println { msg, newline, stderr }` variant to the StatementKind section; note that this is a v0.1 hardcode-expanded built-in macro emission point, will be deprecated when macro_rules! lands in v0.2 | HIGH (B4 closure) |
| `docs/lang-design/07-codegen.md` | Add §15.4 "Inline println! emission" sub-section: codegen translates `StatementKind::Println` to `printf("%s", <msg_global>)` | MEDIUM |
| `docs/lang-design/09-stdlib.md` | Note in `println!` section: "v0.1 emits inline `printf` call via MIR `StatementKind::Println`; v0.2+ will use proper macro expansion" | LOW |

Design docs `02-grammar.md`, `05-ast.md`, `13-stage1-feature-whitelist.md`, `08-bootstrap-strategy.md` need **no** write-back — Stage 13.13 doesn't touch their domains.

---

## 6. Version Policy

Per `stage-13.1-design-alignment.md` §5.4 version policy framework:

| Stage | Version bump | Rationale |
|-------|-------------|-----------|
| Stage 13.13 | v0.24.0 → v0.24.1 | **Patch bump** — bug fix for Stage 13.12 known limitation; no new user-facing feature; no API removal |

Patch bump justification:
- Bug fix (output ordering)
- No new language feature (println! was already "working" in 13.12, just ordered incorrectly)
- No new CLI flag
- No new conformance test (5026 unchanged)
- Backward-compatible MIR side-table field retained

---

## 7. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| `StatementKind::Println` variant exhaustiveness errors in downstream match arms | MEDIUM (3-4 match sites) | LOW (compile-time error, easy fix) | Grep `match.*StatementKind` and update all sites |
| `printf` symbol not declared in LLVMSysEmitter module | LOW (already declared for Stage 13.11) | LOW | Verify `llvm_sys_emitter.rs:1333` has printf declare |
| C wrapper removal of weak symbol breaks existing `--run` smoke tests | LOW (Stage 13.8/13.9 tests don't depend on weak symbol) | LOW | Re-run Stage 13.8/13.9 tests after change |
| Format string injection (msg contains `%`) | MEDIUM (rustc formats with `{}`, but Landin v0.1 println! takes raw string) | LOW (only crashes if user puts `%` in println! message) | Document as v0.1 limitation; v0.2 macro expansion will fix |
| Conformance regression (some .lin file behavior depends on side-table) | LOW (no .lin file inspects println_messages field) | LOW | Run conformance suite post-build |

**Overall risk**: LOW. The change is additive at MIR layer, replacement at codegen layer, simplification at C wrapper layer. All 3 layers have existing test coverage.

---

## 8. Stage Committee Recommendation

**GO** — proceed with implementation.

Conditions:
1. ✅ §13.4 design alignment complete (this document)
2. ✅ §14.4 J1-J6 all PASS (6/6)
3. ✅ §16 interface isolation preserved
4. ✅ §25.8 write-back plan documented (3 design docs)
5. ✅ Version policy: v0.24.0 → v0.24.1 (patch bump, justified)
6. ✅ Test plan: 8 new verification tests + 2279 existing tests + 5026 conformance

No conditions blocking implementation. Proceed to gate-review-13.13.md → implementation → CI/CD.

---

## 9. Next Steps

| Step | Action | Owner | Estimated |
|------|--------|-------|-----------|
| 1 | Create `docs/develop/v0/stage-13/gate-review-13.13.md` | REV-A | 30 min |
| 2 | Implement Strategy B (4 src files) | DEV-A | 2 hours |
| 3 | Create `tests/v0/stage13/plan/stage13_13_tests.rs` (8 tests) | QA-A | 1 hour |
| 4 | Wire `stage13_13_tests` into `tests/all_tests.rs` | DEV-A | 5 min |
| 5 | Bump `Cargo.toml` v0.24.0 → v0.24.1 | DEV-A | 1 min |
| 6 | Run full CI/CD (cargo clean + build + fmt + clippy + test) | QA-A | 30 min |
| 7 | Update `docs/worklog.md` + `RELEASE_NOTES.md` + `api-naming-standard.md` + `docs/tests/matrix.md` | REC-A | 1 hour |
| 8 | Update `docs/llvm/` (new doc + README + execution-pipeline) | REC-A | 1 hour |
| 9 | §25.8 write-back: `06-mir.md`, `07-codegen.md`, `09-stdlib.md` | ARCH-A | 1 hour |
| 10 | Rewrite `README.md` (full refresh) | REC-A | 1 hour |
| 11 | Create zip package | DEV-A | 5 min |

**Total estimated**: ~8 hours (1 session).

---

## 10. Lessons Learned from Stage 13.12

Stage 13.12's mistake was using a side-table (`Vec<String>`) for **ordered side effects** — a category §16 explicitly forbids side-tables from. The right architectural choice (StatementKind variant) was known but deferred "for simplicity". Per §15 "long-term > short-term", this deferral created a known limitation that needed Stage 13.13 to fix.

**Action item for future stages**: When in doubt between a side-table and an inline statement, **always choose the inline statement** if the data carries ordering semantics. Side-tables are for unordered metadata only (e.g., vtable indices, capture lists, span info).

---

## 11. References

- `stage-committee-process.md` v3.21 §13.4, §14.4, §15, §16, §25.8
- `docs/lang-design/06-mir.md` (StatementKind section — write-back target)
- `docs/lang-design/07-codegen.md` §8.1 (Codegen Conventions)
- `docs/lang-design/02-grammar.md` §4.4 (built-in macro expansion policy)
- `docs/develop/v0/stage-13/stage-13.4-design-alignment.md` (Stage 13.4a reframe — established that built-in macro expansion is the design-sanctioned approach)
- `src/mir/body.rs:174` (StatementKind enum — modification target)
- `src/mir/lower/expr_operand.rs:1376` (HirExprKind::Println arm — modification target)
- `src/codegen/mod.rs:289-360` (Stage 13.12 helper function emission — removal target)
- `src/bin/main.rs:155-192` (C wrapper with weak symbol — simplification target)
