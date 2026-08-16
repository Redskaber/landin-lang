# Stage 18.156 — mini-cargo 缺陷1 修复: landinc build --bin

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.424.0 (Stage 18.156 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §3.2 (交付前验收) + §13.4 (重构即架构设计) + §12 (最优>最小)
> **Complexity**: L2 (CLI 扩展 + 链接逻辑提取)
> **Task ID**: stage18.156

## 1. 阶段目标

修复 Stage 18.154 记录的 **缺陷1**: `landinc build` 不链接可执行文件 (仅编译到 MIR + 可选 LLVM IR)。

## 2. 问题分析

### 2.1 原始缺陷

Stage 18.154 的 `landinc build` 仅调用 `compile_project_opt` 生成 MIR，可选生成 LLVM IR 文本，但**不链接可执行文件**。用户必须用 `landinc run` 才能获得可执行文件——但 `run` 会立即执行并删除可执行文件，无法保留。

### 2.2 根因

1. `cmd_build` 没有链接逻辑
2. 链接逻辑只在 `cmd_run` 中内联实现，未提取为可复用函数
3. 链接需要 C wrapper (提供 `main()` + 运行时 stubs)，但该 wrapper 只在 `landin-stage0` 的 `main.rs` 中定义

## 3. 修复方案

### 3.1 新增 `--bin` flag

`landinc build --bin` — 编译 + 链接可执行文件到 `target/<name>` (匹配 `cargo build` 行为)。

### 3.2 提取 `link_object_to_executable` helper (§13.4 J2)

从 `cmd_run` 提取链接逻辑为独立函数:
- 输入: LLVM emitter + obj_path + exe_path
- 输出: 链接后的可执行文件
- 被 `cmd_build --bin` 和 `cmd_run` 共用 (通解>特解)

### 3.3 提取 `LANDIN_C_WRAPPER` 常量

从 `landin-stage0` 的 `main.rs` 提取 C wrapper 源码为 `landinc.rs` 中的常量。该 wrapper 提供:
- `main()` 调用 `landin_main()`
- 所有 `__landin_*` 运行时 stubs (println, panic, str_eq, assert 等)

### 3.4 链接参数

`cc -fno-pie -no-pie <wrapper.c> <obj.o> -o <exe> -lm`

- `-fno-pie -no-pie`: Landin LLVM 模块非 PIC，需禁用 PIE
- `-lm`: 链接数学库 (codegen 可能引用 math 函数)

## 4. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| Flag | `--bin` | 匹配 `cargo build --bin` | ✅ |
| 函数 | `link_object_to_executable` | `<verb>_<noun>_<prep>_<noun>` | ✅ |
| 函数 | `link_and_emit_executable` | `<verb>_<conj>_<verb>_<noun>` | ✅ |
| 常量 | `LANDIN_C_WRAPPER` | `<NOUN>_<NOUN>` (SCREAMING_SNAKE) | ✅ |

## 5. 接口设计 (§11)

- `link_object_to_executable` 是 landinc 内部 helper (`fn`, 非 `pub`)
- `LANDIN_C_WRAPPER` 是 landinc 内部常量
- `link_and_emit_executable` 被 `cmd_build --bin` 调用
- 不修改公共 API — 仅扩展 CLI

## 6. 测试 (§9)

### 6.1 集成测试 (3 个: 2 positive + 1 negative)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_156_build_bin_produces_main` | 正向 | 编译产生 landin_main |
| `stage18_156_build_bin_multi_file` | 正向 | 多文件项目编译 |
| `stage18_156_build_bin_without_main` | 负向 | 库项目无 main 无法链接 |

### 6.2 手动验证 (CLI)

```
$ landinc new myapp
$ cd myapp
$ landinc build --bin
Compiling myapp v0.1.0 (src/main.lin)
Compiling finished (1 MIR bodies)
Executable written to target/myapp

$ ./target/myapp
Hello, Landin!
```

## 7. §3.2 验收 (全套)

按照 `docs/stage-committee-process.md` §3.2 执行完整验收命令:

```bash
cargo clean && cargo build --features llvm-backend && cargo check --features llvm-backend && cargo fmt && cargo clippy --all-targets --features llvm-backend && cargo test --features llvm-backend
```

| 步骤 | 结果 |
|------|------|
| cargo clean | ✅ Removed 1733 files |
| cargo build --features llvm-backend | ✅ Finished in 16.97s |
| cargo check --features llvm-backend | ✅ 0 errors, 0 warnings |
| cargo fmt + cargo fmt --check | ✅ exit 0 |
| cargo clippy --all-targets --features llvm-backend | ✅ 0 warnings |
| cargo test --features llvm-backend | ✅ 653 lib + 2696 integration, 0 failed |

## 8. 简写和缺陷记录

### 8.1 当前简写

**简写1**: C wrapper (`LANDIN_C_WRAPPER`) 在 `landinc.rs` 和 `main.rs` 中重复定义。
- **原因**: 两个 binary 独立编译，无法共享常量 (除非提取到 lib)。
- **修订计划**: 未来 stage 将 C wrapper 提取到 `landin_compiler::codegen::runtime` 模块，两个 binary 共用。

**简写2**: `--bin` 总是链接到 `target/<name>`，不支持自定义输出路径。
- **原因**: MVP 阶段简化。
- **修订计划**: 添加 `-o <path>` flag 支持。

### 8.2 缺陷记录

**无新缺陷**。所有修复完整，手动验证通过。

## 9. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | `--bin` 匹配 `cargo build --bin` 语义 | ✅ |
| J2 单一职责 | `link_object_to_executable` 仅负责链接 | ✅ |
| J3 单向流动 | cmd_build → link_and_emit → link_object (无环) | ✅ |
| J4 编译相关表达完整 | 链接逻辑完整在 landinc.rs | ✅ |
| J5 阶段划分清晰 | landinc 是 driver 层消费者 | ✅ |
| J6 科学合理粒度 | 新增 ~150 LOC (C wrapper + 2 函数), 合理 | ✅ |

## 10. Stage Summary

- **Stage 18.156 PASSED** — mini-cargo 缺陷1 修复: landinc build --bin
- **新增**: `--bin` flag + `link_object_to_executable` helper + `LANDIN_C_WRAPPER` 常量
- **修复**: `landinc build` 现在可链接可执行文件 (之前仅 MIR)
- **手动验证**: `landinc new` → `landinc build --bin` → `./target/myapp` 打印 "Hello, Landin!"
- **测试**: 653 lib + 2696 integration (新增 3), 0 failures
- **§3.2 全套验收**: cargo clean/build/check/fmt/clippy/test 全绿
- **v0.424.0**: patch bump
- **下一步**: v0.2 P1 — stdlib facade 或 format macros
