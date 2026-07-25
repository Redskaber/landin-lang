# Stage 9.4 开发计划: Patterns conformance 扩展

> **阶段**: Stage 9.4 (Stage 9 第 4 个子阶段)
> **版本**: v0.16.2 → v0.16.3
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.3 完成 conformance 98 → 177 (control flow category)。Stage 9.4 继续扩展
**patterns** 类别 (per `17-conformance-suite.md` §2 + `02-grammar.md` §3.5)。

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.5 (Pattern):
  - `pat := "_" | literal_pat | (path "::")? ident (...) | "&" pat | "&mut" pat | "(" pat_list? ")" | "[" pat ("," pat)* ("," ".." ("," pat)*)? "]" | pat "|" pat | ident "@" pat | ".." | range_pat`
  - `literal_pat := integer_lit | float_lit | char_lit | string_lit | bool_lit | "-"? integer_lit`
  - `range_pat := pat "..=" pat | pat ".." pat`
  - `pat_list := pat ("," pat)* ","?`
  - `pat_fields := pat_field ("," pat_field)* ","?`
  - `pat_field := ident ":" pat | ident | ".."`
- `src/parser/pat.rs` (parse_pat + parse_or_pat + parse_pat_no_or)

## 3. 测试设计 (70 个 .lin tests)

### 3.1 Wildcard patterns (5 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_wild_basic.lin | `let _ = 1;` |
| pat_wild_in_match.lin | `match x { _ => 0 }` |
| pat_wild_in_fn_param.lin | `fn f(_: i32) {}` |
| pat_wild_underscore_prefix.lin | `let _x = 1;` (unused warning, but parses) |
| pat_wild_in_closure.lin | `\|_\| 1` |

### 3.2 Identifier patterns (8 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_ident_basic.lin | `let x = 1;` |
| pat_ident_in_match.lin | `match x { y => y }` |
| pat_ident_in_fn_param.lin | `fn f(x: i32) {}` |
| pat_mut_ident.lin | `let mut x = 1;` |
| pat_ref_ident.lin | `let ref x = 1;` |
| pat_ref_mut_ident.lin | `let ref mut x = 1;` |
| pat_ident_in_let.lin | `let (a, b) = (1, 2);` (ident in tuple) |
| pat_ident_at_binding.lin | `let x @ _ = 1;` |

### 3.3 Literal patterns (10 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_lit_int.lin | `match x { 42 => 1, _ => 0 }` |
| pat_lit_int_neg.lin | `match x { -1 => 1, _ => 0 }` |
| pat_lit_float.lin | `match x { 3.14 => 1, _ => 0 }` |
| pat_lit_bool.lin | `match x { true => 1, false => 0 }` |
| pat_lit_char.lin | `match x { 'a' => 1, _ => 0 }` |
| pat_lit_string.lin | `match x { "hello" => 1, _ => 0 }` |
| pat_lit_hex.lin | `match x { 0xff => 1, _ => 0 }` |
| pat_lit_oct.lin | `match x { 0o777 => 1, _ => 0 }` |
| pat_lit_bin.lin | `match x { 0b1010 => 1, _ => 0 }` |
| pat_lit_multi.lin | `match x { 1 => 1, 2 => 2, 3 => 3, _ => 0 }` |

### 3.4 Struct patterns (10 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_struct_basic.lin | `let P { x, y } = p;` |
| pat_struct_with_type.lin | `let P { x: a, y: b } = p;` (renamed bindings) |
| pat_struct_partial.lin | `let P { x, .. } = p;` (partial with ..) |
| pat_struct_empty.lin | `let P { .. } = p;` (only ..) |
| pat_struct_nested.lin | `let Outer { inner: Inner { x } } = o;` |
| pat_struct_in_match.lin | `match p { P { x, y } => x + y }` |
| pat_struct_with_lit.lin | (struct with literal — not standard, omit) |
| pat_struct_ref.lin | `let &P { x, y } = &p;` |
| pat_struct_ref_mut.lin | `let &mut P { x, y } = &mut p;` |
| pat_struct_full.lin | `let P { x: a, y: b, .. } = p;` (mix renamed + ..) |

### 3.5 Tuple patterns (8 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_tuple_basic.lin | `let (a, b) = (1, 2);` |
| pat_tuple_3.lin | `let (a, b, c) = (1, 2, 3);` |
| pat_tuple_nested.lin | `let ((a, b), c) = ((1, 2), 3);` |
| pat_tuple_with_wild.lin | `let (a, _) = (1, 2);` |
| pat_tuple_in_match.lin | `match (a, b) { (1, 2) => 1, _ => 0 }` |
| pat_tuple_empty.lin | `let () = ();` |
| pat_tuple_single.lin | `let (x,) = (1,);` (trailing comma) |
| pat_tuple_ref.lin | `let &(a, b) = &(1, 2);` |

### 3.6 Or patterns (8 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_or_2.lin | `match x { 1 \| 2 => 1, _ => 0 }` |
| pat_or_3.lin | `match x { 1 \| 2 \| 3 => 1, _ => 0 }` |
| pat_or_4.lin | `match x { 1 \| 2 \| 3 \| 4 => 1, _ => 0 }` |
| pat_or_idents.lin | `match x { a \| b => 1, _ => 0 }` |
| pat_or_mixed.lin | `match x { 1 \| "a" \| 'b' => 1, _ => 0 }` |
| pat_or_in_let.lin | `let 1 \| 2 = x;` (in let — may not be allowed) |
| pat_or_paths.lin | `match x { E::A \| E::B => 1, _ => 0 }` |
| pat_or_tuples.lin | `match x { (1, 2) \| (3, 4) => 1, _ => 0 }` |

### 3.7 Range patterns (8 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_range_inclusive.lin | `match x { 1..=10 => 1, _ => 0 }` |
| pat_range_exclusive.lin | `match x { 1..10 => 1, _ => 0 }` |
| pat_range_char.lin | `match x { 'a'..='z' => 1, _ => 0 }` |
| pat_range_neg.lin | `match x { -10..=-1 => 1, _ => 0 }` |
| pat_range_multi.lin | `match x { 1..=10 \| 20..=30 => 1, _ => 0 }` |
| pat_range_in_let.lin | (let with range — likely FAIL) |
| pat_range_open_ended.lin | `match x { 1.. => 1, _ => 0 }` (open-ended) |
| pat_range_nested.lin | `match x { 1..=5 \| 6..=10 => 1, _ => 0 }` |

### 3.8 Array patterns (5 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_array_basic.lin | `let [a, b, c] = arr;` |
| pat_array_with_wild.lin | `let [a, _, c] = arr;` |
| pat_array_rest.lin | `let [a, ..] = arr;` (with rest) |
| pat_array_rest_middle.lin | `let [a, .., z] = arr;` (rest in middle) |
| pat_array_empty.lin | `let [] = [];` |

### 3.9 Reference patterns (5 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_ref_basic.lin | `let &x = &1;` |
| pat_ref_mut_basic.lin | `let &mut x = &mut 1;` |
| pat_ref_nested.lin | `let &&x = &&1;` |
| pat_ref_tuple.lin | `let &(a, b) = &(1, 2);` |
| pat_ref_struct.lin | `let &P { x, y } = &p;` |

### 3.10 At-binding patterns (3 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_at_basic.lin | `let x @ _ = 1;` |
| pat_at_range.lin | `match x { n @ 1..=10 => n, _ => 0 }` |
| pat_at_or.lin | `match x { n @ (1 \| 2 \| 3) => n, _ => 0 }` |

### 3.11 Path patterns (5 tests)

| 测试文件 | 描述 |
|---------|------|
| pat_path_enum.lin | `match e { E::A => 1, _ => 0 }` |
| pat_path_enum_with_data.lin | `match e { E::A(_) => 1, _ => 0 }` |
| pat_path_enum_struct.lin | `match e { E::A { x } => 1, _ => 0 }` |
| pat_path_full.lin | `match e { module::E::A => 1, _ => 0 }` |
| pat_path_tuple.lin | `match e { E::A(a, b) => 1, _ => 0 }` |

### 3.12 边界 & 错误恢复 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| err_pat_missing_body.lin | `FAIL: let = 1;` (missing pattern) |
| err_pat_or_empty.lin | `FAIL: match x { | => 1 }` (empty or-pattern) |
| err_pat_at_no_pat.lin | `FAIL: let x @ = 1;` (missing pattern after @) |
| err_pat_unclosed_paren.lin | `FAIL: let (a, b = (1, 2);` (unclosed paren) |
| err_pat_unclosed_bracket.lin | `FAIL: let [a, b = arr;` (unclosed bracket) |

**累计**: 5 + 8 + 10 + 10 + 8 + 8 + 8 + 5 + 5 + 3 + 5 + 5 = **80 tests** (扩展到 80 以补足 plan 目标 247 — 实际 70+10 extras)

实际上，我会简化为 70 个测试以满足 plan 目标。让我重新规划:

### 简化版 (70 tests total)

| 类别 | 测试数 |
|------|-------|
| Wildcard | 5 |
| Identifier | 6 |
| Literal | 10 |
| Struct | 8 |
| Tuple | 8 |
| Or | 7 |
| Range | 7 |
| Array | 5 |
| Reference | 5 |
| At-binding | 3 |
| Path | 3 |
| Error recovery | 3 |
| **Total** | **70** |

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2136+ tests pass (期望 +12 verification tests = 2148)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 247 passed (177 + 70 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.2 → 0.16.3
- api-naming-standard.md: v2.06 → v2.07

---

**创建日期**: 2026-07-26
