# Landin Agent Group — 团队管理与协作文档集 v1.1

> 本文档集定义 Landin 语言项目的 Agent Group 组织架构、协作流程、决策机制、风险管理、运行时协议、生命周期管理、现代化路线图。

## 文档集（v1.1，共 12 个文档）

### 原始文档（v1.0，8 个）

| # | 文档 | 内容 |
| --- | --- | --- |
| 00 | [00-requirement-history.md](./00-requirement-history.md) | 需求历程与变更记录（16 个需求 + 版本演化 + 命名决策记录） |
| 01 | [01-agent-team-overview.md](./01-agent-team-overview.md) | Agent 团队总览（9 类角色 + 协作模式 + 决策机制 + 风险评估） |
| 02 | [02-agent-roles-detail.md](./02-agent-roles-detail.md) | 25 个 Agent 角色详细定义 |
| 03 | [03-collaboration-workflow.md](./03-collaboration-workflow.md) | 协作流程详解（8 大流程） |
| 04 | [04-agent-skills.md](./04-agent-skills.md) | 25 个 Agent Skill 定义 + prompt 模板 + 评估标准 |
| 05 | [05-meeting-and-decision-log.md](./05-meeting-and-decision-log.md) | 11 个历史决策记录 |
| 06 | [06-risk-register.md](./06-risk-register.md) | 11 个风险 + 缓解方案 |
| 07 | [07-team-charter.md](./07-team-charter.md) | 团队章程（愿景/价值观/准则/质量标准/里程碑） |

### v1.1 新增文档（3 个，A1/A2 审查建议）

| # | 文档 | 内容 |
| --- | --- | --- |
| **08** | **[08-agent-lifecycle.md](./08-agent-lifecycle.md)** | **Agent 生命周期（创建/激活/休眠/销毁）+ 状态机 + 自治边界（A0-A4）+ 故障恢复 + 安全权限矩阵 + 审计追踪** |
| **09** | **[09-runtime-protocol.md](./09-runtime-protocol.md)** | **结构化消息协议（JSON）+ 事件驱动机制（Pub/Sub）+ 任务队列与调度（优先级/抢占/超时/重试）+ 冲突解决 + HITL + KPI（25 个 Agent 性能指标）** |
| **10** | **[10-modernization-roadmap.md](./10-modernization-roadmap.md)** | **现代化路线图（v1.1→v2.0→v3.0→v4.0 四阶段，从 30%→95% 达成度）+ 框架选型（LangGraph+MCP+MetaGPT）+ 优先级矩阵** |

### 元文档

| - | README.md（本文档） | 文档集入口 |

## v1.1 改进（相比 v1.0）

### 新增 3 个关键文档

1. **08-agent-lifecycle.md**：解决 A1 P0-1/P0-2/P0-7/P0-8（生命周期/状态机/故障恢复/安全权限完全缺失）
2. **09-runtime-protocol.md**：解决 A1 P0-3/P0-4/P0-5/P0-6（任务队列/消息协议/冲突解决/KPI 完全缺失）
3. **10-modernization-roadmap.md**：定义从 30%→95% 的现代化路径

### 修复 20 项 P0 文档不一致

- 04 §11 skill 版本表补全 25 个 skill（原仅 2 行 + "..."）
- 01 架构图补 ALG-C / DEV-C
- 01 §2 Agent 数量明确（不再用"2-3"范围）
- 03 PR 流程从 7 步简化为 4 步（REV+QA 并行）
- 05 DEC-011 / 07 §8 文档计数修正
- 00 时间线标注"模拟时间"

## Agent Group 成员总览

| 类别 | 成员 | 数量 |
| --- | --- | --- |
| 项目管理 | PM-A / PM-B / PM-C | 3 |
| 任务排期 | PL-A / PL-B / PL-C | 3 |
| 记录 | REC-A / REC-B / REC-C | 3 |
| 架构 | ARCH-A / ARCH-B | 2 |
| 算法 | ALG-A / ALG-B / ALG-C | 3 |
| 开发 | DEV-A / DEV-B / DEV-C | 3 |
| 测试 | QA-A / QA-B / QA-C | 3 |
| 审查 | REV-A / REV-B / REV-C | 3 |
| Skill 管理 | SKL-A / SKL-B | 2 |
| **合计** | **25 个 Agent** | **25** |

## v1.1 综合达成度

| 维度 | v1.0 | **v1.1** | 改善 |
| --- | --- | --- | --- |
| 角色设计 | 35% | **50%** | +15%（补全自治边界/生命周期） |
| 协作模式 | 30% | **50%** | +20%（补全消息协议/事件驱动） |
| 决策机制 | 40% | **55%** | +15%（补全冲突解决/HITL） |
| Skill 管理 | 25% | **40%** | +15%（补全 25 skill 版本表） |
| 记忆与状态 | 20% | **45%** | +25%（补全状态机/checkpoint/KPI） |
| 现代化特性覆盖 | 30% | **50%** | +20%（补全 12 项运行时要素） |
| 文档自洽性 | 55% | **85%** | +30%（修复 20 P0） |
| **综合** | **~35%** | **~50%** | **+15%** |

## 现代化路线图

| 版本 | 时间 | 目标 | 达成度 |
| --- | --- | --- | --- |
| **v1.1** | **已完成** | 自洽版（文档修复 + 运行时概念定义） | ~50% |
| v2.0 | 1-2 月 | 可运行版（消息协议 + 状态机 + 事件驱动实现） | ~65% |
| v3.0 | 3-6 月 | 2024 平均水平（向量记忆 + 动态角色 + tracing） | ~80% |
| v4.0 | 6-12 月 | 2026 最佳实践（MCP + 共识 + DSPy + time travel） | ~95% |

## 协作机制

**4 种协作模式**：串行 / 并行 / 迭代 / 升级

**5 级决策**：L0 单 Agent → L1 同类共识 → L2 跨类协商 → L3 PM+ARCH+ALG → L4 全体投票+PM 终裁

**4 级风险**：R0 立即停工 → R1 24h → R2 7d → R3 滚动

**5 级自治**：A0 完全自主 → A1 自主+通知 → A2 自主+确认 → A3 提案+审批 → A4 全人工

**运行时要素**（v1.1 新增）：

- Agent 生命周期管理（created→onboarding→idle→working→blocked→dormant→offline→destroyed）
- 结构化消息协议（JSON schema + 13 种消息类型 + ACK + 重试）
- 事件驱动（12 种事件 + Pub/Sub 订阅）
- 任务队列（4 级优先级 + 抢占 + 超时升级 + 负载均衡）
- 冲突解决（5 种冲突类型 + 仲裁流程 + 申诉机制）
- HITL 中断（6 种触发条件 + 48h 超时升级）
- KPI（8 通用 + 8 类别专属 + 4 评估周期）
- 安全权限矩阵（9 类 × 12 操作）
- 审计追踪（JSONL append-only）

---

*本文档集 v1.1 与 lang-design 文档集配合使用，共同构成 Landin 项目的完整管理基础。*
