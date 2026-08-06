# Stage 16.84 — Migrate checker.rs Type Errors to Use Resolver

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.270.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

迁移 checker.rs 中 9 处 `type_kind_to_string` 为 `format_ty`，显示实际类型名。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿：
- Design v1: `stage-16.84-checker-resolver-design.md`
- J1-J6 全部满足

## 3. 实现内容

### 3.1 新增 UnificationTable getter

- `resolver()` → `Option<&TraitResolver>`
- `interner()` → `Option<&Rodeo>`

### 3.2 新增 TypeChecker::format_ty

从 unify 读取 resolver/interner，有则用 `type_to_string_with_resolver`。

### 3.3 替换 9 处 type_kind_to_string

checker.rs 中 9 处错误消息现在显示实际类型名。

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | format_ty_with_resolver_shows_name | positive | 显示 "MyStruct" |
| 2 | format_ty_without_resolver_falls_back | positive | fallback "i32" |
| 3 | compile_expected_function_found_struct_shows_name | negative | "found MyStruct" |
| 4 | compile_if_condition_must_be_bool_shows_name | negative | "found MyStruct" |
| 5 | compile_switch_discriminant_shows_name | negative | "found MyStruct" |
| 6 | compile_match_arm_mismatch_shows_name | negative | 含类型名 |
| 7 | compile_call_non_function_shows_name | negative | "found MyStruct" |
| 8 | compile_method_call_non_function_shows_name | negative | method error |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 405 lib + 2494 integration = 2899 unit tests |

## 6. 结论

GO — checker.rs 类型错误消息改进完成：
- 9 处 type_kind_to_string 替换为 format_ty ✅
- UnificationTable 新增 resolver()/interner() getter ✅
- 8 新测试 1:3 正负比例 ✅

## 7. 后续工作

- Migrate expr_operand.rs type_kind_to_string (MIR lower — 需 resolver threading)
- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
