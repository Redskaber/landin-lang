# Stage 18.165 — Option/Result 内置类型实现 (stdlib 第一步)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.433.0 (Stage 18.165 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.1 (设计对齐) + §13.4 (重构即架构设计)
> **Complexity**: L3 (新增 prelude 注入系统)
> **Task ID**: stage18.165

## 1. 阶段目标

按 Stage 18.163 任务审查, 实现 Option/Result 内置类型 (不依赖 heap, stdlib 第一步)。

## 2. 实现

### 2.1 新增: `src/stdlib/prelude.rs` (150 LOC)

Prelude 注入模块, 包含:
- `inject_prelude(krate, interner)` — 注入 Option/Result 到 AST
- `make_option_enum()` — 创建 `enum Option<T> { None, Some(T) }`
- `make_result_enum()` — 创建 `enum Result<T, E> { Ok(T), Err(E) }`

Per §13.4 J2 (单一职责): 该模块仅负责 prelude 注入。
Per §1.0 原則 6 (通解>特例): 一个注入机制处理所有内置类型。

### 2.2 修改: `src/driver/mod.rs`

在 `compile_inner` 的 parse 之后、HIR lower 之前调用 `inject_prelude`:
```rust
crate::stdlib::prelude::inject_prelude(&mut krate, &mut interner);
```

### 2.3 修改: `src/stdlib/mod.rs`

注册 `pub mod prelude`。

## 3. 验证结果

### 3.1 带前缀用法 (✅ 工作)

```landin
fn main() -> i32 {
    let x = Option::Some(42);
    match x { Option::Some(v) => v, Option::None => 0 }
}
```
✅ 编译成功 ("successfully compiled 3 items")

### 3.2 不带前缀用法 (❌ 不工作)

```landin
fn main() -> i32 {
    let x = Some(42);  // 期望: 自动解析为 Option::Some
    match x { Some(v) => v, None => 0 }
}
```
❌ 解析器将 `Some` 当作函数名, 未识别为 enum variant constructor

## 4. 简写和缺陷记录

### 4.1 简写1: 不带前缀的 variant constructor 不支持

**原因**: 解析器不支持 variant constructor (如 `Some(42)` 不带 `Option::` 前缀)。设计文档 (`09-stdlib.md` §2.4) 显示 `Some(T)` 应该可以不带前缀使用, 但当前解析器将 `Some` 当作普通函数名。

**影响**: 用户必须写 `Option::Some(42)` 而非 `Some(42)`, `Option::None` 而非 `None`。

**修订计划**:
- **TD-VARIANT-CONSTRUCTOR**: 解析器添加 variant constructor 支持
  - 在 name resolution 阶段, 当遇到 `Some(x)` 且 `Some` 是已知 enum variant 时, 自动解析为 `Option::Some(x)`
  - 需要修改 resolver 查找 enum variant 名称
  - 目标: v0.2 P1 (Stage 18.166+)

### 4.2 简写2: Option/Result 方法未实现

**原因**: 本 stage 仅实现类型定义 (enum + variants), 未实现方法 (is_some/unwrap/map 等)。

**影响**: 用户可以构造和模式匹配 Option/Result, 但不能调用方法。

**修订计划**:
- **TD-OPTION-ADVANCED-METHODS**: 实现 Option/Result 方法
  - 需要注入 impl 块到 prelude
  - 基本方法 (is_some/is_none/unwrap): Stage 18.166
  - 高级方法 (map/and_then): 依赖闭包 + trait bound, 后续 stage

### 4.3 简写3: Option/Result 用编译器内置 (AST 注入) 而非 Landin 源码

**原因**: Landin 自举未完成。

**修订计划**: 自举完成后替换为 Landin 源码 (v0.3+)。

## 5. 新技术债

| TD | 描述 | 优先级 | 目标 |
|----|------|--------|------|
| TD-VARIANT-CONSTRUCTOR | 解析器不支持不带前缀的 variant constructor (Some/None/Ok/Err) | P1 | v0.2 P1 (Stage 18.166+) |
| TD-OPTION-ADVANCED-METHODS | Option/Result 方法 (is_some/unwrap/map/and_then) 未实现 | P2 | v0.2 P1 (Stage 18.166+) |

## 6. §3.2 验收 (全套)

| 步骤 | 结果 |
|------|------|
| cargo check --features llvm-backend | ✅ 0 errors, 0 warnings |
| cargo fmt + cargo fmt --check | ✅ exit 0 |
| cargo clippy --all-targets --features llvm-backend | ✅ 0 warnings |
| cargo test --features llvm-backend | ✅ 全部通过 (无回归) |

## 7. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | 对齐 09-stdlib.md §2.4 (Option/Result 定义) | ✅ |
| J2 单一职责 | prelude.rs 仅负责注入 | ✅ |
| J3 单向流动 | driver → inject → HIR lower (无环) | ✅ |
| J4 编译相关表达完整 | Option/Result 定义完整 | ✅ |
| J5 阶段划分清晰 | 注入在 parse 后, HIR lower 前 | ✅ |
| J6 科学合理粒度 | prelude.rs ~150 LOC | ✅ |

## 8. Stage Summary

- **Stage 18.165 PASSED** — Option/Result 内置类型实现 (部分)
- **新增**: `src/stdlib/prelude.rs` (150 LOC) — prelude 注入系统
- **修改**: `compile_inner` 调用 `inject_prelude`
- **结果**: `Option::Some(42)` + `Option::None` + `Result::Ok(42)` + `Result::Err(e)` 可用
- **限制**: 不带前缀的 `Some(42)` 不支持 (TD-VARIANT-CONSTRUCTOR)
- **限制**: 方法未实现 (TD-OPTION-ADVANCED-METHODS)
- **§3.2 全套验收**: cargo check/fmt/clippy/test 全绿
- **v0.433.0**: patch bump
- **下一步**: Stage 18.166 实现 variant constructor (不带前缀) + 基本方法
