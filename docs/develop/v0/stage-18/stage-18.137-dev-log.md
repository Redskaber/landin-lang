# Stage 18.137 — TD-LOC-DRIVER 继续修复 (提取 driver_tests.rs)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.405.0 (Stage 18.137 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 (重构判据 J1-J6) + §12 (最优>最小) + §2.2 (设计原则) + §3.2 (验收)
> **Complexity**: L2 (test module extraction + re-export adjustment)
> **Task ID**: stage18.137

---

## 1. 阶段目标

按用户要求严格读取 `docs/stage-committee-process.md` v6.4 §1-§17, 继续推进 TD-LOC-DRIVER (mod.rs 2351 LOC) 的修复。严格遵循 §13.4 J1-J6 重构判据 + §10 API 命名标准化 + §11 接口隔离 + §2.2 设计原则。

## 2. §17 任务规划 — TD-LOC-DRIVER 继续修复

### 2.1 选定理由

Stage 18.134 后 driver/mod.rs 仍有 2351 LOC。本阶段:
- 提取 test module + test helpers (compile_expect_ok) 到 `driver_tests.rs` (279 LOC)
- mod.rs 从 2351 降至 2082 LOC

### 2.2 §13.4 J1-J6 判据检查

| 判据 | 通过条件 | 本阶段满足情况 |
|------|---------|---------------|
| J1 架构设计对齐 | ✅ | 目录模块结构不变 |
| J2 单一职责 | ✅ | driver_tests.rs = test responsibility |
| J3 单向流动 | ✅ | tests 调用 mod.rs functions, 不回调 |
| J4 编译相关表达完整 | ✅ | test module 完整 |
| J5 阶段划分清晰 | ✅ | 全部在 driver 阶段 |
| J6 科学合理粒度 | ⚠️ | mod.rs 2082 仍超 1500 (compile_inner 1433 LOC) |

## 3. 重构执行

### 3.1 提取 test module

- 从 mod.rs 提取 `#[cfg(test)] mod tests { ... }` + `compile_expect_ok` 到 `driver_tests.rs`
- mod.rs 添加 `mod driver_tests;` + `pub use driver_tests::compile_expect_ok;`
- 移除 `#[cfg(test)]` guard (因为 integration tests 需要 compile_expect_ok)

### 3.2 结果

```
src/driver/mod.rs (2082 LOC) — compile_inner + compile_binary + struct/impl + helpers
src/driver/driver_tests.rs (279 LOC) — test module + compile_expect_ok
```

## 4. §3.2 验收

- ✅ `cargo check` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — exit 0
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --lib` — 640 passed, 0 failed
- ✅ `cargo test --tests` — 2,663 passed, 0 failed, 2 ignored

## 5. Stage Summary

- **Stage 18.137 PASSED** — TD-LOC-DRIVER 继续修复 (提取 driver_tests.rs)
- **拆分结果**: mod.rs 2351 → 2082 LOC + driver_tests.rs 279 LOC
- **§13.4 J1-J6**: J1-J5 全部通过; J6 部分通过 (mod.rs 仍超 1500)
- **§3.2 验收**: 全套通过 (640 lib + 2,663 integration tests, 0 failures)
- **v0.405.0**: patch bump
- **下一步**: Stage 18.138 — compile_inner 拆分 (1433 LOC, 按编译阶段提取 pre-computation + post-typeck sections)
