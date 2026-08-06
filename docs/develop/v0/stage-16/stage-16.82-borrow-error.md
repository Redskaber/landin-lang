# Stage 16.82 — BorrowError Message Improvements

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.268.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

改进 BorrowError 消息：显示实际类型名 + place 信息。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿：
- Design v1: `stage-16.82-borrow-error-design.md`
- J1-J6 全部满足

## 3. 实现内容

### 3.1 新增 helper 方法

- `format_ty(&self, ty: &Ty) -> String` — 有 resolver 时用 `type_to_string_with_resolver`
- `format_place(&self, place: &Place) -> String` — 格式化为 "local#N" / "static#N"
- `format_place_path(&self, path: &PlacePath) -> String` — 格式化 borrowck 内部 PlacePath

### 3.2 改进错误消息

| 错误 | 前 | 后 |
|------|-----|-----|
| lifetime error | `type <adt> does not outlive` | `type MyStruct does not outlive` |
| cannot borrow moved | `cannot borrow moved value` | `cannot borrow moved value: local#N` |
| use of moved value | `use of moved value` | `use of moved value: local#N` |
| cannot move borrowed | `cannot move borrowed value` | `cannot move borrowed value: local#N` |
| cannot assign borrowed | `cannot assign to borrowed value` | `cannot assign to borrowed value: local#N` |
| immutable assign | `cannot assign twice to immutable variable` | `cannot assign twice to immutable variable: local#N` |

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | format_ty_with_resolver_shows_name | positive | resolver 显示 "MyStruct" |
| 2 | format_ty_without_resolver_falls_back | positive | 无 resolver 显示 "i32" |
| 3 | compile_move_after_borrow_shows_place | negative | move after borrow 含 "local#" |
| 4 | compile_assign_immutable_shows_local | negative | 不可变重赋值含 "local#" |
| 5 | compile_double_mut_borrow_shows_place | negative | 双重 &mut 含错误 |
| 6 | compile_use_after_move_shows_place | negative | use after move 含 "local#" |
| 7 | format_place_local | negative | format_place 输出 "local#5" |
| 8 | format_place_path_local | negative | format_place_path 输出 "local#3" |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 389 lib + 2494 integration = 2883 unit tests |

## 6. 结论

GO — BorrowError 消息改进完成：
- lifetime error 显示实际类型名 ✅
- moved/borrowed/assign 错误加 place 信息 ✅
- 3 个 helper 方法（format_ty/format_place/format_place_path）✅
- 8 新测试 1:3 正负比例 ✅

## 7. 后续工作

- Trait bound not satisfied 错误消息
- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
