# Stage 16.83 — Diagnostic Type Name Resolution via Resolver

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.269.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

改进 diagnostic notes 中的类型名显示：用 resolver 解析 Adt 名。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿：
- Design v1: `stage-16.83-diagnostic-resolver-design.md`
- J1-J6 全部满足

## 3. 实现内容

### 3.1 新增 to_diagnostics_with_resolver

```rust
pub fn to_diagnostics_with_resolver(
    &self,
    interner: Option<&Rodeo>,
    resolver: Option<&TraitResolver>,
) -> Vec<Diagnostic>
```

有 resolver 时用 `type_kind_to_string_with_resolver`，否则 fallback。

### 3.2 新增 format_via_diagnostics_with_resolver

### 3.3 保留旧 API

`to_diagnostics` 和 `format_via_diagnostics` 委托给 `_with_resolver(..., None)`。

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | diagnostic_with_resolver_shows_struct_name | positive | notes 含 "MyStruct" |
| 2 | diagnostic_without_resolver_falls_back | positive | 无 resolver fallback |
| 3 | compile_mismatch_diagnostic_note_shows_name | negative | notes 含类型名 |
| 4 | compile_struct_mismatch_diagnostic_full | negative | "Foo" 显示 |
| 5 | compile_enum_mismatch_diagnostic_shows_name | negative | "MyEnum" 显示 |
| 6 | compile_two_struct_diagnostic_shows_both | negative | "Foo"+"Bar" |
| 7 | compile_fn_arg_diagnostic_shows_name | negative | 消息含 "MyStruct" |
| 8 | format_for_user_with_resolver_shows_name | negative | 格式化输出含名 |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 397 lib + 2494 integration = 2891 unit tests |

## 6. 结论

GO — Diagnostic type name resolution 完成：
- to_diagnostics_with_resolver 新 API ✅
- format_via_diagnostics_with_resolver 新 API ✅
- diagnostic notes 显示实际类型名 ✅
- 8 新测试 1:3 正负比例 ✅

## 7. 后续工作

- Migrate remaining type_kind_to_string callers (checker.rs, expr_operand.rs)
- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
