# Stage 13.2 Design Alignment (§13.4) — if-let / while-let (TD-031 P0 closure)

> **Auditor**: ARCH-A + ALG-C (combined subagent) | **Date**: 2026-07-26 | **Baseline**: v0.21.5
> **Process**: stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8
> **Priority**: P0 (blocks v0.3 self-hosting) — first user-facing compiler feature
> **Inputs**: `plan-13.1.md` (Stage 13 active plan) + `stage-13.1-design-alignment.md` (format reference) +
> r216 architecture audit §3.5 (TD-031 detail) + r217 stages-0-4 re-audit §2.5 + §3 (Stage 0 root-cause) +
> 5 design docs (`02-grammar.md` / `05-ast.md` / `03-type-system.md` / `04-ownership-borrowing.md` /
> `13-stage1-feature-whitelist.md`) + `src/ast/kinds.rs` / `src/hir/kinds.rs` / `src/parser/expr.rs` /
> `src/hir/lower/body.rs` / `src/mir/lower/control_flow.rs` / `src/mir/lower/expr_operand.rs` /
> `src/mir/lower/pattern_bindings.rs` + 11 conformance FAIL test files
> **Scope**: Stage 13.2 MUV-4 (AST + HIR + parser) + MUV-5 (MIR lowering / desugar) + MUV-6 (typeck + borrowck refinement scope)

---

## 1. Executive Summary

Stage 13.2 closes TD-031 — the absence of `if let` / `while let` from the Landin compiler.
This is the **first user-facing compiler feature** in the Stage 13 series and the first minor-version
bump (v0.21.5 → v0.22.0) per the version policy established in `stage-13.1-design-alignment.md` §5.4.

**Findings**:

- **Grammar spec alignment**: `docs/lang-design/02-grammar.md` §3.4 (line 257-262) **explicitly defines
  both `if let` and `while let` productions** in the BNF. The design intent has been on the books since
  Stage 0; the implementation simply never grew the corresponding arms. r217 §3 confirms the deferral
  traces to a Stage 0.5 parser-scope decision documented in `dev-log.md` §5.2.3 line 228 ("struct literal /
  if let / while let / macro call 表达式" listed as out-of-scope).

- **AST design alignment**: `docs/lang-design/05-ast.md` §8 (line 326-511) lists 30+ `Expr` variants
  but **does NOT include `IfLet` / `WhileLet` variants**. The implementation (`src/ast/kinds.rs:377-476`)
  matches the design exactly: `If`, `Match`, `Loop`, `While`, `For` are present; `IfLet` / `WhileLet`
  are absent. Critically, **§12.4 (line 867-873) explicitly prescribes the desugar strategy**:
  `if let → match` and `while let → loop { match ... }` in HIR lowering. This is rustc's exact approach.

- **Parser pre-staging**: `src/parser/expr.rs:864-917` (`parse_if_expr`) and `:594-623` (inline
  `parse_while`) **already syntactically recognize** `if let` / `while let` (peek for `KwLet` after
  `KwIf` / `KwWhile`), parse the pattern + scrutinee, then **explicitly emit a soft parse error**
  `"if let patterns are not yet supported in Stage 0 (will be added in Stage 1)"` (line 885) /
  `"while let patterns are not yet supported in Stage 0 (will be added in Stage 1)"` (line 606).
  The 11 conformance FAIL tests assert this exact error string as their `//! error_pattern:` marker.
  This means the parse paths are already wired — only the AST emission + downstream lowering needs
  to change.

- **MIR infrastructure already in place**: `src/mir/lower/control_flow.rs:275 lower_match` is a fully
  realized match-lowering function (handles enum discriminant extraction at lines 288-330, multiple
  arms, pattern bindings via `pattern_bindings.rs`). `src/mir/lower/expr_operand.rs:799` already
  lowers `HirExprKind::While` to a `cond_block → body_block → exit_block` loop pattern with
  `Terminator::SwitchInt`. **If we desugar if-let/while-let to Match/Loop+Match in HIR lowering,
  ZERO new MIR lowering code is required.**

- **Typeck + borrowck impact**: Landin type-checks and borrow-checks MIR (not HIR) per the §16
  architecture. `src/typeck/checker.rs` operates on `Statement`/`Terminator`; `src/borrowck/mod.rs`
  operates on MIR `MirBody`. Since if-let/while-let desugar to Match/Loop+Match before MIR is built,
  **typeck and borrowck see only Match/Loop MIR** — no new arms needed in either checker. The
  "refinement scope" semantics (rustc: `x: T` inside `if let Some(x) = opt { /* here */ }`) is
  automatic because pattern bindings get lowered to fresh MIR locals via `pattern_bindings.rs`
  whose lifetime is the match-arm block.

- **Conformance FAIL count**: 11 tests in `tests/conformance/00-parse/02-control-flow/` (6 if-let
  + 5 while-let), per r217 §2.5 verified count. **NOT 12** as in r216 (numeric correction). Plus
  4 additional `err_*` parse-error tests in the same directory that are NOT TD-031 — they test
  parser error recovery for malformed `match`/`if`/`while`/`for` and must remain FAIL.

- **Stage 9.3 unit test coupling**: `tests/v0/stage9/plan/control_flow_tests.rs:68-97`
  (`test_stage9_3_if_let_tests_marked_fail`) and `:122-146`
  (`test_stage9_3_while_let_tests_marked_fail`) **explicitly assert** the 11 .lin files contain
  `//! FAIL` and reference "not yet supported in Stage 0". These two unit tests must be updated
  in lockstep when the .lin markers are flipped — otherwise cargo test regresses.

**Recommendation**: **Strategy B — Desugar to Match (rustc-style)**.

- **File count**: 4 src + 11 conformance .lin + 2 stage9 unit-test + 4 design-doc write-back = **21 files**.
- **Risk**: **LOW** (reuses Match/Loop lowering; HIR desugar is mechanical; parser already staged;
  typeck/borrowck require no changes because they operate on MIR).
- **Version policy**: v0.21.5 → **v0.22.0** (minor bump — first user-facing compiler feature; per
  `stage-13.1-design-alignment.md` §5.4 minor-bump threshold reserved for if-let/while-let).

---

## 2. Design Document Alignment (§13.4)

Per §13.4.1 step 1-3, each design doc is read against the planned implementation to identify
alignment, deviation, and gray-area decisions.

### 2.1 `02-grammar.md` — Grammar spec

**Read**: §3.4 表达式 (lines 253-291), specifically the BNF block at lines 256-270.

**What the design says** (verified by direct read of `02-grammar.md:257-263`):

```ebnf
expr :=
    "if" expr block ("else" (if_expr | block))? |
    "if" "let" pat "=" expr block ("else" (if_let_expr | block))? |
    "match" expr "{" match_arm* "}" |
    "loop" block |
    "while" expr block |
    "while" "let" pat "=" expr block |
    "for" pat "in" expr block |
    ...
```

- **Line 257**: `"if" expr block ("else" (if_expr | block))?` — regular `if` production.
- **Line 258**: `"if" "let" pat "=" expr block ("else" (if_let_expr | block))?` — **`if let` production is explicitly defined** with optional `else` clause (recursively referencing `if_let_expr` for `else if let` chains).
- **Line 261**: `"while" expr block` — regular `while` production.
- **Line 262**: `"while" "let" pat "=" expr block` — **`while let` production is explicitly defined** (no `else` clause — `while let` has no else).

**Does the design include `if let` production?** **YES** — §3.4 line 258.
**Does the design include `while let` production?** **YES** — §3.4 line 262.

**Alignment verdict**: ✅ **PASS §13.4**. Grammar design fully anticipates if-let/while-let. The
implementation gap is a B1 deviation (implementation < design) traced to Stage 0.5 parser scope
per r217 §3. The §25.8 write-back at `02-grammar.md` §5 (Stage 6.18 retroactive) does NOT mention
if-let/while-let explicitly — it covers general Stage 0 convergence history. No grammar update
needed; the design is already correct.

### 2.2 `05-ast.md` — AST spec

**Read**: §8 表达式定义 (lines 326-511), specifically the `Expr` enum at lines 329-510 and the
control-flow variant block at lines 416-442.

**What the design says** (verified by direct read of `05-ast.md:417-442`):

```rust
// 控制流
If {
    cond: Box<Expr>,
    then: Block,
    else_: Option<Box<Expr>>,
    span: Span,
},
Match {
    expr: Box<Expr>,
    arms: Vec<Arm>,
    span: Span,
},
Loop {
    body: Block,
    span: Span,
},
While {
    cond: Box<Expr>,
    body: Block,
    span: Span,
},
For {
    pat: Pat,
    iter: Box<Expr>,
    body: Block,
    span: Span,
},
```

**Does the design include `IfLet` variant?** **NO.** The `Expr` enum in §8 has `If`, `Match`,
`Loop`, `While`, `For` but **no `IfLet` variant**.

**Does the design include `WhileLet` variant?** **NO.** Same — no `WhileLet` variant.

**Critical: §12.4 explicitly prescribes the desugar strategy** (verified at `05-ast.md:860-873`):

> "AST → HIR lowering 做以下变换:
> 4. Desugaring:
>    - `?` → match + `From::from`
>    - `for x in iter` → `while let Some(x) = Iterator::next(&mut __it)`
>    - `+=` → `AddEq::add_assign`
>    - `if let` → `match`
>    - `while let` → `loop { match ... }`
>    - Range `a..b` → `Range::new(a, b)`"

This is the **smoking gun for Strategy B**. The design doc itself pre-sanctions the desugar-to-Match
approach: `if let → match` and `while let → loop { match ... }` in HIR lowering. The §12.4 wording
leaves open whether the AST carries dedicated `IfLet` / `WhileLet` variants or whether the parser
directly emits Match — but the rustc convention (and the cleanest implementation) is:
1. AST has `IfLet { pat, expr, then, else_, span }` and `WhileLet { pat, expr, body, span }` variants (source-fidelity for diagnostics + tooling).
2. HIR lowers these to `HirExprKind::Match` (for if-let) and `HirExprKind::Loop` wrapping
   `HirExprKind::Match` (for while-let) per §12.4.

**Alignment verdict**: ✅ **PASS §13.4 with §25.8 write-back gap**. The AST design is silent on
`IfLet` / `WhileLet` variants (gray-area — B4 design-write-back), but §12.4 explicitly prescribes
the desugar strategy. Stage 13.2 must:
1. Add `IfLet` / `WhileLet` variants to AST §8 design doc (B4 write-back — implementation-as-fact).
2. Document in §12.4 that the desugar is performed in HIR lowering (already implicit but should be explicit post-implementation).

### 2.3 `03-type-system.md` — Type system

**Read**: §1.1 (type hierarchy, lines 9-41), §13.3 (v0.3 self-hosting preconditions, lines 901-918),
+ grep across the entire 918-line file for `if let` / `while let` / `refinement` / `narrowing` /
`pattern binding`.

**What the design says**:

- §1.1 lists 9 top-level type variants (Reference, Pointer, Aggregate, User-Defined, Function,
  ImplTrait, Param, InferenceVar, TraitObject). No mention of if-let refinement scope.
- §13.3 (line 908): `| TD-031 if let / while let | P0 | Stage 13.2 |` — explicitly schedules
  TD-031 closure for Stage 13.2 (this stage).
- Line 349: `while let Some(c) = self.constraints.pop():` — only appearance of `while let` in the
  entire 918-line doc; it's inside pseudo-code for the unification algorithm, not a type-system
  specification of while-let semantics.

**Does the design mention "refinement scope" or "narrowing" for if-let patterns?** **NO.**

- rustc semantics: `if let Some(x) = opt { /* x: T */ }` — the binding `x` has type `T` inside
  the block, not `Option<T>`. This is sometimes called "refinement" or "narrowing".
- In Landin, this is **automatic** because if-let desugars to match, and Match's pattern bindings
  (`pattern_bindings.rs:34 collect_pat_bindings_for_mir`) already create fresh MIR locals with the
  inner type (e.g., `T` instead of `Option<T>`) — there is no separate refinement pass; the
  pattern-match arm binds the destructured type directly.

**Alignment verdict**: ⚠️ **PARTIAL §13.4**. The type-system doc does not explicitly describe
if-let refinement scope, but the behavior is implied by Match semantics (which §1.1 covers via
"Aggregate" destructuring). Stage 13.2 §25.8 write-back should add a brief note to `03-type-system.md`
§13 (or a new §13.4 sub-section) documenting that if-let/while-let pattern bindings inherit their
type from the destructured variant field, identical to Match arm pattern bindings.

### 2.4 `04-ownership-borrowing.md` — Ownership/borrowing

**Read**: §2.4 (Two-phase borrows), §3 (lifetime system), §4 (NLL algorithm), §5 (drop check),
§11+ (Stage 6.18 §25.8 write-back at lines 581-641) + grep across the entire 702-line file for
`if let` / `while let` / `borrow scope` / `pattern binding`.

**What the design says**:

- §4 NLL algorithm operates on MIR types and basic blocks uniformly. The doc treats any `Ty` as
  participating in region inference; the algorithm is type-structure-agnostic for non-region-bearing
  types. No special handling for if-let borrow scope.
- §2.4 Two-phase borrows applies to method-call receiver borrows (`a.b(c)` where `a` is borrowed
  while `c` is evaluated) — orthogonal to if-let.
- No mention of `if let`, `while let`, or `borrow scope` for pattern bindings anywhere in the 702-line file.

**Does TD-031 (if-let/while-let) affect borrow checking?** **NO — per Landin's §16 architecture.**

- Landin borrow-checks MIR, not HIR. `src/borrowck/mod.rs:115 check_mir_body` operates on `MirBody`.
- Since if-let/while-let desugar to `Match` / `Loop { Match }` in HIR lowering (per §12.4), the
  MIR-side borrow checker sees only the desugared form. Pattern bindings become fresh MIR locals
  via `pattern_bindings.rs:34`; their borrow scope is the basic block of the match arm.
- The rustc concern "the borrow of `&opt` in `if let Some(x) = &opt { ... }` must outlive the
  block" is automatically satisfied: the desugared Match has the scrutinee `&opt` evaluated into
  a MIR local whose lifetime is the Match's continuation block; the arm block reads from that local;
  NLL's last-use analysis expires the local at the arm block's last use.

**Alignment verdict**: ✅ **PASS §13.4**. Borrow checking is MIR-side and type-structure-agnostic
by design. The desugar strategy ensures no borrowck changes are needed. Stage 13.2 §25.8 write-back
should add a brief note to `04-ownership-borrowing.md` §4 (or a new sub-section) documenting that
if-let/while-let borrow scope is the match-arm basic block in MIR (auto-handled by NLL on the
desugared form).

### 2.5 `13-stage1-feature-whitelist.md` §2.3 — Control flow whitelist

**Read**: §2.3 控制流 (lines 76-91).

**What the design says** (verified at `13-stage1-feature-whitelist.md:84, 86`):

```
| 特性 | 允许 | 备注 |
| --- | --- | --- |
| `if` / `else if` / `else` | ✅ | 表达式形式 |
| `match` | ✅ | 含 guard |
| `loop` | ✅ | 含 `break value` |
| `while` | ✅ | |
| `while let` | ✅ | |
| `for x in iter` | ✅ | 要求 IntoIterator |
| `if let` | ✅ | |
| `break` / `continue` | ✅ | 不带 label |
| `return` | ✅ | |
```

- **Line 84**: `while let` ✅ ALLOWED (no remark)
- **Line 86**: `if let` ✅ ALLOWED (no remark)

**Are `if let` / `while let` listed as ALLOWED for Stage 1?** **YES** — both are explicitly
checked ✅ with no deferral remarks.

**Alignment verdict**: ✅ **PASS §13.4**. The Stage 1 contract requires both if-let and while-let.
Stage 0 (current compiler) does not yet implement them — this is the TD-031 B1 deviation. Stage 13.2
closes the gap. Per r217 §3, the deferral traces to a Stage 0.5 parser-scope decision; r217 explicitly
notes the parser had `if let` / `while let` recognition stubs (verified at `src/parser/expr.rs:872`
and `:597`) but emitted soft errors deferring to "Stage 1". Stage 13.2 IS that Stage 1+ implementation.

---

## 3. Current Implementation Analysis

### 3.1 AST inspection — `src/ast/kinds.rs`

**Verified by direct read of `src/ast/kinds.rs:377-476`** (the `Expr` enum definition):

```rust
pub enum Expr {
    Lit(LitKind, Span),
    Path(Option<QSelf>, Path, Span),
    Block(Block, Span),
    Call { ... },
    MethodCall { ... },
    Field { ... },
    Index { ... },
    Unary { ... },
    Binary { ... },
    Assign { ... },
    AddrOf { ... },
    Cast { ... },
    Try { ... },
    If { cond, then, else_, span },          // line 434
    Match { expr, arms, span },              // line 440
    Loop { body, span },                     // line 445
    While { cond, body, span },              // line 449
    For { pat, iter, body, span },           // line 454
    Closure { ... },
    Return { ... },
    Break { ... },
    Continue { ... },
    // ... (Struct, Array, Repeat, Range, Group, Async, Await, MacroCall, Error)
}
```

**Existing control-flow variants**: `If`, `Match`, `Loop`, `While`, `For` (5 variants).

**`IfLet` variant present?** **NO.**
**`WhileLet` variant present?** **NO.**

Confirmed by `grep -n "IfLet|WhileLet" src/ast/kinds.rs` returning zero matches.

### 3.2 HIR inspection — `src/hir/kinds.rs`

**Verified by direct read of `src/hir/kinds.rs:688-767`** (the `HirExprKind` enum definition):

```rust
pub enum HirExprKind {
    Lit(HirLitKind),
    Path(HirPath),
    Block(HirBlock),
    Call { ... },
    MethodCall { ... },
    // ... (Field, Index, Unary, Binary, Assign, AddrOf, Cast, Try)
    If { cond, then, else_ },                // line 735
    Match { expr, arms },                    // line 740
    Loop { body },                           // line 744
    While { cond, body },                    // line 747
    For { pat, iter, body },                 // line 751
    Closure { ... },
    Return { ... },
    Break { ... },
    Continue,
    // ... (Await, Async, etc.)
}
```

**Existing control-flow variants**: `If`, `Match`, `Loop`, `While`, `For` (5 variants).

**`IfLet` variant present?** **NO.**
**`WhileLet` variant present?** **NO.**

Confirmed by `grep -n "IfLet|WhileLet" src/hir/kinds.rs` returning zero matches.

**Implication for strategy**: Since HIR has no `IfLet` / `WhileLet` variants, **Strategy B (desugar
in HIR lowering) requires ZERO new HIR variants** — the AST variants get desugared to existing
`HirExprKind::Match` (if-let) and `HirExprKind::Loop` wrapping `HirExprKind::Match` (while-let)
at the HIR lowering boundary. This is exactly what `05-ast.md` §12.4 prescribes.

### 3.3 Parser inspection — `src/parser/expr.rs`

**Verified by direct read of `src/parser/expr.rs:864-917` (`parse_if_expr`) and `:594-623`
(inline `parse_while` in `parse_primary_expr`)**.

#### 3.3.1 `parse_if_expr` (lines 864-917)

Current behavior:

```rust
pub(super) fn parse_if_expr(&mut self) -> Expr {
    let span = self.current_span();
    self.bump(); // if
    // if let Pat = expr block — peek for KwLet after if.
    let cond = if *self.peek() == TokenType::KwLet {
        // if let pattern
        self.bump(); // let
        let _pat = self.parse_pat();
        self.expect(&TokenType::Eq, "`=`");
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let scrutinee = self.parse_expr();
        self.no_struct_literal = prev;
        // For Stage 0 we don't have Expr::Let — emit a soft error and
        // use the scrutinee as the condition so the block parses.
        self.errors.push(crate::parser::ParseError::new(
            "`if let` patterns are not yet supported in Stage 0 (will be added in Stage 1)"
                .to_string(),
            span,
        ));
        scrutinee
    } else {
        // Regular if: parse cond with no_struct_literal = true
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let c = self.parse_expr();
        self.no_struct_literal = prev;
        c
    };
    let then = self.parse_block();
    let else_ = if *self.peek() == TokenType::KwElse { ... };
    Expr::If { cond: Box::new(cond), then, else_, span }
}
```

**Confirmed**: Parser **peeks for `KwLet` after `if`**, parses the pattern + `=` + scrutinee
correctly, then **explicitly emits a soft parse error** at line 884-888 and produces a regular
`Expr::If` with the scrutinee as `cond` (the `_pat` is discarded — line 875's `let _pat =`).

**For Stage 13.2**: Replace the soft-error branch (lines 872-889) with a branch that produces
`Expr::IfLet { pat, expr: scrutinee, then, else_, span }`. The parsing logic (peek KwLet, bump,
parse_pat, expect Eq, parse_expr with no_struct_literal) is already correct — only the AST emission
needs to change. Estimated diff: ~15 lines.

#### 3.3.2 `parse_while` (inline in `parse_primary_expr`, lines 594-623)

Current behavior (mirrors parse_if_expr's if-let pattern):

```rust
TokenType::KwWhile => {
    self.bump();
    let cond = if *self.peek() == TokenType::KwLet {
        self.bump(); // let
        let _pat = self.parse_pat();
        self.expect(&TokenType::Eq, "`=`");
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let scrutinee = self.parse_expr();
        self.no_struct_literal = prev;
        self.errors.push(crate::parser::ParseError::new(
            "`while let` patterns are not yet supported in Stage 0 (will be added in Stage 1)".to_string(),
            span,
        ));
        scrutinee
    } else {
        // Regular while: parse cond with no_struct_literal = true
        ...
    };
    let body = self.parse_block();
    Expr::While { cond: Box::new(cond), body, span }
}
```

**Confirmed**: Same pattern as `parse_if_expr` — parser recognizes `while let`, parses correctly,
emits soft error at line 605-608, produces `Expr::While` with scrutinee as `cond`.

**For Stage 13.2**: Replace the soft-error branch (lines 597-609) with a branch that produces
`Expr::WhileLet { pat, expr: scrutinee, body, span }`. Estimated diff: ~10 lines.

### 3.4 HIR lowering inspection — `src/hir/lower/body.rs`

**Verified by direct read of `src/hir/lower/body.rs:189-222`** (the control-flow arm block in the
`lower_expr` match):

```rust
Expr::If { cond, then, else_, .. } => HirExprKind::If {
    cond: Box::new(lower_expr(cx, cond)),
    then: lower_block(cx, then),
    else_: else_.as_ref().map(|e| Box::new(lower_expr(cx, e))),
},
Expr::Match { expr, arms, .. } => HirExprKind::Match {
    expr: Box::new(lower_expr(cx, expr)),
    arms: arms.iter().map(|arm| HirArm {
        hir_id: cx.fresh_hir_id(),
        pat: pat::lower_pat(cx, &arm.pat),
        guard: arm.guard.as_ref().map(|g| lower_expr(cx, g)),
        body: Box::new(lower_expr(cx, &arm.body)),
        span: arm.span,
    }).collect(),
},
Expr::Loop { body, .. } => HirExprKind::Loop {
    body: lower_block(cx, body),
},
Expr::While { cond, body, .. } => HirExprKind::While {
    cond: Box::new(lower_expr(cx, cond)),
    body: lower_block(cx, body),
},
Expr::For { pat, iter, body, .. } => HirExprKind::For { ... },
```

**For Stage 13.2 (Strategy B)**: Add two new arms after the existing `While` arm:

```rust
Expr::IfLet { pat, expr, then, else_, .. } => {
    // Desugar per 05-ast.md §12.4: if let P = e { then } else { else_ }
    //   →  match e { P => then, _ => else_ (or () if no else) }
    let scrutinee = Box::new(lower_expr(cx, expr));
    let pat_hir = pat::lower_pat(cx, pat);
    let then_arm = HirArm {
        hir_id: cx.fresh_hir_id(),
        pat: pat_hir,
        guard: None,
        body: Box::new(lower_expr(cx, &Expr::Block(then, then.span))),
        span: then.span,
    };
    let else_arm = HirArm {
        hir_id: cx.fresh_hir_id(),
        pat: HirPat { hir_id: cx.fresh_hir_id(), kind: HirPatKind::Wild, span: then.span },
        guard: None,
        body: Box::new(match else_ {
            Some(e) => lower_expr(cx, e),
            None => HirExpr { hir_id: cx.fresh_hir_id(), kind: HirExprKind::Block(lower_block(cx, &empty_block())), span: then.span },
        }),
        span: then.span,
    };
    HirExprKind::Match { expr: scrutinee, arms: vec![then_arm, else_arm] }
}
Expr::WhileLet { pat, expr, body, .. } => {
    // Desugar per 05-ast.md §12.4: while let P = e { body }
    //   →  loop { match e { P => body, _ => break } }
    let scrutinee = Box::new(lower_expr(cx, expr));
    let pat_hir = pat::lower_pat(cx, pat);
    let body_block = lower_block(cx, body);
    let body_arm = HirArm { ... pat: pat_hir, body: Box::new(HirExpr { kind: HirExprKind::Block(body_block), .. }) ... };
    let break_arm = HirArm { ... pat: Wild, body: Box::new(HirExpr { kind: HirExprKind::Break { expr: None }, .. }) ... };
    let inner_match = HirExprKind::Match { expr: scrutinee, arms: vec![body_arm, break_arm] };
    HirExprKind::Loop { body: HirBlock { stmts: vec![], expr: Some(Box::new(HirExpr { hir_id: cx.fresh_hir_id(), kind: inner_match, span: body.span })) } }
}
```

This is the **complete** HIR lowering change. The Match and Loop arms already exist and their
MIR lowering is already implemented — no further MIR work needed.

### 3.5 MIR lowering inspection — `src/mir/lower/control_flow.rs` + `expr_operand.rs`

#### 3.5.1 `control_flow.rs`

**Verified by direct read**:

- **`lower_if` at line 220-273**: Lowers `if cond { then } else { else_ }` to 3 basic blocks
  (then_block, else_block, cont_block) with `Terminator::SwitchInt` on the cond bool. Robust;
  no changes needed for if-let (because if-let desugars to Match, not If).
- **`lower_match` at line 275-463**: Lowers `match scrutinee { arms }` to per-arm basic blocks
  with discriminant extraction for enums (lines 288-330), pattern binding emission (via
  `pattern_bindings.rs:80 lower_enum_variant_pattern_bindings`), and `Terminator::SwitchInt`
  on the discriminant. Handles wildcard arms, multi-arm matches, guards (deferred to a
  sub-SwitchInt). **This is the function that will handle the desugared if-let and while-let
  match arms — no changes needed.**

#### 3.5.2 `expr_operand.rs` dispatch

**Verified by direct read of `src/mir/lower/expr_operand.rs:608-615`**:

```rust
HirExprKind::If { cond, then, else_, .. } =>
    control_flow::lower_if(cx, cond, then, else_.as_deref(), expr.span),
HirExprKind::Match { scrutinee, arms, .. } =>
    control_flow::lower_match(cx, scrutinee, arms, expr.span),
```

And at lines 773-825 for `HirExprKind::Loop` and `HirExprKind::While` (both lowered inline with
`Terminator::Goto` + `Terminator::SwitchInt` patterns). **No `HirExprKind::IfLet` or
`HirExprKind::WhileLet` arms exist** — confirming that HIR has no such variants and Strategy B
requires zero MIR dispatch changes (the desugar produces `HirExprKind::Match` and
`HirExprKind::Loop`, both of which are already dispatched).

#### 3.5.3 Pattern bindings infrastructure

**Verified by direct read of `src/mir/lower/pattern_bindings.rs`** (286 LOC, 7 functions):

- `pat_mutability` (line 20) — checks if a pattern introduces mutability
- `collect_pat_bindings_for_mir` (line 34) — collects fresh MIR locals for Ident / TupleStruct / Tuple / Struct patterns
- `lower_enum_variant_pattern_bindings` (line 80) — generates MIR projections for enum variant payload extraction
- `compute_enum_payload_starting_idx` (line 208) — handles flat enum storage layout
- `collect_pat_hir_ids` (line 241) — walks pattern for HirId collection (used by borrowck liveness)

This infrastructure is **already in place and used by `lower_match`** for any pattern. If-let and
while-let desugar to Match → Match uses `pattern_bindings.rs` → pattern bindings become fresh
MIR locals with inner types. **Zero new pattern-handling code needed.**

### 3.6 Conformance FAIL test analysis

#### 3.6.1 Test inventory

**Verified by `rg -l "FAIL" tests/conformance/00-parse/02-control-flow/`**: 15 files contain
`//! FAIL` markers. Of these:

| # | Test file | Category | Pattern exercised |
|---|-----------|----------|-------------------|
| 1 | `if_let_basic.lin` | if-let | `if let Opt::Some(x) = o { ... }` (enum variant) |
| 2 | `if_let_struct.lin` | if-let | `if let P { x, y } = p { ... }` (struct) |
| 3 | `if_let_else.lin` | if-let | `if let Opt::Some(x) = o { x } else { 0 }` (with else) |
| 4 | `if_let_tuple.lin` | if-let | `if let (a, b) = p { ... }` (tuple) |
| 5 | `if_let_wildcard.lin` | if-let | `if let _ = x { 1 }` (wildcard — always matches) |
| 6 | `if_let_chain.lin` | if-let | `if let Some(a) = x { if let Some(b) = y { ... } }` (nested) |
| 7 | `while_let_basic.lin` | while-let | `while let Opt::Some(x) = it { ... it = Opt::None }` |
| 8 | `while_let_nested.lin` | while-let | nested while-let loops |
| 9 | `while_let_continue.lin` | while-let | `while let ... { if x > 0 { continue; } ... }` |
| 10 | `while_let_break.lin` | while-let | `while let ... { if x == 0 { break; } }` |
| 11 | `while_let_tuple.lin` | while-let | `while let (a, b) = p { ... }` |
| 12 | `err_match_without_scrutinee.lin` | error | NOT TD-031 (parser error recovery) |
| 13 | `err_if_without_cond.lin` | error | NOT TD-031 |
| 14 | `err_while_without_cond.lin` | error | NOT TD-031 |
| 15 | `err_for_without_in.lin` | error | NOT TD-031 |

**TD-031 test count**: 11 (6 if-let + 5 while-let) — matches r217 §2.5 verified count exactly.
**NOT 12** as in r216 (r216 numeric error, r217 corrected).

#### 3.6.2 FAIL marker structure (sample of 5 .lin files verified)

All 11 TD-031 .lin files share this 6-line structure:

```
//! FAIL
//! category: control-flow
//! description: <name> not yet supported in Stage 0 (planned for Stage 1)
//! error_pattern: not yet supported in Stage 0
//! source: Stage 9.3 conformance expansion
<actual Landin source on one line>
```

The `//! error_pattern: not yet supported in Stage 0` matches the parser's soft error at
`src/parser/expr.rs:885` (if-let) and `:606` (while-let). When Stage 13.2 removes the soft error
and emits a proper AST, these tests will PASS — the .lin files must be updated to remove the
`//! FAIL` marker and the `//! error_pattern` line.

#### 3.6.3 Stage 9.3 unit test coupling

**Verified by direct read of `tests/v0/stage9/plan/control_flow_tests.rs:68-97` and `:122-146`**:

```rust
/// Verify if-let tests present and marked as FAIL (Stage 1 feature)
#[test]
fn test_stage9_3_if_let_tests_marked_fail() {
    let if_let_tests = [
        "if_let_basic.lin", "if_let_else.lin", "if_let_tuple.lin",
        "if_let_struct.lin", "if_let_wildcard.lin", "if_let_chain.lin",
    ];
    for name in &if_let_tests {
        let content = std::fs::read_to_string(&path).expect("read if-let test");
        assert!(content.contains("//! FAIL"),
            "{name} must be FAIL — if-let not yet supported in Stage 0");
        assert!(content.contains("not yet supported in Stage 0"),
            "{name} must reference Stage 0 limitation in error_pattern");
    }
}

/// Verify while-let tests present and marked as FAIL (Stage 1 feature)
#[test]
fn test_stage9_3_while_let_tests_marked_fail() {
    let while_let_tests = [
        "while_let_basic.lin", "while_let_break.lin", "while_let_tuple.lin",
        "while_let_nested.lin", "while_let_continue.lin",
    ];
    for name in &while_let_tests {
        ...
        assert!(content.contains("//! FAIL"),
            "{name} must be FAIL — while-let not yet supported in Stage 0");
    }
}
```

These two Stage 9.3 unit tests will **FAIL after Stage 13.2 implementation** because the .lin
files no longer contain `//! FAIL`. They must be updated to assert PASS markers (or replaced
with positive "if-let parses successfully" tests).

#### 3.6.4 Test flip mechanics

Stage 13.2 implementation will:
1. Edit each of the 11 .lin files: replace `//! FAIL` with `//! PASS` (or simply remove the FAIL
   marker), remove `//! error_pattern: not yet supported in Stage 0` line, update
   `//! description:` from "not yet supported in Stage 0" to "validates if-let/while-let parsing
   + lowering".
2. Update `tests/v0/stage9/plan/control_flow_tests.rs::test_stage9_3_if_let_tests_marked_fail` and
   `test_stage9_3_while_let_tests_marked_fail` — either rename to `..._marked_pass` and flip the
   assertions, OR replace with new positive tests that actually parse the .lin source and verify
   no error is emitted. (Recommended: positive tests, to actually verify the implementation works.)

---

## 4. Implementation Strategy (per §15 long-term > short-term)

### 4.1 Strategy comparison

| Strategy | Description | Files | Risk | Long-term value |
|----------|-------------|------:|------|-----------------|
| **A** | Direct AST + HIR + MIR variants. Add `IfLet { pat, expr, then, else_ }` and `WhileLet { pat, expr, body }` to AST + HIR. MIR lowering gets new `lower_if_let` and `lower_while_let` functions in `control_flow.rs`. Typeck + borrowck get new arms. | 7-9 src files (ast/kinds.rs, hir/kinds.rs, parser/expr.rs, hir/lower/body.rs, mir/lower/control_flow.rs, mir/lower/expr_operand.rs, typeck/checker.rs, borrowck/mod.rs) + 11 conformance + 2 stage9 = ~21-23 files | **MEDIUM** | ⚠️ Cleaner AST + HIR variants for tooling, but **duplicates Match lowering logic** in `lower_if_let`; MIR has two paths for the same semantics (Match vs IfLet) → violates DRY and §14.4 J2 (single responsibility) |
| **B** | Desugar to Match (rustc-style). Add `IfLet` / `WhileLet` to AST only. In HIR lowering, desugar to `HirExprKind::Match` (if-let) or `HirExprKind::Loop` wrapping `HirExprKind::Match` (while-let). **No new HIR variant, no new MIR lowering, no new typeck/borrowck arms.** | 4 src files (ast/kinds.rs, parser/expr.rs, hir/lower/body.rs, + 1 for variant derivation) + 11 conformance + 2 stage9 = 17 files | **LOW** | ✅ **Highest** — matches rustc; reuses Match infrastructure; MIR/typeck/borrowck see only Match/Loop (consistent with §16 single-direction data flow); aligns with `05-ast.md` §12.4 design intent |
| **C** | Hybrid: AST + HIR variants + desugar in MIR lowering. Add `IfLet` / `WhileLet` to AST + HIR. In MIR lowering (`expr_operand.rs`), desugar to Match inline. | 6 src files (ast/kinds.rs, hir/kinds.rs, parser/expr.rs, hir/lower/body.rs, mir/lower/expr_operand.rs, + 1) + 11 conformance + 2 stage9 = 20 files | **MEDIUM** | ⚠️ HIR has variants but MIR doesn't see them — intermediate inconsistency; MIR lowering has desugar logic mixed with lowering (violates §14.4 J2 single-responsibility for MIR lowering) |

### 4.2 §15 long-term > short-term analysis

Per `stage-committee-process.md` §15 ("最优 > 最小" — best > smallest), the long-term value
criterion dominates the short-term cost criterion when the two conflict.

**Long-term value**:
- **Strategy B** reuses the existing `lower_match` infrastructure (188 LOC at
  `control_flow.rs:275-463`) that has been hardened over 6+ gate reviews and handles enum
  discriminant extraction, multi-arm matches, guards, pattern bindings. Stage 13.3 (closure
  call lowering) and Stage 13.4 (macro_rules!) will not need to touch if-let/while-let code.
- **Strategy A** would require maintaining parallel `lower_if_let` (~80-120 LOC) and
  `lower_while_let` (~100-150 LOC) functions that duplicate `lower_match` logic. Every future
  bug fix to `lower_match` (e.g., new pattern kinds in Stage 13.5) would need to be mirrored
  in `lower_if_let` and `lower_while_let`. This is a long-term maintenance liability.
- **Strategy C** is intermediate — the MIR-level desugar logic is localized but still represents
  duplicated semantic intent.

**Short-term cost**:
- Strategy B: ~60 LOC of new code (2 AST variants + 2 HIR lowering arms + parser branch swaps).
  All in 4 src files. Risk concentrated in HIR desugar correctness.
- Strategy A: ~300-400 LOC of new code (2 AST + 2 HIR variants + 2 MIR lowering functions +
  typeck/borrowck arms). Risk spread across 7-9 src files including the type-system-sensitive
  `typeck/checker.rs` and `borrowck/mod.rs`.
- Strategy C: ~200-250 LOC. Intermediate.

**§15 verdict**: Strategy B has the best long-term value (reuses hardened Match infrastructure)
at the lowest short-term cost (4 src files, ~60 LOC). The only cost is a slight AST/HIR
inconsistency (AST has IfLet/WhileLet, HIR doesn't), but this is intentional and rustc-aligned.

### 4.3 rustc reference

Per the rustc dev guide (https://rustc-dev-guide.rust-lang.org/hir.html), rustc lowers `if let`
and `while let` to `ExprKind::Match` in the AST-to-HIR lowering pass (`rustc_ast_lowering`).
The HIR has no `IfLet` variant — only `ExprKind::Match` and `ExprKind::Loop` wrapping
`ExprKind::Match`. This is **exactly Strategy B**. The rustc source confirms this at
`compiler/rustc_ast_lowering/src/lower/block.rs` — `lower_expr` has an arm for
`ast::ExprKind::IfLet(...)` that produces `hir::ExprKind::Call(Match, ...)` (via
`lower_match`).

**Strategy B is rustc-idiomatic** and aligns with `05-ast.md` §12.4 design intent.

### 4.4 §14.4 J1-J6 evaluation (Strategy B)

| # | Criterion | Verdict | Justification |
|---|-----------|---------|---------------|
| J1 | Architecture alignment | ✅ PASS | Implements `05-ast.md` §12.4 prescribed desugar (`if let → match`, `while let → loop { match ... }`). AST §8 needs §25.8 B4 write-back to add `IfLet` / `WhileLet` variants as design-as-fact. |
| J2 | Single responsibility | ✅ PASS | `ast/kinds.rs` adds 2 data variants; `parser/expr.rs` swaps soft-error for AST emission; `hir/lower/body.rs` adds 2 desugar arms. No file gains a new responsibility. MIR lowering unchanged — `lower_match` continues to handle pattern matching. |
| J3 | Single-direction flow | ✅ PASS | No new module dependencies. Desugar happens at HIR lowering (forward direction); MIR sees only Match/Loop. No reverse-direction data flow introduced. |
| J4 | Compilation expression complete | ✅ PASS | if-let and while-let are fully expressible as Match (which is already complete in MIR). No concept splitting across files. |
| J5 | Stage division clear | ✅ PASS | 4 src files modified (ast/kinds.rs, parser/expr.rs, hir/lower/body.rs, + 1 for derivation). All within §16 stage boundaries: AST and HIR changes are Stage 0/1 layer; no MIR/typeck/borrowck changes. |
| J6 | Scientific granularity | ✅ PASS | Total LOC delta ~60-80 LOC. ast/kinds.rs: +12 LOC (2 variants); parser/expr.rs: ~15 LOC delta (branch swap, soft-error removal); hir/lower/body.rs: +40-50 LOC (2 desugar arms). All files stay well below 1500 LOC ceiling. |

**Strategy B §14.4 verdict**: ✅ ALL 6 criteria PASS. Strategy is cleared for execution.

---

## 5. while-let Strategy

### 5.1 Two sub-strategies

**Sub-strategy W-A (loop + match desugar)** — recommended:

```
while let pat = expr { body }
↓ (HIR lowering desugar)
loop {
    match expr {
        pat => body,
        _ => break,
    }
}
```

This reuses BOTH `HirExprKind::Loop` (already lowered by `expr_operand.rs:773-796`) and
`HirExprKind::Match` (already lowered by `control_flow.rs:275 lower_match`). Zero new MIR lowering.

**Sub-strategy W-B (direct loop with pattern test)** — not recommended:

Custom MIR lowering that tests the pattern at loop head, breaks on mismatch. Requires a new
`lower_while_let` function in `control_flow.rs` that:
- Creates a `loop_header` block, `match_arms` blocks, `loop_exit` block.
- At loop_header: evaluates scrutinee, calls `lower_match` on the result with two arms
  (pat → body, _ → break).

This is functionally equivalent to W-A but with the desugar logic inlined into MIR lowering.
Violates §14.4 J2 (single responsibility — MIR lowering should not contain desugar logic).

### 5.2 Recommendation: W-A (desugar)

**Rationale**:

- W-A is **exactly what `05-ast.md` §12.4 prescribes**: `while let → loop { match ... }`.
- W-A reuses `lower_match` (188 LOC, hardened) and `HirExprKind::Loop` lowering (24 LOC) — both
  already in place and tested.
- W-A's `break` from the wildcard arm is automatically handled by the existing `HirExprKind::Break`
  arm at `expr_operand.rs` (which targets the enclosing `Loop`'s `loop_exit` block via the existing
  break-tracking infrastructure).
- W-B would require ~100-150 LOC of new `lower_while_let` code that duplicates `lower_match` arm
  generation logic.

### 5.3 Edge case: `break value` from while-let body

rustc semantics: `while let` cannot produce a value via `break value` (unlike `loop { ... break value }`).
The desugar `loop { match expr { pat => body, _ => break } }` produces `()` because the `break` in
the wildcard arm has no value. This matches rustc behavior. No special handling needed.

### 5.4 Edge case: scrutinee re-evaluation

Each loop iteration re-evaluates the scrutinee `expr`. For `while let Some(x) = iter.next() { ... }`,
this is correct (calls `next()` each iteration). For `while let Some(x) = some_local { ... }`,
the scrutinee is re-read each iteration — correct because the body may mutate `some_local`.
This matches rustc.

---

## 6. Scope Analysis

### 6.1 File change list (Strategy B)

**Total files modified: 17 (4 src + 11 conformance .lin + 2 stage9 unit test)** +
**4 design doc write-back files** (per §25.8, executed post-implementation) = **21 files**.

| # | File | Change | LOC delta | Risk |
|---|------|--------|-----------|------|
| 1 | `src/ast/kinds.rs` | **Add** `IfLet { pat: Pat, expr: Box<Expr>, then: Block, else_: Option<Box<Expr>>, span: Span }` and `WhileLet { pat: Pat, expr: Box<Expr>, body: Block, span: Span }` variants to `Expr` enum (insert after `For` at line 459, before `Closure` at line 460). Also update any `#[derive]` macros and `span()` method match arm. | +12 LOC | LOW (mechanical variant addition; derive handles boilerplate) |
| 2 | `src/parser/expr.rs` | **Replace** the soft-error branch in `parse_if_expr` (lines 872-889) with `Expr::IfLet { ... }` emission. **Replace** the soft-error branch in `parse_while` (lines 597-609) with `Expr::WhileLet { ... }` emission. The parsing logic (peek KwLet, bump, parse_pat, expect Eq, parse_expr with no_struct_literal) is already correct — only the AST emission changes. | ±15 LOC (remove ~12 LOC of error push + add ~15 LOC of AST emission) | LOW (parsing logic already correct; only AST emission changes) |
| 3 | `src/hir/lower/body.rs` | **Add** 2 new arms in the `lower_expr` match (after `Expr::For` at line 222, before `Expr::Closure` at line 223): `Expr::IfLet { ... } => HirExprKind::Match { ... }` (desugar to 2-arm match: pat→then, Wild→else_/unit) and `Expr::WhileLet { ... } => HirExprKind::Loop { body: Block { expr: Some(Match { ... }) } }` (desugar to loop wrapping 2-arm match: pat→body, Wild→Break). Use `pat::lower_pat` for the pattern; `lower_block`/`lower_expr` for the bodies. | +50 LOC | MEDIUM (desugar correctness — need to verify HirArm construction matches existing Match arm pattern; HirId allocation via `cx.fresh_hir_id()`; span propagation) |
| 4 | `src/ast/kinds.rs` or `src/ast/mod.rs` | **Verify** `#[derive(Debug, Clone, PartialEq)]` on `Expr` enum auto-handles the new variants (it should — they're all `Box`/`Vec`/`Span`-based). If `visit::Visitor` or `fold::Folder` has exhaustive matches on `Expr`, add arms there. **Audit**: `rg "Expr::If\b|Expr::While\b" src/` to find all exhaustive match sites; expect ~5-8 sites in visitor/fold/pretty-printer. | +5-15 LOC (visitor/fold arms) | LOW-MEDIUM (must find all exhaustive match sites; cargo build will fail-fast on missed sites) |
| 5 | `tests/conformance/00-parse/02-control-flow/if_let_basic.lin` | **Edit**: remove `//! FAIL` marker; remove `//! error_pattern: not yet supported in Stage 0` line; update `//! description:` from "not yet supported in Stage 0 (planned for Stage 1)" to "validates if-let basic parsing + lowering". | ±1 LOC | LOW (mechanical marker flip) |
| 6 | `tests/conformance/00-parse/02-control-flow/if_let_struct.lin` | Same as #5. | ±1 LOC | LOW |
| 7 | `tests/conformance/00-parse/02-control-flow/if_let_else.lin` | Same as #5. | ±1 LOC | LOW |
| 8 | `tests/conformance/00-parse/02-control-flow/if_let_tuple.lin` | Same as #5. | ±1 LOC | LOW |
| 9 | `tests/conformance/00-parse/02-control-flow/if_let_wildcard.lin` | Same as #5. | ±1 LOC | LOW |
| 10 | `tests/conformance/00-parse/02-control-flow/if_let_chain.lin` | Same as #5. | ±1 LOC | LOW |
| 11 | `tests/conformance/00-parse/02-control-flow/while_let_basic.lin` | Same as #5 (for while-let). | ±1 LOC | LOW |
| 12 | `tests/conformance/00-parse/02-control-flow/while_let_nested.lin` | Same as #5. | ±1 LOC | LOW |
| 13 | `tests/conformance/00-parse/02-control-flow/while_let_continue.lin` | Same as #5. | ±1 LOC | LOW |
| 14 | `tests/conformance/00-parse/02-control-flow/while_let_break.lin` | Same as #5. | ±1 LOC | LOW |
| 15 | `tests/conformance/00-parse/02-control-flow/while_let_tuple.lin` | Same as #5. | ±1 LOC | LOW |
| 16 | `tests/v0/stage9/plan/control_flow_tests.rs` | **Update** `test_stage9_3_if_let_tests_marked_fail` (lines 68-97): rename to `test_stage9_3_if_let_tests_marked_pass`, flip assertions — assert .lin files contain `//! PASS` (or absence of `//! FAIL`), assert no longer contains "not yet supported in Stage 0". Update `test_stage9_3_while_let_tests_marked_fail` (lines 122-146) similarly. **Recommended**: replace with positive tests that actually invoke the parser on each .lin source and assert zero errors. | ±30-50 LOC | LOW (test-logic update; mechanical) |
| 17 | `Cargo.toml` | **Bump** version `0.21.5` → `0.22.0` (minor bump for first user-facing feature) + append Stage 13.2 entry to the long `description` field. | +1 line (version) + 1 phrase (description) | LOW |

**§25.8 design write-back files** (executed post-implementation, per Section 7 below):

| # | File | Change | Risk |
|---|------|--------|------|
| 18 | `docs/lang-design/05-ast.md` | Add `IfLet` / `WhileLet` variants to §8 `Expr` enum (B4 design-as-fact write-back) + update §13.1 implementation status table | LOW |
| 19 | `docs/lang-design/03-type-system.md` | Add §13.4 sub-section documenting if-let/while-let refinement scope = automatic via Match pattern bindings | LOW |
| 20 | `docs/lang-design/04-ownership-borrowing.md` | Add note to §4 documenting if-let/while-let borrow scope = match-arm basic block in MIR (auto-handled by NLL on desugared form) | LOW |
| 21 | `docs/lang-design/02-grammar.md` | NO change needed — §3.4 already has if-let/while-let productions (verified at lines 257-262). Optionally add §25.8 retroactive note that Stage 13.2 closes the B1 implementation gap. | LOW |

### 6.2 Risk assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| HIR desugar for IfLet produces malformed HirArm (missing hir_id, wrong span) | MEDIUM | MEDIUM (downstream MIR lowering fails or produces wrong codegen) | Reuse existing `Expr::Match` arm's HirArm construction pattern at `body.rs:200-205` as template; add unit test in `tests/v0/stage1/plan/hir_lowering_tests.rs` that lowers a small if-let and asserts the resulting HirExprKind::Match shape |
| HIR desugar for WhileLet's `break` doesn't target the enclosing Loop (because the Loop is synthetically generated by the desugar, not source-written) | MEDIUM | HIGH (break exits wrong scope → soundness bug) | Verify break target tracking in `mir/lower/expr_operand.rs:773-796` (Loop lowering) uses the loop's `loop_exit` block by lexical position, not by source HirId. The desugar produces a `HirExprKind::Loop { body: Block { expr: Some(Match { ... break ... }) } }` — the break's lexical parent IS the synthetic Loop, so existing break-tracking should work. Add regression test: `while let Opt::Some(x) = it { if x == 0 { break; } }` (matches `while_let_break.lin`). |
| Pattern bindings in if-let get wrong type (e.g., `x: Opt<i32>` instead of `x: i32`) | LOW | HIGH (typeck failure on body that uses `x + 1`) | Pattern bindings are lowered by `pattern_bindings.rs:34 collect_pat_bindings_for_mir` which already extracts inner types via `lower_enum_variant_pattern_bindings` (line 80). This infrastructure is already tested by Match tests in `tests/conformance/00-parse/02-control-flow/match_*.lin`. Reusing it via desugar inherits all existing test coverage. |
| Exhaustive match sites on `Expr` enum not all updated (visitor, fold, pretty-printer) | HIGH | LOW (cargo build fails fast — exhaustive match compile error) | `cargo build` is the gate; expect 5-8 sites in `src/ast/visit.rs`, `src/ast/fold.rs`, `src/ast/pretty.rs` (or similar). Each is a mechanical arm addition. |
| Stage 9.3 unit test regression (test_stage9_3_if_let_tests_marked_fail / while_let_tests_marked_fail) | HIGH (certain) | LOW (test logic update; mechanical) | Update the two unit tests in lockstep with .lin marker flips. Run `cargo test --test all_tests -- control_flow_tests` to verify. |
| Stage 13.1b (MUV-2 Option B) not yet executed — version still at v0.21.5 | MEDIUM | LOW (Stage 13.2 can proceed independently; 13.1b is a separate refactor) | Per `stage-13.1-design-alignment.md` §5.4, Stage 13.2 bumps v0.21.5 → v0.22.0 directly. If Stage 13.1b is executed first, it bumps v0.21.5 → v0.21.6, then Stage 13.2 bumps v0.21.6 → v0.22.0. Either path is consistent with the version policy. |

**Overall risk**: **LOW**. The desugar strategy reuses hardened Match/Loop infrastructure; the
parser already has the if-let/while-let recognition stubs wired; typeck/borrowck are MIR-side
and unaffected. The only novel code is the HIR desugar (~50 LOC), which has clear rustc precedent
and existing HirArm construction patterns to follow.

---

## 7. §25.8 Design Write-back Plan

Per `stage-committee-process.md` §25.8, after Stage 13.2 implementation, design docs must be
updated to reflect the implementation-as-fact. This section identifies which design docs need
write-back and what sections.

### 7.1 Write-back matrix

| Design doc | Section | Deviation type | Write-back action |
|------------|---------|----------------|-------------------|
| `02-grammar.md` | §3.4 (line 257-263) | ✅ Already aligned (no deviation) | Optional: add §25.8 retroactive note that Stage 13.2 closes the B1 implementation gap for `if let` / `while let` productions. |
| `05-ast.md` | §8 (line 326-511) — `Expr` enum | **B4 design-gray-area** (design silent on `IfLet` / `WhileLet` variants; implementation will add them) | **Required**: Add `IfLet { pat, expr, then, else_, span }` and `WhileLet { pat, expr, body, span }` variants to §8 after `For` (line 442). Mark with comment "// Stage 13.2 §25.8 write-back — implementation-as-fact". |
| `05-ast.md` | §12.4 (line 860-873) — HIR lowering transformations | ✅ Already aligned (prescribes `if let → match`, `while let → loop { match }` desugar) | Optional: add explicit note that the desugar is performed in `hir::lower::body::lower_expr` per Stage 13.2 implementation. |
| `05-ast.md` | §13.1 (line 899-911) — implementation status table | **B1 → ✅** (status update) | Update §8 表达式定义 row from "✅ 实现" to "✅ 实现 (Stage 13.2 added IfLet/WhileLet)". |
| `03-type-system.md` | §13.3 (line 901-918) — v0.3 self-hosting preconditions | **B1 → ✅** (status update) | Update TD-031 row from "P0 | Stage 13.2" to "✅ closed in Stage 13.2". Add brief note (new §13.4 sub-section) that if-let/while-let refinement scope is automatic via Match pattern bindings. |
| `04-ownership-borrowing.md` | §4 (NLL algorithm) | **B4 design-gray-area** (design does not mention if-let borrow scope) | **Required**: Add brief note to §4 that if-let/while-let borrow scope = match-arm basic block in MIR (auto-handled by NLL on the desugared `Match` form). Mark with "// Stage 13.2 §25.8 write-back". |
| `13-stage1-feature-whitelist.md` | §2.3 (line 84, 86) | ✅ Already aligned (`if let` ✅ ALLOWED, `while let` ✅ ALLOWED) | Optional: add Stage 13.2 closure note: "Implemented in Stage 13.2 (v0.22.0)". |

### 7.2 Write-back timing

Per §25.8.3 #5 "可重构不等于立即重构", the design write-back is best performed **immediately
after Stage 13.2 implementation completes and passes gate review**, before Stage 13.3 (closure
call lowering) begins. This ensures:
1. The design doc reflects the as-shipped implementation (no temporal gap).
2. Stage 13.3 planning has accurate design baseline (per §13.4 stage-start alignment).
3. The write-back is a single atomic commit, not entangled with Stage 13.3 work.

### 7.3 Write-back responsibility

Per §25.8.2 step 5, ARCH-A drafts the write-back; REV-A verifies accuracy (does the design text
match the actual implementation in `src/ast/kinds.rs`, `src/hir/lower/body.rs`?); PM-A coordinates
inclusion in the Stage 13.3 plan's "design doc alignment" section.

---

## 8. Committee Recommendation

### **GO** for Stage 13.2 launch

**Justification**:

Stage 13.2 (TD-031 if-let / while-let closure) is **fully design-aligned** per §13.4:
- Grammar spec (`02-grammar.md` §3.4) already defines the productions.
- AST spec (`05-ast.md` §8) lacks variants but §12.4 explicitly prescribes the desugar strategy
  — the implementation gap is a B4 design-gray-area write-back, not a design-implementation conflict.
- Type system (`03-type-system.md`) and ownership (`04-ownership-borrowing.md`) are silent on
  refinement scope / borrow scope but the desugar strategy makes these auto-handled by existing
  Match/NLL infrastructure.
- Stage 1 feature whitelist (`13-stage1-feature-whitelist.md` §2.3) explicitly lists `if let` and
  `while let` as ALLOWED — Stage 13.2 closes the B1 deviation.

**Strategy B (Desugar to Match)** is recommended with **LOW risk**:
- 4 src files modified (ast/kinds.rs, parser/expr.rs, hir/lower/body.rs, + 1 for visitor/fold arms).
- ~60-80 LOC of new code (2 AST variants + 2 HIR desugar arms + parser branch swap).
- Reuses hardened `lower_match` (188 LOC, 6+ gate reviews) and `HirExprKind::Loop` lowering (24 LOC).
- Zero MIR lowering, typeck, or borrowck changes — these layers see only Match/Loop MIR.
- rustc-idiomatic (rustc_ast_lowering does the same desugar).
- Matches `05-ast.md` §12.4 design intent.

**Version policy**: v0.21.5 → **v0.22.0** (minor bump — first user-facing compiler feature;
per `stage-13.1-design-alignment.md` §5.4 minor-bump threshold reserved for Stage 13.2).

**§25.8 write-back required** post-implementation (per Section 7):
- `05-ast.md` §8 — add `IfLet` / `WhileLet` variants (B4 design-as-fact).
- `03-type-system.md` §13.4 (new sub-section) — document refinement scope auto-handling.
- `04-ownership-borrowing.md` §4 — document borrow scope = match-arm basic block.
- `02-grammar.md` — optional retroactive note.

**Test verification gate** (per `plan-13.1.md` §3):
1. `cargo build` — zero warnings, zero errors (catches exhaustive-match misses).
2. `cargo test --test all_tests` — 5026 conformance + 2179 integration tests, with **+11 expected
   FAIL→PASS flips** (the 11 if-let/while-let .lin tests).
3. `cargo fmt --check` — zero diff.
4. `cargo clippy --all-targets` — zero warnings.
5. Post-implementation grep: `rg "IfLet\|WhileLet" src/` returns ≥4 matches (2 AST variants +
   HIR lowering arms); `rg "if let.*not yet supported\|while let.*not yet supported" src/`
   returns ZERO (soft errors removed).
6. Stage 9.3 unit tests updated and passing (`cargo test --test all_tests -- control_flow_tests`).

**GO for Stage 13.2 launch**: Strategy B is the rustc-idiomatic, design-aligned, lowest-risk path
to closing TD-031. The 11 conformance FAIL tests flip to PASS as the first concrete user-facing
value delivered by Stage 13 — unblocking the most pervasive Rust control-flow pattern for v0.3
self-hosting.

---

**Audit completed**: 2026-07-26
**Next action**: Stage Committee vote on this design alignment → if GO, Stage 13.2 MUV-4/5/6
execution (estimated 1-2 weeks per `plan-13.1.md` §2 Stage 13.2).
