# 07 — 团队章程

> 本文档定义 Landin Agent Group 的工作原则、行为准则、质量标准。

---

## 1. 项目愿景

设计并实现 **Landin** —— 一门静态类型、编译型、内存安全的系统级编程语言，定位介于 C 与 Rust 之间，目标 v0.1（可用编译器）+ v0.3（自举完成）。

## 2. 核心价值观

1. **实证优先**：所有决策基于数据与研究，非主观判断
2. **Soundness 第一**：类型系统健全性高于功能丰富性
3. **MIR-first**：所有静态分析在 MIR 上做，不走 AST-based 老路
4. **渐进式自举**：v0.1 不自举，v0.3 自举完成
5. **文档即代码**：文档与实现同等重要，SSOT 原则

## 3. Agent 行为准则

### 3.1 工作原则

- **先读后写**：每个 Agent 工作前必须先读 worklog.md
- **追加不覆盖**：worklog.md 是 append-only
- **SSOT 遵守**：元信息以 19-project-meta.md 为准
- **引用出处**：所有技术声明必须引用 rustc 源码或论文
- **风险上报**：发现风险立即上报 PM-B

### 3.2 协作原则

- **任务 ID**：每个任务必须有全局唯一 Task ID
- **明确分派**：任务必须明确分派给具体 Agent
- **及时反馈**：阻塞时 24h 内通知 PL-C
- **尊重审查**：REV-A/B/C 审查意见必须认真对待

### 3.3 质量原则

- **测试先行**：DEV 实现前 QA-A 先写测试
- **Conformance 100%**：所有 conformance 测试必须通过
- **Soundness 零容忍**：soundness 漏洞是 R0 风险
- **文档同步**：代码变更必须同步更新文档

## 4. 质量标准

### 4.1 代码质量

- Rust best practices
- 单元测试覆盖率 ≥ 85%
- 无 clippy warning
- conformance 测试 100% 通过

### 4.2 设计质量

- 必须引用 rustc 源码出处
- 必须通过 REV-B soundness 审查
- 必须更新 18-glossary.md 术语表
- 必须含伪代码或 Rust struct 定义

### 4.3 文档质量

- 版本号统一（v1.3.3）
- 术语一致（参考 18-glossary.md）
- 元信息一致（参考 19-project-meta.md SSOT）
- 无 TODO/TBD（v0.2 推迟项明确标注）

### 4.4 测试质量

- conformance 套件 ≥ 5,000 测试
- soundness 套件 ≥ 500 测试
- fuzzing 每夜运行
- CI 多平台覆盖

## 5. 版本发布标准

### 5.1 v0.1 发布标准

- Stage 0 完整（130-180k 行 Rust）
- Conformance 5,000 测试 100% 通过
- Soundness 500 测试 100% 通过
- 5 个目标平台 CI 通过
- 文档集 v1.3.3+ 同步
- REV-A/B/C 全面审查通过

### 5.2 v0.3 发布标准

- Stage 1 用 Landin 重写完成
- Stage 2 自编译验证通过
- 干净环境 bootstrap 成功
- Conformance 同一套件通过

## 6. 冻结与变更管理

### 6.1 设计冻结

v1.3.3 文档集已冻结。变更需通过 RFC 流程：

- L3-L4 决策
- REV-B/C 审查
- REC-A 记录

### 6.2 实现阶段变更

- P0（阻塞实现）：立即修文档 + 重新冻结
- P1（不阻塞但应修）：月度集中修
- P2（cosmetic）：v0.1 完成时统一修

## 7. 团队会议

### 7.1 例会

- **日站会**：PL-C 主持，15 分钟，进度同步
- **周例会**：PM-A 主持，1 小时，里程碑 review
- **月度审查**：REV-A/B/C 主持，2 小时，全面审查

### 7.2 专题会议

- **设计评审**：ARCH-A 主持，按需
- **风险评审**：PM-B 主持，按需
- **决策会议**：PM-A 主持，按需

## 8. 知识管理

### 8.1 文档集

- `/home/z/my-project/download/lang-design/`：语言设计蓝图（23 文档）
- `/home/z/my-project/download/agent-team/`：Agent 团队管理（7 文档）
- `/home/z/my-project/worklog.md`：工作日志（append-only）

### 8.2 知识更新

- 每次任务完成后 REC-C 整理 worklog
- 每月 REC-C 更新知识库
- 每版本 SKL-A/B 评估 skill 质量

## 9. 项目里程碑

| 里程碑 | 预期时间 | 交付物 |
| --- | --- | --- |
| 月 1 | 项目骨架 | Cargo workspace + conformance 仓库 + RFC 仓库 |
| 月 2 | Lexer + Parser | 200 parse 测试通过 |
| 月 3 | HIR + Name resolution | 50 name resolution 测试通过 |
| 月 4 | Type check 基础 | 100 typeck 测试通过 |
| 月 5 | Trait resolution | 100 trait 测试通过 |
| 月 6 | MIR + NLL | 200 borrowck 测试通过 |
| 月 7 | LLVM codegen | 150 codegen 测试通过 |
| 月 8 | stdlib core + alloc | 50 stdlib 测试通过 |
| 月 9 | mini-cargo + test runner | 100 集成测试通过 |
| 月 10+ | Conformance 完成 | 5,000 测试通过 |
| 月 15+ | v0.1 发布 | 预编译二进制 + 源码 |
| 月 27-40 | v0.1 完成 | 可用编译器 |
| 月 43-64 | v0.3 完成 | 自举成功 |

---

## 10. 紧急响应

### 10.1 R0 风险响应

1. PM-A 立即召集全员
2. 停止所有非相关任务
3. 24h 内制定紧急方案
4. REC-A 记录全过程

### 10.2 ICE（内部编译器错误）响应

1. DEV 收到 ICE 报告
2. 立即创建 GitHub issue
3. REV-A 评估严重度
4. DEV 修复 + QA-B 验证

---

**下一文档**: [`08-agent-lifecycle.md`](./08-agent-lifecycle.md) — Agent 生命周期与状态机

---

## 附录：文档集导航（v1.1，12 个文档）

| # | 文档 | 内容 |
| --- | --- | --- |
| 00 | `00-requirement-history.md` | 需求历程与变更记录 |
| 01 | `01-agent-team-overview.md` | Agent 团队总览 |
| 02 | `02-agent-roles-detail.md` | 各 Agent 角色详细定义 |
| 03 | `03-collaboration-workflow.md` | 协作流程详解 |
| 04 | `04-agent-skills.md` | Agent Skill 定义与管理 |
| 05 | `05-meeting-and-decision-log.md` | 会议与决策日志 |
| 06 | `06-risk-register.md` | 风险登记册 |
| 07 | `07-team-charter.md` | 团队章程（本文档） |
| 08 | `08-agent-lifecycle.md` | Agent 生命周期与状态机 |
| 09 | `09-runtime-protocol.md` | 运行时消息协议 |
| 10 | `10-modernization-roadmap.md` | 现代化路线图 |
| - | `README.md` | 文档集入口 |
