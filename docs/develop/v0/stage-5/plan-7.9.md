# Stage 7.9 — 系统性审查 + 设计文档同步检查 + v0.2 规划

> **阶段**: Stage 7.9
> **版本**: v0.14.8 → v0.14.9
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §25 + §13.4 + §17.1

## 1. 系统性审查：当前项目状态

### 1.1 版本与测试

| 指标 | 值 |
|------|-----|
| 版本 | v0.14.8 |
| 测试数 | 2035 (126 unit + 1909 integration) |
| 源代码 LOC | 31,073 (86 files) |
| 测试文件 | 116 files |
| 流程版本 | v3.21 |
| API 命名标准 | v1.95 |

### 1.2 Stage 完成状态

| Stage | 状态 | 子阶段 |
|-------|------|--------|
| Stage 0-4 | ✅ Complete | — |
| Stage 5 | ✅ Complete | 99 sub-stages (5.1-5.99) |
| Stage 6 | ✅ Complete | 18 sub-stages (6.1-6.18, architecture refactoring) |
| Stage 7 | ✅ Complete | 8 sub-stages (7.1-7.8, TD-015 + TD-018 + §25.8 + §25 deep review) |

### 1.3 设计文档同步状态

| 文档 | §25.8 回写 | 状态 |
|------|-----------|------|
| 01-language-specification.md | ✅ §13 | Synced |
| 02-grammar.md | ✅ §5 | Synced |
| 03-type-system.md | ✅ §10 + §11 | Synced (Stage 7 update) |
| 04-ownership-borrowing.md | ✅ §11 + §12 | Synced (Stage 7 update) |
| 05-ast.md | ✅ §13 | Synced |
| 06-mir.md | ✅ §14 | Synced |
| 07-codegen.md | ✅ §14 | Synced |
| 09-stdlib.md | ✅ §11 | Synced |

**结论**: ✅ 所有 8 份核心设计文档已完成 §25.8 回写。

### 1.4 技术债清单

| ID | 描述 | 优先级 | 状态 |
|----|------|--------|------|
| TD-011 | mir/lower/mod.rs LOC | P2 | ✅ CLOSE (6.1-6.10, -76.9%) |
| TD-014 | L5 trait dispatch vtable | P2 | ✅ CLOSE (5.80) |
| TD-015 | Region inference | P2 | ✅ CLOSE (7.1-7.5) |
| TD-016 | dyn Trait return type | P3 | ✅ CLOSE (5.82) |
| TD-017 | codegen/mod.rs LOC | P3 | ✅ CLOSE (6.7-6.8) |
| TD-018 | dyn Trait 仅 stdlib | P3 | ✅ CLOSE (7.6) |
| TD-019 | expr_operand 巨型 match | P3 | OPEN (收益不足暂不拆) |
| TD-022-027 | 各阶段拆分 TD | P3 | ✅ CLOSE |

**结论**: 仅有 TD-019 OPEN（用户指示收益不足时暂不拆分）。

### 1.5 最大文件（当前状态）

| 文件 | LOC | 状态 |
|------|-----|------|
| borrowck/region_inference.rs | 1462 | ✅ < 1500 |
| mir/lower/expr_operand.rs | 1275 | ✅ < 1300 |
| borrowck/mod.rs | 1202 | ✅ < 1300 |
| typeck/checker.rs | 1156 | ✅ < 1300 |
| stdlib/trait_methods.rs | 1103 | ✅ < 1300 |
| codegen/mod.rs | 1050 | ✅ < 1300 |
| parser/expr.rs | 1026 | ✅ < 1300 |

**结论**: ✅ 所有文件 < 1500 LOC，架构健康。

### 1.6 Worklog 同步状态

**问题**: worklog.md 仅包含到 Stage 5.82 的条目。Stage 6.1-6.18 + Stage 7.1-7.8
的工作记录存在于项目文件中（plan/gate-review/dev-log），但未追加到共享 worklog.md。

**修复**: 本轮将追加 Stage 6/7 的 worklog 摘要。

## 2. v0.2 规划

### 2.1 v0.2 目标（参考 12-roadmap.md）

| 特性 | 设计文档 | 复杂度 | 优先级 |
|------|---------|--------|--------|
| Lifetime 标注激活 | §3.1-3.4 | 中 | P1 |
| extern "C" ABI | §13.2 | 中 | P2 |
| Unwind + drop elaboration | §5 | 高 | P2 |
| async/await | §10 | 高 | P3 |
| Object safety 规则 | §2.3 | 低 | P2 |

### 2.2 推荐下一步

**Stage 8 (v0.2 启动)**:
1. **8.1**: 激活 region inference — MIR lower 传递真实 lifetime 标注
2. **8.2**: Lifetime elision 规则实现（§3.2）
3. **8.3**: Object safety 规则检查（§2.3）
4. **8.4**: extern "C" ABI 基础（§13.2）
5. **8.5+**: Drop elaboration + async/await

## 3. 验收标准

- [ ] 系统性审查文档创建
- [ ] worklog.md 追加 Stage 6/7 摘要
- [ ] 测试文件创建
- [ ] cargo clean + cargo test + cargo fmt + cargo clippy 全绿
- [ ] 版本 v0.14.8 → v0.14.9

---

**创建日期**: 2026-07-25
