# Stage 18.30 — Parser Println Special Case: Dead Code Documentation

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-07
> **Version**: v0.308.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.27 激活了 `__landin_println` macro body，使 println!
走通解路径 (Call)。parser 的 Println 特解现在已是死代码 — macro 展开
在 parse 之前将 `println!` 转换为 `__landin_println(...)`，所以 parser
永远不会遇到 `println!` 调用。

**用户反馈**: "不能偏离正确的设计和实现（正确 > 妥协）"

**具体目标**:
1. 在 parser 的 Println 特解代码中添加注释，标记为死代码
2. 不移除代码（避免破坏现有测试），但文档标记为 deprecated
3. 为 Phase 3 完整移除做准备

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | 记录特解已被通解取代 |
| 正确 > 妥协 | 不移除代码（风险），但明确标记状态 |
| 避免死代码 | 标记为 deprecated，Phase 3 移除 |
| 避免分散内容 | 注释集中在 parser 特解位置 |

## 3. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿 ✅
