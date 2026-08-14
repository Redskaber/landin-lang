# Stage 18.95 — TraitError Location Migration (driver.rs → traits/error.rs)

> **Author**: redskaber
> **Date**: 2026-08-10
> **Version**: v0.362.0 → v0.363.0
> **Status**: Complete

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/stage-committee-process.md` §10.1 规则 5 (DRY 单一真理源)
- `docs/stage-committee-process.md` §6 (单一数据源原则)
- `docs/develop/v0/v0.1-capability-boundaries.md` v0.2 路线图 P1

### 1.2 设计意图摘要

错误类型应在拥有模块定义（§6 单一数据源原则）。当前 TraitError 定义在
`driver.rs`（编排层），但 traits 系统的 error 应该在 `traits/error.rs`。
与 TypeError (typeck/error.rs), BorrowError (borrowck/error.rs) 保持一致。

### 1.3 已实现 / 偏差 / 未实现

| 项目 | 状态 |
|------|------|
| 新建 src/traits/error.rs | ✅ |
| TraitError + format_with_interner + format_without_interner 迁移 | ✅ |
| src/traits/mod.rs 添加 pub mod error + pub use | ✅ |
| src/driver.rs 移除 TraitError 定义 + 改为 use | ✅ |
| src/lib.rs 更新 re-export | ✅ |
| 修复引用 (typeck/checker.rs + 2 test files) | ✅ |

## 2. 任务拆分（MUV）

| ID | 任务 | 验收标准 |
|----|------|---------|
| 18.95.1 | 新建 src/traits/error.rs | TraitError enum + 2 format 函数 |
| 18.95.2 | 更新 src/traits/mod.rs | pub mod error + pub use error::TraitError |
| 18.95.3 | 更新 src/driver.rs | 移除 ~100 行 TraitError 定义 + 改为 use |
| 18.95.4 | 更新 src/lib.rs | pub use driver::TraitError → pub use traits::TraitError |
| 18.95.5 | 修复引用 | typeck/checker.rs + 2 test files |

## 3. 接口隔离分析（§11）

TraitError 迁移不改变接口隔离 — driver 仍然通过 `use crate::traits::TraitError`
导入。其他模块（typeck）也通过 `crate::traits::TraitError` 导入，符合 §11 数据
契约：错误类型在拥有模块定义，跨模块通过 `pub use` re-export。

## 4. API 命名合规性（§10）

- ✅ `TraitError` 后缀 `Error`（§10.1 规则 8）
- ✅ 模块路径 `traits::error::TraitError`（与 typeck::error::TypeError 一致）
- ✅ 显式 re-export `pub use error::TraitError`（§10.1 规则 4，无 glob）

## 5. 验收（§3.2）

- ✅ cargo build --features llvm-backend
- ✅ cargo fmt --check
- ✅ cargo clippy --all-targets --features llvm-backend -- -D warnings (0 warnings)
- ✅ cargo test --features llvm-backend (638 lib + 2648 integration = 3286 unit tests)
- ✅ python3 tests/conformance/run_all.py (2935 conformance tests)

## 6. Stage Summary

- Stage 18.95 PASSED — TraitError 位置迁移完成
- TraitError 从 driver.rs 迁移到 traits/error.rs
  → 遵循 §6 单一数据源原则 (error 类型在拥有模块定义)
  → 与 TypeError (typeck/error.rs), BorrowError (borrowck/error.rs) 一致
- 3286 unit + 2935 conformance = 6221 total tests, 0 failures
- v0.363.0: minor bump (TraitError location migration)
- v0.2 路线图 P1 TraitError 位置迁移 ✅ 完成
