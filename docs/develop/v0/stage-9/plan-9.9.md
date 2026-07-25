# Stage 9.9 开发计划: Modules conformance 扩展

> **阶段**: Stage 9.9 (Stage 9 第 9 个子阶段)
> **版本**: v0.16.7 → v0.16.8
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.8 完成 conformance 397 → 437 (closures category, approaching 3/4!).
Stage 9.9 继续扩展 **modules** 类别 (per `17-conformance-suite.md` §2 +
`02-grammar.md` §3.1 + §3.7).

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.1 (mod + vis):
  - `"mod" ident "{" item* "}"` (inline module)
  - `"mod" ident ";"` (external module — file-based)
  - `vis := "pub" ("(" ("crate" | "super" | "self" | "in" path) ")")?`
- `docs/lang-design/02-grammar.md` §3.7 (use declarations):
  - `use_decl := "use" use_tree ";"`
  - `use_tree := path (":" ":")? "{" use_tree_list "}" | path "as" ident | path "*"`
  - `use_tree_list := use_tree ("," use_tree)*`
- `src/parser/items.rs` (parse_use + parse_use_tree + parse_visibility + parse_mod)

## 3. 测试设计 (60 个 .lin tests)

### 3.1 Module declarations (12 tests)

| 测试文件 | 描述 |
|---------|------|
| mod_inline_empty.lin | `mod m {}` (empty inline module) |
| mod_inline_fn.lin | `mod m { fn f() {} }` (module with fn) |
| mod_inline_struct.lin | `mod m { struct S; }` (module with struct) |
| mod_inline_multi.lin | `mod m { fn f() {} struct S; const C: i32 = 0; }` (multi items) |
| mod_inline_nested.lin | `mod a { mod b { fn f() {} } }` (nested modules) |
| mod_inline_3_levels.lin | `mod a { mod b { mod c {} } }` (3-level nesting) |
| mod_inline_with_vis.lin | `mod m { pub fn f() {} }` (module with pub items) |
| mod_inline_use.lin | `mod m { use foo::bar; }` (module with use) |
| mod_external.lin | `mod m;` (external module — file-based) |
| mod_external_pub.lin | `pub mod m;` (pub external module) |
| mod_in_fn.lin | (may not be supported — module in fn) |
| mod_multi.lin | `mod a {} mod b {} mod c {}` (multiple modules) |

### 3.2 Use declarations — basic (12 tests)

| 测试文件 | 描述 |
|---------|------|
| use_simple.lin | `use foo::bar;` (simple path) |
| use_multi_segment.lin | `use foo::bar::baz;` (multi-segment path) |
| use_self.lin | `use self::foo;` (self path) |
| use_super.lin | `use super::foo;` (super path) |
| use_crate.lin | `use crate::foo;` (crate path) |
| use_as.lin | `use foo::bar as baz;` (renamed import) |
| use_as_self.lin | `use foo as self;` (may not be allowed) |
| use_glob.lin | `use foo::*;` (glob import) |
| use_nested.lin | `use foo::{bar, baz};` (nested import) |
| use_nested_multi.lin | `use foo::{bar, baz, qux};` (multi nested) |
| use_nested_glob.lin | `use foo::{bar, *};` (nested with glob) |
| use_nested_as.lin | `use foo::{bar as b, baz};` (nested with rename) |

### 3.3 Use declarations — advanced (8 tests)

| 测试文件 | 描述 |
|---------|------|
| use_nested_deep.lin | `use foo::{bar::{baz, qux}};` (deeply nested) |
| use_nested_3_levels.lin | `use foo::{bar::{baz::{qux}}};` (3-level nested) |
| use_nested_self.lin | `use foo::{self};` (self in nested) |
| use_nested_super.lin | `use foo::{super::bar};` (super in nested) |
| use_path_with_generics.lin | (may not be supported — generics in use path) |
| use_in_module.lin | `mod m { use foo::bar; }` (use inside module) |
| use_multi.lin | `use foo::bar; use baz::qux;` (multiple use declarations) |
| use_visibility.lin | `pub use foo::bar;` (pub use — re-export) |

### 3.4 Visibility — pub (10 tests)

| 测试文件 | 描述 |
|---------|------|
| vis_pub_fn.lin | `pub fn f() {}` (pub fn) |
| vis_pub_struct.lin | `pub struct S;` (pub struct) |
| vis_pub_enum.lin | `pub enum E { A, B }` (pub enum) |
| vis_pub_trait.lin | `pub trait T {}` (pub trait) |
| vis_pub_const.lin | `pub const C: i32 = 0;` (pub const) |
| vis_pub_static.lin | `pub static S: i32 = 0;` (pub static) |
| vis_pub_mod.lin | `pub mod m {}` (pub module) |
| vis_pub_use.lin | `pub use foo::bar;` (pub use) |
| vis_pub_type.lin | `pub type T = i32;` (pub type alias) |
| vis_pub_field.lin | `struct S { pub x: i32 }` (pub field) |

### 3.5 Visibility — restricted (8 tests)

| 测试文件 | 描述 |
|---------|------|
| vis_pub_crate.lin | `pub(crate) fn f() {}` (pub(crate)) |
| vis_pub_super.lin | `pub(super) fn f() {}` (pub(super)) |
| vis_pub_self.lin | `pub(self) fn f() {}` (pub(self)) |
| vis_pub_in_path.lin | `pub(in path) fn f() {}` (pub(in path)) |
| vis_pub_crate_struct.lin | `pub(crate) struct S;` (pub(crate) struct) |
| vis_pub_crate_field.lin | `struct S { pub(crate) x: i32 }` (pub(crate) field) |
| vis_pub_crate_mod.lin | `pub(crate) mod m {}` (pub(crate) mod) |
| vis_pub_crate_use.lin | `pub(crate) use foo::bar;` (pub(crate) use) |

### 3.6 边界 & 错误恢复 (10 tests)

| 测试文件 | 描述 |
|---------|------|
| err_mod_unclosed.lin | `FAIL or PASS: mod m { fn f() {}` (unclosed module) |
| err_use_no_semi.lin | `FAIL or PASS: use foo::bar` (missing semicolon) |
| err_use_no_path.lin | `FAIL or PASS: use ;` (missing path) |
| err_use_invalid_glob.lin | `FAIL or PASS: use foo::**;` (invalid glob) |
| err_vis_no_item.lin | `FAIL or PASS: pub;` (visibility without item) |
| err_vis_invalid.lin | `FAIL or PASS: pub(bad) fn f() {}` (invalid vis) |
| err_use_unclosed_nested.lin | `FAIL or PASS: use foo::{bar;` (unclosed nested) |
| err_mod_no_name.lin | `FAIL or PASS: mod {}` (missing module name) |
| err_use_no_tree.lin | `FAIL or PASS: use;` (no use tree) |
| err_use_double_colon.lin | `FAIL or PASS: use foo:::bar;` (double colon) |

**累计**: 12 + 12 + 8 + 10 + 8 + 10 = **60 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2197+ tests pass (期望 +12 verification tests = 2209)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 497 passed (437 + 60 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.7 → 0.16.8
- api-naming-standard.md: v2.11 → v2.12

---

**创建日期**: 2026-07-26
