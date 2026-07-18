# 03 — 协作流程详解

> 本文档定义 Agent 团队在各个项目阶段的具体协作流程。

---

## 1. 需求接收流程

```
用户需求 → PM-A 接收
→ REC-A 记录需求（更新 00-requirement-history.md）
→ PM-A + PL-A 评估需求类型与优先级
→ PL-A 分解为任务
→ PM-C 分派给相应 Agent Group
→ REC-C 记录分派
```

### 1.1 需求类型与分派

| 需求类型 | 主导 Agent | 协作 Agent | 决策等级 |
| --- | --- | --- | --- |
| 语言特性变更 | ARCH-A + ALG-A | DEV-A/B + QA-A + REV-B | L3-L4 |
| 架构变更 | ARCH-A | ALG-A/B/C + DEV-A/B/C + REV-A/B | L3 |
| Bug 修复 | DEV-A/B/C | QA-A + REV-A | L0-L1 |
| 测试补充 | QA-A/B/C | DEV-A/B/C + REV-A | L1 |
| 文档更新 | REC-B/C | REV-C + 相关 Agent | L0-L1 |
| 命名/元信息 | PM-A | ARCH-A + REV-C + REC-B | L3-L4 |
| 审查请求 | REV-A/B/C | 相关 Agent | L1-L2 |

---

## 2. 设计流程

```
PM-A 提出设计需求
→ ARCH-A 主导架构设计
→ ALG-A/B/C 并行设计算法
  ├─ ALG-A：类型系统
  ├─ ALG-B：借用检查
  └─ ALG-C：优化与 codegen
→ REV-B 审查设计（soundness + rustc 对照）
→ ARCH-A 修改 → REV-B 再审
→ PM-A 确认 → REC-A 记录决策
→ PL-A 分解为实现任务
```

### 2.1 设计文档要求

- 必须含伪代码或 Rust struct 定义
- 必须引用 rustc 源码出处
- 必须通过 REV-B soundness 审查
- 必须更新 18-glossary.md 术语表

---

## 3. 实现流程

```
PL-A 分解任务 → PL-B 排期 → PL-C 跟踪
→ DEV-A/B/C 实现
  ├─ DEV-A：前端（Lexer/Parser/AST/HIR）
  ├─ DEV-B：中端（Typeck/Trait/MIR/Borrowck）
  └─ DEV-C：后端（Codegen/Stdlib/Linker）
→ QA-A 编写 conformance 测试
→ QA-B 编写 fuzzing/soundness 测试
→ REV-A 代码审查
→ QA-C CI 集成测试
→ REC-C 记录工作日志
```

### 3.1 PR 流程

```
DEV 完成 → 创建 PR
→ REV-A+B 并行审查（代码质量 + 设计一致性）
→ QA-A+C 并行验证（conformance + CI）
→ PM-C 合并 PR
→ REC-B 记录变更
```

```

### 3.2 PR 验收标准
- [ ] conformance 测试 100% 通过
- [ ] 单元测试覆盖率 ≥ 85%
- [ ] REV-A 代码审查通过
- [ ] REV-B 设计一致性确认
- [ ] 无 P0 bug
- [ ] worklog.md 已更新

---

## 4. 审查流程

```

REV-A/B/C 接收审查任务
→ REV-A：代码审查（质量/bug/风格）
→ REV-B：设计审查（soundness/rustc 对照）
→ REV-C：文档审查（同步性/一致性/完备性）
→ 汇总审查报告
→ 分派修复任务
→ 修复后再审
→ 通过 → REC-A 记录

```

### 4.1 审查类型

| 审查类型 | 审查 Agent | 频率 | 检查项 |
|---|---|---|---|
| 代码审查 | REV-A | 每个 PR | 代码质量、bug、风格、性能 |
| 设计审查 | REV-B | 每个设计变更 | soundness、rustc 一致性、算法正确性 |
| 文档审查 | REV-C | 每个版本 | 同步性、术语一致、版本号、完备性 |
| 安全审查 | REV-B + ALG-A | 每个里程碑 | soundness 漏洞、unsafe 边界 |
| 架构审查 | ARCH-B | 每个里程碑 | 架构一致性、技术债 |

---

## 5. 决策流程

### 5.1 L0 决策（单 Agent）
```

Agent 自主决策 → REC-C 记录 → 继续

```

### 5.2 L1 决策（同类共识）
```

Agent 提案 → 同类 Agent 讨论 → 共识 → REC-A 记录

```

### 5.3 L2 决策（跨类协商）
```

Agent 提案 → 相关类 Agent 讨论 → ARCH-A 仲裁 → REC-A 记录

```

### 5.4 L3 决策（重大设计）
```

ARCH-A 提案 → PM-A + ARCH-A + ALG-A 讨论
→ REV-B 评估 soundness → PM-A 决策
→ REC-A 记录 → PL-A 分解任务

```

### 5.5 L4 决策（项目级）
```

PM-A 提案 → 全体投票（2/3 多数）
→ PM-A 终裁（含否决权）→ REC-A 记录
→ REC-B 更新版本历史

```

---

## 6. 风险管理流程

### 6.1 风险识别
```

任何 Agent 发现风险 → 填写风险报告
→ PM-B 评估等级（R0-R3）
→ REC-A 记录到风险登记册

```

### 6.2 风险响应

| 等级 | 响应时间 | 响应流程 |
|---|---|---|
| R0 | 立即 | PM-A 召集全员 → 停工 → 紧急方案 → REC-A 记录 |
| R1 | 24h | PM-B 制定缓解方案 → ARCH-A 审查 → 执行 |
| R2 | 7d | 相关 Agent 制定方案 → PM-B 审查 → 排期执行 |
| R3 | 滚动 | 记录到 backlog → 下个迭代处理 |

### 6.3 风险报告格式
```markdown
---
风险 ID: RISK-<编号>
报告 Agent: <Agent 名称>
等级: R0/R1/R2/R3
描述: <风险描述>
影响: <影响范围>
概率: <高/中/低>
缓解方案: <方案>
状态: 待处理/处理中/已解决
```

---

## 7. 版本发布流程

```
PL-C 确认所有里程碑任务完成
→ QA-A 确认 conformance 100% 通过
→ QA-B 确认 soundness 测试通过
→ QA-C 确认 CI 全平台通过
→ REV-A/B/C 全面审查
→ PM-A 批准发布
→ REC-B 更新版本历史 + CHANGELOG
→ REC-C 归档工作日志
→ PM-A 发布
```

---

## 8. 知识管理流程

```
Agent 完成任务 → REC-C 收集 worklog
→ REC-C 整理经验教训
→ SKL-A 提取 skill 模板
→ SKL-B 评估 skill 质量
→ REC-C 归档到知识库
→ 18-glossary.md 更新术语
```

---

**下一文档**: [`04-agent-skills.md`](./04-agent-skills.md) — Agent Skill 定义与管理
