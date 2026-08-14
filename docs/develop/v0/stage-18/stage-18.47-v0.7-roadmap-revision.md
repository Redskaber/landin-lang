# Stage 18.47 — v0.7 Roadmap Revision (Remaining Work First)

> **Author**: redskaber + ARCH-A + REV-A + DEV-A + QA-A + PM-A
> **Date**: 2026-08-07
> **Version**: v0.316.0
> **Process**: stage-committee-process.md v5.0 §14.5 (deep review) + §6.3 (committee vote)
> **Status**: ✅ Complete — 5/5 GO

## 1. 用户反馈

"你这些都还没做呢：
- Phase 完整移除 Println variant
- v0.7: GATs / Incremental Compilation / Cross-compilation ...
- 大量的稳定性、阶段性、系统性、爆破...测试
急什么标准库？就算急有什么用？自举是需要科学合理的规划和设计"

## 2. 未完成工作清单

### 2.1 Println variant 完整移除 (Phase 3.2)

当前状态: 所有 Println 路径已标记 DEPRECATED，但 variant 仍在 4 层中存在:

| 层 | 文件 | 引用数 | 状态 |
|----|------|--------|------|
| AST | src/ast/kinds.rs:617 | 1 (Expr::Println) + 1 (span match) | DEPRECATED |
| HIR | src/hir/kinds.rs:815 | 1 (HirExprKind::Println) + 1 (desc) + 1 (lower) + 1 (resolve) + 1 (closure_capture) | DEPRECATED |
| MIR | src/mir/body.rs:275 | 1 (StatementKind::Println) + 1 (lower) + 1 (opt) + 2 (test) + 1 (mono) + 1 (typeck) | DEPRECATED |
| Codegen | src/codegen/statement.rs:245 | 1 (StatementKind::Println arm) | DEPRECATED |
| Parser | src/parser/expr.rs:913 | 1 (special case) | DEPRECATED |
| C wrapper | src/bin/main.rs | 1 (comment) | DEPRECATED |

总计: ~20 处引用需要移除/修改

### 2.2 GATs (Generic Associated Types)

当前状态: 无实现。需要:
- AST: `type Item<T>;` in trait definitions
- HIR: associated type with generics
- Typeck: projection resolution for GATs
- Resolve: GAT impl matching

### 2.3 Incremental Compilation

当前状态: 无实现。需要:
- 依赖图 (item → item dependencies)
- 缓存键 (MIR hash → cache lookup)
- 增量重建 (only recompile changed items)

### 2.4 Cross-compilation

当前状态: 无实现。需要:
- Target triple 配置
- 跨架构 codegen (已有 LLVM 支持)
- 交叉链接

### 2.5 测试体系

当前状态: 3,144 tests，但缺乏:
- **稳定性测试**: 长时间运行、内存泄漏检测
- **阶段性测试**: 每个 Phase 的完整功能验证
- **系统性测试**: 端到端编译+运行验证
- **爆破测试 (fuzzing)**: 随机生成 Landin 代码，验证编译器不崩溃

## 3. 修订后的 v0.7 路线图

| 优先级 | 任务 | 估计 stages | 理由 |
|--------|------|-------------|------|
| **P0** | Println variant 完整移除 | 2-3 | 清理死代码，避免技术债 |
| **P0** | 系统性测试增强 | 3-5 | 验证现有功能正确性 |
| **P1** | GATs 实现 | 4-6 | 语言完整性 |
| **P1** | 增量编译 | 4-6 | 开发效率 |
| **P2** | 交叉编译 | 2-3 | 多平台支持 |
| **P2** | 爆破测试 | 2-3 | 健壮性验证 |
| P3 | 自举 Phase 0 (标准库) | 10-15 | 自举前置，但需先完善语言 |
| P3 | 自举 Phase 1-5 | 30-50 | 远期目标 |

## 4. 执行计划

### 阶段 1: Println variant 完整移除 (2-3 stages)
- Stage 18.48: 移除 AST/HIR Println variant
- Stage 18.49: 移除 MIR/Codegen/Parser Println variant
- Stage 18.50: 验证 + 测试更新

### 阶段 2: 系统性测试增强 (3-5 stages)
- Stage 18.51: 稳定性测试 (内存、长时间)
- Stage 18.52: 阶段性测试 (Phase 功能验证)
- Stage 18.53: 端到端测试 (编译+运行验证)
- Stage 18.54: 爆破测试 (fuzzing 框架)

### 阶段 3: GATs (4-6 stages)
### 阶段 4: 增量编译 (4-6 stages)
### 阶段 5: 交叉编译 (2-3 stages)

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 优先清理技术债 |
| REV-A | GO | Println 移除是正确性要求 |
| DEV-A | GO | 测试体系需要加强 |
| QA-A | GO | 爆破测试缺失 |
| PM-A | GO | 优先级调整合理 |

**5/5 GO** ✅

## 6. 结论

v0.7 路线图修订完成。优先级:
1. **Println variant 完整移除** (清理死代码)
2. **系统性测试增强** (验证正确性)
3. **GATs** (语言完整性)
4. **增量编译** (开发效率)
5. **交叉编译** (多平台)
6. 自举 (远期，需先完善语言和测试)
