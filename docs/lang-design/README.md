# Landin 语言设计蓝图 v1.3.2 (Final — Landin 重命名版)

> **Landin** — 静态类型、编译型、内存安全的系统级编程语言。定位介于 C 与 Rust 之间。
>
> **v1.3.2** 经 25 路研究 + 9 轮迭代审查 + 100+ 项问题修正 + N1-N6 命名与元信息审查，于 2026-07-18 **正式冻结**。命名历程：Forge（17+ 冲突）→ Quench（N3/N4 发现商标冲突）→ Fuller（N5 指出语义链思维定势）→ **Landin**（N5 推荐，PL 学术人名，零冲突）。冻结详情见 [FREEZE-REPORT.md](./FREEZE-REPORT.md)。
>
> **重命名依据**：N5 调研 15 门成功语言命名模式，发现无一采用父语言语义链命名（Rust≠"Safer-C"，Haskell≠"Lambda-ML"）。推荐 Landin（Peter Landin，1930-2009，ISWIM 创造者，《The Next 700 Programming Languages》论文作者，ML→Haskell→Rust traits 血脉源头）：已故无需征求同意 + PL 冲突无 + 包管理器全 free + 域名全 free + 无软件商标冲突 + 故事最强（向系统函数式语言血脉源头致敬）。

## 设计哲学

1. **MIR-first**：所有静态分析在 MIR 上做，不走 Rust 1.0 之前 AST-based 老路
2. **拒绝"语言层特判"**：Box/Vec/String 全部是普通泛型库类型，编译器一视同仁
3. **渐进式自举**：v0.1 = 可用编译器（Rust 实现），v0.3 = 自举完成（不要求 v0.1 自举）

## 文档集（v1.3.2，共 23 个文档，~13,500 行）

### 核心文档（16 个）

| # | 文档 | 内容 |
| --- | --- | --- |
| 00 | [00-overview.md](./00-overview.md) | 总览、设计哲学、决策摘要、MVP vs v0.2 特性表 |
| 01 | [01-language-specification.md](./01-language-specification.md) | 语言规范 |
| 02 | [02-grammar.md](./02-grammar.md) | 完整 EBNF 文法、词法结构、Pratt 优先级表 |
| 03 | [03-type-system.md](./03-type-system.md) | 类型系统、trait 三阶段 resolution、constraint-based 推导 |
| 04 | [04-ownership-borrowing.md](./04-ownership-borrowing.md) | 所有权、NLL 算法（含 universal region + SCC + Drop check + Two-phase + Disjoint closure） |
| 05 | [05-ast.md](./05-ast.md) | AST + HIR 数据结构（含 HirId/Body/OwnerNodes + Range/Slice variant） |
| 06 | [06-mir.md](./06-mir.md) | MIR 完整定义（10 StatementKind / 11 TerminatorKind / 7 CastKind / 与 rustc master 一致） |
| 07 | [07-codegen.md](./07-codegen.md) | MIR → LLVM IR 映射（含 OperandValue 4 形态 + FunctionCx） |
| 08 | [08-bootstrap-strategy.md](./08-bootstrap-strategy.md) | 渐进式自举策略（预编译二进制 frozen blob） |
| 09 | [09-stdlib.md](./09-stdlib.md) | core / alloc / std 三层标准库 API |
| 10 | [10-toolchain.md](./10-toolchain.md) | landin / landinc / landin-test 工具链 |
| 11 | [11-testing.md](./11-testing.md) | 测试金字塔（5,000 测试 + Soundness 套件） |
| 12 | [12-roadmap.md](./12-roadmap.md) | 修正后路线图（v0.1/v0.3 分期）+ 6 级应急降级 + Cargo.toml 模板 |
| 13 | [13-stage1-feature-whitelist.md](./13-stage1-feature-whitelist.md) | Stage 1 源码特性白皮书（26 宏 / 22 属性） |
| 14 | [14-soundness-considerations.md](./14-soundness-considerations.md) | Soundness 论证 |
| 15 | [15-attributes.md](./15-attributes.md) | 属性系统完整清单 |
| 16 | [16-diagnostics.md](./16-diagnostics.md) | 诊断系统与 12 类错误代码注册表 |

### 补充文档（5 个）

| # | 文档 | 内容 |
| --- | --- | --- |
| 17 | [17-conformance-suite.md](./17-conformance-suite.md) | Conformance 测试套件规范（5,000 测试） |
| 18 | [18-glossary.md](./18-glossary.md) | 术语表（90+ 术语统一定义） |
| **19** | **[19-project-meta.md](./19-project-meta.md)** | **项目元信息 SSOT（v1.3.2 新增：编译器名/文件后缀/CLI/目录约定/工具链命名/lang items/intrinsics/ABI/target triple）** |
| - | [CHANGELOG.md](./CHANGELOG.md) | v1.0 → v1.1 变更日志 |
| - | [FREEZE-REPORT.md](./FREEZE-REPORT.md) | v1.3.2 冻结报告 |
| - | README.md（本文档） | 文档集入口 |

## v1.3.2 重大变化（相比 v1.2.3）

### 1. 语言重命名历程：Forge → Quench → Fuller → Landin

**第 1 轮：Forge（v1.0-v1.2.3）** — N1 报告（37 轮调研）发现 17+ 冲突：

- 4 个同名编程语言（zesterer/Forge、humancto/forge-lang、Bill Cox/CodeRhapsody Forge、Treechcer/FORGE）
- Foundry `forge` CLI（Solidity 标准工具，Web3 高频命令）
- Rust 官方 `forge.rust-lang.org`（contributor 文档站）
- Atlassian Forge / Autodesk Forge®（USPTO 注册商标 #6231989）
- crates.io / PyPI / npm 全部被占用
- Minecraft Forge / Forgejo / Eclipse Forge 等强势项目

**第 2 轮：Quench（v1.3.0）** — N3/N4 双重独立验证发现 Quench 也致命冲突：

- crates.io/crates/quench v0.3.0 是同名编程语言（虽已改名 Moss，但 crates.io 名称永久锁定）
- GitHub quench-lang 组织永久占用
- QUENCH 商标由 Quench.ai Ltd（伦敦 AI 公司）在 USPTO Class 42 + EUIPO Class 9/35/36/38/41/42/45 + UK 多国注册，活跃持有人
- 至少 12 个独立 Quench 品牌在运营

**第 3 轮：Fuller（v1.3.1）** — N4 推荐（29/40 分），无 fatal conflict，但 N5 指出：

- "锻造工具"故事陌生（多数英文母语者不知 fuller 是何物）
- 仍陷于"金属工艺语义链"思维定势
- 品牌个性弱（评分 5/10）

**第 4 轮：Landin（v1.3.2，最终）** — N5 推荐（38/50 分，Top 1，PL 学术人名）：

- 无同名编程语言冲突
- GitHub landin-lang 组织可用（HTTP 404）
- landin-lang.org / landinlang.org 域名可用（NXDOMAIN）
- crates.io/PyPI/npm 顶级 + landin-lang / landinc / landinup 后缀全可用
- 无软件类商标冲突（Landin 作为姓氏在 Lanham Act §2(e)(4) 下天然难注册——双向防御）
- Peter Landin（1930-2009 已故）无需征求同意
- 故事最强：向"系统函数式语言"血脉的源头致敬（Landin → ML → Haskell → Rust → Landin）
- 与 14 门可比语言零混淆

### 2. 新增 19-project-meta.md 元信息 SSOT

N2 报告发现元信息分散在 22 个文档中，存在 3 项 P0 + 9 项 P1 不一致。v1.3.2 新增 19-project-meta.md 作为单一来源，包含 16 个章节：

- 项目身份（名称/版本/通道/命名决策依据/备选候选否决理由）
- 文件后缀权威清单（`.lin` / `.lin` / `.linrs` / `.lino` / `.linlib` / `landin.toml`）
- 项目目录约定
- CLI 命令权威清单
- 环境变量
- 退出码
- 命令行选项
- 工具链命名（stage 0/1/2）
- 仓库组织
- lang items 清单（12 个 MVP）
- intrinsic 函数清单
- ABI 名称权威清单
- 标准库 crate 完整清单
- 跨平台 target triple 完整清单

### 3. 元信息统一（P0 修复）

- 文件后缀：`.fg` → `.lin`，`.fgrs` → `.linrs`，`.fgo` → `.lino`，`.fglib` → `.linlib`
- CLI 命令：`forge` → `landin`，`forgec` → `landinc`，`forgeup` → `landinup`
- 工具：`forge-doc` → `landin-doc`，`forge-fmt` → `landin-fmt`，`forge-lsp` → `landin-lsp`
- 环境变量：`FORGE_*` → `QUENCH_*` → `FULLER_*` → `LANDIN_*`
- 代码块标记：` ```forge` → ` ```landin`
- mangling 前缀：`_FRG` → `_LND`
- panic 函数：`__forge_panic_*` → `__landin_panic_*`
- 仓库名：`forge-lang/*` → `landin-lang/*`
- 域名：`forge-lang.org` → `landin-lang.org`

## 研究基础

本蓝图基于 **25 路研究**：

| 研究 ID | 主题 | 关键产出 |
| --- | --- | --- |
| R1-R4 | 初步研究 | Rust 自举史 / rustc 架构 / 编译原理 / 可比语言案例 |
| R5-R9 | 第 1 轮审查 | 7 个 soundness 漏洞 / 25 个 rustc 遗漏 / 10 处引用错误 / 工作量低估 / 13 处矛盾 |
| R10-R13 | 第 2 轮收敛审查 | 49 项承诺落实核查 / rustc 终验 / 完备性 / 启动性 |
| R14-R17 | 第 3 轮零残留终审 | v1.2 残留 5 P0 + 9 rustc 事实错误 + 3 严重不一致 + 5 启动性 P0 全部识别 |
| R18-R20 | 第 4 轮 v1.2.1 终审 | 3 P0 + 4 rustc 错误识别 |
| R21-R22 | 第 5 轮 v1.2.2 独立验证 | 5 P0 + 决策评估 |
| R23 | 第 6 轮 v1.2.3 最终抽样 | 0 P0 确认 + 5 项元文档 hotfix |
| **N1** | **命名调研** | **Forge 17+ 冲突 → 推荐 Landin（66/80）** |
| **N2** | **元信息审查** | **3 P0 + 9 P1 → 新增 19-project-meta.md** |

## v1.3.2 综合评分

| 维度 | v1.2.3 | **v1.3.2** |
| --- | --- | --- |
| 健全性 | 9.5 | **9.5** |
| 完整性 | 9.5 | **9.7**（+19-project-meta.md） |
| 实现可行性 | 8.5 | **8.7**（元信息 SSOT 提升） |
| 文档一致性 | 9.5 | **9.7**（Landin 重命名统一） |
| 启动性 | 8.5 | **8.7**（19 文档提供完整元信息） |
| 命名合规性 | 3.0（Forge 17+ 冲突） | **9.5**（Landin 零冲突） |
| **综合** | 9.2 | **9.3/10** |

## 启动实现

设计阶段已**正式冻结**（25 路研究 + 9 轮审查 + 100+ 项修正 + Fuller → Landin 重命名 + 元信息 SSOT + 0 P0 残留）。下一步：

1. **立即抢注**：`landin-lang.org` 域名 + GitHub `landin-lang` org + crates.io `landin` / `landinc` / `landin-lang`
2. 创建 stage 0 Cargo workspace：参考 [12-roadmap.md §9.1](./12-roadmap.md) 的 Cargo.toml 模板（注意 `[[bin]] name = "landin-stage0"`）
3. 创建 conformance 测试仓库：参考 [17-conformance-suite.md](./17-conformance-suite.md)
4. 创建 RFC 仓库：`landin-lang/rfcs`（GitHub）
5. 按月 2 里程碑实现 Lexer（参考 [02-grammar.md](./02-grammar.md)）
6. 同步按 [11-testing.md](./11-testing.md) 建立测试套件

## 设计变更管理

冻结后，所有 v0.1 → v0.2 的破坏性变更必须通过 **RFC 流程**：

- RFC 仓库：`landin-lang/rfcs`
- RFC 模板：动机 / 详细设计 / 替代方案 / 影响分析
- v0.2+ 特性在 RFC 仓库讨论，不影响 v0.1 实现

## 文档版本

- **版本**：v1.3.2 (Final — Landin 重命名版)
- **日期**：2026-07-18
- **状态**：✅ **设计冻结**（25 路研究 + 9 轮审查 + 100+ 项修正 + Fuller → Landin 重命名 + 元信息 SSOT + 0 P0 残留）
- **冻结依据**：[FREEZE-REPORT.md](./FREEZE-REPORT.md)
- **元信息依据**：[19-project-meta.md](./19-project-meta.md)
- **下一里程碑**：月 1 — 项目骨架 + 抢注 landin-lang 域名/org + 月 2 — Lexer + Parser 实现

---

*本文档集是 Landin 语言的"标准编译器文档集合"，目标是为 v0.1（可用编译器）与 v0.3（自举完成）提供完整、自洽、可执行的设计基础。所有决策均经 25 路研究、9 轮审查、100+ 条问题反馈、N1 命名调研（Forge → Landin）、N2 元信息审查（新增 19-project-meta.md SSOT）后正式冻结。*
