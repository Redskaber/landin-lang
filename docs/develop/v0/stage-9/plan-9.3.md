# Stage 9.3 开发计划: Control flow conformance 扩展

> **阶段**: Stage 9.3 (Stage 9 第 3 个子阶段)
> **版本**: v0.16.1 → v0.16.2
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.2 完成 conformance 38 → 98 (operators category)。Stage 9.3 继续扩展
**control flow** 类别 (per `17-conformance-suite.md` §2 + `02-grammar.md` §3.4
+ §3.6)。

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.4 (expr control flow forms):
  - `"if" expr block ("else" (if_expr | block))?`
  - `"if" "let" pat "=" expr block ("else" (if_let_expr | block))?`
  - `"match" expr "{" match_arm* "}"`
  - `"loop" block`
  - `"while" expr block`
  - `"while" "let" pat "=" expr block`
  - `"for" pat "in" expr block`
  - `"unsafe" block`
  - `"return" expr?`
  - `"break" expr?`
  - `"continue"`
- `docs/lang-design/02-grammar.md` §3.6 (stmt + block):
  - `stmt := "let" pat (":" type)? "=" expr ";" | expr ";" | expr | ";"`
  - `block := "{" stmt* expr? "}"`
- `docs/lang-design/02-grammar.md` §3.4 match_arm:
  - `match_arm := pat ("if" expr)? "=>" (expr "," | block)`
- `src/parser/expr.rs` parse_if_expr + parse_match_expr

## 3. 测试设计 (80 个 .lin tests)

### 3.1 if / else (12 tests)

| 测试文件 | 描述 |
|---------|------|
| if_basic.lin | `if cond { 1 }` (无 else) |
| if_else.lin | `if cond { 1 } else { 2 }` (已存在, 保留) |
| if_else_if.lin | `if a { 1 } else if b { 2 } else { 3 }` (else-if chain) |
| if_no_else.lin | `if cond { let x = 1; }` (statement form) |
| if_in_let.lin | `let x = if c { 1 } else { 2 };` (expression form) |
| if_nested.lin | `if a { if b { 1 } else { 2 } } else { 3 }` |
| if_cond_cmp.lin | `if x > 0 { 1 }` (comparison condition) |
| if_cond_logic.lin | `if a && b { 1 }` (logical condition) |
| if_cond_call.lin | `if f() { 1 }` (function call condition) |
| if_block_multi_stmt.lin | `if c { let x = 1; let y = 2; x + y }` (multi-stmt block) |
| if_empty_block.lin | `if c { }` (empty block) |
| if_expr_returns.lin | `if c { 1 } else { 2 }` used as return value |

### 3.2 if let (6 tests)

| 测试文件 | 描述 |
|---------|------|
| if_let_basic.lin | `if let Some(x) = opt { 1 }` |
| if_let_else.lin | `if let Some(x) = opt { 1 } else { 0 }` |
| if_let_tuple.lin | `if let (a, b) = pair { 1 }` |
| if_let_struct.lin | `if let P { x, y } = p { 1 }` |
| if_let_wildcard.lin | `if let _ = x { 1 }` |
| if_let_chain.lin | `if let Some(a) = x { if let Some(b) = y { 1 } }` |

### 3.3 while (8 tests)

| 测试文件 | 描述 |
|---------|------|
| while_basic.lin | `while cond { ... }` |
| while_cond_cmp.lin | `while i < 10 { i += 1; }` |
| while_cond_logic.lin | `while a && !done { ... }` |
| while_empty.lin | `while c { }` (empty body) |
| while_break.lin | `while true { break; }` |
| while_continue.lin | `while c { if x { continue; } ... }` |
| while_nested.lin | `while a { while b { ... } }` |
| while_in_fn.lin | `fn f() { while c { ... } }` |

### 3.4 while let (5 tests)

| 测试文件 | 描述 |
|---------|------|
| while_let_basic.lin | `while let Some(x) = iter.next() { ... }` |
| while_let_else.lin | (no else — Rust syntax: while let has no else) |
| while_let_break.lin | `while let Some(x) = it { if x == 0 { break; } }` |
| while_let_tuple.lin | `while let (a, b) = pair { ... }` |
| while_let_nested.lin | nested while-let |

### 3.5 for (8 tests)

| 测试文件 | 描述 |
|---------|------|
| for_basic.lin | `for x in iter { ... }` |
| for_range.lin | `for i in 0..10 { ... }` |
| for_range_inclusive.lin | `for i in 0..=10 { ... }` |
| for_break.lin | `for x in iter { if x == 0 { break; } }` |
| for_continue.lin | `for x in iter { if x == 0 { continue; } }` |
| for_nested.lin | `for i in a { for j in b { ... } }` |
| for_pat_tuple.lin | `for (i, v) in pairs { ... }` |
| for_empty.lin | `for x in iter { }` |

### 3.6 loop (6 tests)

| 测试文件 | 描述 |
|---------|------|
| loop_basic.lin | `loop { ... }` |
| loop_break.lin | `loop { break; }` |
| loop_break_value.lin | `let x = loop { break 42; };` |
| loop_continue.lin | `loop { if c { continue; } break; }` |
| loop_nested.lin | `loop { loop { break; } break; }` |
| loop_while_interplay.lin | `loop { while c { ... } break; }` |

### 3.7 match (15 tests)

| 测试文件 | 描述 |
|---------|------|
| match_basic.lin | `match x { 1 => 1, _ => 0 }` |
| match_multi_arm.lin | `match x { 1 => 1, 2 => 2, _ => 0 }` |
| match_wildcard.lin | `match x { _ => 0 }` |
| match_ident.lin | `match x { y => y }` |
| match_tuple.lin | `match (a, b) { (1, 2) => 1, _ => 0 }` |
| match_struct.lin | `match p { P { x, y } => x + y }` |
| match_enum.lin | `match e { E::A => 1, E::B(_) => 2 }` |
| match_guard.lin | `match x { y if y > 0 => 1, _ => 0 }` |
| match_block_arm.lin | `match x { 1 => { let y = 1; y } _ => 0 }` |
| match_range_pat.lin | `match x { 1..=10 => 1, _ => 0 }` |
| match_or_pat.lin | `match x { 1 \| 2 => 1, _ => 0 }` |
| match_nested.lin | `match x { 1 => match y { 2 => 1, _ => 0 } _ => 2 }` |
| match_in_let.lin | `let z = match x { 1 => 1, _ => 0 };` |
| match_expr_scrutinee.lin | `match f() { 0 => 1, _ => 0 }` |
| match_empty.lin | `match x { }` (may be FAIL or PASS via synthetic) |

### 3.8 break / continue / return (10 tests)

| 测试文件 | 描述 |
|---------|------|
| break_basic.lin | `loop { break; }` |
| break_value.lin | `loop { break 42; }` |
| break_in_while.lin | `while c { break; }` |
| break_in_for.lin | `for x in it { break; }` |
| continue_basic.lin | `while c { continue; }` |
| continue_in_for.lin | `for x in it { continue; }` |
| continue_in_loop.lin | `loop { continue; }` |
| return_basic.lin | `fn f() -> i32 { return 42; }` |
| return_void.lin | `fn f() { return; }` |
| return_in_match.lin | `fn f(x: i32) -> i32 { match x { 0 => return 1, _ => 0 } }` |

### 3.9 block + statement (5 tests)

| 测试文件 | 描述 |
|---------|------|
| block_basic.lin | `fn f() { { let x = 1; x } }` (nested block) |
| block_expr.lin | `let x = { 1 + 2 };` (block as expression) |
| block_trailing_expr.lin | `fn f() -> i32 { let x = 1; x + 1 }` |
| stmt_let.lin | `let x = 1;` (basic let) |
| stmt_let_with_type.lin | `let x: i32 = 1;` (let with type annotation) |

### 3.10 边界 & 错误恢复 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| err_if_without_cond.lin | `FAIL: if { 1 }` (missing condition) |
| err_match_without_scrutinee.lin | `FAIL: match { 1 => 1 }` (missing scrutinee) |
| err_while_without_cond.lin | `FAIL: while { 1 }` (missing condition) |
| err_for_without_in.lin | `FAIL: for x iter { 1 }` (missing `in`) |
| err_break_outside_loop.lin | `PASS or FAIL: fn f() { break; }` (break outside loop — depends on parser behavior) |

**累计**: 12 + 6 + 8 + 5 + 8 + 6 + 15 + 10 + 5 + 5 = **80 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2122+ tests pass (期望 +11 verification tests = 2133)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 178 passed (98 + 80 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.1 → 0.16.2
- api-naming-standard.md: v2.05 → v2.06

---

**创建日期**: 2026-07-26
