# 08 — Agent 生命周期与状态机

> 本文档定义 Agent 的完整生命周期管理、状态机、自治边界、故障恢复。v1.1 新增（A1/A2 审查建议）。

---

## 1. Agent 生命周期

### 1.1 生命周期阶段

```
创建 → Onboarding → 激活 → 工作 → 休眠 → 唤醒 → ... → 离线 → 销毁
```

| 阶段 | 触发 | 动作 | 状态 |
| --- | --- | --- | --- |
| **创建** | PM-A 决定新增 Agent | 分配 Agent ID + 角色 + 初始 skill | `created` |
| **Onboarding** | 创建后自动 | 加载角色定义 + skill + worklog 上下文 + 术语表 | `onboarding` |
| **激活** | Onboarding 完成 | Agent 进入 idle，等待任务 | `idle` |
| **工作** | 收到任务分派 | 执行任务 | `working` |
| **阻塞** | 等待依赖/审查/资源 | 等待解除 | `blocked` |
| **休眠** | 空闲超时（30min） | 释放上下文，保留状态 checkpoint | `dormant` |
| **唤醒** | 新任务到达 | 从 checkpoint 恢复 | `idle` |
| **离线** | PM-A 决定下线 | 完成当前任务 → 交接 → 离线 | `offline` |
| **销毁** | 离线后 7d | 清理资源 → 归档 worklog | `destroyed` |

### 1.2 Onboarding 流程

```
新 Agent 创建
→ REC-A 记录创建（Agent ID / 角色 / 创建原因）
→ 加载角色定义（02-agent-roles-detail.md 对应章节）
→ 加载 skill（04-agent-skills.md 对应 skill）
→ 加载 worklog 上下文（最近 100 条 + 相关历史决策）
→ 加载术语表（18-glossary.md）
→ 加载元信息 SSOT（19-project-meta.md）
→ SKL-A 验证 skill 可用性
→ REV-C 验证文档一致性
→ PM-C 确认资源分配
→ 激活 → REC-A 记录
```

### 1.3 离线流程

```
PM-A 决定下线
→ Agent 完成当前任务（或转交 PL-C 重新分派）
→ REC-C 整理 Agent worklog 归档
→ SKL-B 评估 Agent skill 使用数据
→ PM-C 释放资源
→ REC-A 记录离线
→ 7d 后销毁
```

---

## 2. Agent 状态机

### 2.1 状态转换图

```
                    ┌──────────┐
                    │ created  │
                    └────┬─────┘
                         │ onboarding
                    ┌────▼─────┐
          ┌────────│  idle    │────────┐
          │        └────┬─────┘        │
          │ task        │              │ timeout(30m)
          │             │ task         │
     ┌────▼─────┐  ┌───▼──────┐  ┌───▼──────┐
     │ working  │  │ working  │  │ dormant  │
     └────┬─────┘  └───┬──────┘  └───┬──────┘
          │             │             │ new task
          │ done        │ blocked     │
     ┌────▼─────┐  ┌───▼──────┐      │
     │  idle    │  │ blocked  │      │
     └──────────┘  └───┬──────┘ ┌────▼─────┐
                        │ unblock│  idle    │
                        └───────►└──────────┘
```

### 2.2 状态定义

| 状态 | 描述 | 可执行任务 | 可接收消息 | 资源占用 |
| --- | --- | --- | --- | --- |
| `created` | 刚创建，未 onboarding | ❌ | ❌ | 低 |
| `onboarding` | 加载上下文 | ❌ | ❌ | 中 |
| `idle` | 空闲，等待任务 | ❌ | ✅ | 低 |
| `working` | 执行任务 | ✅ | ✅（排队） | 高 |
| `blocked` | 等待依赖 | ❌ | ✅ | 中 |
| `dormant` | 休眠，checkpoint 保存 | ❌ | ✅（触发唤醒） | 低 |
| `offline` | 已下线 | ❌ | ❌ | 无 |
| `destroyed` | 已销毁 | ❌ | ❌ | 无 |

### 2.3 状态转换规则

| 从 | 到 | 触发条件 | 动作 |
| --- | --- | --- | --- |
| created | onboarding | 自动 | 开始加载上下文 |
| onboarding | idle | 加载完成 | 通知 PL-C 可分派 |
| idle | working | 收到任务 | 开始执行 |
| working | idle | 任务完成 | 通知 PL-C + REC-C 记录 |
| working | blocked | 依赖未满足 | 通知 PL-C + 等待 |
| blocked | working | 依赖满足 | 恢复执行 |
| idle | dormant | 30min 无任务 | checkpoint 保存 |
| dormant | idle | 新任务到达 | 从 checkpoint 恢复 |
| any | offline | PM-A 决定 | 完成任务 + 交接 |
| offline | destroyed | 离线 7d | 清理 + 归档 |

---

## 3. Agent 自治边界

### 3.1 自治等级

| 等级 | 名称 | 描述 | 示例 |
| --- | --- | --- | --- |
| A0 | 完全自主 | Agent 自主决策，事后记录 | 变量命名、函数拆分 |
| A1 | 自主+通知 | Agent 自主决策，实时通知相关方 | 选择测试策略 |
| A2 | 自主+确认 | Agent 决策后需 1 人确认 | 算法选型 |
| A3 | 提案+审批 | Agent 提案，需多人审批 | IR 修改 |
| A4 | 全人工 | Agent 仅提供建议，人工决策 | 版本发布、命名变更 |

### 3.2 各 Agent 自治等级

| Agent | 默认自治 | 可升级到 | 可降级到 | 升级触发 |
| --- | --- | --- | --- | --- |
| PM-A | A4 | - | - | - |
| PM-B | A2 | A3 | A1 | R0/R1 风险 |
| PM-C | A1 | A2 | A0 | 资源冲突 |
| PL-A | A2 | A3 | A1 | 跨类任务 |
| PL-B | A1 | A2 | A0 | 里程碑变更 |
| PL-C | A0 | A1 | - | 进度延期 > 3d |
| REC-A/B/C | A0 | A1 | - | 记录争议 |
| ARCH-A | A3 | A4 | A2 | 架构一票否决 |
| ARCH-B | A1 | A2 | A0 | 技术债 > R2 |
| ALG-A/B/C | A2 | A3 | A1 | soundness 相关 |
| DEV-A/B/C | A0 | A1 | - | 代码审查未通过 |
| QA-A/B/C | A1 | A2 | A0 | soundness 测试失败 |
| REV-A | A1 | A2 | A0 | 代码质量 < 70% |
| REV-B | A2 | A3 | A1 | soundness 漏洞 |
| REV-C | A1 | A2 | A0 | 同步性 P0 |
| SKL-A/B | A1 | A2 | A0 | 新 skill 上线 |

### 3.3 自治资源预算

每个 Agent 执行 A0-A1 自治决策时有资源预算：

- **Token 预算**：单次决策 ≤ 10,000 tokens
- **时间预算**：单次决策 ≤ 5 分钟
- **工具调用**：单次决策 ≤ 10 次工具调用
- **文件修改**：单次决策 ≤ 3 个文件

超出预算自动升级到 A2（需确认）。

---

## 4. 故障恢复

### 4.1 故障类型

| 故障 | 检测 | 恢复 |
| --- | --- | --- |
| Agent 超时 | PL-C 监控，超时 30min | PL-C 重新分派任务 |
| Agent 崩溃 | PL-C 心跳检测 | 从最后一个 checkpoint 恢复 |
| Agent 输出错误 | REV-A 审查发现 | REV-A 驳回 → DEV 修复 |
| Agent 间死锁 | PL-C 依赖图检测 | PM-C 介入 → 强制解锁 |
| Agent 资源耗尽 | PM-C 监控 | PM-C 重新分配资源 |

### 4.2 Checkpoint 机制

每个 Agent 在 `working` 状态每 5 分钟自动 checkpoint：

```
checkpoint = {
    agent_id: <Agent ID>,
    task_id: <Task ID>,
    timestamp: <ISO 8601>,
    state: <当前执行状态>,
    progress: <已完成步骤>,
    remaining: <剩余步骤>,
    worklog_snapshot: <最近 worklog 条目>,
}
```

Checkpoint 保存到 `/home/z/my-project/.checkpoints/<agent_id>/<task_id>.json`。

### 4.3 任务接管

Agent 故障时，任务接管流程：

```
PL-C 检测故障 → 通知 PM-C
→ PM-C 评估：可恢复 / 需接管 / 需重做
→ 可恢复：从 checkpoint 恢复（同 Agent 重启）
→ 需接管：分派给同类其他 Agent（如 DEV-A → DEV-B）
→ 需重做：从头开始（checkpoint 不可用）
→ REC-A 记录故障 + 恢复
```

---

## 5. 安全与权限

### 5.1 操作权限矩阵

| 操作 | PM | PL | REC | ARCH | ALG | DEV | QA | REV | SKL |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 读 worklog | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 写 worklog | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 读设计文档 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 写设计文档 | ❌ | ❌ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| 写代码 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| 写测试 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ |
| 合并 PR | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 批准 PR | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| 修改元信息 SSOT | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 创建/销毁 Agent | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 修改风险等级 | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 修改 skill 定义 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

### 5.2 审计追踪

每个 Agent 操作记录审计日志：

```json
{
    "audit_id": "<UUID>",
    "timestamp": "<ISO 8601>",
    "agent_id": "<Agent ID>",
    "action": "<read|write|approve|reject|create|destroy>",
    "target": "<文件/任务/PR/决策>",
    "details": "<操作详情>",
    "authorization": "<自治等级 + 权限依据>"
}
```

审计日志保存到 `/home/z/my-project/.audit/audit-<date>.jsonl`，append-only，不可修改。

---

**下一文档**: [`09-runtime-protocol.md`](./09-runtime-protocol.md) — 运行时消息协议
