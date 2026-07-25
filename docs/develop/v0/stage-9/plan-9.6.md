# Stage 9.6 开发计划: Attributes conformance 扩展

> **阶段**: Stage 9.6 (Stage 9 第 6 个子阶段)
> **版本**: v0.16.4 → v0.16.5
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.5 完成 conformance 247 → 307 (types category, past halfway!). Stage 9.6
继续扩展 **attributes** 类别 (per `17-conformance-suite.md` §2 +
`02-grammar.md` §3.1 + §4.3).

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.1 (attr := "#" "[" meta "]")
- `docs/lang-design/02-grammar.md` §3.1 (meta := ident ("=" expr | "(" meta_args? ")")?)
- `docs/lang-design/02-grammar.md` §4.3 (outer `#[...]` vs inner `#![...]`)
- `docs/lang-design/15-attributes.md` (full attribute spec)
- `src/parser/items.rs` (parse_outer_attrs + parse_attr_args)
- Parser note: "Inner attributes `#![...]` are handled at crate level (Stage 1);
  for Stage 0 we only parse outer attributes here."

## 3. 测试设计 (40 个 .lin tests)

### 3.1 Outer attributes on items (12 tests)

| 测试文件 | 描述 |
|---------|------|
| attr_outer_fn.lin | `#[foo] fn f() {}` |
| attr_outer_struct.lin | `#[foo] struct S;` |
| attr_outer_enum.lin | `#[foo] enum E { A, B }` |
| attr_outer_trait.lin | `#[foo] trait T {}` |
| attr_outer_impl.lin | `#[foo] impl T for S {}` |
| attr_outer_const.lin | `#[foo] const C: i32 = 0;` |
| attr_outer_static.lin | `#[foo] static S: i32 = 0;` |
| attr_outer_mod.lin | `#[foo] mod m {}` |
| attr_outer_use.lin | `#[foo] use foo::bar;` |
| attr_outer_type.lin | `#[foo] type T = i32;` |
| attr_outer_multi.lin | `#[a] #[b] #[c] fn f() {}` (multiple attrs) |
| attr_outer_external.lin | `#[foo] extern "C" {}` |

### 3.2 Derive attribute (8 tests)

| 测试文件 | 描述 |
|---------|------|
| attr_derive_single.lin | `#[derive(Clone)] struct S;` |
| attr_derive_multi.lin | `#[derive(Clone, Copy)] struct S;` |
| attr_derive_debug.lin | `#[derive(Debug)] struct S;` |
| attr_derive_default.lin | `#[derive(Default)] struct S;` |
| attr_derive_partial_eq.lin | `#[derive(PartialEq)] struct S;` |
| attr_derive_3.lin | `#[derive(Clone, Copy, Debug)] struct S;` |
| attr_derive_4.lin | `#[derive(Clone, Copy, Debug, Default)] struct S;` |
| attr_derive_enum.lin | `#[derive(Clone)] enum E { A, B }` |

### 3.3 Attribute arguments (10 tests)

| 测试文件 | 描述 |
|---------|------|
| attr_arg_empty.lin | `#[foo] fn f() {}` (no args) |
| attr_arg_eq_literal.lin | `#[foo = "bar"] fn f() {}` (eq + literal) |
| attr_arg_eq_int.lin | `#[foo = 42] fn f() {}` (eq + int) |
| attr_arg_list_empty.lin | `#[foo()] fn f() {}` (empty list) |
| attr_arg_list_single.lin | `#[foo(bar)] fn f() {}` (single arg) |
| attr_arg_list_multi.lin | `#[foo(a, b, c)] fn f() {}` (multiple args) |
| attr_arg_list_named.lin | `#[foo(key = "value")] fn f() {}` (named arg) |
| attr_arg_list_mixed.lin | `#[foo(a, b = 1, c)] fn f() {}` (mixed) |
| attr_arg_path.lin | `#[foo::bar] fn f() {}` (path attribute) |
| attr_arg_path_with_args.lin | `#[foo::bar(a, b)] fn f() {}` (path + args) |

### 3.4 Attribute on various positions (5 tests)

| 测试文件 | 描述 |
|---------|------|
| attr_on_enum_variant.lin | `enum E { #[foo] A, B }` (attr on variant) |
| attr_on_struct_field.lin | `struct S { #[foo] x: i32 }` (attr on field) |
| attr_on_fn_param.lin | `fn f(#[foo] x: i32) {}` (attr on fn param) |
| attr_on_let.lin | (may not be supported — let stmt attrs) |
| attr_on_block.lin | (may not be supported — block attrs) |

### 3.5 Inner attributes (3 tests)

| 测试文件 | 描述 |
|---------|------|
| attr_inner_no_std.lin | `#![no_std]` (inner attr — may be Stage 1) |
| attr_inner_module.lin | `#![foo] mod m {}` (inner attr on module) |
| attr_inner_mixed.lin | `#![a] #[b] fn f() {}` (inner + outer) |

### 3.6 边界 & 错误恢复 (2 tests)

| 测试文件 | 描述 |
|---------|------|
| err_attr_unclosed.lin | `FAIL: #[foo fn f() {}` (unclosed attr) |
| err_attr_missing_path.lin | `FAIL: #[] fn f() {}` (missing path) |

**累计**: 12 + 8 + 10 + 5 + 3 + 2 = **40 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2166+ tests pass (期望 +12 verification tests = 2178)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 347 passed (307 + 40 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.4 → 0.16.5
- api-naming-standard.md: v2.08 → v2.09

---

**创建日期**: 2026-07-26
