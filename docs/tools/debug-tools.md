# 调试工具文档

> **Author**: redskaber
> **Date**: 2026-09-05
> **Version**: v1.0
> **Status**: Active
> **关联流程**: docs/stage-committee-process.md §18 (依赖与基础设施审查)

## 概述

Landin 编译器开发过程中使用的调试工具集合。当环境缺失工具时，通过
`scripts/setup-debug-tools.sh` 进行布署。

## 工具清单

### 1. LLVM 工具链 (已内置)

LLVM 22.1.8 在 `/tmp/llvm-22-prefix/bin/` 下提供:

| 工具 | 用途 |
|------|------|
| `llvm-config` | LLVM 配置查询 (版本、prefix、libs) |
| `llvm-as` | LLVM IR → LLVM bitcode (验证 IR 有效性) |
| `llc` | LLVM IR → 汇编/目标文件 (验证 codegen 正确性) |
| `llvm-addr2line` | 地址 → 源码行号映射 (crash backtrace 分析) |
| `llvm-nm` | 符号表查询 (linker error 分析) |
| `llvm-objdump` | 目标文件反汇编 |
| `llvm-readelf` | ELF 文件分析 |

**使用场景**: Stage 113 调试 TD-LLVM-OBJ-EMIT-CRASH 时，用 `llvm-as` +
`llc` 验证 IR 有效性，用 `llvm-addr2line` 解析 crash 地址。

### 2. addr2line (系统已安装)

`/usr/bin/addr2line` — GNU binutils 版本，用于将 crash backtrace 中的
地址映射到源码行号。

**使用方式**:
```bash
# 从 crash 日志中提取地址，映射到源码行
addr2line -e target/release/landin-stage0 -f 0x1234567
```

### 3. RUST_BACKTRACE (Rust 内置)

Rust 的内置 backtrace 机制，通过环境变量控制:

| 值 | 行为 |
|------|------|
| `0` | 不打印 backtrace |
| `1` | 打印简短 backtrace (函数名) |
| `full` | 打印完整 backtrace (函数名 + 源码路径 + 行号) |

**注意**: `RUST_BACKTRACE` 只捕获 Rust panic backtrace。对于 native
SIGSEGV (如 LLVM C API 崩溃)，需要用 `addr2line` 或 `lldb`/`gdb`。

### 4. 调试脚本

| 脚本 | 用途 |
|------|------|
| `scripts/debug_obj_emit_crash.sh` | 复现 + 诊断 TD-LLVM-OBJ-EMIT-CRASH |
| `scripts/stability_v2.sh` | 100 次跑稳定性验证 (非确定性 SIGSEGV 检测) |

### 5. 未安装但可用的工具

以下工具在当前环境未安装 (需要 root 权限)，但可通过替代方案使用:

| 工具 | 替代方案 | 安装方式 |
|------|---------|---------|
| `lldb` | `addr2line` + `RUST_BACKTRACE=full` | `apt install lldb` (需要 root) |
| `valgrind` | `addr2line` + 手动内存分析 | `apt install valgrind` (需要 root) |
| `gdb` | `addr2line` + `RUST_BACKTRACE=full` | `apt install gdb` (需要 root) |
| `rust-lldb` | 需要 `lldb` 先安装 | `rustup component add rust-lldb` |

## 调试工作流

### 场景 1: 确定性 SIGSEGV (如 TD-LLVM-OBJ-EMIT-CRASH)

```bash
# 1. 复现 crash
bash scripts/debug_obj_emit_crash.sh

# 2. 验证 IR 有效性 (排除 IR 本身的问题)
llvm-as output.ll -o output.bc
llc output.ll -o output.s
llc output.ll -filetype=obj -o output.o

# 3. 如果 IR 有效但 --emit-obj 崩溃 → 问题在 LLVM C API binding
#    用 debug build + RUST_BACKTRACE=full 获取更多信息
cargo build --features llvm-backend  # debug build
RUST_BACKTRACE=full target/debug/landin-stage0 --emit-obj input.lin

# 4. 用 addr2line 解析 crash 地址 (如果有地址)
addr2line -e target/debug/landin-stage0 -f 0x<address>
```

### 场景 2: 非确定性 SIGSEGV (如 Stage 105 RCA)

```bash
# 1. 运行稳定性脚本
bash scripts/stability_v2.sh 10

# 2. 如果有失败，分析失败模式 (不同失败集 = 非确定性)
# 3. 比较 LLVM IR (成功跑 vs 失败跑)
# 4. 用 ASLR off 减少 (但不消除) crash 率
setarch -R target/release/landin-stage0 --emit-obj input.lin
```

## 环境布署

```bash
# LLVM 22 环境 (自动下载 + 提取 .deb)
bash scripts/setup-llvm-env.sh

# Rust 工具链
bash scripts/setup-rust-env.sh

# 完整环境 (LLVM + Rust)
source scripts/env.sh
```
