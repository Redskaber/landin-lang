# Stage 5.40 开发计划：stdlib vtable symbol name planner

> **阶段**: Stage 5.40
> **版本**: v0.11.35 → v0.11.36
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.39（vtable 构造计划）基础上，添加**vtable 符号名规划器**：
把 codegen 当前内联的 `format!(".vtable.{}.{}", trait_str, type_str)` /
`format!("landin_{}_{}", self_ty_str, method_str)` /
`format!(".dynptr.{}.{}", trait_str, type_str)` 等字符串格式化逻辑提取
到 stdlib 的纯函数中，让 codegen 直接消费预格式化的字符串。

这是 codegen 修改前的**最后一次字符串提取**——Stage 5.41+ 修改 codegen
时，只需调用这些 planner 函数，不再有散落的 `format!` 调用。

## 2. 现有 codegen 命名约定（必须严格匹配）

从 `src/codegen/mod.rs` + `src/traits/resolver.rs` 提取的现有约定：

| 用途 | 现有 format! | 例子 |
|------|-------------|------|
| impl 方法符号 | `format!("landin_{}_{}", self_ty_str, method_str)` | `landin_S_bar` |
| vtable 全局名 | `format!(".vtable.{}.{}", trait_str, type_str)` | `.vtable.Foo.S` |
| dynptr 全局名 | `format!(".dynptr.{}.{}", trait_str, type_str)` | `.dynptr.Foo.S` |
| data 全局名 | `format!(".data.{}", type_str)` | `.data.S` |

## 3. 设计

### 3.1 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_vtable_global_name` | `(trait_name, type_name) -> String` | `.vtable.<trait>.<type>` |
| `stdlib_dynptr_global_name` | `(trait_name, type_name) -> String` | `.dynptr.<trait>.<type>` |
| `stdlib_data_global_name` | `(type_name) -> String` | `.data.<type>` |
| `stdlib_impl_method_symbol` | `(type_name, method_name) -> String` | `landin_<type>_<method>` |
| `stdlib_vtable_method_symbols` | `(trait_name, type_name, provided_methods) -> Option<Vec<String>>` | 完整方法符号列表（按 slot 顺序） |

### 3.2 计算规则

- `vtable_global_name` = `format!(".vtable.{}.{}", trait, type)`
- `dynptr_global_name` = `format!(".dynptr.{}.{}", trait, type)`
- `data_global_name` = `format!(".data.{}", type)`
- `impl_method_symbol` = `format!("landin_{}_{}", type, method)`
- `vtable_method_symbols` = 遍历 `stdlib_vtable_plan(trait, provided)` 的 entries:
  - `provided=true` → `stdlib_impl_method_symbol(type, method_name)`
  - `provided=false` → `"null"` 字符串（codegen 会直接 emit）
  - 未注册 trait → `None`

### 3.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_vtable_global_name` | `<noun>_<noun>_<adj>_<noun>` | ✅ |
| `stdlib_dynptr_global_name` | `<noun>_<noun>_<adj>_<noun>` | ✅ |
| `stdlib_data_global_name` | `<noun>_<noun>_<adj>_<noun>` | ✅ |
| `stdlib_impl_method_symbol` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_method_symbols` | `<noun>_<noun>_<noun>_<noun>` | ✅ |

### 3.4 §16 接口隔离

所有新 API 输入 `&str`，输出 `String` / `Vec<String>`，不引用
`codegen::EmitType` / `mir::ty` / `traits::TraitResolver`，无循环依赖。
纯函数，可在任意阶段调用。

### 3.5 与现有 codegen 的一致性

这些函数**严格复现** codegen 当前的 `format!` 字符串。Stage 5.41+ 修改
codegen 时，将 codegen 内的 `format!` 替换为对这些函数的调用——行为完全
等价，但字符串格式化逻辑集中到 stdlib，便于未来调整命名约定（例如加入
module path 前缀）。

## 4. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1190 + 新增 ~14 = ~1204）
4. §1.2 交付前验收：全绿
5. 生成的字符串与 codegen 现有 `format!` 输出**逐字节一致**（测试覆盖）

## 5. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_global_name` | `.vtable.Foo.S` |
| `test_stdlib_vtable_global_name_special` | trait/type 含点号 / 空 |
| `test_stdlib_dynptr_global_name` | `.dynptr.Foo.S` |
| `test_stdlib_data_global_name` | `.data.S` |
| `test_stdlib_impl_method_symbol` | `landin_S_bar` |
| `test_stdlib_impl_method_symbol_multi_part` | `landin_Vec_push` |
| `test_stdlib_vtable_method_symbols_clone_complete` | Clone + S + [clone, clone_from] → 2 symbols |
| `test_stdlib_vtable_method_symbols_clone_partial` | Clone + S + [clone] → [landin_S_clone, null] |
| `test_stdlib_vtable_method_symbols_drop` | Drop + S + [drop] → 1 symbol |
| `test_stdlib_vtable_method_symbols_marker` | Copy + S + [] → 空 Vec |
| `test_stdlib_vtable_method_symbols_unknown_trait` | BogusTrait → None |
| `test_stdlib_vtable_method_symbols_ordered` | 顺序 = slot_index 升序 |
| `test_stdlib_vtable_method_symbols_match_codegen_format` | 字符串与 codegen format! 一致 |
| `test_stdlib_vtable_global_name_match_codegen` | vtable global name 与 codegen 一致 |

## 6. 后续依赖

- **Stage 5.41+ (codegen vtable emission refactor)**: 替换 codegen 内的
  `format!` 调用为 `stdlib_vtable_global_name()` /
  `stdlib_dynptr_global_name()` / `stdlib_impl_method_symbol()` /
  `stdlib_vtable_method_symbols()`，行为等价但字符串逻辑集中。
- **Stage 5.42+ (dyn Trait MIR lowering)**: MIR lowering 调用
  `stdlib_vtable_method_symbols()` 获取方法符号列表，构造 vtable 常量。

---

**创建日期**: 2026-07-23
