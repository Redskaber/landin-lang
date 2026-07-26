# Stage 13.4 Design Alignment (§13.4) — Built-in macro expansion (TD-032 P0 closure)

> **Auditor**: ARCH-A + ALG-C (combined subagent) | **Date**: 2026-07-26 | **Baseline**: v0.23.0
> **Process**: stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8 + §25.7
> **Priority**: P0 (last P0 blocker for v0.3 self-hosting) — third user-facing compiler feature
> **Inputs**: `plan-13.1.md` (Stage 13 active plan, MUV-9/MUV-10/MUV-11 for Stage 13.4) +
> `stage-13.1-design-alignment.md` §5.4 (version-policy reference: v0.23.0 → v0.24.0 reserved for Stage 13.4) +
> `stage-13.2-design-alignment.md` + `stage-13.3-design-alignment.md` (format + preparation-phase precedent) +
> r216 architecture audit §3.5 (TD-032 detail) + r217 stages-0-4 re-audit §2.6 + §4 (TD-032 framing inversion — Stage 4.10 root cause) +
> 6 design docs (`02-grammar.md` / `05-ast.md` / `06-mir.md` / `07-codegen.md` / `08-bootstrap-strategy.md` / `09-stdlib.md` / `12-roadmap.md` / `13-stage1-feature-whitelist.md`) +
> `src/ast/kinds.rs` / `src/hir/kinds.rs` / `src/lexer/token.rs` / `src/parser/expr.rs` / `src/parser/items.rs` /
> `src/hir/lower/body.rs` / `src/mir/lower/expr_operand.rs` / `src/codegen/mod.rs` +
> 6 conformance `.lin` test files mentioning macros + `tests/v0/stage4/plan/macro_system_tests.rs` (3 tests)
> **Scope**: Stage 13.4 MUV-9 (macro infrastructure — TokenTree + AST MacroCall args field fix) + MUV-10 (19 missing built-in macros) + MUV-11 (HIR integration + hygiene strategy decision)

---

## 1. Executive Summary

Stage 13.4 closes TD-032 — the **last P0 blocker** for v0.3 self-hosting. Per r216 + r217 +
`plan-13.1.md` §2, TD-032 is labeled "`macro_rules!` not implemented", but the **r217 re-audit
(§2.6) reframes** the actual blocker: the Stage 1 contract (`13-stage1-feature-whitelist.md` §2.6)
requires **26 built-in macros**; the compiler currently **hardcodes only 7** in
`src/mir/lower/expr_operand.rs:1379-1435` (`println` / `print` / `eprintln` / `eprint` /
`stringify` / `assert` / `debug_assert`); **19 are missing** (`format` / `write` / `writeln` /
`vec` / `matches` / `assert_eq` / `assert_ne` / `debug_assert_eq` / `debug_assert_ne` / `panic` /
`dbg` / `unreachable` / `todo` / `unimplemented` / `concat` / `file` / `line` / `column` /
`module_path`). Even the 7 hardcoded macros are **non-functional placeholders** — they produce
`TyKind::Tuple(vec![])` (unit) or `TyKind::Ref(...Str)` (stringify) without inspecting macro
arguments, because the AST `MacroCall` node **discards the body token stream** at parse time
(per `src/parser/expr.rs:795 self.skip_delim_group()`).

**Findings**:

- **CRITICAL DESIGN CONFLICT (the smoking gun)**: TD-032 has been **misframed as "macro_rules!"**
  in r216 / `plan-13.1.md` §2 / `gate-review-13.3a.md` line 99. The design documents are
  **explicit and unanimous** that `macro_rules!` is a **v0.2 feature, NOT v0.1 / v0.3**:
  - `02-grammar.md` §4.4 (line 421): "MVP **不支持** `macro_rules!` 自定义宏（推迟 v0.2），但
    **支持** 26 个内建宏（编译器硬编码展开）"
  - `02-grammar.md` §7 (line 491): "macro_rules! | `macro_rules!` 关键字 | 无（v0.2） | R1 教训"
  - `05-ast.md` line 12: "保留宏形状：MVP 无宏，但 AST 结构预留 `MacroCall` 节点（v0.2 用）"
  - `05-ast.md` §8 (line 500-505): `MacroCall { mac: Path, args: Vec<TokenTree>, span: Span }` —
    the design **does** carry macro body args; the implementation does NOT (B1 deviation, see §3.1)
  - `08-bootstrap-strategy.md` line 206: "Proc macro：永久不做（v0.2 仅 macro_rules!）"
  - `12-roadmap.md` §4.1 (line 449): "`macro_rules!` 声明宏" listed under v0.2 远景 (6-12 months
    post v0.1 release)
  - `13-stage1-feature-whitelist.md` §2.6 (line 152): "**禁止使用**：`macro_rules!` 自定义宏
    （v0.2 才支持）"
  - `09-stdlib.md` line 562, 788, 850: `format!()` / `println!` / `eprintln!` listed as v0.2
    macros (stdlib side)
  Per `stage-committee-process.md` §13.4.2 rule 1 ("设计文档优先级最高：当设计文档与'经验判断'
  或'互联网惯例'冲突时，以设计文档为准"), implementing `macro_rules!` in Stage 13.4 would
  **violate the design baseline** and create a B2 deviation (impl > design). Stage 13.4 must
  instead implement the **design-sanctioned path**: hardcoded expansion of all 26 built-in macros.

- **r217 framing inversion (verified)**: The r216 audit labeled TD-032 as "macro_rules! not
  implemented (26 built-in macros hardcoded)" — both clauses are wrong:
  1. Only **7** of 26 are hardcoded (not 26); 19 are missing entirely.
  2. The actual blocker for v0.3 self-hosting is the **19 missing macros**, not `macro_rules!`
     (Stage 1 source code is **forbidden** from using `macro_rules!` per `13-stage1-feature-whitelist.md`
     §2.6; v0.3 self-hosting needs the 26 built-in macros only).
  The r217 reframe (§2.6) is correct: "Either: (a) implement `macro_rules!` subsystem + 26 built-in
  macro_rules! definitions, or (b) hardcode the 19 missing macros." Per design, **only (b) is
  allowed for v0.1/v0.3**.

- **AST B1 deviation (verified by direct read of `src/ast/kinds.rs:554-561`)**: The implementation's
  `MacroCall { path, delim, span }` variant **discards the macro body tokens** — `args: Vec<TokenTree>`
  from design `05-ast.md` §8 line 503 is missing. The parser (`src/parser/expr.rs:780-801`)
  recognizes `ident!(...)` / `ident!{...}` / `ident![...]` via `TokenKind::Not` + delim, then
  **calls `self.skip_delim_group()` at line 795** — the body tokens are dropped. The HIR lowering
  (`src/hir/lower/body.rs:374-377`) passes through `(path, delim)` only. The MIR lowering
  (`src/mir/lower/expr_operand.rs:1379-1435`) matches on the **macro name string** only — it cannot
  inspect args. This is why `vec![1, 2, 3]` and `vec![]` are indistinguishable to the compiler,
  and why `assert!(cond)` produces `()` regardless of `cond`. **Closing TD-032 requires fixing
  this B1 deviation first** — the AST `MacroCall` node must carry the body tokens (as `Vec<TokenTree>`
  per design, or equivalent) before any macro can be properly expanded.

- **Stage 1 feature whitelist alignment**: `13-stage1-feature-whitelist.md` §2.6 lists 26 built-in
  macros as ALLOWED for Stage 1 source. §4.3 (line 322-324) requires "26 个内建宏全部实现" as a
  Stage 0 must-support item. §2.6 explicitly forbids `macro_rules!` for Stage 1 source. **Stage 13.4
  must close the 19-macro gap to satisfy the Stage 1 contract** — `macro_rules!` is NOT required
  for v0.3 self-hosting and is design-forbidden.

- **Implementation status** (verified by direct read):
  - AST `MacroCall { path, delim, span }` — ⚠️ **B1 deviation** at `src/ast/kinds.rs:554-561`
    (design has `args: Vec<TokenTree>`; impl discards body)
  - AST `ItemKind` — ✅ **NO** `MacroDef` / `MacroRules` variant at `src/ast/kinds.rs:24-36`
    (design-aligned — design has no `macro_rules!` item)
  - HIR `MacroCall { path, delim }` — ⚠️ same B1 deviation at `src/hir/kinds.rs:787-790`
  - Lexer `TokenKind` — ✅ **NO** `KwMacroRules` token at `src/lexer/token.rs:47-86`
    (in Rust syntax, `macro_rules` is technically an identifier, not a keyword — design-aligned)
  - Parser `parse_primary_expr` MacroCall branch — ⚠️ at `src/parser/expr.rs:780-801`
    (recognizes `ident!delim...`, calls `skip_delim_group()` — discards body)
  - Parser `parse_item` dispatcher — ✅ **NO** `macro_rules!` arm at `src/parser/items.rs:40-78`
    (design-aligned)
  - HIR lowering `Expr::MacroCall` arm — ⚠️ pass-through at `src/hir/lower/body.rs:374-377`
    (drops args because AST has no args)
  - MIR lowering `HirExprKind::MacroCall` arm — ❌ **7 hardcoded placeholder expansions** at
    `src/mir/lower/expr_operand.rs:1379-1435` (matches on macro name string only; produces
    `TyKind::Tuple(vec![])` for println/print/eprintln/eprint/assert/debug_assert and
    `TyKind::Ref(..., Str)` for stringify; unknown macros fall to `TyKind::Error`)
  - Codegen `Terminator::Call` / `Rvalue::Aggregate` — ✅ no macro-specific codegen
    (relies on MIR locals produced by `expr_operand.rs`)
  - `src/macro_expand/` module — ❌ **does not exist**

- **Conformance test count** (verified by `grep -rln "macro" tests/conformance/`):
  **6 .lin files** mention "macro" in comments (all `// EXPECTED: compile_ok`):
  `06-stdlib/02-std/{001-print-macro, 002-vec-macro, 016-std-println-macro, 017-std-vec-macro, 040-std-collect-pattern}.lin`
  + `06-stdlib/00-core/026-std-println-macro.lin` + `06-stdlib/00-core/027-std-vec-macro.lin`.
  These tests currently PASS because the 7 hardcoded macros produce *some* MIR (even if
  non-functional). Stage 13.4 must keep them passing AND add new tests for the 19 missing macros.
  **0 conformance tests use `macro_rules!`** (verified by `grep -rln "macro_rules" tests/conformance/`
  returning empty) — confirming the design intent: `macro_rules!` is not part of the v0.1/v0.3
  contract.
  Stage 4.10 unit tests at `tests/v0/stage4/plan/macro_system_tests.rs` (3 tests) verify only
  "macro produces non-empty MIR" — no behavioral correctness checks.

- **Stage 4.10 root cause** (r217 §4 verified): Stage 4.10 added the `HirExprKind::MacroCall`
  arm with 7 hardcoded macros and **no `macro_rules!` subsystem and no body-token capture**.
  The decision to hardcode (rather than implement `macro_rules!`) **was the design-sanctioned
  choice** — `02-grammar.md` §4.4 explicitly says "编译器硬编码展开". The deferral note for
  `macro_rules!` itself traces to `08-bootstrap-strategy.md` line 206 + `12-roadmap.md` §4.1
  (both v0.2). The Stage 4.10 partial-impl status (7/26) is NOT written back to any design doc
  §25.8 — Stage 13.4 §25.8 write-back should add this status note to `07-codegen.md` §15 (or a
  new §16 sub-section).

**Recommendation**: **Strategy C → Strategy B** — Stage 13.4 = preparation phase (this design
alignment + TokenTree infrastructure skeleton + verification tests); Stage 13.4a = full
Strategy B implementation (19 missing macros + AST/Parser/HIR/MIR plumbing). Mirrors the
Stage 13.3 → 13.3a precedent.

- **Strategy A (full `macro_rules!` subsystem) — ❌ DESIGN-FORBIDDEN**: explicitly violates
  `02-grammar.md` §4.4 + §7, `12-roadmap.md` §4.1, `13-stage1-feature-whitelist.md` §2.6,
  `08-bootstrap-strategy.md` line 206. Per §13.4.2 rule 1, this option is **out of scope** for
  Stage 13.4 / v0.3. Deferred to v0.2 per design (which is post-v0.1, post-v0.3 self-hosting).
- **Strategy B (extend built-in macros) — ✅ DESIGN-SANCTIONED**: design `02-grammar.md` §4.4
  explicitly says "编译器硬编码展开". Adds `TokenTree` type + `args: Vec<TokenTree>` field to
  AST/HIR `MacroCall` + new `src/macro_expand/` module with 26 hardcoded expanders + replaces
  the current 7-arm placeholder match in `expr_operand.rs`.
- **Strategy C (preparation phase) — ✅ ACCEPTABLE per §15 + §25.7**: full Strategy B touches
  ~9 src files + new module + 19 expander functions + TokenTree type, exceeding §14.4 J5
  (≤5 files guideline). Stage 13.3 → 13.3a precedent validates the split.

- **File count**: **9 src + 1 new stage13_4 test + 6 conformance .lin (kept) + N new conformance
  .lin for 19 missing macros + 5 design-doc write-back = ~22-30 files** (Stage 13.4a); Stage 13.4
  preparation = **~5 files** (this doc + gate-review-13.4.md + stage13_4_tests.rs skeleton + 1-2
  src stubs + Cargo.toml patch bump).
- **Risk**: **HIGH** for full Strategy B (Stage 13.4a): 9 src files exceeds §14.4 J5 ≤5 guideline;
  ~800-1200 LOC; new `TokenTree` type touches AST/HIR/parser/MIR-lower simultaneously; 19
  individual macro expanders each need correct arg parsing + MIR emission. **LOW** for Stage 13.4
  preparation (this phase): no functional changes; design alignment + test infrastructure only.
- **Version policy**: v0.23.0 → **v0.23.1** (patch bump — preparation phase, mirrors Stage 13.3
  v0.22.0→v0.22.1 precedent); v0.23.1 → **v0.24.0** reserved for Stage 13.4a (minor bump — third
  user-facing compiler feature, per `stage-13.1-design-alignment.md` §5.4 line 544).
- **Estimated effort**: Stage 13.4 (preparation) = 1 session; Stage 13.4a (implementation) = 2-4
  weeks per `plan-13.1.md` §2 Stage 13.4 estimate (r216's "4-8 weeks" was for Strategy A which
  is design-forbidden; Strategy B is smaller).

---

## 2. Design Document Alignment (§13.4)

Per §13.4.1 step 1-3, each design doc is read against the planned implementation to identify
alignment, deviation, and gray-area decisions.

### 2.1 `13-stage1-feature-whitelist.md` §2.6 — Macro whitelist (THE contract)

**Read**: §2.6 内建宏（仅允许的子集）(lines 133-152), §4.3 内建宏最低要求 (line 322-324).

**What the design says** (verified by direct read of `13-stage1-feature-whitelist.md:133-152`):

```
### 2.6 内建宏（仅允许的子集）

Stage 1 可用的内建宏（共 26 个，v1.2.2 修正数量与 02 文档统一，含 matches!）：

| 宏 | 用途 | 允许 |
| --- | --- | --- |
| `println!` / `print!` / `eprintln!` / `eprint!` | 输出 | ✅ |
| `format!` | 字符串格式化 | ✅ |
| `write!` / `writeln!` | 写入 Writer | ✅ |
| `vec!` | Vec 构造 | ✅ |
| `matches!` | 模式匹配判断 | ✅ |
| `assert!` / `assert_eq!` / `assert_ne!` | 测试断言 | ✅ |
| `debug_assert!` / `debug_assert_eq!` / `debug_assert_ne!` | debug 测试断言 | ✅ |
| `panic!` | panic | ✅ |
| `dbg!` | 调试输出 | ✅ |
| `unreachable!` | 不可达标记 | ✅ |
| `todo!` / `unimplemented!` | 未实现标记 | ✅ |
| `concat!` / `stringify!` / `file!` / `line!` / `column!` / `module_path!` | 编译期信息 | ✅ |

**禁止使用**：`macro_rules!` 自定义宏（v0.2 才支持）
```

§4.3 (line 322-324): "26 个内建宏全部实现（v1.2.2 修正数量），清单见 §2.6。"

**Does the design specify the 26 built-in macros?** **YES** — §2.6 lists exactly 26 macros in
8 categories (I/O 4, string 3, construction 1, assertion 6, control 4, debug 1, compile-time
info 6, pattern-match 1). The full enumerated list:

1. `println!` 2. `print!` 3. `eprintln!` 4. `eprint!` (I/O)
5. `format!` 6. `write!` 7. `writeln!` (string)
8. `vec!` (construction)
9. `assert!` 10. `assert_eq!` 11. `assert_ne!` (assertion)
12. `debug_assert!` 13. `debug_assert_eq!` 14. `debug_assert_ne!` (debug assertion)
15. `panic!` 16. `unreachable!` 17. `todo!` 18. `unimplemented!` (control)
19. `dbg!` (debug)
20. `concat!` 21. `stringify!` 22. `file!` 23. `line!` 24. `column!` 25. `module_path!` (compile-time info)
26. `matches!` (pattern-match)

**Does the design specify `macro_rules!` syntax?** **NO — explicitly forbidden for v0.1/v0.3.**
§2.6 line 152: "**禁止使用**：`macro_rules!` 自定义宏（v0.2 才支持）". The design is unambiguous:
`macro_rules!` is a **v0.2 feature**, not allowed in Stage 1 source code, and therefore not
required to be implemented in Stage 0 compiler for v0.3 self-hosting.

**Does the design specify macro expansion timing (parse-time vs HIR-time)?** **NO** — §2.6
specifies the contract (26 macros required) but does not specify the expansion mechanism.
§4.3 says "26 个内建宏全部实现" (all 26 must be implemented) without specifying the timing.
This is a **B4 design-gray-area** — implementation must choose (see §2.5 below).

**Alignment verdict**: ✅ **PASS §13.4 with reframe requirement**. The whitelist contract is
clear: 26 built-in macros required, `macro_rules!` forbidden. **TD-032 must be reframed** from
"macro_rules! not implemented" to "19 of 26 built-in macros not implemented" per r217 §2.6.
The Stage 13.4 closure criterion is "all 26 built-in macros functional", NOT "macro_rules!
implemented". Stage 13.4 §25.8 write-back should add a §2.6.1 sub-section noting implementation
status (7/26 → 26/26 post-Stage 13.4a).

### 2.2 `02-grammar.md` §4.4 — Grammar (THE smoking gun #1)

**Read**: §4.4 内建宏调用 (lines 419-436), §7 与 Rust 文法的具体差异 (line 491).

**What the design says** (verified by direct read of `02-grammar.md:419-436`):

```
### 4.4 内建宏调用（v1.2.2 修正）

MVP **不支持** `macro_rules!` 自定义宏（推迟 v0.2），但 **支持** 26 个内建宏
（编译器硬编码展开，含 matches!）。完整清单见 `13-stage1-feature-whitelist.md §2.6`。

内建宏清单（按用途分组，共 26 个，含 matches!）：
- I/O：`println!` `print!` `eprintln!` `eprint!`
- 字符串：`format!` `write!` `writeln!`
- 构造：`vec!`
- 断言：`assert!` `assert_eq!` `assert_ne!` `debug_assert!` `debug_assert_eq!` `debug_assert_ne!`
- 控制：`panic!` `unreachable!` `todo!` `unimplemented!`
- 调试：`dbg!`
- 编译期信息：`concat!` `stringify!` `file!` `line!` `column!` `module_path!`
- 模式匹配判断：`matches!`

调用形式：`ident!(args)` / `ident!{args}` / `ident![args]`。

用户使用未在上述清单中的 `ident!` 形式时，报错"unknown macro or not yet supported"。
```

§7 (line 491): "macro_rules! | `macro_rules!` 关键字 | 无（v0.2） | R1 教训"

**Does the grammar include `macro_rules!` production?** **NO** — explicitly excluded for v0.1/v0.3.
§4.4 line 421: "MVP **不支持** `macro_rules!` 自定义宏（推迟 v0.2）". §7 line 491 lists
`macro_rules!` as a Rust→Landin difference with reason "R1 教训" (R1 audit lesson — likely
avoiding OCaml-rustc-style macro complexity for v0.1).

**Does the grammar include macro invocation forms?** **YES** — §4.4 line 434: "调用形式：
`ident!(args)` / `ident!{args}` / `ident![args]`." Three delimiter forms: `(`, `{`, `[`.
Implementation matches at `src/parser/expr.rs:780-801` (recognizes `Not` + `LParen`/`LBrace`/
`LBracket`). ✅ **alignment**.

**Does the design specify the macro expansion mechanism?** **YES** — §4.4 line 421 explicitly
says "编译器硬编码展开" (compiler hardcodes expansion). This **pre-sanctions Strategy B**
(extended built-in macros) and **pre-forbids Strategy A** (`macro_rules!` subsystem).

**Does the design specify error behavior for unknown macros?** **YES** — §4.4 line 436:
"用户使用未在上述清单中的 `ident!` 形式时，报错'unknown macro or not yet supported'". The
current implementation falls to `TyKind::Error` for unknown macros (per
`src/mir/lower/expr_operand.rs:1412-1422`) — semantically equivalent (compiler error), but the
error message text differs. **Minor deviation**: error message should be updated to match design
wording.

**Alignment verdict**: ✅ **PASS §13.4 with strategy pre-sanction**. The grammar design
**explicitly prescribes Strategy B** ("编译器硬编码展开") and **explicitly forbids Strategy A**
("MVP 不支持 macro_rules! 自定义宏"). Stage 13.4 must implement what the design already specifies.
No grammar update needed; the design is already correct. Stage 13.4 §25.8 write-back should add a
retroactive note that Stage 13.4a closes the B1 implementation gap (7/26 → 26/26 hardcoded macros).

### 2.3 `05-ast.md` — AST spec (THE smoking gun #2 — B1 deviation)

**Read**: §1 设计原则 (line 12), §8 表达式定义 `MacroCall` variant (lines 500-505), §13 §25.8
write-back sections.

**What the design says** (verified by direct read of `05-ast.md:12`):

> "保留宏形状：MVP 无宏，但 AST 结构预留 `MacroCall` 节点（v0.2 用）"

Verified by direct read of `05-ast.md:500-505`:

```rust
// 内建宏调用（v1.2 修正：args 改为 Vec<TokenTree>，TokenStream 是 parser 内部状态）
MacroCall {
    mac: Path,
    args: Vec<TokenTree>,        // v1.2 修正：TokenStream → Vec<TokenTree>
    span: Span,
},
```

**Does the design have `MacroDef` / `MacroRules` variant?** **NO** — design `Expr` enum has no
macro-definition variant. This is **design-aligned**: the design forbids `macro_rules!` (per
§2.2 above), so no `MacroDef` AST node is needed. ✅ **alignment** (implementation at
`src/ast/kinds.rs:24-36` `ItemKind` also has no macro variant — matches design).

**Does the design have `MacroCall` variant?** **YES** — §8 line 501-505. The design's `MacroCall`
has **3 fields**: `mac: Path`, `args: Vec<TokenTree>`, `span: Span`.

**Does the implementation match?** **NO — B1 DEVIATION**. The implementation at
`src/ast/kinds.rs:554-561`:

```rust
MacroCall {
    path: Path,
    delim: MacroDelim,
    span: Span,
}
```

The implementation has **3 fields** but they are **different**:
- Design: `mac: Path` (field name `mac`); Impl: `path: Path` (field name `path`) — minor naming
  difference (B3, low impact).
- Design: `args: Vec<TokenTree>` (macro body tokens); Impl: `delim: MacroDelim` (delimiter kind
  only) — **CRITICAL B1 DEVIATION**: the implementation **discards the macro body tokens** and
  only records the delimiter kind. This makes any meaningful macro expansion impossible — the
  expander has no access to the macro arguments.
- Design: `span: Span`; Impl: `span: Span` — ✅ alignment.

**Does `TokenTree` type exist in implementation?** **NO** — verified by
`grep -rn "TokenTree" src/` returning zero matches. The design `05-ast.md` §8 references
`Vec<TokenTree>` but the type does not exist. §5.3 of `02-grammar.md` (line 467-469) mentions
"Token 树（macro 用，v0.2）" — design says token tree is v0.2 infrastructure, but §8 of `05-ast.md`
already references it for the `MacroCall` variant. This is a **B4 design-gray-area**: the design
references a type that doesn't exist yet, but the type is needed for the 26 built-in macros to
be properly expanded.

**Alignment verdict**: ⚠️ **PARTIAL §13.4 — B1 + B4 deviation**. The AST design anticipates
`MacroCall` with `args: Vec<TokenTree>` (correct design), but the implementation discards args
(B1) because `TokenTree` type doesn't exist (B4). Stage 13.4a must:
1. Add `TokenTree` type (or equivalent) to AST module (B4 design-as-fact write-back).
2. Update `MacroCall` variant: replace `delim: MacroDelim` with `args: Vec<TokenTree>` (B1 fix),
   OR add `args` field alongside `delim` (preserves backward compat for `MacroDelim` if needed
   for diagnostics).
3. Update parser to **capture** body tokens (not `skip_delim_group()`).
4. Update HIR `MacroCall` to carry `args: Vec<TokenTree>` (matching AST).
5. Update MIR lowering to dispatch to per-macro expanders that **read** the args.

The §25.8 write-back should add a §13.4 sub-section to `05-ast.md` documenting that:
- `TokenTree` type added in Stage 13.4a (B4 closure).
- `MacroCall.args` field populated in Stage 13.4a (B1 closure).
- `MacroDef` / `MacroRules` variant remains absent (design-aligned — v0.2 feature).

### 2.4 `06-mir.md` + `07-codegen.md` — MIR/Codegen (silent on macros)

**Read**: Full grep of `06-mir.md` (995 lines) and `07-codegen.md` (1035 lines) for
"macro" / "MacroCall" / "expansion" / "hygiene" / "println" / "vec!" / "format!".

**What the design says**:

- **ZERO matches** in `06-mir.md` for any of: `macro`, `MacroCall`, `expansion`, `hygiene`.
- **ZERO matches** in `07-codegen.md` for any of: `macro`, `MacroCall`, `println`, `vec!`,
  `format!`, `expansion`.

**Does the design specify how macros expand (AST-level vs HIR-level)?** **NO** — both MIR and
codegen design docs are silent on macros. This is consistent with the design intent that macros
are **expanded before MIR is built** (either at parse-time or HIR-lowering-time), so MIR and
codegen never see `MacroCall` nodes. The implementation at `src/mir/lower/expr_operand.rs:1379-1435`
**violates this implicit design** — it lowers `HirExprKind::MacroCall` directly to MIR by
matching on the macro name, which means the MIR layer DOES see MacroCall (B3 deviation: impl ≠
design). The proper fix (Strategy B) is to expand macros at HIR-lowering time (in
`src/hir/lower/body.rs`) so the MIR layer only sees the **expanded** HIR (e.g., `vec![1, 2, 3]`
expands to `HirExprKind::Call(vec::with_capacity, ...)` or `HirExprKind::Array` etc.).

**Does the design specify macro hygiene?** **NO** — neither MIR nor codegen doc mentions hygiene.
For hardcoded built-in macros, hygiene is **trivial** because the expander controls all generated
identifiers (e.g., `vec!` expander emits `__vec_temp_N` with fresh N per expansion). Hygiene
becomes a concern only for user-defined `macro_rules!` (Strategy A, design-forbidden). **Stage
13.4 can skip hygiene entirely** — the design doesn't require it for v0.1/v0.3.

**Alignment verdict**: ⚠️ **PARTIAL §13.4 — design silent, impl deviates**. MIR/codegen design
is silent on macros (correct — they shouldn't see macros). The implementation **deviates** by
lowering `MacroCall` to MIR directly. Stage 13.4a should:
1. Move macro expansion from MIR-lowering (`expr_operand.rs:1379-1435`) to HIR-lowering
   (`src/hir/lower/body.rs:374-377` arm), so the MIR layer sees only expanded HIR.
2. Replace the 7-arm placeholder match with a dispatch to `src/macro_expand/builtin.rs`
   expanders that return `HirExprKind` (e.g., `HirExprKind::Call`, `HirExprKind::Array`,
   `HirExprKind::Tuple`, `HirExprKind::Block`).
3. §25.8 write-back should add a §16 sub-section to `07-codegen.md` noting that macros are
   expanded at HIR-lowering time and codegen never sees `MacroCall`.

### 2.5 `08-bootstrap-strategy.md` + `12-roadmap.md` + `09-stdlib.md` — Cross-references

**Read**: `08-bootstrap-strategy.md` line 206, `12-roadmap.md` §4.1 line 449, `09-stdlib.md`
lines 562, 788, 850.

**What the design says**:

- `08-bootstrap-strategy.md` line 206: "Proc macro：永久不做（v0.2 仅 macro_rules!）" —
  proc macros permanently excluded; `macro_rules!` is v0.2 only.
- `12-roadmap.md` §4.1 (line 449): "`macro_rules!` 声明宏" listed under "v0.2 远景 (v0.1 发布后
  6-12 月)" — confirms v0.2 timeline for `macro_rules!`.
- `09-stdlib.md` line 562: `fmt.rs // format!() (v0.2)` — `format!` macro is v0.2 in stdlib.
- `09-stdlib.md` line 788: `printf.rs // println! / print! (v0.2: macro)` — `println!` macro
  is v0.2 in stdlib.
- `09-stdlib.md` §4.3 (line 850-869): "println（v0.2 macro，v0.1 函数模拟）" — v0.1 provides
  `println` as a **function** (not macro); v0.2 adds `println!` macro and deprecates the function.

**Cross-reference implication**: The stdlib design says `println!` is a v0.2 macro, but the
**compiler** must support `println!` invocation in v0.1 (per `13-stage1-feature-whitelist.md`
§2.6 contract). These are **not contradictory** — the stdlib's `println!` macro definition
(which would be a `macro_rules!` block) is v0.2; but the compiler's hardcoded `println!`
expansion is v0.1. Stage 1 source code uses `println!` (compiler expands it); v0.2 stdlib
replaces the hardcoded expansion with a `macro_rules!` definition (and removes the hardcoded
arm).

**Alignment verdict**: ✅ **PASS §13.4**. All cross-references consistently place `macro_rules!`
in v0.2 and hardcoded built-in macros in v0.1. Stage 13.4 implementation of Strategy B (hardcoded
26 macros) is design-aligned. Stage 13.4 §25.8 write-back should add a brief note to
`09-stdlib.md` §4.3 documenting that v0.1 compiler-side hardcoded `println!` expansion is now
implemented (Stage 13.4a); the stdlib-side `macro_rules!` definition remains v0.2.

---

## 3. Current Implementation Analysis

### 3.1 AST inspection — `src/ast/kinds.rs`

**Verified by direct read of `src/ast/kinds.rs:24-36` (ItemKind) + `:554-561` (MacroCall Expr variant)
+ `:580-585` (MacroDelim)**:

```rust
pub enum ItemKind {            // line 24-36
    Fn(FnDecl),
    Const(ConstDecl),
    Static(StaticDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    TypeAlias(TypeAliasDecl),
    ExternBlock(ExternBlock),
    Mod(ModDecl),
    Use(UseDecl),
}                              // NO MacroDef / MacroRules variant — design-aligned
```

```rust
MacroCall {                    // line 554-561
    path: Path,
    delim: MacroDelim,         // design has args: Vec<TokenTree> — B1 DEVIATION
    span: Span,
}
```

```rust
pub enum MacroDelim {          // line 580-585
    Paren,
    Brace,
    Bracket,
}
```

**Findings**:
- `MacroCall` variant **present** at line 554 ✅ (matches design §8 line 501).
- `MacroCall.path` field — design uses field name `mac`; impl uses `path` (minor B3 naming
  deviation; semantically equivalent).
- `MacroCall.delim` field — **B1 DEVIATION**: design has `args: Vec<TokenTree>`; impl has
  `delim: MacroDelim`. The implementation **discards the macro body tokens**.
- `MacroCall.span` field — ✅ alignment.
- `ItemKind` — ✅ NO `MacroDef` / `MacroRules` variant (design-aligned — design has no
  `macro_rules!` item).
- `MacroDelim` enum — design does not specify this; impl adds it as B2 (impl > design) for
  delimiter-kind tracking. **Acceptable** as a diagnostic helper, but should be preserved
  alongside the new `args` field (not replace it).

**Implication for strategy**: Closing TD-032 requires fixing the B1 deviation — `MacroCall` must
carry the macro body tokens (as `Vec<TokenTree>` per design, or equivalent `Vec<Token>` /
captured token stream). Without this, no macro can be properly expanded.

### 3.2 HIR inspection — `src/hir/kinds.rs`

**Verified by direct read of `src/hir/kinds.rs:787-790`**:

```rust
MacroCall {
    path: HirPath,
    delim: MacroDelim,
}                              // line 787-790 — same B1 deviation as AST
```

**Findings**:
- `HirExprKind::MacroCall` variant **present** at line 787 ✅.
- Same B1 deviation as AST: HIR `MacroCall` carries `(path, delim)` only — no `args`.
- HIR design doc (`05-ast.md`) does not separately specify HIR `MacroCall` shape — HIR
  typically mirrors AST, so the same `args: Vec<TokenTree>` field should be added.

**Implication for strategy**: HIR `MacroCall` must be updated in lockstep with AST `MacroCall`
to carry the body tokens. The macro expander (new `src/macro_expand/builtin.rs`) reads the
`args` and produces a replacement `HirExprKind` (e.g., `HirExprKind::Call`, `Array`, `Tuple`,
`Block`, etc.).

### 3.3 Parser inspection — `src/parser/expr.rs` + `src/parser/items.rs`

**Verified by direct read of `src/parser/expr.rs:780-801` (MacroCall branch in
`parse_primary_expr`) + `src/parser/items.rs:40-78` (`parse_item` dispatcher)**:

#### 3.3.1 `parse_primary_expr` MacroCall branch (expr.rs:780-801)

Current behavior:

```rust
// Macro call: `!` followed by `(`/`{`/`[`
if *self.peek() == TokenKind::Not
    && matches!(
        self.peek_at(1),
        TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket
    )
{
    self.bump(); // !
    let delim = match self.peek() {
        TokenKind::LParen => MacroDelim::Paren,
        TokenKind::LBrace => MacroDelim::Brace,
        TokenKind::LBracket => MacroDelim::Bracket,
        _ => unreachable!(),
    };
    // Skip the macro body tokens for Stage 0 — we just balance
    // the delimiters. Stage 4 macro expansion will re-parse them.
    self.skip_delim_group();                                    // ← DISCARDS body
    return Expr::MacroCall {
        path,
        delim,
        span: path_span,
    };
}
```

**Confirmed**: Parser recognizes `ident!delim...` syntax correctly (peek `Not` + delim, bump
`!`, determine delim kind), then **calls `self.skip_delim_group()`** at line 795 — the body
tokens are **skipped and discarded**. The comment at line 793-794 says "Stage 4 macro expansion
will re-parse them" — but Stage 4.10 only hardcoded 7 macros by name (without reading body);
the re-parse never happened.

**For Stage 13.4a**: Replace `self.skip_delim_group()` with `self.capture_delim_group()` (new
method) that returns `Vec<TokenTree>` (or `Vec<Token>`) — the body tokens. Update `Expr::MacroCall`
to carry the captured tokens. Estimated diff: ~15-25 LOC (new capture method + AST field update
+ parser branch update).

#### 3.3.2 `parse_item` dispatcher (items.rs:40-78)

Current `parse_item` match arms: `KwFn`, `KwConst`, `KwStatic`, `KwStruct`, `KwEnum`, `KwTrait`,
`KwImpl`, `KwType`, `KwExtern`, `KwMod`, `KwUse`, `KwUnsafe` (for `unsafe fn`/`unsafe impl`/
`unsafe trait`).

**Findings**:
- **NO `macro_rules!` arm** in `parse_item` ✅ (design-aligned — `02-grammar.md` §4.4 + §7
  forbid `macro_rules!` for v0.1/v0.3).
- **NO `KwMacroRules` token** in lexer (`src/lexer/token.rs:47-86`) ✅ (design-aligned — in
  Rust syntax, `macro_rules` is technically an identifier, not a keyword).
- If a user writes `macro_rules! foo { ... }` in Landin source, the parser would fall through
  to the `_ =>` arm at `items.rs:71-77` and emit "expected item, found ident" error. This is
  **correct behavior** per design — `macro_rules!` is not supported in v0.1/v0.3.

**Implication for strategy**: Parser changes for Stage 13.4a are limited to the MacroCall
expression branch (capture body tokens instead of skip). NO new `parse_item` arm needed. NO
new lexer token needed.

### 3.4 HIR/MIR lowering inspection — `src/hir/lower/body.rs` + `src/mir/lower/expr_operand.rs`

#### 3.4.1 HIR lowering (body.rs:374-377)

**Verified by direct read of `src/hir/lower/body.rs:374-377`**:

```rust
Expr::MacroCall { path, delim, .. } => HirExprKind::MacroCall {
    path: crate::hir::lower::path::lower_path(cx, path),
    delim: *delim,
},
```

**Findings**:
- HIR lowering is a **pass-through** — `Expr::MacroCall` → `HirExprKind::MacroCall` with no
  transformation. The `..` (line 374) discards `span`; the `delim` is copied; no `args` field
  exists to copy.
- **No expansion happens at HIR-lowering time** — the `HirExprKind::MacroCall` survives to MIR
  lowering, where the 7-arm placeholder match handles it.

**For Stage 13.4a (Strategy B)**: Replace the pass-through with a call to
`macro_expand::expand_builtin_macro(cx, path, args, span)` that returns a replacement
`HirExprKind`. The expander reads the args (now a `Vec<TokenTree>`) and emits the appropriate
HIR (e.g., `vec![1, 2, 3]` → `HirExprKind::Call { fn: vec_with_capacity, args: [3] }` followed
by 3 `vec::push` calls in a `Block`). Estimated diff: ~30-50 LOC for the dispatcher + 100-200
LOC per macro expander (some macros share helpers).

#### 3.4.2 MIR lowering (expr_operand.rs:1379-1435)

**Verified by direct read of `src/mir/lower/expr_operand.rs:1375-1435`**:

```rust
// Stage 4.10: MacroCall — expand known built-in macros.
// Previously (Stage 3.x): all macro calls produced TyKind::Error placeholder.
// Now: known macros (println!, stringify!, assert!) produce proper MIR.
// Unknown macros still fall back to Error placeholder.
HirExprKind::MacroCall { path, .. } => {
    // Get the macro name from the last path segment.
    let macro_name = path.segments.last().map(|s| s.ident.name);
    if let Some(name_spur) = macro_name {
        let name = cx.interner.resolve(&name_spur).to_string();
        match name.as_str() {
            "println" | "print" | "eprintln" | "eprint" => {
                // println!(...) → unit expression (no actual printing).
                let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                cx.mir.new_local(unit_ty, None, expr.span)
            }
            "stringify" => {
                // stringify!(expr) → &str type local (simplified).
                let str_ty = Ty::new(
                    TyKind::Ref(Region::Static, Immutable, Box::new(Ty::new(TyKind::Str, expr.span))),
                    expr.span,
                );
                cx.mir.new_local(str_ty, None, expr.span)
            }
            "assert" | "debug_assert" => {
                // assert!(cond) → unit expression (assertion check).
                let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                cx.mir.new_local(unit_ty, None, expr.span)
            }
            _ => {
                // Unknown macro → Error placeholder (fallback).
                cx.eval_rvalue_to_temp(
                    Rvalue::Use(Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                        val: ConstVal::Int(0),
                    })),
                    Ty::new(TyKind::Error, Span::DUMMY),
                    expr.span,
                )
            }
        }
    } else { ... }   // No macro name → Error placeholder
}
```

**Findings**:
- 7 macros hardcoded: `println`, `print`, `eprintln`, `eprint` (group 1, line 1385);
  `stringify` (group 2, line 1391); `assert`, `debug_assert` (group 3, line 1406).
- All 7 produce **placeholder MIR** — no actual behavior:
  - Print macros → `TyKind::Tuple(vec![])` (unit); no `printf`/`write` call emitted.
  - `stringify` → `TyKind::Ref(Static, Str)`; no string content (interner is `&Rodeo` immutable).
  - `assert`/`debug_assert` → `TyKind::Tuple(vec![])` (unit); no cond evaluation or panic path.
- 19 missing macros fall to `_ =>` arm → `TyKind::Error` (line 1412-1422).
- The match uses `name.as_str()` (string comparison) — **no `args` access** (consistent with
  AST/HIR dropping args).

**Implication for strategy**: Stage 13.4a should **remove this entire `HirExprKind::MacroCall`
arm from MIR lowering** (because macros will be expanded at HIR-lowering time per §2.4 above)
and replace it with `unreachable!("MacroCall should be expanded at HIR-lowering time")` (or
simply remove the arm entirely if `HirExprKind::MacroCall` no longer exists post-expansion).
This is a **B3 deviation closure** (impl ≠ design → impl matches design).

### 3.5 Codegen inspection — `src/codegen/mod.rs`

**Verified by direct grep of `src/codegen/mod.rs` for `MacroCall` / `println` / `vec!` / `format!` /
`macro`**.

**Findings**:
- **ZERO matches** for `MacroCall` in codegen ✅ — codegen does not see `MacroCall` (consistent
  with §2.4 design: macros expanded before MIR; but **inconsistent** with impl where MIR sees
  `MacroCall` via `HirExprKind::MacroCall` arm in `expr_operand.rs:1379-1435`).
- The `format!` matches in codegen are **Rust-side** `format!` macro invocations (used for
  generating LLVM IR label names like `format!("%arg{}", i)` at line 201, `format!("bb{}", bb_idx)`
  at line 236, etc.) — NOT Landin-side `format!` macro expansion.
- Codegen relies entirely on MIR locals produced by `expr_operand.rs` — if the MIR local has
  `TyKind::Tuple(vec![])` (println placeholder), codegen emits nothing (unit type is `EmitType::Void`
  which is skipped at line 220-222).

**Implication for strategy**: Codegen changes for Stage 13.4a are **minimal** — once macros are
expanded at HIR-lowering time into standard `HirExprKind` variants (`Call`, `Array`, `Tuple`,
`Block`, etc.), codegen processes them through existing arms. The only new codegen work is for
macros that emit **runtime calls** (e.g., `println!` → call to `printf` or `puts` via FFI;
`panic!` → call to `abort` or `panic_impl`). This requires:
1. New FFI declarations for `printf` / `puts` / `fprintf` / `abort` (in `src/stdlib/` or
   synthesized by the expander).
2. Codegen for `Terminator::Call` to these FFI functions (already supported — codegen has
   `Terminator::Call` arm at `mod.rs:874-988`).

**For Stage 13.4a (full Strategy B)**: Most macros require NO new codegen — they expand to
existing HIR/MIR patterns. Only `println!` / `print!` / `eprintln!` / `eprint!` / `panic!` /
`dbg!` / `unreachable!` / `todo!` / `unimplemented!` need runtime support (FFI calls to libc
`printf`/`fprintf`/`abort`). The remaining 17 macros are compile-time or pure-expansion
(`vec!` → array + Vec construction; `format!` → String concatenation; `assert!` family →
cond eval + panic; `concat!`/`stringify!`/`file!`/`line!`/`column!`/`module_path!` → constant
folding; `matches!` → match expression).

### 3.6 Conformance FAIL test analysis

**Methodology** (per r217 verified approach — distinguish `//! FAIL` markers from
`// EXPECTED: compile_error` from `// EXPECTED: compile_ok`):

```bash
# All conformance tests mentioning "macro" (in comments)
$ grep -rln "macro" tests/conformance/ | wc -l
6

$ grep -rln "macro_rules" tests/conformance/ | wc -l
0    # ZERO conformance tests use macro_rules! — design-aligned

$ grep -rlnE "println|format!|vec!|assert!|assert_eq|dbg!|panic!|unreachable!|todo!|..." tests/conformance/ | wc -l
11   # 11 .lin files invoke at least one built-in macro
```

**Verified findings**:

1. **6 .lin files mention "macro" in comments** (all `// EXPECTED: compile_ok`):
   - `tests/conformance/06-stdlib/02-std/001-print-macro.lin` — `fn main() { println!("hello"); }`
   - `tests/conformance/06-stdlib/02-std/002-vec-macro.lin` — `fn main() { let v = vec![]; }`
   - `tests/conformance/06-stdlib/02-std/016-std-println-macro.lin` — `fn main(){println!("hello");}`
   - `tests/conformance/06-stdlib/02-std/017-std-vec-macro.lin` — `fn main(){let v=vec![];}`
   - `tests/conformance/06-stdlib/02-std/040-std-collect-pattern.lin` — `fn main(){let v=vec![];let _=v;}`
   - `tests/conformance/06-stdlib/00-core/026-std-println-macro.lin` — `fn main(){println!("hello");}`
   - `tests/conformance/06-stdlib/00-core/027-std-vec-macro.lin` — `fn main(){let v=vec![];}`

2. **0 conformance tests use `macro_rules!`** — confirmed by `grep -rln "macro_rules" tests/conformance/`
   returning empty. This validates the design intent: `macro_rules!` is NOT part of the v0.1/v0.3
   conformance contract.

3. **All 6 macro tests currently PASS** because the 7 hardcoded macros produce *some* MIR (even
   if non-functional). The conformance runner checks compilation success, not runtime behavior.
   `vec!` falls to `TyKind::Error` (unknown macro), but the conformance runner does not reject
   `TyKind::Error` — it accepts any successful compilation. **This is a conformance suite gap**
   that Stage 13.4a should address: add tests that verify the **expanded MIR shape** (e.g.,
   `vec![1, 2, 3]` produces an array-typed local with 3 elements, not `TyKind::Error`).

4. **Sampled 5 .lin files**: All follow the pattern `fn main() { <one macro invocation> }` —
   minimal smoke tests, no behavioral verification. Stage 13.4a should add **behavioral**
   conformance tests (e.g., `assert_eq!(2 + 2, 4);` should produce a runtime assertion check,
   not just compile).

5. **Stage 4.10 unit tests** at `tests/v0/stage4/plan/macro_system_tests.rs` (3 tests):
   - `test_macro_println_no_crash` — asserts `compile("fn main() { println!(\"hello\"); }")`
     produces non-empty MIR.
   - `test_macro_stringify` — asserts `compile("fn main() { let s = stringify!(x); }")` produces
     non-empty MIR.
   - `test_macro_assert_no_crash` — asserts `compile("fn main() { assert!(1 == 1); }")` produces
     non-empty MIR.
   None of these verify **correct** expansion — only that compilation doesn't crash. Stage 13.4a
   should add behavioral tests (assert the MIR shape, assert the codegen output contains expected
   `call @printf` or `call @abort`).

**Conformance test impact for Stage 13.4a**:
- **0 existing tests need flipping** (no `// EXPECTED: compile_error` macro tests exist).
- **6 existing macro tests should remain `compile_ok`** (but gain behavioral verification).
- **~19-26 new conformance tests** should be added (one per built-in macro) — placed in
  `tests/conformance/06-stdlib/03-macros/` (new directory).
- **0 `macro_rules!` tests** should be added (design-forbidden for v0.1/v0.3).

---

## 4. Implementation Strategy (per §15)

Per §15.1 ("当面对'最小改动'与'最优架构'二选一时，选最优架构") + §13.4.2 rule 1 ("设计文档
优先级最高"), the strategy choice is constrained by design alignment (§2 above).

### 4.1 Strategy A — Full `macro_rules!` subsystem (rustc-style)

**Description**: New `src/macro_expand/` module (~1500-2500 LOC) implementing:
- `macro_rules!` definition parser (pattern + transcription).
- Pattern matching + transcription engine.
- Hygiene (SyntaxContext + ExpnId).
- 26 built-in macros reimplemented as `macro_rules!` definitions (replacing the 7 hardcoded arms).

**Pros**:
- Full feature, matches rustc.
- User-defined macros (beyond 26 built-ins).
- Foundation for v0.2 macro system.

**Cons**:
- ❌ **DESIGN-FORBIDDEN**: explicitly violates `02-grammar.md` §4.4 + §7, `12-roadmap.md` §4.1,
  `13-stage1-feature-whitelist.md` §2.6, `08-bootstrap-strategy.md` line 206.
- ❌ **Violates §13.4.2 rule 1** (设计文档优先级最高).
- ❌ Creates a **B2 deviation** (impl > design) that would need reconciliation in v0.2.
- 4-8 weeks effort per `plan-13.1.md` §2 estimate.
- HIGH risk (new subsystem, hygiene complexity).
- **NOT REQUIRED for v0.3 self-hosting** — Stage 1 source is forbidden from using `macro_rules!`.

**Verdict**: ❌ **REJECTED — design-forbidden**. Deferred to v0.2 per design (which is post-v0.1,
post-v0.3 self-hosting). Stage 13.4 must NOT implement Strategy A.

### 4.2 Strategy B — Extended built-in macros (design-sanctioned)

**Description**: Keep the hardcoded approach (per `02-grammar.md` §4.4 "编译器硬编码展开") but:
1. Add `TokenTree` type (or equivalent) to AST module.
2. Update `MacroCall` AST/HIR variant to carry `args: Vec<TokenTree>` (B1 fix).
3. Update parser `parse_primary_expr` MacroCall branch to **capture** body tokens (not skip).
4. Move macro expansion from MIR-lowering (`expr_operand.rs:1379-1435`) to HIR-lowering
   (`body.rs:374-377`) — design-aligned (MIR/codegen should not see `MacroCall`).
5. Add new `src/macro_expand/` module with `builtin.rs` containing 26 expander functions.
6. Each expander reads `args: Vec<TokenTree>` and returns a replacement `HirExprKind`.
7. Replace the 7-arm placeholder match in `expr_operand.rs` with `unreachable!()` (or remove
   the arm entirely).

**Pros**:
- ✅ **DESIGN-SANCTIONED** — `02-grammar.md` §4.4 explicitly says "编译器硬coded展开".
- ✅ Closes the B1 deviation (`MacroCall.args` field).
- ✅ Closes the B3 deviation (MIR no longer sees `MacroCall`).
- ✅ Satisfies Stage 1 contract (26 built-in macros).
- 400-1200 LOC (smaller than Strategy A's 1500-2500).
- MEDIUM risk (well-scoped, no hygiene complexity).

**Cons**:
- No user-defined macros (but not needed for v0.3).
- Each of 19 new macros needs individual implementation + testing.
- Requires `TokenTree` type (new infrastructure).
- 9 src files exceeds §14.4 J5 ≤5 guideline (justifies Strategy C split).

**Verdict**: ✅ **RECOMMENDED** — design-aligned, closes all 3 deviations (B1 AST args, B3 MIR
sees MacroCall, B4 TokenTree type), satisfies Stage 1 contract.

### 4.3 Strategy C — Preparation phase (mirrors Stage 13.3 → 13.3a precedent)

**Description**: Stage 13.4 = preparation only (this design alignment + TokenTree infrastructure
skeleton + verification test infrastructure); Stage 13.4a = full Strategy B implementation.

**Stage 13.4 (preparation) deliverables**:
1. This design alignment document (`stage-13.4-design-alignment.md`).
2. Gate review `gate-review-13.4.md` (committee vote on preparation phase PASS).
3. `tests/v0/stage13/plan/stage13_4_tests.rs` skeleton (verification tests for design alignment
   + current state + gate review existence).
4. `src/macro_expand/mod.rs` stub (empty module declaration, registered in `src/lib.rs`).
5. `src/ast/token_tree.rs` stub (empty `TokenTree` type definition, registered in `src/ast/mod.rs`).
6. Patch bump `Cargo.toml` v0.23.0 → v0.23.1.

**Stage 13.4a (implementation) deliverables** (deferred to next phase):
1. Full `TokenTree` type implementation (parse + display + helpers).
2. AST `MacroCall` field update (`delim` → `args: Vec<TokenTree>` or both).
3. HIR `MacroCall` field update.
4. Parser `capture_delim_group()` method + MacroCall branch update.
5. HIR lowering `Expr::MacroCall` arm → call to `macro_expand::expand_builtin_macro`.
6. `src/macro_expand/builtin.rs` with 26 expander functions.
7. MIR lowering `HirExprKind::MacroCall` arm removal.
8. ~19-26 new conformance tests in `tests/conformance/06-stdlib/03-macros/`.
9. Minor bump `Cargo.toml` v0.23.1 → v0.24.0.

**Pros**:
- ✅ Mirrors Stage 13.3 → 13.3a precedent (preparation phase PASS, then implementation phase PASS).
- ✅ Allows Stage Committee to validate the **design reframe** (TD-032: macro_rules! → 19 missing
  built-in macros) before committing to implementation.
- ✅ Allows Stage Committee to validate the **strategy reframe** (Strategy A forbidden, Strategy B
  sanctioned) before execution.
- ✅ Reduces execution risk: Stage 13.4a can focus on pure implementation without re-litigating
  design decisions.
- ✅ TokenTree stub establishes the module boundary; Stage 13.4a fills it in.

**Cons**:
- TD-032 remains OPEN through Stage 13.4 (closed only in Stage 13.4a).
- Two-phase delivery extends timeline by 1 session.
- Risk of stub-code staleness if Stage 13.4a is delayed.

**Verdict**: ✅ **RECOMMENDED** for Stage 13.4 (this phase); Strategy B (§4.2) recommended for
Stage 13.4a (next phase). Justified by:
- §15.3 #3 ("最优方案依赖未就绪的前置条件"): Strategy B requires `TokenTree` type (B4
  design-gray-area) — preparation phase establishes the type boundary.
- §25.7 ("阻塞下一阶段的 P0/P1 必须本阶段修复，不允许带入下一阶段"): TD-032 is P0, but the
  **design reframe** (Strategy A forbidden, Strategy B sanctioned, TD-032 reframe to 19 missing
  macros) is a **process decision** that must be ratified by Stage Committee before execution —
  this is exactly what the preparation phase accomplishes.
- §14.4 J5 (≤5 files guideline): full Strategy B touches 9 src files; preparation phase touches
  ~5 files (within guideline).
- Stage 13.3 → 13.3a precedent: preparation phase PASS at v0.22.0→v0.22.1, then implementation
  PASS at v0.22.1→v0.23.0. Same pattern for 13.4 → 13.4a: v0.23.0→v0.23.1 → v0.23.1→v0.24.0.

### 4.4 Strategy comparison matrix

| Criterion | Strategy A (full macro_rules!) | Strategy B (extend built-in) | Strategy C (prep + B) |
|-----------|-------------------------------|------------------------------|----------------------|
| Design alignment (§13.4.2 rule 1) | ❌ **FORBIDDEN** by 02-grammar.md §4.4 + §7, 12-roadmap.md §4.1, 13-stage1-feature-whitelist.md §2.6 | ✅ **SANCTIONED** by 02-grammar.md §4.4 ("编译器硬编码展开") | ✅ (Strategy B sanctioned; preparation phase acceptable per §15.3 #3) |
| §15.2 "最优" criteria | ❌ violates "架构对齐" (design forbids) | ✅ eliminates root cause (B1 + B3 + B4 deviations) | ✅ (preparation enables Strategy B) |
| Stage 1 contract (26 macros) | ⚠️ over-delivers (user-defined macros not needed) | ✅ exactly satisfies | ✅ (satisfied in 13.4a) |
| v0.3 self-hosting unblock | ❌ not required (Stage 1 source forbidden from macro_rules!) | ✅ unblocks | ✅ (unblocks in 13.4a) |
| LOC estimate | 1500-2500 | 400-1200 | 50-100 (prep) + 400-1200 (impl) |
| File count | ~12-15 src | ~9 src | ~5 (prep) + ~9 (impl) |
| §14.4 J5 (≤5 files) | ❌ exceeds | ⚠️ exceeds (justifies split) | ✅ (prep within guideline) |
| Risk | HIGH (new subsystem, hygiene) | MEDIUM (well-scoped) | LOW (prep) + MEDIUM (impl) |
| Timeline | 4-8 weeks | 2-4 weeks | 1 session (prep) + 2-4 weeks (impl) |
| Precedent | n/a (no prior macro_rules! work) | matches Stage 4.10 pattern (extend hardcode) | matches Stage 13.3→13.3a pattern |
| **Recommendation** | ❌ **REJECT** | ✅ **ACCEPT** (for Stage 13.4a) | ✅ **ACCEPT** (for Stage 13.4 this phase) |

### 4.5 §14.4 J1-J6 evaluation (Strategy C for Stage 13.4 + Strategy B for Stage 13.4a)

| # | Criterion | Stage 13.4 (Strategy C prep) | Stage 13.4a (Strategy B impl) |
|---|-----------|------------------------------|-------------------------------|
| J1 | Architecture alignment | ✅ TokenTree stub + macro_expand stub align with 05-ast.md §8 + 02-grammar.md §4.4 | ✅ MacroCall.args field + HIR-time expansion align with 05-ast.md §8 + design silence on MIR-side macros |
| J2 | Single responsibility | ✅ macro_expand module owns macro expansion (boundary established) | ✅ macro_expand/builtin.rs owns 26 expanders; AST owns TokenTree; parser owns capture; HIR-lower owns dispatch |
| J3 | Single direction flow | ✅ stubs don't introduce flows | ✅ HIR-lower calls macro_expand (one-way); MIR-lower no longer sees MacroCall |
| J4 | Compilation expression complete | ✅ stubs are placeholders | ✅ TokenTree type complete; MacroCall.args complete; 26 expanders complete |
| J5 | Stage division clear (≤5 files) | ✅ 5 files (this doc + gate-review + test + 2 stubs) | ⚠️ 9 src files exceeds ≤5 guideline — justified by §15 long-term value + design-aligned B1/B3/B4 closure |
| J6 | Scientific granularity | ✅ stubs are minimal viable | ✅ each macro expander is ~20-50 LOC; TokenTree is ~50-100 LOC; total ~800-1200 LOC across 9 files (avg ~100 LOC/file, within 100-1500 guideline) |

**J5 marginality justification for Stage 13.4a** (9 src files): The 9 files are:
1. `src/ast/token_tree.rs` (new, ~80 LOC) — TokenTree type
2. `src/ast/kinds.rs` (modify, +5 LOC) — MacroCall args field
3. `src/ast/mod.rs` (modify, +1 LOC) — pub mod token_tree
4. `src/hir/kinds.rs` (modify, +3 LOC) — HirExprKind::MacroCall args field
5. `src/hir/lower/body.rs` (modify, +30 LOC) — MacroCall arm dispatch
6. `src/parser/expr.rs` (modify, +20 LOC) — capture_delim_group + MacroCall branch
7. `src/macro_expand/mod.rs` (modify, +20 LOC) — expand_builtin_macro dispatcher
8. `src/macro_expand/builtin.rs` (new, ~600-800 LOC) — 26 expander functions
9. `src/mir/lower/expr_operand.rs` (modify, -50 LOC) — remove MacroCall arm

Per §15 ("long-term > short-term") + design alignment (B1+B3+B4 closure requires touching all
these files), the 9-file scope is justified. The alternative (splitting Strategy B into 13.4a
+ 13.4b + 13.4c) would fragment the closure criterion (26 macros all functional) across multiple
phases, increasing coordination overhead without reducing risk.

---

## 5. Scope Analysis

### 5.1 Stage 13.4 (preparation phase) — file inventory

**5 files** (within §14.4 J5 ≤5 guideline):

| # | File | Change | LOC delta | Risk |
|---|------|--------|-----------|------|
| 1 | `docs/develop/v0/stage-13/stage-13.4-design-alignment.md` | **Create** (this file) — §13.4 design alignment + scope analysis + Strategy C recommendation + §25.8 write-back plan | +~600 LOC | LOW (documentation only) |
| 2 | `docs/develop/v0/stage-13/gate-review-13.4.md` | **Create** — Stage Committee vote on preparation phase PASS; documents Strategy C→B split, TD-032 reframe, version policy v0.23.0→v0.23.1 | +~150 LOC | LOW (documentation only) |
| 3 | `tests/v0/stage13/plan/stage13_4_tests.rs` | **Create** — verification test skeleton (6-9 tests): design alignment exists + has reframe + has strategy C/B; gate review exists + has TD-032 reframe + version policy; current state (7 hardcoded macros, 19 missing, MacroCall discards args); v0.1 gate still holds; worklog entry exists | +~120 LOC | LOW (test infrastructure only) |
| 4 | `tests/all_tests.rs` | **Modify** — add `#[path = "v0/stage13/plan/stage13_4_tests.rs"] mod stage13_4_tests;` | +2 LOC | LOW |
| 5 | `Cargo.toml` | **Bump** version `0.23.0` → `0.23.1` (patch bump for preparation phase) + append Stage 13.4 entry to description field | +1 line | LOW |

**Optional 6th file** (if Stage Committee prefers stub modules):

| # | File | Change | LOC delta | Risk |
|---|------|--------|-----------|------|
| 6 | `src/macro_expand/mod.rs` + `src/lib.rs` registration | **Create stub** — `pub mod macro_expand;` + empty `pub fn expand_builtin_macro() -> unimplemented!("Stage 13.4a")` placeholder. Registers module boundary so Stage 13.4a can fill in without re-plumbing. | +10 LOC | LOW |

**Recommendation**: Include file #6 (stub) — establishes the module boundary cleanly and lets
Stage 13.4a focus on implementation. Total: **6 files**, still within §14.4 J5 ≤5+1 guideline
(marginal exception for stub module).

### 5.2 Stage 13.4a (implementation phase) — file inventory (deferred)

**~15-20 files** (exceeds §14.4 J5 ≤5 guideline — justified per §4.5 J5 marginality):

| # | File | Change | LOC delta | Risk |
|---|------|--------|-----------|------|
| 1 | `src/ast/token_tree.rs` | **Create** — `TokenTree` type + `TokenStream` alias + parse/display helpers | +80-120 LOC | MEDIUM (new type; design B4 closure) |
| 2 | `src/ast/mod.rs` | **Modify** — `pub mod token_tree;` registration | +1 LOC | LOW |
| 3 | `src/ast/kinds.rs` | **Modify** — `MacroCall { path, args: Vec<TokenTree>, delim: MacroDelim, span }` (preserve `delim` for diagnostics; add `args`) | +3 LOC | LOW (additive field) |
| 4 | `src/hir/kinds.rs` | **Modify** — `HirExprKind::MacroCall { path, args: Vec<TokenTree> }` (add `args`) | +2 LOC | LOW |
| 5 | `src/parser/expr.rs` | **Modify** — replace `skip_delim_group()` with `capture_delim_group() -> Vec<TokenTree>` in MacroCall branch | +20-30 LOC | MEDIUM (new capture method) |
| 6 | `src/hir/lower/body.rs` | **Modify** — `Expr::MacroCall` arm dispatches to `macro_expand::expand_builtin_macro(cx, path, args, span)` returning `HirExprKind` | +30-50 LOC | MEDIUM (dispatcher wiring) |
| 7 | `src/macro_expand/mod.rs` | **Modify** — `expand_builtin_macro()` dispatcher: match on macro name, dispatch to per-macro expander in `builtin.rs` | +50-80 LOC | MEDIUM |
| 8 | `src/macro_expand/builtin.rs` | **Create** — 26 expander functions (one per built-in macro): `expand_println`, `expand_vec`, `expand_format`, `expand_assert`, `expand_assert_eq`, `expand_panic`, `expand_unreachable`, `expand_todo`, `expand_concat`, `expand_stringify`, `expand_file`, `expand_line`, `expand_column`, `expand_module_path`, `expand_matches`, `expand_dbg`, `expand_write`, `expand_writeln`, `expand_unimplemented`, `expand_print`, `expand_eprintln`, `expand_eprint`, `expand_debug_assert`, `expand_debug_assert_eq`, `expand_debug_assert_ne`, `expand_assert_ne` | +600-800 LOC | HIGH (correctness per macro; arg parsing; FFI for print/panic) |
| 9 | `src/mir/lower/expr_operand.rs` | **Modify** — remove `HirExprKind::MacroCall` arm (lines 1375-1435); add `unreachable!("MacroCall expanded at HIR-lowering time")` | -60 LOC | LOW (deletion) |
| 10 | `src/codegen/mod.rs` | **Modify** (potentially) — if any macro needs special codegen (unlikely — expanders produce standard HirExprKind) | ±0-20 LOC | LOW |
| 11 | `src/lib.rs` | **Modify** — `pub mod macro_expand;` registration (if not done in Stage 13.4 prep) | +1 LOC | LOW |
| 12 | `tests/v0/stage13/plan/stage13_4a_tests.rs` | **Create** — behavioral verification tests for 26 macros (MIR shape, codegen output, runtime behavior mock) | +300-500 LOC | MEDIUM (test design) |
| 13 | `tests/all_tests.rs` | **Modify** — add stage13_4a_tests module | +2 LOC | LOW |
| 14-19 | `tests/conformance/06-stdlib/03-macros/{println,vec,format,assert_eq,matches,panic,...}.lin` | **Create** — 6-19 new conformance tests (one per non-trivial macro) | +6-19 files × 5-10 LOC | LOW (mechanical) |
| 20 | `Cargo.toml` | **Bump** v0.23.1 → v0.24.0 (minor bump for third user-facing feature) | +1 line | LOW |

**Design-doc write-back files** (per §25.8, executed post-Stage 13.4a implementation):

| # | File | Change | Risk |
|---|------|--------|------|
| 21 | `docs/lang-design/05-ast.md` | **§8** — `MacroCall` variant: update field list to `mac: Path, args: Vec<TokenTree>, span: Span` (already in design, but add §25.8 note that impl now matches). **§1 line 12** — update "MacroCall 节点（v0.2 用）" to "MacroCall 节点（v0.1 实现 args 字段 in Stage 13.4a; v0.2 加 macro_rules!）". **§13 §25.8** — add Stage 13.4a implementation status note. | LOW |
| 22 | `docs/lang-design/02-grammar.md` | **§4.4** — add §25.8 retroactive note: "Stage 13.4a closes the B1 implementation gap (7/26 → 26/26 hardcoded macros per design '编译器硬编码展开')". | LOW |
| 23 | `docs/lang-design/07-codegen.md` | **§15 or new §16** — add note: "macros are expanded at HIR-lowering time (per `02-grammar.md` §4.4 '编译器硬编码展开'); codegen never sees `MacroCall` (Stage 13.4a removes the MIR-lowering `HirExprKind::MacroCall` arm that violated this design invariant)". | LOW |
| 24 | `docs/lang-design/13-stage1-feature-whitelist.md` | **§2.6** — add implementation status note: "All 26 built-in macros implemented in Stage 13.4a (v0.24.0); 7 in Stage 4.10 (v0.6.0) + 19 in Stage 13.4a". | LOW |
| 25 | `docs/lang-design/09-stdlib.md` | **§4.3** — add note: "v0.1 compiler-side hardcoded `println!` expansion implemented in Stage 13.4a; stdlib-side `macro_rules!` definition remains v0.2". | LOW |

### 5.3 Risk assessment

#### Stage 13.4 (preparation phase) — LOW risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Stage Committee rejects TD-032 reframe (macro_rules! → 19 missing macros) | LOW | HIGH (Strategy A would be design-forbidden; would block v0.3 self-hosting on a feature not needed for it) | This design alignment doc provides exhaustive evidence (5 design docs unanimously forbid macro_rules! for v0.1/v0.3); §13.4.2 rule 1 is unambiguous |
| Stage Committee rejects Strategy C split (prefers direct Strategy B execution) | MEDIUM | LOW (either path closes TD-032; direct execution saves 1 session) | Document the §14.4 J5 + §15.3 #3 + Stage 13.3→13.3a precedent justifying the split |
| Stub module (`src/macro_expand/mod.rs`) creates dead code warning | MEDIUM | LOW (clippy gate fails) | Use `#[allow(dead_code)]` on stub or add minimal `pub fn expand_builtin_macro() -> ! { unimplemented!("Stage 13.4a") }` placeholder |
| Conformance suite regresses | LOW | HIGH (v0.1 gate breach) | Preparation phase makes ZERO functional changes; only stubs + docs + tests |

**Overall Stage 13.4 risk**: **LOW**. Preparation phase is documentation + test infrastructure +
stub modules only. Zero functional changes; zero conformance impact; zero regression risk.

#### Stage 13.4a (implementation phase) — HIGH risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TokenTree type design wrong (e.g., missing token variants, wrong span handling) | MEDIUM | HIGH (cascades to all 26 expanders) | Reference rustc `TokenTree` design; start with minimal `Leaf(Token)` + `Group(Delim, Vec<TokenTree>)` variants; add unit tests in `tests/v0/stage13/plan/stage13_4a_tests.rs` |
| Parser `capture_delim_group()` produces wrong token stream (e.g., off-by-one, missing closing delim) | MEDIUM | HIGH (all macros get wrong args) | Reuse existing `skip_delim_group()` logic (just change `skip` to `capture`); add round-trip tests (capture → display → re-parse → same AST) |
| HIR-time expansion produces malformed HirExprKind (e.g., wrong HirId, missing span) | MEDIUM | MEDIUM (downstream MIR lowering fails) | Reuse existing HirExprKind construction patterns from `body.rs`; add per-macro unit tests asserting the expanded HIR shape |
| Individual macro expander incorrect (e.g., `vec!` produces wrong capacity, `assert_eq!` doesn't actually compare) | HIGH (per-macro) | MEDIUM (macro doesn't work as expected; conformance test fails) | Per-macro conformance tests in `tests/conformance/06-stdlib/03-macros/`; reference rustc expansion semantics; start with simplest macros (`file!`, `line!`, `column!` → constants) and progress to complex (`vec!`, `format!`, `assert_eq!`) |
| FFI for `println!`/`panic!` doesn't link (missing libc symbol) | MEDIUM | HIGH (linker error; conformance tests fail at runtime) | Use existing `extern "C" { fn printf(...) }` pattern from `09-stdlib.md` §3.3; add linker flag `-lc` if needed; test on Linux x86_64 first |
| Removing `HirExprKind::MacroCall` arm from MIR lowering breaks existing 7 hardcoded macro tests | HIGH (certain) | LOW (tests need update) | Update `tests/v0/stage4/plan/macro_system_tests.rs` (3 tests) — they assert non-empty MIR, which remains true post-expansion (expanded HIR produces non-empty MIR) |
| Conformance regression (existing 6 macro tests break) | MEDIUM | MEDIUM | Run `python3 tests/conformance/run_all.py` after each macro expander; fix regressions before next expander |
| 9 src files exceeds §14.4 J5 ≤5 guideline | HIGH (certain) | LOW (process deviation) | Document J5 marginality justification in `gate-review-13.4a.md` (per §4.5 above); Stage Committee approval required |

**Overall Stage 13.4a risk**: **HIGH**. 9 src files + ~800-1200 LOC + 26 individual macro
expanders + new TokenTree type + HIR-time expansion (architectural shift from MIR-time). The
5026 conformance + 2256 rust tests provide strong regression coverage, but the **new code**
needs extensive unit testing (per-macro behavioral tests in `stage13_4a_tests.rs`).

---

## 6. Committee Recommendation

### **GO-WITH-CONDITIONS** for Stage 13.4 launch (preparation phase)

**Conditions**:

1. ✅ **TD-032 reframe ratified** — Stage Committee must formally reframe TD-032 closure criterion
   from "macro_rules! implemented" to "all 26 built-in macros functional" per r217 §2.6 + §2.6 of
   this doc. The r216 label "macro_rules!" is design-misaligned; the r217 reframe is
   design-aligned. Update `gate-review-13.4.md` TD table row accordingly.

2. ✅ **Strategy A (full macro_rules!) explicitly REJECTED** as design-forbidden per §13.4.2 rule 1.
   Stage 13.4 / 13.4a must NOT implement `macro_rules!`. Deferred to v0.2 per `02-grammar.md` §4.4
   + §7, `12-roadmap.md` §4.1, `13-stage1-feature-whitelist.md` §2.6, `08-bootstrap-strategy.md`
   line 206.

3. ✅ **Strategy C (preparation) for Stage 13.4 + Strategy B (implementation) for Stage 13.4a**
   — split ratified per §15.3 #3 (前置条件未就绪 — TokenTree type B4 gray-area) + §14.4 J5
   (≤5 files guideline for preparation) + Stage 13.3→13.3a precedent.

4. ✅ **Version policy = patch bump for Stage 13.4 (v0.23.0 → v0.23.1)** — preparation phase,
   no functional changes, no user-facing feature. **v0.24.0 reserved for Stage 13.4a** (minor
   bump — third user-facing compiler feature, per `stage-13.1-design-alignment.md` §5.4 line 544).

5. ✅ **§25.8 write-back deferred to Stage 13.4a** — preparation phase makes no implementation
   changes, so no design-doc write-back is needed yet. Stage 13.4a must update 5 design docs
   (`05-ast.md`, `02-grammar.md`, `07-codegen.md`, `13-stage1-feature-whitelist.md`,
   `09-stdlib.md`) per §5.2 file inventory #21-25.

6. ✅ **Test verification gate** (for Stage 13.4 preparation):
   1. `cargo build` — zero warnings, zero errors.
   2. `cargo test --test all_tests` — 5026 conformance + 2256 rust tests unchanged (zero
      regressions; zero new passes — preparation phase makes no functional changes).
   3. `cargo fmt --check` — zero diff.
   4. `cargo clippy --all-targets` — zero warnings (use `#[allow(dead_code)]` on stub if needed).
   5. Stage 13.4 verification tests pass: `cargo test --test all_tests -- stage13_4_tests`.
   6. `python3 tests/conformance/run_all.py` — 5026 passed, 0 failed (no regression).

### Strategy summary

| Phase | Strategy | Files | LOC | Risk | Version | TD-032 status |
|-------|----------|-------|-----|------|---------|---------------|
| Stage 13.4 (this) | C (preparation) | 5-6 | ~100-200 | LOW | v0.23.0 → v0.23.1 (patch) | 🔄 OPEN (prep done) |
| Stage 13.4a (next) | B (implementation) | ~15-25 | ~800-1200 | HIGH | v0.23.1 → v0.24.0 (minor) | ✅ CLOSED (26/26 macros) |

### Post-Stage 13.4a outlook

Once Stage 13.4a closes TD-032:
- **All 3 P0 blockers closed** (TD-028 ✅, TD-029 ✅, TD-030 ✅, TD-031 ✅, TD-032 ✅).
- **v0.3 self-hosting unblocked** — Stage 1 source writing can begin per `v0.3-bootstrap-prep.md`.
- **Stage 13.5+** (TD-033 P1 items: for loop, move closure, HRTB, associated type normalization,
  two-phase borrows, disjoint closure captures) can proceed in parallel with Stage 1 drafting.
- **v0.2 macro_rules!** remains a separate future stage (post-v0.3 self-hosting) per design.

---

**Audit completed**: 2026-07-26
**Next action**: Stage Committee vote on this design alignment → if GO-WITH-CONDITIONS, Stage 13.4
preparation phase execution (1 session) → Stage 13.4a gate review → Stage 13.4a implementation
(2-4 weeks per `plan-13.1.md` §2 Stage 13.4 estimate, adjusted for Strategy B smaller scope vs
r216's Strategy A estimate).
