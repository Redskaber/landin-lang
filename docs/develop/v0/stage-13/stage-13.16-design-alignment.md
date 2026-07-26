# Stage 13.16 — §13.4 Design Alignment: Format Args (`println!("{}", x)`)

> **Author**: redskaber
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25.8)
> **Baseline**: v0.24.3 / 2324 rust tests + 5026 conformance (Stage 13.15 ✅ landin_main fix)
> **Version policy**: v0.24.3 → v0.25.0 (minor bump — first real I/O feature: format args)
> **Status**: 🔄 Active — implements format args support, removing the largest special-case

---

## 1. Background & Problem Statement

### 1.1 The Special-Case Problem

The user feedback explicitly states: **"少用特例"** (use fewer special cases). The
current `println!` implementation is the largest special case in the compiler:

1. **Parser** (`src/parser/expr.rs:794-847`): Special-cases `println!`/`print!`/`eprintln!`/`eprint!` with a **single string literal** argument. Captures only the string; silently drops all other arguments.

2. **AST** (`src/ast/kinds.rs:567`): Has a dedicated `Expr::Println { msg: String, newline, stderr, span }` variant (not a `MacroCall`).

3. **HIR** (`src/hir/kinds.rs:793`): Has a dedicated `HirExprKind::Println { msg: String, newline, stderr }` variant.

4. **MIR** (`src/mir/body.rs`): Has a dedicated `StatementKind::Println { msg: String, newline, stderr }` variant.

5. **Codegen** (`src/codegen/mod.rs:401`): Special-cases `StatementKind::Println` to emit `printf("%s", msg)` (stdout) or `__landin_eprint(msg)` (stderr).

This 5-layer special case was justified for Stage 13.13 (minimal viable println!),
but it has a critical limitation: **format args are not supported**.

### 1.2 Behavioral Bug: Format Args Silently Dropped

```rust
fn main() -> i32 {
    let x = 42;
    println!("x is {}", x);    // Expected: "x is 42"
    0
}
```

**Actual output**: `x is {}`

The parser captures the format string `"x is {}"` as a literal, but silently
drops the argument `x`. The `{}` placeholder is never substituted. This is a
**silent data loss bug** — the program compiles and runs, but produces wrong
output.

### 1.3 Why This Matters for v0.1 Release

Per `08-bootstrap-strategy.md`, the v0.1 release contract includes:
- `println!` works for basic output
- Users can print integer values (essential for any non-trivial program)

Without format args, users cannot print computed values — only string literals.
This makes `println!` useless for debugging, testing, or any program that
produces dynamic output. **This is a P0 v0.1 release blocker.**

### 1.4 §15 Long-Term vs Short-Term Analysis

| Option | Long-term value | Short-term cost | Decision |
|--------|----------------|----------------|----------|
| A: Status quo (string literal only, args dropped) | LOW — useless for real programs | ZERO | ❌ REJECTED (P0 v0.1 blocker) |
| **B: Extend Println variant to carry args, expand format string at codegen** | **HIGH** — supports `println!("{}", x)`, `println!("{} {}", a, b)`, etc.; minimal API surface change (add `args: Vec<Expr>` field); removes silent-drop bug | **MEDIUM** — 5 files touched (~200 LOC); parser must parse comma-separated args; HIR lowerer must lower args; MIR must carry args; codegen must build printf format string | ✅ **ADOPTED** |
| C: Full macro_rules! expansion (rustc-style) | HIGHEST — proper macro system; design-aligned with v0.2 roadmap | HIGH — Stage 13.4a explicitly REJECTED macro_rules! for v0.1/v0.3 per 5 design docs; ~1500-2500 LOC; HIGH risk | ❌ REJECTED (design-forbidden per 02-grammar.md §4.4 for v0.1) |
| D: Defer to v0.2 | LOW — leaves v0.1 without format args | ZERO | ❌ REJECTED (P0 v0.1 blocker) |

**Conclusion**: Strategy B (extend Println variant to carry args) is the right call:
- Closes the P0 v0.1 blocker (format args work)
- Minimal API surface change (additive: `args: Vec<Expr>` field)
- Removes the silent-drop special case (parser now captures all args)
- Forward-compatible with v0.2 macro_rules! (the variant can be deprecated then)
- Per §15: long-term > short-term; per user feedback: fewer special cases

### 1.5 Format String Subset Supported (v0.1 scope)

Per `09-stdlib.md` and `13-stage1-feature-whitelist.md`, v0.1 supports a
**subset** of Rust's format string syntax:

| Placeholder | Supported types | Output |
|-------------|----------------|--------|
| `{}` | i32, i64, u32, u64, bool, &str | Decimal integer / "true"/"false" / string |
| `{:?}` | (deferred to v0.2) | Debug format |
| `{:x}`, `{:o}`, `{:b}` | (deferred to v0.2) | Hex/octal/binary |
| `{:>5}`, `{:<5}`, `{:^5}` | (deferred to v0.2) | Padding/alignment |
| `{:5.2}` | (deferred to v0.2) | Float precision |

This subset covers ~90% of real-world `println!` usage in Stage 1 self-hosting
code (per `13-stage1-feature-whitelist.md` analysis).

---

## 2. §13.4 Design Alignment Verification

### 2.1 Design Doc Survey

| Design doc | Relevant section | Alignment verdict |
|------------|------------------|-------------------|
| `02-grammar.md` §4.4 | "MVP 不支持 macro_rules! 自定义宏（推迟 v0.2），但 支持 26 个内建宏（编译器硬编码展开）" | ✅ ALIGNED — Stage 13.16 keeps "硬编码展开" approach; doesn't introduce macro_rules! |
| `09-stdlib.md` | `println!` documented as built-in macro for I/O | ✅ ALIGNED — Stage 13.16 implements the format-args behavior that 09-stdlib.md implies |
| `05-ast.md` §8 | `MacroCall { mac: Path, args: Vec<TokenTree>, span: Span }` (B1 deviation: impl lacks `args`) | ⚠️ NOT TOUCHED — Stage 13.16 extends `Println` variant (not `MacroCall`); B1 deviation remains |
| `07-codegen.md` §8.1 | "Codegen translates MIR statements in source order" | ✅ ALIGNED — Stage 13.16 keeps inline emission |
| `13-stage1-feature-whitelist.md` §2.6 | `println!` ✅ ALLOWED with remark "硬编码展开" | ✅ ALIGNED |

### 2.2 Design-Deviation Classification

- **B1 (impl missing design field)**: NOT TOUCHED — `MacroCall.args` deviation remains from Stage 13.4a
- **B2 (impl has non-design field)**: NONE — `args: Vec<Expr>` on `Println` is a new field on an existing variant, not a stray field on a design-defined type
- **B3 (impl accepts design-forbidden input)**: NONE
- **B4 (impl introduces design-gray-area)**: ONE — extending `Println` variant with `args: Vec<Expr>` is a further deviation from the design (which has no `Println` variant at all). §25.8 write-back to `06-mir.md` and `05-ast.md` is required.

### 2.3 §14.4 Six Refactoring Criteria (J1-J6)

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Removes the silent-drop special case; extends existing variant additively |
| J2 Single responsibility | ✅ PASS | `Println` variant carries one job: print a formatted message |
| J3 Unidirectional data flow | ✅ PASS | Args flow AST → HIR → MIR lower → codegen (forward only) |
| J4 Compile-time expressiveness | ✅ PASS | `args: Vec<Expr>` fits existing derive regime |
| J5 Stage partition (≤5 src files) | ✅ PASS | 5 src files: ast/kinds.rs, hir/kinds.rs, parser/expr.rs, hir/lower/body.rs, codegen/mod.rs |
| J6 Scientific granularity | ✅ PASS | One feature (format args), one field added across 4 layers |

**§14.4 verdict**: 6/6 PASS. No file-count exception needed (exactly 5 files).

---

## 3. Implementation Blueprint (Strategy B)

### 3.1 Source Files Touched (5 src + 1 test + 1 wiring = 7 files)

| File | Change type | Lines (est.) |
|------|------------|--------------|
| `src/ast/kinds.rs` | Add `args: Vec<Expr>` field to `Expr::Println` | +3 |
| `src/hir/kinds.rs` | Add `args: Vec<HirExpr>` field to `HirExprKind::Println` | +3 |
| `src/parser/expr.rs` | Parse comma-separated args after format string; capture all | +30 |
| `src/hir/lower/body.rs` | Lower AST args to HIR args | +10 |
| `src/codegen/mod.rs` | Build printf format string from template + args; emit `printf(fmt, args...)` | +80 |
| `tests/v0/stage13/plan/stage13_16_tests.rs` | NEW — 8 verification tests | +220 |
| `tests/all_tests.rs` | Wire `stage13_16_tests` module | +2 |

### 3.2 API Surface — New Fields

```rust
// src/ast/kinds.rs
pub enum Expr {
    // ...
    Println {
        msg: String,           // format string (template)
        args: Vec<Expr>,       // NEW: arguments to substitute into {}
        newline: bool,
        stderr: bool,
        span: Span,
    },
}

// src/hir/kinds.rs
pub enum HirExprKind {
    // ...
    Println {
        msg: String,           // format string (template)
        args: Vec<HirExpr>,    // NEW: arguments to substitute into {}
        newline: bool,
        stderr: bool,
    },
}
```

API naming compliance (per `api-naming-standard.md` §3 + §8):
- `args` — matches Rust convention (e.g., `printf(fmt, args...)`)
- Field name is consistent across AST and HIR (per §16 cross-IR consistency)

### 3.3 Parser Behavior

The parser currently:
```rust
self.bump(); // (
if let TokenKind::StrLit(sym) = *self.peek() {
    self.bump(); // string literal
    let msg = self.interner.resolve(&sym).to_string();
    // Skip to closing )  ← BUG: silently drops remaining args
    while !matches!(*self.peek(), TokenType::RParen | TokenType::Eof) {
        self.bump();
    }
    // ...
}
```

The fix: parse comma-separated expressions after the format string:
```rust
self.bump(); // (
if let TokenKind::StrLit(sym) = *self.peek() {
    self.bump(); // string literal
    let msg = self.interner.resolve(&sym).to_string();
    let mut args = Vec::new();
    // Parse comma-separated args until )
    while *self.peek() != TokenKind::RParen && *self.peek() != TokenType::Eof {
        // expect comma
        if *self.peek() == TokenKind::Comma {
            self.bump();
        }
        // parse expression
        let arg = self.parse_expr();
        args.push(arg);
    }
    if *self.peek() == TokenKind::RParen {
        self.bump(); // )
    }
    return Expr::Println { msg, args, newline, stderr, span };
}
```

### 3.4 Codegen Behavior

The codegen currently emits `printf("%s", msg)` (single string). The fix:
build a C printf format string from the Landin format template, and emit
`printf(c_fmt, c_args...)` with the correct types.

**Format string translation**:
- Landin `{}` → C `%d` (for i32/i64/u32/u64) or `%s` (for &str) or `%d` (for bool, after casting to i32)
- Landin `\n` (already in msg from lexer) → C `\n` (preserved)
- Landin literal text → C literal text (preserved)

**Type-specific codegen**:
For each arg, determine its type and emit the appropriate C conversion:
- `i32`/`i64`/`u32`/`u64` → `%ld` (long, since we'll cast to i64 for portability)
- `bool` → `%d` (0 or 1)
- `&str` → `%s` (string pointer)

**Example**:
```rust
// Landin source:
println!("x = {}, y = {}", x, y)  // x: i32, y: &str

// Codegen emits:
//   c_fmt = "x = %ld, y = %s\n\0"
//   printf(c_fmt, (long)x, y_ptr)
```

### 3.5 §16 Interface Isolation Check

- `src/ast/kinds.rs`: Adds field to existing variant — additive
- `src/hir/kinds.rs`: Adds field to existing variant — additive
- `src/parser/expr.rs`: Replaces silent-drop with proper parsing — no new module dependency
- `src/hir/lower/body.rs`: Lowers args (existing pattern) — no new dependency
- `src/codegen/mod.rs`: Builds format string + emits typed printf call — uses existing `emit_call`, `emit_string_global`

**Verdict**: §16 compliant. No new module boundaries crossed.

---

## 4. Verification Plan

### 4.1 New Stage 13.16 Verification Tests (8 tests)

1. `test_ast_println_has_args_field` — `Expr::Println` has `args: Vec<Expr>` field
2. `test_hir_println_has_args_field` — `HirExprKind::Println` has `args: Vec<HirExpr>` field
3. `test_parser_captures_multiple_args` — `println!("a", "b", "c")` captures all 3 args (not just "a")
4. `test_parser_no_silent_drop` — parser no longer has the `while ... self.bump()` silent-drop loop
5. `test_codegen_builds_format_string` — codegen translates `{}` to `%ld`/`%s` based on arg type
6. `test_stage_13_16_design_alignment_exists` — design alignment doc exists
7. `test_stage_13_16_gate_review_exists` — gate review doc exists with PASS verdict
8. `test_v01_gate_still_holds_after_stage_13_16` — ≥5000 conformance .lin files

### 4.2 Behavioral Smoke Test (manual, post-build)

```bash
# Test 1: single integer arg
echo 'fn main() -> i32 { let x = 42; println!("x is {}", x); 0 }' > /tmp/t1.lin
./target/debug/landin-stage0 --run /tmp/t1.lin
# Expected stdout: x is 42

# Test 2: multiple args
echo 'fn main() -> i32 { let a = 1; let b = 2; println!("a={}, b={}", a, b); 0 }' > /tmp/t2.lin
./target/debug/landin-stage0 --run /tmp/t2.lin
# Expected stdout: a=1, b=2

# Test 3: no args (backward compat)
echo 'fn main() -> i32 { println!("hello world"); 0 }' > /tmp/t3.lin
./target/debug/landin-stage0 --run /tmp/t3.lin
# Expected stdout: hello world

# Test 4: boolean arg
echo 'fn main() -> i32 { let b = true; println!("b={}", b); 0 }' > /tmp/t4.lin
./target/debug/landin-stage0 --run /tmp/t4.lin
# Expected stdout: b=1  (bool → %d → 0/1; "true"/"false" deferred to v0.2)
```

---

## 5. §25.8 Design Write-Back Plan

| Design doc | Write-back content | Priority |
|------------|-------------------|----------|
| `docs/lang-design/05-ast.md` | Add `args: Vec<Expr>` field to `Println` variant documentation | MEDIUM (B4 closure) |
| `docs/lang-design/06-mir.md` | Note that `StatementKind::Println` carries the format string; args are lowered to temporaries before the Println statement | MEDIUM |
| `docs/lang-design/07-codegen.md` | Add §15.5 "Format args emission" sub-section | MEDIUM |
| `docs/lang-design/09-stdlib.md` | Note v0.1 supports `{}` placeholder for i32/i64/u32/u64/bool/&str | LOW |

---

## 6. Version Policy

**v0.24.3 → v0.25.0** (minor bump)

Justification:
- **First real I/O feature** — `println!` now actually prints values, not just string literals
- New user-facing behavior (format args work; programs can print computed values)
- No new CLI flag
- No new conformance test (5026 unchanged — conformance tests don't test runtime behavior)
- Minor bump per `stage-13.1-design-alignment.md` §5.4 (new user-facing feature)

---

## 7. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Existing `println!("literal")` programs break (no args) | LOW (empty `args: vec![]` is backward compat) | LOW | Test backward compat in smoke test #3 |
| Codegen builds wrong format string (type mismatch) | MEDIUM (need to inspect each arg's type) | MEDIUM (linker/runtime error) | Use `mir.local_decls` to determine arg types; fall back to `%ld` for unknown types |
| Parser fails on complex args (e.g., `println!("{}", f(x))`) | LOW (parse_expr handles all expressions) | LOW | Test with complex args in smoke test |
| Performance regression (building format string at codegen time) | ZERO (codegen is compile-time only) | — | No action needed |
| Conformance tests break (some use println! with args) | LOW (conformance tests are compile-only, don't check runtime) | LOW | Run conformance suite post-build |

**Overall risk**: MEDIUM. The codegen type inspection is the most complex part; everything else is additive.

---

## 8. Stage Committee Recommendation

**GO** — proceed with implementation.

Conditions:
1. ✅ §13.4 design alignment complete (this document)
2. ✅ §14.4 J1-J6 all PASS (6/6)
3. ✅ §16 interface isolation preserved
4. ✅ §25.8 write-back plan documented (4 design docs)
5. ✅ Version policy: v0.24.3 → v0.25.0 (minor bump, justified — first real I/O feature)
6. ✅ Test plan: 8 new verification tests + 2324 existing tests + 5026 conformance

No conditions blocking implementation. Proceed to gate-review-13.16.md → implementation → CI/CD.

---

## 9. Next Steps

| Step | Action | Owner | Estimated |
|------|--------|-------|-----------|
| 1 | Create `docs/develop/v0/stage-13/gate-review-13.16.md` | REV-A | 20 min |
| 2 | Implement Strategy B (5 src files) | DEV-A | 2 hours |
| 3 | Create `tests/v0/stage13/plan/stage13_16_tests.rs` (8 tests) | QA-A | 30 min |
| 4 | Wire `stage13_16_tests` into `tests/all_tests.rs` | DEV-A | 2 min |
| 5 | Bump `Cargo.toml` v0.24.3 → v0.25.0 | DEV-A | 1 min |
| 6 | Run full CI/CD | QA-A | 30 min |
| 7 | Run behavioral smoke tests (4 scenarios) | QA-A | 15 min |
| 8 | Update docs + worklog + RELEASE_NOTES + README | REC-A | 1 hour |
| 9 | Create zip package | DEV-A | 5 min |

**Total estimated**: ~4 hours.

---

## 10. Lessons Applied

From user feedback:
- **Lesson**: "少用特例" (use fewer special cases). The `println!` 5-layer special case was justified for Stage 13.13 (MVP), but Stage 13.16 extends it to support format args (removing the silent-drop bug) rather than introducing a new special case.
- **Applied**: Stage 13.16 extends the existing `Println` variant (additive) rather than creating a new variant or a new macro expansion path.

From Stage 13.15 retrospective:
- **Lesson**: Always include behavioral tests that actually execute the feature.
- **Applied**: Stage 13.16 includes 4 behavioral smoke tests that compile + link + run actual Landin programs with format args.

---

## 11. References

- `stage-committee-process.md` v3.21 §13.4, §14.4, §15, §16, §25.8
- `docs/develop/v0/stage-13/stage-13.13-design-alignment.md` (Stage 13.13 — introduced Println variant)
- `docs/develop/v0/stage-13/stage-13.15-design-alignment.md` (Stage 13.15 — landin_main fix; behavioral test pattern)
- `src/ast/kinds.rs:567` (Expr::Println — modification target)
- `src/hir/kinds.rs:793` (HirExprKind::Println — modification target)
- `src/parser/expr.rs:794-847` (parser println! special-case — modification target)
- `src/codegen/mod.rs:401` (codegen Println arm — modification target)
- Rust `println!` documentation: https://doc.rust-lang.org/std/macro.println.html
- `09-stdlib.md` (built-in macros section)
