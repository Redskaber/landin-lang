# 10 — 现代化路线图

> 本文档定义 Agent 团队文档集从 v1.0（传统组织蓝图）→ v2.0（可运行版）→ v3.0（2024 平均水平）→ v4.0（2026 最佳实践）的现代化路线。v1.1 新增（A1/A2 审查建议）。

---

## 1. 当前状态评估（A1/A2 审查结论）

| 维度 | 达成度（vs 业界平均） | 关键差距 |
| --- | --- | --- |
| 角色设计 | 35% | 角色过细 / 无自治边界 / 无动态切换 |
| 协作模式 | 30% | 概念图非可执行图 / 轮询驱动 / 无消息协议 |
| 决策机制 | 40% | L0-L4 是组织等级非算法 / 无共识算法 |
| Skill 管理 | 25% | prompt 模板非可执行 / 无组合 / 无评估 |
| 记忆与状态 | 20% | 单层 markdown / 无向量 / 无 checkpoint |
| 现代化特性覆盖 | 30% | 25 项标配特性缺失 12+ 项 |
| **综合** | **≈ 30-42%** | 传统组织蓝图达标，AI Agent 运行时规范严重不足 |

---

## 2. 现代化路线图

### Phase 1: v1.1 自洽版（已完成，本次迭代）

**目标**：修复文档自身不一致 + 补全运行时要素

**交付**：

- ✅ 新增 08-agent-lifecycle.md（生命周期 + 状态机 + 自治边界 + 故障恢复 + 安全权限）
- ✅ 新增 09-runtime-protocol.md（消息协议 + 事件驱动 + 任务队列 + 冲突解决 + HITL + KPI）
- ✅ 新增 10-modernization-roadmap.md（本文档）
- ✅ 修复 20 项 P0 文档不一致（计数/架构图/版本表/PR 流程）

**达成度**：≈ 50%（补全了运行时概念定义，但仍为文档规范非可执行代码）

---

### Phase 2: v2.0 可运行版（1-2 月）

**目标**：将文档规范转化为可执行的 Agent 运行时

**交付**：

1. **结构化消息协议实现**
   - worklog.md 升级为 JSON 消息队列（SQLite/文件）
   - 实现 09-runtime-protocol.md §1 消息格式
   - 支持消息路由 + ACK + 重试

2. **Agent 状态机实现**
   - 实现 08-agent-lifecycle.md §2 状态转换
   - 支持 checkpoint 保存/恢复
   - 支持 dormant 唤醒

3. **事件驱动引擎**
   - 实现 09-runtime-protocol.md §2 事件订阅
   - 替代 worklog 轮询
   - 支持 Pub/Sub 模式

4. **Skill 可执行化**
   - 04-agent-skills.md prompt 模板转为可执行函数
   - 添加 JSON Schema 参数定义
   - 支持 skill 注册中心

5. **任务调度器**
   - 实现 09-runtime-protocol.md §3 任务队列
   - 支持优先级抢占 + 超时升级 + 负载均衡
   - 支持依赖管理

6. **HITL 中断机制**
   - 实现 09-runtime-protocol.md §5 HITL 流程
   - 支持 L3-L4 决策暂停

**达成度目标**：≈ 65%（可运行但无向量记忆/共识算法/MCP 对齐）

---

### Phase 3: v3.0 2024 平均水平（3-6 月）

**目标**：对齐 2024 年主流 Agent 框架（MetaGPT/CrewAI/AutoGen）

**交付**：

1. **向量记忆 / RAG**
   - worklog + 决策库 + 风险登记册索引到向量库
   - 支持语义检索（"上次类似问题怎么解决的？"）
   - 三层记忆：短期（State）+ 中期（Checkpoint）+ 长期（向量 Store）

2. **动态角色切换**
   - 基于 Swarm handoff 模式
   - Agent 运行时切换角色（如 DEV-A 临时支援 REV-A）
   - handoff 函数定义

3. **Tracing / 可观测性**
   - 每次 Agent 调用 trace（latency / token / cost / success）
   - 可视化仪表盘
   - 性能瓶颈定位

4. **Skill A/B 测试 + 评估**
   - SKL-B 实现 skill A/B 测试框架
   - 04 §11 版本表全部评估
   - 质量评分 < 7/10 的 skill 自动优化

5. **代码执行沙箱**
   - DEV/QA Agent 通过 Docker 执行代码验证
   - 安全隔离
   - 执行结果自动反馈

6. **子 Agent / 分层团队**
   - 25 平行 Agent 重组为 9 类 hierarchical team
   - 每队有 supervisor Agent
   - 复杂任务分解为子任务

**达成度目标**：≈ 80%（2024 主流水平，但无 MCP/共识算法/time travel）

---

### Phase 4: v4.0 2026 最佳实践（6-12 月）

**目标**：对齐 2026 年前沿（LangGraph + MCP + DSPy）

**交付**：

1. **MCP 标准化对齐**
   - Skill 注册为 MCP server
   - 跨框架复用（可被 LangChain/AutoGen/CrewAI 调用）
   - MCP permissions 模型

2. **共识算法**
   - L4 决策采用 Raft（小规模）
   - L3 决策采用 QUORUM
   - 冲突解决形式化

3. **自动 Skill 优化（DSPy 风格）**
   - 自动"编译"prompt 模板
   - Assertion-based 评估
   - 质量自动提升

4. **时间旅行调试**
   - 基于 LangGraph time travel
   - 可回到任意历史状态
   - 调试 Agent 决策链

5. **多模态 I/O**
   - 支持代码 AST 视觉
   - 支持调用图/架构图
   - 支持设计文档图表

6. **流式输出**
   - Agent 输出 streaming
   - 长任务实时反馈
   - UX 提升

7. **跨会话用户偏好**
   - 用户上下文持久化
   - 个性化 Agent 行为

8. **异步运行时**
   - 全异步 Agent 执行
   - 高并发

**达成度目标**：≈ 95%（2026 最佳实践前沿）

---

## 3. 框架选型建议

| Landin 需求 | 推荐借鉴框架 | 理由 |
| --- | --- | --- |
| SOP 驱动的研发流程 | MetaGPT | SOP 标准化 + 结构化输出 |
| 角色定义 + 委派 | CrewAI | role/goal/backstory + delegation |
| 多 Agent 对话 + 代码执行 | AutoGen 0.4 | 异步事件驱动 + Docker 沙箱 |
| 状态机 + checkpoint | LangGraph | 图引擎 + 持久化 + HITL 最成熟 |
| 轻量 handoff 路由 | OpenAI Swarm | 极简，适合动态角色切换 |
| 软件开发流水线 | ChatDev | ChatChain + 角色扮演最贴合编译器项目 |
| 自主规划 + 向量记忆 | AutoGPT | 自主循环 + Pinecone 长期记忆 |
| Skill 标准化 | MCP | 跨框架行业标准 |

**推荐主框架**：LangGraph（状态机 + checkpoint + HITL 最成熟）+ MCP（skill 标准化）+ MetaGPT（SOP 流程）

---

## 4. 优先级矩阵

| 改进项 | 价值 | 成本 | 优先级 |
| --- | --- | --- | --- |
| 结构化消息协议 | 高 | 中 | P0 |
| Agent 状态机 | 高 | 中 | P0 |
| 事件驱动 | 高 | 高 | P1 |
| Skill 可执行化 | 高 | 中 | P0 |
| 向量记忆 | 中 | 高 | P1 |
| HITL 中断 | 中 | 低 | P1 |
| 动态角色切换 | 中 | 高 | P2 |
| MCP 对齐 | 中 | 高 | P2 |
| 共识算法 | 低 | 高 | P3 |
| 时间旅行 | 低 | 高 | P3 |

---

## 5. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| 过度现代化拖延实现 | 高 | 高 | v2.0 即可启动 stage 0 实现；现代化与实现并行 |
| 框架依赖锁定 | 中 | 中 | MCP 标准化降低锁定；保留退出策略 |
| Agent 复杂度膨胀 | 高 | 中 | 25 Agent 已是上限；新增需求通过 skill 组合而非新增 Agent |
| 运行时性能开销 | 中 | 中 | checkpoint 频率可调；事件总线轻量实现 |

---

## 6. 版本规划

| 版本 | 时间 | 目标 | 对应 Phase |
| --- | --- | --- | --- |
| **v1.1** | **已完成** | 自洽版（文档修复 + 运行时概念定义） | Phase 1 |
| v2.0 | 1-2 月 | 可运行版（消息协议 + 状态机 + 事件驱动） | Phase 2 |
| v3.0 | 3-6 月 | 2024 平均水平（向量记忆 + 动态角色 + tracing） | Phase 3 |
| v4.0 | 6-12 月 | 2026 最佳实践（MCP + 共识 + DSPy + time travel） | Phase 4 |

---

**文档集结束**

---

## 附录：文档集导航（v1.1，12 个文档）

| # | 文档 | 内容 | 版本 |
| --- | --- | --- | --- |
| 00 | `00-requirement-history.md` | 需求历程与变更记录 | v1.0 |
| 01 | `01-agent-team-overview.md` | Agent 团队总览 | v1.0 |
| 02 | `02-agent-roles-detail.md` | 各 Agent 角色详细定义 | v1.0 |
| 03 | `03-collaboration-workflow.md` | 协作流程详解 | v1.0 |
| 04 | `04-agent-skills.md` | Agent Skill 定义与管理 | v1.0 |
| 05 | `05-meeting-and-decision-log.md` | 会议与决策日志 | v1.0 |
| 06 | `06-risk-register.md` | 风险登记册 | v1.0 |
| 07 | `07-team-charter.md` | 团队章程 | v1.0 |
| **08** | **`08-agent-lifecycle.md`** | **Agent 生命周期与状态机（v1.1 新增）** | **v1.1** |
| **09** | **`09-runtime-protocol.md`** | **运行时消息协议（v1.1 新增）** | **v1.1** |
| **10** | **`10-modernization-roadmap.md`** | **现代化路线图（v1.1 新增）** | **v1.1** |
| - | `README.md` | 文档集入口 | v1.1 |
