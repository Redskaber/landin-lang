# Stage 9.8 开发计划: Closures conformance 扩展

> **阶段**: Stage 9.8 (Stage 9 第 8 个子阶段)
> **版本**: v0.16.6 → v0.16.7
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.7 完成 conformance 347 → 397 (generics category, over 2/3!). Stage 9.8
继续扩展 **closures** 类别 (per `17-conformance-suite.md` §2 + `02-grammar.md`
§3.4 + §4.2).

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.4 (closure forms):
  - `"move" closure | closure | ...` (primary expr forms)
  - `closure := "||" fn_params? expr | "||" block | "|" fn_params "|" expr_or_block`
- `docs/lang-design/02-grammar.md` §4.2 (closure vs binary OR disambiguation):
  - "在表达式上下文，`|` 后面跟随 pat 时识别为 closure"
  - "在模式上下文，`|` 始终是 or-pattern"
- `src/parser/expr.rs` (parse_primary_expr — `TokenKind::Or | OrOr` arm + `KwMove` arm)
- Per §1 lexical: `move` is v0.2 keyword (line 30 + line 38: "v1.2.2 修正：move 已在严格保留列表，v0.2 启用 move closure")

## 3. 测试设计 (40 个 .lin tests)

### 3.1 Basic closures (10 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_empty.lin | `\|\| 1` (no params, expr body) |
| closure_empty_block.lin | `\|\| { 1 }` (no params, block body) |
| closure_single_param.lin | `\|x\| x` (single param, expr body) |
| closure_single_param_block.lin | `\|x\| { x }` (single param, block body) |
| closure_multi_params.lin | `\|x, y\| x + y` (multi params) |
| closure_typed_param.lin | `\|x: i32\| x` (typed param) |
| closure_typed_multi.lin | `\|x: i32, y: i32\| x + y` (typed multi params) |
| closure_in_let.lin | `let f = \|\| 1;` (closure in let) |
| closure_call.lin | `(\|x\| x)(42)` (immediate call) |
| closure_nested.lin | `\|\| \|\| 1` (nested closures) |

### 3.2 Move closures (8 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_move_empty.lin | `move \|\| 1` (move, no params) |
| closure_move_param.lin | `move \|x\| x` (move, single param) |
| closure_move_block.lin | `move \|\| { 1 }` (move, block body) |
| closure_move_multi.lin | `move \|x, y\| x + y` (move, multi params) |
| closure_move_typed.lin | `move \|x: i32\| x` (move, typed param) |
| closure_move_in_let.lin | `let f = move \|\| 1;` (move closure in let) |
| closure_move_capture.lin | `let x = 1; let f = move \|\| x;` (move capture) |
| closure_move_nested.lin | `move \|\| move \|\| 1` (nested move closures) |

### 3.3 Closure with captures (7 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_capture_ref.lin | `let x = 1; let f = \|\| x;` (capture by ref) |
| closure_capture_mut.lin | `let mut x = 1; let f = \|\| { x += 1; x };` (capture mut) |
| closure_capture_multi.lin | `let a = 1; let b = 2; let f = \|\| a + b;` (multi captures) |
| closure_capture_move.lin | `let x = 1; let f = move \|\| x;` (move capture) |
| closure_capture_in_fn.lin | `fn f() { let x = 1; let g = \|\| x; }` (capture in fn) |
| closure_capture_nested.lin | `let a = 1; let f = \|\| { let b = 2; \|\| a + b };` (nested capture) |
| closure_capture_string.lin | `let s = String::new(); let f = move \|\| s;` (move capture string) |

### 3.4 Closure as argument (5 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_arg_basic.lin | `fn f(g: \|\| -> i32) {}` (closure type — may not parse) |
| closure_arg_call.lin | `fn g(f: \|\|(i32) -> i32, x: i32) { f(x) }` (closure param) |
| closure_arg_pass.lin | `fn f(x: i32, g: \|\| -> i32) {}` (passing closure) |
| closure_arg_inline.lin | `map(\|x\| x + 1)` (inline closure arg) |
| closure_arg_move.lin | `map(move \|x\| x)` (move closure arg) |

### 3.5 Closure return types (5 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_ret_unit.lin | `\|\| { } ` (return unit) |
| closure_ret_int.lin | `\|\| -> i32 { 1 }` (explicit return type — may not parse) |
| closure_ret_ref.lin | `\|\| -> &i32 { &1 }` (return ref — may not parse) |
| closure_ret_closure.lin | `\|\| \|\| 1` (return closure) |
| closure_ret_block.lin | `\|\| { let x = 1; x }` (return from block) |

### 3.6 Closure disambiguation (3 tests)

| 测试文件 | 描述 |
|---------|------|
| closure_vs_bitor.lin | `\|x\| x \| y` (closure body is `x \| y` bitwise) |
| closure_in_match.lin | `match x { _ => \|y\| y }` (closure in match arm) |
| closure_chain.lin | `\|x\| \|y\| x + y` (curried closure) |

### 3.7 边界 & 错误恢复 (2 tests)

| 测试文件 | 描述 |
|---------|------|
| err_closure_unclosed.lin | `FAIL: \|x 1` (unclosed closure param) |
| err_closure_no_body.lin | `FAIL: \|x\| ;` (closure with no body) |

**累计**: 10 + 8 + 7 + 5 + 5 + 3 + 2 = **40 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2186+ tests pass (期望 +12 verification tests = 2198)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 437 passed (397 + 40 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.6 → 0.16.7
- api-naming-standard.md: v2.10 → v2.11

---

**创建日期**: 2026-07-26
