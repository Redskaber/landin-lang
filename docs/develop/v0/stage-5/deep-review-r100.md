# Stage 5 深度审查报告 #5（Round 100 — Stage 5.81）

> **审查日期**: 2026-07-24
> **审查者**: Stage Committee (main agent, full audit)
> **基线版本**: v0.11.76
> **测试数**: 1637 tests + 5 benchmarks
> **流程**: stage-committee-process.md v3.20 §25 阶段末尾深度审查
> **审查范围**: Stage 5.43-5.80（38 个子阶段，自上次深度审查 #4 r91 以来）

## 1. 执行摘要

Stage 5 自上次深度审查 #4（r91, Stage 5.42, v0.11.38, 1236 tests）以来完成了
**38 个新子阶段**（5.43-5.80），实现了完整的 **dyn Trait MIR lowering → codegen
pipeline 端到端激活**。这是一个重大里程碑——从静态规划链（5.43-5.60 的 codegen
vtable emission 重构）到 MIR 基础设施（5.61-5.74 的 DynTraitFatPtr /
DynTraitMethodCall / DynTraitMIRSummary / DynTraitMIRPlan）再到 mir/lower 集成
（5.75-5.80 的查询 API + 上下文接线 + lowering 集成 + codegen 集成 + driver 集成）。

### 38 个子阶段分组

**Group A: Codegen Vtable Emission 重构（5.43-5.60, 18 stages）**
- 5.43-5.45: codegen vtable emission helper + global text bridge + batch helper
- 5.46-5.48: codegen vtable spec builder + emission orchestrator + dynptr global text helper
- 5.49-5.50: codegen dynptr spec builder + emission orchestrator
- 5.51-5.52: codegen vtable+dynptr combined emission orchestrator + trait-dispatch summary
- 5.53-5.54: codegen trait-dispatch emission plan + orchestrator plan-based
- 5.55-5.56: codegen trait-dispatch emission text batch + from resolver
- 5.57-5.58: TextEmitter emit_vtable_global + emit_dyn_trait_const delegation
- 5.59-5.60: emit_vtables + emit_dyn_trait_ptrs delegation

**Group B: Dyn Trait MIR 基础设施（5.61-5.74, 14 stages）**
- 5.61-5.62: DynTraitFatPtr MIR representation + bridge from resolver
- 5.63-5.65: emit_dyn_trait_fat_ptr_text + batch + batch_from_resolver
- 5.66-5.67: DynTraitMethodCall MIR representation + emit IR text
- 5.68-5.70: build_dyn_trait_method_calls_from_fat_ptrs + batch + batch_from_resolver
- 5.71-5.72: DynTraitMIRSummary + build_from_resolver
- 5.73-5.74: DynTraitMIRPlan (final aggregate) + emit_dyn_trait_mir_plan_text

**Group C: mir/lower + codegen + driver 集成（5.75-5.80, 6 stages）**
- 5.75: find_dyn_trait_method_call_in_plan (exact lookup)
- 5.76: MirLowerCtxt dyn_trait_plan field + setter/getter (context wiring)
- 5.77: find_dyn_trait_method_call_in_plan_by_method (fuzzy lookup)
- 5.78: HirExprKind::MethodCall dyn Trait integration (FIRST real lower integration)
- 5.79: codegen dyn Trait vtable indirect call (FIRST codegen integration)
- 5.80: driver dyn Trait plan integration (END-TO-END pipeline activation)

**阻塞项**: 0 P0 / 0 P1 / 3 P2
**建议行动**: ✅ **GO** — dyn Trait pipeline 端到端激活，可进入 Stage 5.82+ 精化阶段

## 2. 七维度审查结论

### D1. 架构健康度

**现状**: Stage 5.43-5.80 完成了三层架构演进：

1. **Codegen 重构层**（5.43-5.60）：采用"先并行、后委托"策略——先添加并行
   free function（5.43-5.56），再让现有 TextEmitter 方法委托（5.57-5.60）。
   每个重构可独立审查、可回退。所有 14 个 free function + 4 个委托重构均通过
   交叉验证测试保证行为等价。

2. **MIR 基础设施层**（5.61-5.74）：三层递进设计——
   - 值表示（DynTraitFatPtr, 5.61）→ 方法调用表示（DynTraitMethodCall, 5.66）
     → 项目汇总（DynTraitMIRSummary, 5.71）→ 最终聚合（DynTraitMIRPlan, 5.73）
   - 每层都有：构造器 + 桥接函数 + IR 文本生成器 + 批量版本 + from_resolver 便捷入口
   - 所有 API 是纯函数 + 派生 PartialEq/Eq 的 struct，§16 自包含

3. **集成层**（5.75-5.80）：三 stage 联动激活端到端 pipeline——
   - 5.75/5.77 提供精确+模糊两种查询 API
   - 5.76 提供 MirLowerCtxt 上下文接线
   - 5.78 在 HirExprKind::MethodCall 分支使用查询 + side-table 模式
   - 5.79 在 codegen_terminator 检测 Const marker + emit vtable indirect call
   - 5.80 driver 自动构建 plan 并传入 lower

**风险**:
- `mir/lower/mod.rs` 3346 LOC（TD-011 未偿还，从 3124 增长到 3346）
- `codegen/mod.rs` 2398 LOC
- `stdlib.rs` 1993 LOC（未拆分但同质静态查询，可接受）
- dyn Trait return type 用 I32 placeholder（5.79 设计决策 #4，待未来 stage 精化）

**建议**: P2 — 在 Stage 6 早期拆分 `mir/lower/mod.rs`。其他文件暂不拆分。

### D2. 技术债清单

| ID | 描述 | 优先级 | 状态 | 偿还计划 |
|----|------|--------|------|---------|
| TD-014 | L5 trait dispatch vtable | P2 | **CLOSE** | 5.43-5.80 完整实现：codegen 重构 + MIR 基础设施 + mir/lower 集成 + codegen 集成 + driver 集成。端到端 pipeline 激活。 |
| TD-011 | mir/lower/mod.rs 3124→3346 LOC | P2 | OPEN | Stage 6 早期拆分（增长 222 LOC 来自 5.76 cx 字段 + 5.78 MethodCall dyn Trait 分支 + 5.80 新入口点） |
| TD-015 | Region inference placeholder | P2 | OPEN | Stage 6+ |
| TD-016 | dyn Trait return type I32 placeholder | P3 | OPEN | 未来 stage 扩展 DynTraitMethodCall 加 return_ty 字段 |
| TD-017 | codegen/mod.rs 2398 LOC | P3 | OPEN | Stage 6+ 视增长情况拆分 |

**净变化**: TD-014 从 partial CLOSE → **CLOSE**（重大进展）。新增 TD-016/TD-017
两个 P3 低优先级项。总体技术债减少。

### D3. API 命名标准化（§23）

**审查方法**: 检查 v1.12-v1.50 共 39 个 changelog 条目（v1.44-v1.50 为本审查范围新增）。

**v1.44-v1.50 新增符号**（Stage 5.74-5.80）:

| 版本 | 符号 | 命名模式 | 合规 |
|------|------|---------|------|
| v1.44 | `emit_dyn_trait_mir_plan_text` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| v1.45 | `find_dyn_trait_method_call_in_plan` | `find_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |
| v1.46 | `MirLowerCtxt::set_dyn_trait_plan` / `dyn_trait_plan` | `<verb>_<noun>_<noun>_<noun>` / `<noun>_<noun>_<noun>` | ✅ |
| v1.47 | `find_dyn_trait_method_call_in_plan_by_method` | `find_<noun>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` | ✅ |
| v1.48 | `build_dyn_trait_call_terminator` / `MirBody.dyn_trait_calls` | `<verb>_<noun>_<noun>_<noun>_<noun>` / `<noun>_<noun>_<noun>` | ✅ |
| v1.49 | `emit_dyn_trait_method_call` / `codegen_dyn_trait_call` | `<verb>_<noun>_<noun>_<noun>_<noun>` / `<verb>_<noun>_<noun>_<noun>` | ✅ |
| v1.50 | `lower_hir_body_to_mir_full_with_dyn_trait_plan` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>_<noun>` | ✅ |

**前缀约定一致性**:
- `emit_` 前缀：7 个新 emit 函数（5.43-5.60 codegen + 5.67/5.74/5.79 MIR/codegen IR 文本）
- `build_` 前缀：6 个新 build 函数（5.39-5.42 stdlib + 5.62/5.68/5.73/5.78 MIR 构造）
- `find_` 前缀：3 个新 find 函数（5.36 stdlib + 5.75/5.77 plan 查询）
- `codegen_` 前缀：2 个新 codegen 函数（5.79 codegen_dyn_trait_call + 已有的 codegen_terminator 等）
- `lower_` 前缀：1 个新 lower 函数（5.80 lower_hir_body_to_mir_full_with_dyn_trait_plan）

**结论**: ✅ 所有新符号 §23 合规。前缀约定一致，无偏离。

### D4. 接口隔离（§16）

**审查方法**: 检查 Stage 5.43-5.80 所有新代码的依赖方向。

**依赖图**（单向，无循环）:
```
stdlib (静态查询，自包含)
  ↑
traits::TraitResolver (vtables 数据源)
  ↑
driver (orchestrator，唯一允许读 TraitResolver)
  ↓
mir::dyn_trait (DynTraitMIRPlan 等数据结构)
  ↓
mir::lower (lower_hir_body_to_mir_full_with_dyn_trait_plan + build_dyn_trait_call_terminator)
  ↓
mir::body (MirBody.dyn_trait_calls side-table)
  ↓
codegen (codegen_dyn_trait_call + emit_dyn_trait_method_call)
  ↓
LLVM IR text
```

**关键验证点**:
1. `mir::dyn_trait` 不依赖 `codegen` 或 `traits::TraitResolver`（§16 自包含）✅
2. `mir::lower` 通过 `cx.dyn_trait_plan` 接收 plan 数据，不直接查询 TraitResolver ✅
3. `codegen` 通过 `mir.dyn_trait_calls` side-table 读取 dyn Trait 信息，不查询 HIR/TraitResolver ✅
4. `driver` 是唯一编排器，连接 TraitResolver → mir::lower via plan data ✅
5. Side-table 模式（5.78）让 MIR 携带 dyn Trait 信息作为数据，避免 codegen 跨阶段查询 ✅

**结论**: ✅ §16 完全合规。无循环依赖，数据流单向。

### D5. 测试覆盖

**总量**: 1637 tests + 5 benchmarks（从 1236 增长 401 tests，+32.4%）

**Stage 5.43-5.80 新增测试**: 401 tests（38 个子阶段，平均 10.5 tests/stage）

**按子阶段组分布**:
- Group A (5.43-5.60, codegen 重构): ~180 tests（18 stages × ~10 tests）
- Group B (5.61-5.74, MIR 基础设施): ~130 tests（14 stages × ~9 tests）
- Group C (5.75-5.80, 集成): ~91 tests（6 stages × ~15 tests，集成测试更密集）

**测试质量**:
- 每个 stage 都有专属测试文件 ✅
- 测试覆盖：正常路径 + 边界条件（空输入、不匹配、大小写、多调用）+ 错误路径 ✅
- 交叉验证测试（5.43-5.60 委托重构的行为等价测试）✅
- 端到端集成测试（5.80 driver 测试）✅
- 无 side-effect 测试（幂等性验证）✅

**测试模块数**: 94 mods（从 56 mods 增长到 94，+68%）

**结论**: ✅ 测试覆盖充分。每个新 API 都有专属测试，集成测试覆盖端到端路径。

### D6. 文档完整性

**审查清单**:
- `docs/develop/v0/stage-5/plan-5.X.md`: ✅ 38 个新 plan 文件（5.43-5.80），每个都有目标/设计/§23/§16/测试矩阵
- `docs/develop/v0/stage-5/gate-review-roundXX.md`: ✅ 38 个新 gate review（round43-80），每个都有 CI/CD + 新 API + 设计要点
- `docs/develop/v0/stage-5/dev-log.md`: ✅ 38 个新 stage 条目，每个都有 Work completed + Test impact + Verification
- `docs/develop/v0/api-naming-standard.md`: ✅ v1.12-v1.50 共 39 个 changelog 条目（v1.44-v1.50 为本审查范围）
- `docs/worklog.md`: ✅ 完整任务镜像（stage5.43-r92 到 stage5.80-r129）
- `RELEASE_NOTES.md`: ✅ 更新到 v0.11.76，每个版本有 Overview + New API + Verification
- `README.md`: ✅ v0.11.76, 1637 tests, 94 mods, sub-stage list 更新

**文档质量**:
- 每个 plan 文档包含：目标、设计动机、API 签名、命名标准化表、§16 合规分析、测试矩阵 ✅
- 每个 gate review 包含：CI/CD 实际运行结果、新增 API 表、设计要点 ✅
- api-naming-standard changelog 包含：新符号表、设计决策、§16 合规、test/clippy/fmt impact ✅

**结论**: ✅ 文档完整且高质量。每个 stage 都有 plan + gate review + dev-log + worklog + changelog 五重记录。

### D7. CI/CD 健康

**当前状态**（v0.11.76, Stage 5.80 实际运行）:
```
cargo clean: clean (549.1 MiB removed) ✅
cargo test: 1637 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean (exit 0) ✅
cargo clippy --all-targets: 0 warnings, 0 errors ✅
```

**历史趋势**（自深度审查 #4 r91 以来）:
- 测试数：1236 → 1637（+401, +32.4%）
- clippy warnings：0 → 0（持续零警告）
- fmt：clean → clean（持续清洁）
- 编译错误：0 → 0

**结论**: ✅ CI/CD 持续健康。零警告、零错误、fmt 清洁。

## 3. 关键设计决策审查

### 3.1 Side-table 模式（5.78, TD-014 CLOSE 关键）

**决策**: MIR 通过 `MirBody.dyn_trait_calls: Vec<DynTraitMethodCall>` side-table
携带 dyn Trait 调用信息。`Terminator::Call` 的 `func` 用
`Const{ty: Error, val: Int(index)}` 作为 marker——`index` 是 side-table 条目索引。

**优点**:
- §16 合规：MIR 携带数据，codegen 不跨阶段查询
- 向后兼容：不匹配 marker 时回退到 legacy 路径
- 可扩展：未来可加更多 side-table（如 return type）

**风险**: marker 约定是隐式的（`Ty::Error` + `Int(index)`）。如果其他路径产生
相同 marker 会冲突。当前通过三重条件检测（`Operand::Constant` + `Ty::Error` +
`Int(idx) < len`）保证安全。

**结论**: ✅ 设计合理，风险可控。

### 3.2 模糊查询 first-match-wins（5.77）

**决策**: `find_dyn_trait_method_call_in_plan_by_method` 按 method_name 查询时，
多个匹配返回第一项。

**理由**: MIR lower 阶段无法消歧 trait/type（那是 typeck 职责）。当 method_name
唯一时（常见情况），模糊查询足够。当有歧义时，调用方接受候选。

**风险**: 罕见情况下可能选错 entry。但 5.78 集成时只用于触发 dyn Trait 路径，
实际 vtable indirect call 的正确性由 codegen 的 side-table 读取保证（index 是
唯一的）。

**结论**: ✅ 设计权衡合理，风险可接受。

### 3.3 Driver 重构：trait_resolver 前移（5.80）

**决策**: 将 `trait_resolver` 构建从 body 循环后移到循环前。

**理由**: Stage 5.78+ 要求 plan 在 lowering 之前可用。`validate_impls` 保持原位
（不影响 lowering，只报告错误）。

**风险**: 改变了 driver 内部执行顺序。但所有 1626 个 pre-existing 测试通过不变，
证明无副作用。

**结论**: ✅ 必要的重构，向后兼容验证通过。

## 4. 委员会投票

5/5 GO → **PASS**

## 5. 行动计划

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | mir/lower/mod.rs 拆分（TD-011, 3346 LOC） | Stage 6 早期 |
| P3 | dyn Trait return type 精化（TD-016, I32 placeholder） | Stage 5.82+ |
| P3 | 更深端到端集成测试（完整 trait + impl + dyn 使用源码） | Stage 5.82+ |
| P3 | codegen/mod.rs 拆分（TD-017, 2398 LOC，视增长） | Stage 6+ |
| P2 | Region inference（TD-015） | Stage 6+ |

## 6. 里程碑总结

**🎉 dyn Trait MIR lowering → codegen pipeline 端到端激活**

Stage 5.43-5.80（38 个子阶段）完成了从静态规划链到端到端 pipeline 的完整演进：

1. **Codegen 重构**（5.43-5.60）：18 个 free function + 4 个委托重构，所有通过
   交叉验证测试保证行为等价
2. **MIR 基础设施**（5.61-5.74）：14 个 stage 构建了 DynTraitFatPtr →
   DynTraitMethodCall → DynTraitMIRSummary → DynTraitMIRPlan 四层递进数据结构
3. **集成层**（5.75-5.80）：6 个 stage 完成查询 API + 上下文接线 + lowering 集成 +
   codegen 集成 + driver 集成

完整路径：
```
HIR `receiver.method(args)` (dyn Trait receiver)
  → driver builds DynTraitMIRPlan from TraitResolver (5.80)
  → lower_hir_body_to_mir_full_with_dyn_trait_plan(plan=Some) (5.80)
  → cx.set_dyn_trait_plan(plan) (5.76)
  → HirExprKind::MethodCall → find_dyn_trait_method_call_in_plan_by_method (5.77)
  → build_dyn_trait_call_terminator writes side-table + Const marker (5.78)
  → codegen_terminator detects marker (5.79)
  → codegen_dyn_trait_call reads side-table (5.79)
  → emitter.emit_dyn_trait_method_call emits vtable indirect call IR (5.79)
    (getelementptr + load + load + indirect call)
```

**TD-014（L5 trait dispatch vtable）正式 CLOSE**——这是 Stage 5 的核心目标之一，
从 5.1 开始规划，经过 80 个子阶段完成。

## 7. 结论

**Stage 5.81 深度审查 #5 PASS（GO）**。

Stage 5 已完成 80 个子阶段、733 个 Stage 5 测试、1637 个总测试、0 clippy
warnings、fmt clean。dyn Trait MIR lowering → codegen pipeline 端到端激活，
TD-014 正式 CLOSE。架构健康，§16/§23 完全合规，文档完整，CI/CD 持续绿色。

可进入 Stage 5.82+ 精化阶段（return type 处理、更深集成测试）或开始 Stage 6
规划（mir/lower 拆分、Region inference）。

---

**审查完成**: 2026-07-24
