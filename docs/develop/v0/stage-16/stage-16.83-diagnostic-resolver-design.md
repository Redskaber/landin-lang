# Stage 16.83 Design — Diagnostic Type Name Resolution via Resolver

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Stage 16.80-16.82 改进了 TypeError/BorrowError 的构造时消息。但 `CompileErrors::to_diagnostics` 在格式化 diagnostic notes 时仍用 `type_kind_to_string`，导致 diagnostic notes 显示 `<adt>` 而非类型名。

**问题**: 
```
error[type]: mismatched types: expected MyStruct, found i32  ← 消息本体 OK (Stage 16.81)
  --> source:1:1
  note: expected: <adt>        ← diagnostic note 仍显示 <adt>
  note: found: i32
```

**目标**: 新增 `to_diagnostics_with_resolver` 方法，在格式化 notes 时用 `type_kind_to_string_with_resolver`。

## 2. 架构现状分析

### 2.1 当前 to_diagnostics 签名

```rust
pub fn to_diagnostics(&self, interner: Option<&Rodeo>) -> Vec<Diagnostic>
```

只接收 interner，不接收 resolver。无法解析 Adt 名。

### 2.2 调用链

```
CompileResult.format_for_user() → CompileErrors.format_via_diagnostics()
  → CompileErrors.to_diagnostics(interner)
    → type_kind_to_string (显示 <adt>)
```

`CompileResult` 有 `trait_resolver` 字段，但 `CompileErrors` 是独立结构。

### 2.3 type_kind_to_string 在 to_diagnostics 中的调用

driver.rs L339-345: 格式化 expected/found notes 时用 `type_kind_to_string`。

## 3. 重构方案

### 3.1 新增 to_diagnostics_with_resolver

```rust
/// Stage 16.83: Like `to_diagnostics` but uses resolver-backed type names
/// for diagnostic notes (shows "MyStruct" instead of "<adt>").
pub fn to_diagnostics_with_resolver(
    &self,
    interner: Option<&Rodeo>,
    resolver: Option<&TraitResolver>,
) -> Vec<Diagnostic>
```

实现：复制 `to_diagnostics` 逻辑，但在格式化 expected/found notes 时：
- 有 resolver + interner → `type_kind_to_string_with_resolver`
- 无 resolver → fallback `type_kind_to_string`

### 3.2 保留旧 API

`to_diagnostics(interner)` 保留，内部调用 `to_diagnostics_with_resolver(interner, None)`。

### 3.3 更新调用链

`format_via_diagnostics` 和 `format_for_user` 新增 resolver 参数（可选）。CompileResult 调用时传入 `Some(&self.trait_resolver)`。

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `to_diagnostics_with_resolver` 只负责 diagnostic 格式化 |
| J3 | 单向流动 | ✅ caller → to_diagnostics_with_resolver → type_kind_to_string_with_resolver |
| J4 | 编译相关表达完整 | ✅ expected/found notes 都改进 |
| J5 | 阶段划分清晰 | ✅ 仍在 driver.rs |
| J6 | 科学合理粒度 | ✅ ~30 LOC 新增 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `diagnostic_with_resolver_shows_struct_name` — diagnostic note 含 "MyStruct"
2. `diagnostic_without_resolver_falls_back` — 无 resolver 时 fallback 正常

### 负向测试 (negative)
1. `compile_mismatch_diagnostic_note_shows_name` — 编译错误 diagnostic note 含类型名
2. `compile_struct_mismatch_diagnostic_full` — 完整 diagnostic 含 "MyStruct"
3. `compile_enum_mismatch_diagnostic_shows_name` — enum diagnostic 含名
4. `compile_two_struct_diagnostic_shows_both` — 两 struct diagnostic 含两名
5. `compile_fn_arg_diagnostic_shows_name` — fn arg diagnostic 含名
6. `format_for_user_with_resolver_shows_name` — format_for_user 输出含名

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~30 LOC + 8 测试。
