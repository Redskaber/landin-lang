# Stage 18.157 — 修复 Stage 18.156 简写1: 提取 C wrapper 到 library (DRY)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.425.0 (Stage 18.157 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 (重构即架构设计) + §1.0 原則 6 (通解>特例) + §3.2 (交付前验收)
> **Complexity**: L2 (提取共享常量 + 更新 2 个 binary)
> **Task ID**: stage18.157

## 1. 阶段目标

修复 Stage 18.156 记录的 **简写1**: C wrapper (`LANDIN_C_WRAPPER`) 在 `landinc.rs` 和 `main.rs` 中重复定义。

## 2. 问题分析

### 2.1 原始简写

Stage 18.156 在 `src/bin/landinc.rs` 中定义了 `LANDIN_C_WRAPPER` 常量（C wrapper 源码），但 `src/bin/main.rs` (landin-stage0) 中也有内联的相同 C wrapper 源码（约 120 行）。这违反 DRY 原则：

- 两份相同代码，维护时需同步更新
- 如果一份更新了另一份没更新，会导致行为不一致
- 违反 §1.0 原則 6 (通解>特例) — 应该有一个共享来源

### 2.2 根因

两个 binary 独立编译，各自包含 C wrapper 源码。没有提取到 library 中共享。

## 3. 修复方案

### 3.1 新增 `src/codegen/runtime.rs` 模块

创建公共模块 `landin_compiler::codegen::runtime`，包含 `LANDIN_C_WRAPPER` 常量。

Per §13.4 J2 (单一职责): 该模块仅负责 C 运行时定义。
Per §10 (API 命名): `LANDIN_C_WRAPPER` 遵循 `<NOUN>_<NOUN>` 常量命名。

### 3.2 更新两个 binary

- `src/bin/landinc.rs`: 移除本地 `LANDIN_C_WRAPPER`，改用 `use landin_compiler::codegen::runtime::LANDIN_C_WRAPPER`
- `src/bin/main.rs`: 移除内联 C wrapper 字符串，改用 `landin_compiler::codegen::runtime::LANDIN_C_WRAPPER`

### 3.3 版本选择

使用 `main.rs` 中的完整版本（包含历史注释）作为 canonical source，因为它有更详细的设计文档记录。

## 4. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 模块 | `codegen::runtime` | `<stage>::<noun>` | ✅ |
| 常量 | `LANDIN_C_WRAPPER` | `<NOUN>_<NOUN>` (SCREAMING_SNAKE) | ✅ |

## 5. 接口设计 (§11)

- `LANDIN_C_WRAPPER` 是 `pub const` — 公共 API，供 binary 使用
- `runtime` 模块是 `pub mod` — 公共模块
- 不跨阶段调用 — 仅 codegen 层定义，driver 层消费
- 提取后两个 binary 共用同一来源，消除重复

## 6. 测试 (§9)

### 6.1 Lib 测试 (3 个, in `codegen/runtime.rs`)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_157_c_wrapper_contains_all_stubs` | 正向 | 11 个 `__landin_*` 运行时 stub 全部存在 |
| `stage18_157_c_wrapper_has_main_entry` | 正向 | `main()` 调用 `landin_main()` |
| `stage18_157_c_wrapper_includes_headers` | 正向 | 包含 stdio/stdlib/stdarg 头文件 |

### 6.2 手动验证

```
$ landinc new shared && cd shared
$ landinc build --bin
Executable written to target/shared
$ ./target/shared → "Hello, Landin!"

$ echo 'fn main() { println!("from stage0!"); }' > test.lin
$ landin-stage0 test.lin --emit-bin
$ ./test.out → "from stage0!"
```

两个 binary 均使用共享 wrapper 正常工作。

## 7. §3.2 验收 (全套)

按照 `docs/stage-committee-process.md` §3.2 执行完整验收命令:

| 步骤 | 结果 |
|------|------|
| cargo clean | ✅ Removed 2110 files |
| cargo build --features llvm-backend | ✅ Finished in 18.77s |
| cargo check --features llvm-backend | ✅ 0 errors, 0 warnings |
| cargo fmt + cargo fmt --check | ✅ exit 0 |
| cargo clippy --all-targets --features llvm-backend | ✅ 0 warnings |
| cargo test --features llvm-backend | ✅ 656 lib + 2696 integration, 0 failed |

## 8. 简写和缺陷记录

### 8.1 已修复简写

**简写1 (Stage 18.156)**: C wrapper 重复定义 → ✅ Resolved Stage 18.157
- 提取到 `src/codegen/runtime.rs`，两个 binary 共用

### 8.2 剩余简写 (从 Stage 18.155-18.156 继承)

**简写2**: `--bin` 不支持自定义输出路径 (`-o` flag)。
- **修订计划**: 添加 `-o <path>` flag。

**简写4**: `--release` 无 LLVM 级优化差异。
- **修订计划**: 未来 stage 扩展 LLVM `TargetOptions` 支持 opt-level=2/3。

### 8.3 缺陷记录

**无新缺陷**。所有修复完整，手动验证通过。

## 9. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | C wrapper 属于 codegen 层 (codegen 生成需要 runtime 的代码) | ✅ |
| J2 单一职责 | `runtime.rs` 仅负责 C 运行时定义 | ✅ |
| J3 单向流动 | binary → codegen::runtime (无环) | ✅ |
| J4 编译相关表达完整 | C wrapper 完整定义在一个模块 | ✅ |
| J5 阶段划分清晰 | runtime 在 codegen 层，binary 消费 | ✅ |
| J6 科学合理粒度 | runtime.rs ~180 LOC (C source + tests), 合理 | ✅ |

## 10. Stage Summary

- **Stage 18.157 PASSED** — 修复 Stage 18.156 简写1: 提取 C wrapper 到 library
- **新增**: `src/codegen/runtime.rs` (公共模块 + `LANDIN_C_WRAPPER` 常量 + 3 tests)
- **修改**: `landinc.rs` + `main.rs` 移除重复 C wrapper，改用共享常量
- **消除重复**: ~120 行 C 代码从 2 份 → 1 份 (DRY)
- **手动验证**: `landinc build --bin` + `landin-stage0 --emit-bin` 均通过
- **测试**: 656 lib + 2696 integration (新增 3), 0 failures
- **§3.2 全套验收**: cargo clean/build/check/fmt/clippy/test 全绿
- **v0.425.0**: patch bump
- **下一步**: v0.2 P1 — stdlib facade 或 format macros
