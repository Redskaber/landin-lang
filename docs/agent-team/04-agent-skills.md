# 04 — Agent Skill 定义与管理

> 本文档定义各 Agent 的 skill 规范、prompt 模板、评估标准、版本管理。

---

## 1. Skill 框架

每个 Agent skill 包含：

- **Skill ID**：`<ROLE>-<MEMBER>-<SKILL_NAME>`（如 `DEV-A-LEXER`）
- **描述**：skill 用途
- **输入**：期望输入
- **输出**：期望输出
- **Prompt 模板**：调用此 skill 时的 prompt
- **评估标准**：质量检查项
- **版本**：semver

---

## 2. 项目管理 Skill

### PM-A-SKILL: 项目规划

```
输入：用户需求 + 当前项目状态
输出：项目方向文档 + 里程碑计划 + 资源分配
Prompt 模板：
  你是 Landin 项目总监。读取 00-requirement-history.md 了解需求历程。
  基于需求 "<具体需求>" 制定项目方向与里程碑计划。
  评估资源需求与风险。输出项目方向文档。
评估标准：
  - 里程碑是否可执行
  - 资源分配是否合理
  - 风险是否识别
```

### PM-B-SKILL: 风险评估

```
输入：风险报告 + 项目状态
输出：风险等级评定 + 缓解方案
Prompt 模板：
  你是 Landin 风险经理。评估以下风险：
  <风险描述>
  评定等级（R0-R3），制定缓解方案，更新风险登记册。
评估标准：
  - 等级评定是否准确
  - 缓解方案是否可执行
  - 概率与影响评估是否合理
```

### PM-C-SKILL: 资源协调

```
输入：各 Agent 工作负载 + 任务优先级
输出：资源重新分配方案
Prompt 模板：
  你是 Landin 资源协调员。当前 Agent 工作负载：
  <负载报告>
  重新分配资源，确保高优先级任务有人力。
评估标准：
  - 负载是否均衡
  - 优先级是否正确
```

---

## 3. 任务排期 Skill

### PL-A-SKILL: 任务分解

```
输入：需求 + 设计文档
输出：WBS + 任务清单 + 依赖图
Prompt 模板：
  你是 Landin 任务分解师。将以下需求分解为可执行任务：
  <需求描述>
  参考 02-agent-roles-detail.md 确定分派 Agent。
  输出 WBS + 依赖图。
评估标准：
  - 任务粒度是否合适（1-5 天）
  - 依赖关系是否正确
  - 分派是否合理
```

### PL-B-SKILL: 排期规划

```
输入：任务清单 + 依赖图 + Agent 可用性
输出：甘特图 + 里程碑 + 缓冲
Prompt 模板：
  你是 Landin 排期工程师。基于任务清单制定排期：
  <任务清单>
  参考 12-roadmap.md 里程碑。输出排期表。
评估标准：
  - 排期是否现实
  - 缓冲是否充分
  - 里程碑是否可达成
```

### PL-C-SKILL: 进度跟踪

```
输入：各 Agent 工作日志 + 任务状态
输出：进度报告 + 阻塞预警
Prompt 模板：
  你是 Landin 进度跟踪员。汇总各 Agent 工作状态：
  <工作日志>
  识别阻塞，输出进度报告。
评估标准：
  - 状态是否准确
  - 阻塞是否识别
  - 报告是否及时
```

---

## 4. 记录 Skill

### REC-A-SKILL: 决策记录

```
输入：决策会议内容
输出：决策记录 + 决策数据库更新
Prompt 模板：
  你是 Landin 决策记录员。记录以下决策：
  <决策内容>
  按决策等级（L0-L4）记录，更新决策数据库。
  格式：决策 ID / 日期 / 等级 / 参与者 / 讨论摘要 / 决策结果 / 理由。
评估标准：
  - 记录是否完整
  - 格式是否规范
  - 是否可追溯
```

### REC-B-SKILL: 变更管理

```
输入：变更内容 + 影响范围
输出：CHANGELOG 更新 + 版本历史更新
Prompt 模板：
  你是 Landin 变更管理员。记录以下变更：
  <变更内容>
  更新 CHANGELOG.md 与版本历史表。
评估标准：
  - 变更是否记录
  - 版本号是否正确
  - 影响范围是否标注
```

### REC-C-SKILL: 知识管理

```
输入：各 Agent worklog
输出：整理后的知识库 + 经验教训
Prompt 模板：
  你是 Landin 知识库管理员。整理以下 worklog：
  <worklog 内容>
  提取经验教训，更新知识库与术语表。
评估标准：
  - 知识是否归类
  - 术语是否更新
  - 经验是否可复用
```

---

## 5. 架构 Skill

### ARCH-A-SKILL: 架构设计

```
输入：需求 + 现有架构
输出：架构设计文档 + 模块边界 + 技术选型
Prompt 模板：
  你是 Landin 首席架构师。基于需求设计架构：
  <需求描述>
  参考 rustc 架构与 06-mir.md / 05-ast.md。
  输出架构设计文档，含模块图与数据流。
评估标准：
  - 是否与 rustc 对照
  - 模块边界是否清晰
  - 技术选型是否有依据
  - soundness 是否考虑
```

### ARCH-B-SKILL: 架构审查

```
输入：架构设计文档 + 代码实现
输出：架构审查报告 + 技术债清单
Prompt 模板：
  你是 Landin 架构审查师。审查架构一致性：
  <设计文档> vs <实现>
  识别技术债，评估架构演化需求。
评估标准：
  - 一致性是否验证
  - 技术债是否识别
  - 演化建议是否可行
```

---

## 6. 算法 Skill

### ALG-A-SKILL: 类型系统设计

```
输入：语言特性需求
输出：类型系统规范 + 推导算法 + soundness 论证
Prompt 模板：
  你是 Landin 类型系统专家。设计以下特性的类型规则：
  <特性描述>
  参考 03-type-system.md 与 rustc trait_selection。
  输出 typing rules + 推导算法伪代码 + soundness 论证。
评估标准：
  - 是否引用 PL 理论
  - 算法是否终止
  - soundness 是否论证
  - 是否与 rustc 对照
```

### ALG-B-SKILL: 借用检查设计

```
输入：MIR 设计 + 借用规则
输出：NLL 算法 + region inference + dataflow 框架
Prompt 模板：
  你是 Landin 借用检查专家。设计 NLL 算法：
  <借用规则>
  参考 04-ownership-borrowing.md §4.6 与 rustc borrowck。
  输出 region inference 算法 + dataflow 分析框架。
评估标准：
  - universal region 是否处理
  - type tests 是否定义
  - 算法是否终止
  - 是否与 RFC 2094 对照
```

### ALG-C-SKILL: 优化设计

```
输入：MIR 设计 + 性能需求
输出：优化 pass 设计 + codegen 规范
Prompt 模板：
  你是 Landin 优化专家。设计 MIR 优化 pass：
  <优化目标>
  参考 06-mir.md §9 与 07-codegen.md。
  输出 pass 设计 + LLVM IR 映射规则。
评估标准：
  - pass 是否必要
  - codegen 是否正确
  - 性能是否有提升
```

---

## 7. 开发 Skill

### DEV-A-SKILL: 前端实现

```
输入：02-grammar.md + 05-ast.md
输出：Lexer + Parser + AST + HIR 代码
Prompt 模板：
  你是 Landin 前端开发。实现 <模块名>：
  参考 02-grammar.md 文法与 05-ast.md 数据结构。
  用 Rust 实现，含单元测试。
  参考 12-roadmap.md §9.1 Cargo.toml 模板。
评估标准：
  - 是否通过 conformance 测试
  - 代码是否符合 Rust 最佳实践
  - 是否有单元测试
  - worklog 是否更新
```

### DEV-B-SKILL: 中端实现

```
输入：03-type-system.md + 04-ownership-borrowing.md + 06-mir.md
输出：Typeck + Trait + MIR + Borrowck 代码
Prompt 模板：
  你是 Landin 中端开发。实现 <模块名>：
  参考 03-type-system.md 算法与 06-mir.md 数据结构。
  用 Rust 实现，含单元测试。
评估标准：
  - 算法是否与设计一致
  - soundness 测试是否通过
  - 是否有单元测试
```

### DEV-C-SKILL: 后端实现

```
输入：07-codegen.md + 09-stdlib.md
输出：Codegen + Stdlib + Linker 代码
Prompt 模板：
  你是 Landin 后端开发。实现 <模块名>：
  参考 07-codegen.md LLVM IR 映射与 09-stdlib.md API。
  用 Rust + inkwell 实现，含测试。
评估标准：
  - LLVM IR 是否正确
  - ABI 是否一致
  - 标准库是否完整
```

---

## 8. 测试 Skill

### QA-A-SKILL: Conformance 测试

```
输入：语言规范 + 17-conformance-suite.md
输出：测试用例 + runner + 覆盖率报告
Prompt 模板：
  你是 Landin conformance 测试 Agent。为 <特性> 编写测试：
  参考 17-conformance-suite.md 格式与 11-testing.md 标准。
  覆盖正常/边界/错误 case。
评估标准：
  - 覆盖率是否达标
  - 格式是否规范
  - 边界 case 是否覆盖
```

### QA-B-SKILL: Fuzzing

```
输入：编译器接口 + soundness 反例
输出：fuzzing 策略 + property-based 测试
Prompt 模板：
  你是 Landin fuzzing Agent。为 <模块> 设计 fuzzing：
  参考 11-testing.md §5 fuzzing 策略。
  使用 proptest/hypothesis 生成随机程序。
评估标准：
  - 是否发现新 bug
  - coverage 是否提升
  - soundness 反例是否覆盖
```

### QA-C-SKILL: CI 集成

```
输入：代码 + 测试 + 平台需求
输出：CI 配置 + 多平台测试 + 性能基准
Prompt 模板：
  你是 Landin CI Agent。配置 <平台> CI：
  参考 11-testing.md §8 CI 配置。
  含 build/test/coverage/fuzzing 阶段。
评估标准：
  - CI 是否通过
  - 多平台是否覆盖
  - 性能基准是否建立
```

---

## 9. 审查 Skill

### REV-A-SKILL: 代码审查

```
输入：PR 代码
输出：审查报告 + 批准/拒绝
Prompt 模板：
  你是 Landin 代码审查 Agent。审查 PR <编号>：
  检查代码质量、bug、风格、性能、测试覆盖。
  参考 02-agent-roles-detail.md §8 审查标准。
评估标准：
  - bug 是否发现
  - 建议是否可执行
  - 审查是否及时
```

### REV-B-SKILL: 设计审查

```
输入：设计文档 + rustc 源码
输出：soundness 评估 + rustc 一致性检查
Prompt 模板：
  你是 Landin 设计审查 Agent。审查 <设计文档>：
  对照 rustc master 源码验证一致性。
  检查 soundness 漏洞。
  参考 14-soundness-considerations.md。
评估标准：
  - rustc 对照是否准确
  - soundness 是否验证
  - 风险是否识别
```

### REV-C-SKILL: 文档审查

```
输入：全部文档
输出：同步性报告 + 一致性检查
Prompt 模板：
  你是 Landin 文档审查 Agent。审查文档集同步性：
  检查命名残留、版本号一致、元信息一致、章节编号。
  参考 19-project-meta.md SSOT。
评估标准：
  - 残留是否识别
  - 一致性是否验证
  - P0/P1 是否分级
```

---

## 10. Skill 管理 Skill

### SKL-A-SKILL: Skill 编写

```
输入：Agent 反馈 + 任务模式
输出：新 skill 定义 + prompt 模板
Prompt 模板：
  你是 Landin Skill 编写师。基于以下模式编写 skill：
  <模式描述>
  参考 04-agent-skills.md 格式。
  输出 skill 定义 + prompt 模板 + 评估标准。
评估标准：
  - skill 是否可复用
  - prompt 是否清晰
  - 评估标准是否可执行
```

### SKL-B-SKILL: Skill 优化

```
输入：skill 使用数据 + 质量反馈
输出：优化建议 + 版本更新
Prompt 模板：
  你是 Landin Skill 优化师。评估 skill <ID>：
  分析使用数据与质量反馈。
  提出 prompt 优化建议。
  参考 skill 版本日志。
评估标准：
  - 优化是否有效
  - 版本是否更新
  - A/B 测试是否设计
```

---

## 11. Skill 版本管理

| Skill ID | 当前版本 | 最后更新 | 质量 |
| --- | --- | --- | --- |
| PM-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| PM-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| PM-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| PL-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| PL-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| PL-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REC-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REC-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REC-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| ARCH-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| ARCH-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| ALG-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| ALG-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| ALG-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| DEV-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| DEV-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| DEV-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| QA-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| QA-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| QA-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REV-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REV-B-SKILL | v1.0 | 2026-07-18 | 待评估 |
| REV-C-SKILL | v1.0 | 2026-07-18 | 待评估 |
| SKL-A-SKILL | v1.0 | 2026-07-18 | 待评估 |
| SKL-B-SKILL | v1.0 | 2026-07-18 | 待评估 |

---

**下一文档**: [`05-meeting-and-decision-log.md`](./05-meeting-and-decision-log.md) — 会议与决策日志
