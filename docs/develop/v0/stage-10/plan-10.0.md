# Stage 10.0 开发计划: Format migration + CLI upgrade + Runner upgrade

> **阶段**: Stage 10.0 (Stage 10 第 0 个子阶段 — 基础设施)
> **版本**: v0.17.1 → v0.17.2
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2

## 1. 背景

v0.1 gap analysis (r196) 识别了 6 个 gaps, 其中 GAP-02 (format) + GAP-03 (CLI) +
GAP-05 (runner) 需要在 Stage 10.0 解决, 为后续 7 个 conformance categories
(10.1-10.7) 提供基础设施。

## 2. 完成内容

### 2.1 CLI 升级 (GAP-03)

`src/bin/main.rs` 新增:
- `--compile`: 完整编译 (lex + parse + resolve + typeck + borrowck + codegen)
  - 使用 `driver::compile(&source_file.src)` 运行完整 pipeline
  - 成功 exit 0, 失败 exit 1
- `--emit-llvm-ir`: 输出 LLVM IR (implies --compile)
  - 使用 `codegen::codegen_crate(&result)` 生成 LLVM IR

### 2.2 Runner 升级 (GAP-05)

`tests/conformance/run_all.py` 升级:
- `--mode parse` (default): 使用 `--emit-ast` (legacy, 仅验证 parse)
- `--mode compile`: 使用 `--compile` (验证完整 pipeline)
- 支持 spec `//` 格式 (`EXPECTED: compile_ok/compile_error`, `ERROR_PATTERN: ...`)
- 向后兼容 legacy `//!` 格式 (`PASS`/`FAIL`/`error_pattern: ...`)

### 2.3 格式迁移 (GAP-02) — 推迟到 Stage 10.1

格式迁移 (600 .lin 从 `//!` → `//`) 推迟到 Stage 10.1, 与 01-typecheck category
创建同步进行。当前 runner 双格式兼容, 无需立即迁移。

## 3. 验收

- ✅ CLI `--compile` 成功编译 valid program (exit 0)
- ✅ CLI `--compile` 正确报告 errors (exit 1)
- ✅ CLI `--emit-llvm-ir` 输出 LLVM IR
- ✅ Runner `--mode parse` 向后兼容 (600/600 pass)
- ✅ Runner 支持 `--mode compile`
- ✅ Runner 支持 spec `//` 格式 + legacy `//!` 格式

---

**创建日期**: 2026-07-26
