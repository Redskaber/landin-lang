# 09 — 运行时消息协议

> 本文档定义 Agent 间结构化消息协议、事件驱动机制、任务队列与调度。v1.1 新增（A1/A2 审查建议）。

---

## 1. 消息协议

### 1.1 消息格式

所有 Agent 间通信使用 JSON 结构化消息：

```json
{
    "msg_id": "<UUID>",
    "timestamp": "<ISO 8601>",
    "from": "<Agent ID>",
    "to": "<Agent ID | broadcast>",
    "type": "<task_assign | task_complete | task_blocked | review_request | review_result | decision_request | decision_result | risk_alert | query | response | handoff | checkpoint | heartbeat>",
    "priority": "P0 | P1 | P2 | P3",
    "payload": {
        "task_id": "<Task ID>",
        "content": "<消息内容>",
        "attachments": ["<文件路径>"],
        "references": ["<相关 msg_id>"]
    },
    "ack_required": true | false,
    "expires_at": "<ISO 8601 | null>"
}
```

### 1.2 消息类型

| 类型 | 用途 | from → to | ack |
| --- | --- | --- | --- |
| `task_assign` | 分派任务 | PL → Agent | ✅ |
| `task_complete` | 任务完成 | Agent → PL | ✅ |
| `task_blocked` | 任务阻塞 | Agent → PL | ✅ |
| `review_request` | 审查请求 | DEV → REV | ✅ |
| `review_result` | 审查结果 | REV → DEV | ✅ |
| `decision_request` | 决策请求 | Agent → PM | ✅ |
| `decision_result` | 决策结果 | PM → Agent | ✅ |
| `risk_alert` | 风险告警 | Any → PM-B | ✅ |
| `query` | 信息查询 | Any → Any | ❌ |
| `response` | 查询响应 | Any → Any | ❌ |
| `handoff` | 任务转交 | Agent → Agent | ✅ |
| `checkpoint` | 状态保存 | Agent → self | ❌ |
| `heartbeat` | 心跳 | Agent → PL-C | ❌ |

### 1.3 消息路由

```
消息发送 → 写入消息队列（/home/z/my-project/.msgqueue/）
→ 目标 Agent 读取（事件驱动，非轮询）
→ 处理 → 发送 ACK
→ 发送方收到 ACK → 消息完成
→ 超时未 ACK → 重试（3 次）→ 升级到 PM-C
```

### 1.4 消息持久化

所有消息持久化到 `/home/z/my-project/.msgqueue/` 目录：

- `inbox-<agent_id>.jsonl`：每个 Agent 的收件箱
- `outbox-<agent_id>.jsonl`：每个 Agent 的发件箱
- `audit-<date>.jsonl`：每日审计日志

---

## 2. 事件驱动机制

### 2.1 事件类型

| 事件 | 触发 | 订阅者 |
| --- | --- | --- |
| `task_assigned` | PL 分派任务 | 被分派 Agent + REC-C |
| `task_completed` | Agent 完成任务 | PL-C + REC-C + 依赖该任务的 Agent |
| `task_blocked` | Agent 阻塞 | PL-C + PM-B |
| `review_requested` | DEV 请求审查 | REV-A/B/C |
| `review_completed` | REV 完成审查 | DEV + PL-C |
| `decision_made` | 决策完成 | 相关 Agent + REC-A |
| `risk_detected` | 风险识别 | PM-B + PM-A |
| `risk_escalated` | 风险升级 | PM-A + 相关 Agent |
| `doc_updated` | 文档更新 | REV-C + REC-B |
| `skill_updated` | Skill 更新 | SKL-B + 相关 Agent |
| `agent_created` | Agent 创建 | REC-A + SKL-A |
| `agent_offline` | Agent 离线 | REC-A + PM-C |

### 2.2 事件订阅规则

每个 Agent 声明订阅的事件类型（类似 MetaGPT Watch）：

```json
{
    "agent_id": "DEV-A",
    "subscribes": [
        "task_assigned",
        "review_completed",
        "doc_updated",
        "risk_detected"
    ]
}
```

事件到达即触发 Agent 响应，无需轮询。

### 2.3 事件流

```
事件产生 → 写入事件总线
→ 事件总线通知所有订阅者
→ 订阅者 Agent 唤醒（如 dormant）→ 处理事件
→ 处理完成 → 可能产生新事件（链式触发）
```

---

## 3. 任务队列与调度

### 3.1 任务队列

每个 Agent 有独立任务队列：

```json
{
    "agent_id": "DEV-A",
    "queue": [
        {
            "task_id": "T-001",
            "priority": "P0",
            "estimated_hours": 4,
            "deadline": "2026-08-01",
            "dependencies": ["T-000"],
            "status": "pending | in_progress | blocked | done"
        }
    ]
}
```

### 3.2 优先级

| 优先级 | 描述 | 响应时间 | 示例 |
| --- | --- | --- | --- |
| P0 | 紧急 | 立即 | R0 风险修复、soundness 漏洞 |
| P1 | 高 | 24h | conformance 测试失败、PR 审查 |
| P2 | 中 | 7d | 功能实现、文档更新 |
| P3 | 低 | 滚动 | 重构、优化、P2/P3 文档修复 |

### 3.3 调度规则

1. **优先级抢占**：P0 任务可抢占 P1-P3 任务（被抢占任务进入 `blocked`）
2. **超时升级**：P1 任务 24h 未完成 → 升级到 P0 + 通知 PM-C
3. **负载均衡**：PM-C 监控各 Agent 队列长度，负载 > 5 任务时重新分配
4. **依赖阻塞**：任务依赖未完成 → 自动 `blocked` + 通知 PL-C
5. **重试策略**：Agent 失败 → 自动重试 2 次 → 第 3 次失败通知 PM-C → 人工介入

### 3.4 任务分发算法

```
PL-A 分解任务 → 生成任务清单
→ PL-B 排期 → 按优先级排序
→ PM-C 分派 → 按类别 + 负载分配
  ├─ 同类 Agent 中选负载最低的
  ├─ 如全满 → 排入队列
  └─ 如紧急 → 唤醒 dormant Agent
→ PL-C 跟踪 → 每日状态报告
```

---

## 4. 冲突解决机制

### 4.1 冲突类型

| 冲突 | 示例 | 解决者 | 规则 |
| --- | --- | --- | --- |
| 技术分歧 | DEV-A 用方案 X，REV-A 要求方案 Y | ARCH-A | 技术方案以 ARCH-A 判断为准 |
| 质量分歧 | REV-A 认为 PR 不合格，DEV 认为合格 | ARCH-A + ALG-A | 代码质量以 conformance 测试结果为客观依据 |
| 优先级分歧 | DEV-A 认为任务 P0，PL-B 认为 P2 | PM-A | 优先级以 PM-A 判断为准 |
| 设计分歧 | ALG-A 与 ALG-B 算法选型不同 | ARCH-A | 架构相关以 ARCH-A 一票否决 |
| 命名分歧 | 团队对命名意见不一 | PM-A + 全体投票 | L4 决策 |

### 4.2 冲突解决流程

```
冲突发生 → 双方各提交 500 字以内陈述
→ 仲裁者 24h 内裁决（L1 同类 → ARCH-A / L2 跨类 → PM-A）
→ 裁决结果含：方案选择 + 理由 + 反对意见记录
→ 如败方不服 → 24h 内申诉 → 升级到 L3/L4
→ L3/L4 裁决为终裁
→ REC-A 记录全过程
```

### 4.3 冲突预防

1. **设计前置**：ARCH-A + ALG-A 在实现前达成设计共识
2. **测试先行**：QA-A 在 DEV 实现前编写测试，以测试为客观标准
3. **每日站会**：PL-C 主持 15 分钟站会，早期发现分歧
4. **A/B 方案**：技术分歧时，如时间允许，两个方案各实现一个 prototype，以 conformance 结果决定

---

## 5. Human-in-the-Loop (HITL)

### 5.1 HITL 触发条件

| 条件 | 触发 | 等待 |
| --- | --- | --- |
| L3-L4 决策 | 自动 interrupt | 用户/PM-A 确认 |
| R0-R1 风险 | 自动 interrupt | PM-B 确认缓解方案 |
| soundness 漏洞 | 自动 interrupt | REV-B + ALG-A 确认修复方案 |
| 命名变更 | 自动 interrupt | 用户确认 |
| 破坏性变更 | 自动 interrupt | 用户 + PM-A 确认 |
| Agent 创建/销毁 | 自动 interrupt | PM-A 确认 |

### 5.2 HITL 流程

```
Agent 触发 HITL → 发送 decision_request 消息
→ 暂停执行（状态 → blocked）
→ 等待人工确认（超时 48h → 升级 PM-A）
→ 确认/拒绝/修改 → Agent 恢复执行
→ REC-A 记录 HITL 全过程
```

---

## 6. Agent 性能指标 (KPI)

### 6.1 通用 KPI

| 指标 | 定义 | 目标 | 告警阈值 |
| --- | --- | --- | --- |
| 任务完成率 | 完成 / 分派 | ≥ 95% | < 85% |
| 任务准时率 | 准时完成 / 总完成 | ≥ 90% | < 75% |
| 审查通过率 | 一次通过 / 总审查 | ≥ 80% | < 60% |
| 平均响应时间 | 收到任务 → 开始执行 | ≤ 1h | > 4h |
| 平均完成时间 | 开始 → 完成 | ≤ 估算的 120% | > 估算的 150% |
| 消息 ACK 率 | ACK / 发送 | ≥ 98% | < 90% |
| 检查点保存率 | 实际 / 应保存 | 100% | < 95% |
| 故障恢复率 | 恢复成功 / 故障 | ≥ 90% | < 75% |

### 6.2 类别专属 KPI

| 类别 | 专属 KPI | 目标 |
| --- | --- | --- |
| PM | 风险识别提前量（风险发现 → 影响发生的时间差） | ≥ 7d |
| PL | 排期准确率（实际 / 估算 ±20%） | ≥ 80% |
| ARCH | 架构一致性（审查通过 / 审查总数） | ≥ 95% |
| ALG | soundness 测试通过率 | 100% |
| DEV | conformance 测试通过率 | 100% |
| QA | 测试覆盖率 | ≥ 85% |
| REV | 审查准确率（误拒率 + 误通过率） | < 5% |
| SKL | skill 质量评分 | ≥ 7/10 |

### 6.3 KPI 评估周期

- **每日**：PL-C 汇报任务完成率 + 准时率
- **每周**：PM-C 汇报 Agent 负载 + KPI 趋势
- **每月**：PM-A 评估各 Agent KPI → 调整资源 / 自治等级
- **每版本**：REV-C 评估文档同步性 + 知识管理质量

---

**下一文档**: [`10-modernization-roadmap.md`](./10-modernization-roadmap.md) — 现代化路线图
