# Stage 18.148 — 全模块重新审理 + TD-PROJECTION-RESOLVER 修复

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.416.0 (Stage 18.148 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 J1-J6 + §11 (接口隔离) + §12 (最优>最小)
> **Complexity**: L2 (全模块审计 + projection_resolver 迁移)
> **Task ID**: stage18.148

## 1. 全模块重新审理

### 1.1 typeck/ (12 files, 6126 LOC)
- **J2 合规**: ✅ 所有文件单一职责清晰
- **唯一问题**: projection_resolver.rs 位置错误 (在 typeck/ 但由 driver 调用)
- **修复**: 移动到 driver/ (见下文)

### 1.2 mir/lower/ (14 files, 11195 LOC)
- **J2 合规**: ✅ 所有文件单一职责清晰
- **control_flow.rs (2228 LOC)**: 大但内聚 (if/match/loop/while/for lowering)
- **评估**: ACCEPTABLE — 进一步拆分会碎片化控制流 lowering

### 1.3 borrowck/ (8 files, 5767 LOC)
- **J2 合规**: ✅ 所有文件单一职责清晰
- **mod.rs (1857 LOC)**: BorrowChecker + check_mir_body 单一入口
- **评估**: ACCEPTABLE — 进一步拆分会碎片化 borrow checking 流

### 1.4 parser/macro_expand/ (2 files, 5973 LOC)
- **J2 合规**: ✅ mod.rs (真实 1562 + 测试 2342) + builtin_macros (27 个独立定义)
- **评估**: ACCEPTABLE — 测试代码按 §13.3.5 无法迁移

### 1.5 driver/ (7 files, 4392 LOC)
- **J2 合规**: ✅ (Stage 18.147 修复后)
- **新增**: projection_resolver.rs (从 typeck/ 迁移)

## 2. TD-PROJECTION-RESOLVER 修复

### 问题
- `projection_resolver.rs` 在 `typeck/` 下，但只被 `driver/mod.rs` 调用
- 违反 §11 (接口隔离): typeck 阶段不调用它，它是 driver post-typeck 操作

### 修复
- 移动: `src/typeck/projection_resolver.rs` → `src/driver/projection_resolver.rs`
- 更新: `typeck/mod.rs` 移除 `pub mod projection_resolver;`
- 更新: `driver/mod.rs` 添加 `mod projection_resolver;`
- 更新: 调用点 `crate::typeck::projection_resolver::` → `projection_resolver::`

### §11 合规
- ✅ typeck/ 不再包含 driver-stage 操作
- ✅ driver/ 包含所有 post-typeck 操作 (projection_resolver + driver_codegen_prep + driver_validations)

## 3. §3.2 验收

- ✅ cargo check 0 errors / 0 warnings
- ✅ cargo fmt --check exit 0
- ✅ cargo clippy 0 warnings
- ✅ cargo test --lib 640 passed
- ✅ cargo test --tests 2663 passed

## 4. Stage Summary

- **Stage 18.148 PASSED** — 全模块重新审理 + TD-PROJECTION-RESOLVER 修复
- **全模块审计**: typeck/ ✅, mir/lower/ ✅, borrowck/ ✅, parser/macro_expand/ ✅, driver/ ✅
- **TD-PROJECTION-RESOLVER**: ✅ Resolved (projection_resolver 从 typeck/ 移到 driver/)
- **v0.416.0**: patch bump
