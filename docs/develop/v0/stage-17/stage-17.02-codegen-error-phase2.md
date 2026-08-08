# Stage 17.02 — CodegenError Phase 2 (to_object_file migration)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.275.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

迁移 `to_object_file` 从 `Result<(), String>` 到 `Result<(), CodegenError>`，消除 3 处 `unwrap()`。

## 2. 实现

- `to_object_file` 返回类型: `Result<(), String>` → `CodegenResult<()>`
- 4 处 `Err(String)` → `Err(CodegenError::new(..., Span::DUMMY))`
- 3 处 `CString::new().unwrap()` / `.map_err(|e| e.to_string())?` → `cstr_result()?`
- 移除 `#[allow(dead_code)]` / `#[allow(unused_imports)]`
- 调用者（main.rs + tests.rs）无需修改（Display 兼容）

## 3. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 423 lib + 2529 integration = 2952 unit tests |

## 4. 后续

CodegenError 系统基本完成。剩余 Emitter trait 内部的 `unwrap()` 是 CString 构造（对 Landin 标识符不会 panic），不需要迁移。下一步可开始 Trait Solver (P1)。
