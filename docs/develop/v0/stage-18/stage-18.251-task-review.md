# Stage 18.251 — Task Review: TD-EXPECT Audit Resolution

> **Date**: 2026-08-24
> **Version**: v0.492.0 (no bump — audit + documentation)
> **Task ID**: stage18.251
> **Reviewer**: Super Z (main) — ARCH-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §17.8

## 1. 触发场景

Per tech-debt-register: TD-EXPECT-TYPECK-SOLVER and TD-EXPECT-PARSER-ITEMS
are 🟡 MEDIUM — "审计每个 expect 的 message"。

## 2. Audit Results

### 2.1 TD-EXPECT-TYPECK-SOLVER (src/typeck/solver.rs)

**Finding**: ALL 37 `.expect()` calls are inside `#[cfg(test)] mod tests`.
The `#[cfg(test)]` marker is at line 262, and ALL expect calls are at
line 286+ (after the test module begins).

**Status**: ✅ Already acceptable — test-code expects with descriptive
messages ("Foo should be interned", "S not found in type_by_def_id", etc.)
are standard practice. No production code has bare `.expect()`.

**Per §1.0 原則 9 (正确>妥协)**: test expects are correct — they provide
descriptive panic messages that aid debugging. No action needed.

### 2.2 TD-EXPECT-PARSER-ITEMS (src/parser/items.rs)

**Finding**: ALL 37 calls are to `self.expect(&TokenKind, &str)` — the
parser's built-in `expect` method (defined in parser.rs:271). This is NOT
`Option::expect()` or `Result::expect()` — it's a custom parser method
that pushes a `ParseError` (doesn't panic).

The `what` parameter already has descriptive messages: "`]`", "`)`",
"`{` or `;`", "`:`", "`=`", "`;`", etc.

**Status**: ✅ Already acceptable — `self.expect()` is a non-panicking
parser helper that pushes parse errors. The `what` parameter provides
the expected token description. No bare `.expect()` exists.

**Per §1.0 原則 4 (报错>静默)**: parser's `expect()` correctly reports
errors (doesn't silently fail). Per §1.0 原則 6 (通解>特解): one
`expect()` method handles all token expectations.

## 3. Decision: CLOSE both TDs

Both TDs are false positives:
- TD-EXPECT-TYPECK-SOLVER: all expects are in test code with descriptive messages
- TD-EXPECT-PARSER-ITEMS: `expect()` is a custom parser method, not `Option::expect()`

No code changes needed. Both TDs should be marked ✅ Resolved (audited, no action needed).

## 4. Documentation Updates

Update tech-debt-register to close both TDs.
