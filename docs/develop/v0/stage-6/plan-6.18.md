# Stage 6.18 开发计划：Stage 6 收尾 — §25.8 完整设计回写 + 重构阶段告一段落

> **阶段**: Stage 6.18（Stage 6 收尾里程碑）
> **版本**: v0.13.6 → v0.14.0
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §25.8（阶段末尾设计回写协议）+ §1.2 验收

## 1. 背景：重构阶段告一段落

用户明确指示：

> 像这种重构之后的收益不够时不需要现状去重构它，所以回退你对
> expr_operand.rs 的重构（当前不需要）；并且明确当前重构阶段
> 已经告一段落（可以接下来内容，继续重构只会收益不成正比）。

依据该指示：

1. **回退 Stage 6.17**（expr_operand.rs 子模块提取）— 已执行，`expr_operand.rs` 恢复到 1275 LOC
2. **宣布架构性重构阶段告一段落** — Stage 6.1-6.16 已完成 47 模块拆分，所有大文件 < 1300 LOC
3. **进入 Stage 6 收尾阶段** — 执行 §25.8 完整设计回写

## 2. §25.8 完整设计回写计划

依据 v3.21 §25.8（阶段末尾设计回写协议），Stage 6 末尾必须对照
`docs/lang-design/` 与项目实际实现，识别 B1-B4 偏差并回写设计文档。

### 2.1 已回写文档（Stage 6.11）

| 文档 | 状态 |
|------|------|
| `06-mir.md` | ✅ §14 实现状态（B1/B3/B4 偏差清单 + dyn Trait lowering 算法补写） |
| `07-codegen.md` | ✅ §14 实现扩展（Trait dispatch codegen 子系统补写） |

### 2.2 本阶段需回写的文档

| 文档 | 回写内容 | 偏差类型 |
|------|---------|---------|
| `01-language-specification.md` | §6 名称解析实现状态 + §7 模块系统实现状态 | B1/B4 |
| `02-grammar.md` | §1-§3 语法实现状态（lexer + parser 已完成架构拆分） | B4 |
| `03-type-system.md` | §4 类型推导实现状态 + §8 Subtyping 实现状态 | B1/B4 |
| `04-ownership-borrowing.md` | §4 NLL 算法实现状态 + §6 借用错误诊断实现状态 | B1/B4 |
| `05-ast.md` | §8 表达式定义实现状态（HIR vs AST 差异） | B3/B4 |
| `09-stdlib.md` | stdlib 实现状态（trait methods + vtable layout 已实现） | B4 |

### 2.3 不需回写的文档

| 文档 | 理由 |
|------|------|
| `00-overview.md` | 元文档（项目概览），无实现偏差 |
| `10-toolchain.md` | 工具链规划，v0.2+ 才实现 |
| `11-testing.md` | 测试策略文档，已通过 conformance suite 验证 |
| `12-roadmap.md` | 路线图，按计划推进中 |
| `13-18-*.md` | 元文档（feature whitelist / soundness / attributes / diagnostics / conformance / glossary） |

## 3. §14.4 J1-J6 判据（本阶段不适用）

本阶段不是重构阶段，是 §25.8 设计回写阶段。§14.4 J1-J6 判据不适用。

## 4. 执行计划

### 4.1 回写 6 份设计文档

每份文档在末尾添加"§N 实现状态（v0.14.0，§25.8 回写）"小节，包含：
- 已实现项清单
- B1/B3/B4 偏差清单
- 偏差处理计划

### 4.2 版本号变更

v0.13.6 → **v0.14.0**（Stage 6 收尾里程碑，minor 版本 +1）

理由：Stage 6 架构性重构阶段告一段落 + §25.8 完整设计回写完成，是 Stage 6 的
收尾里程碑，符合 SemVer 0.x→0.y 的 minor bump。

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] 6 份设计文档完成 §25.8 回写
- [ ] 文档：plan-6.18.md + gate-review-6.18.md + dev-log + api-naming-standard v1.87 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.6 → v0.14.0

## 6. 后续 Stage 7+ 候选

Stage 6 收尾后，下一大阶段（Stage 7）候选：

- **TD-015**: Region inference（生命周期推断）
- **TD-018**: 用户自定义 trait dyn 支持
- **TD-019**: expr_operand 巨型 match 细拆（当收益足够时再考虑）
- **v0.2 特性**: async/await、extern "C" ABI、unwind、drop elaboration

---

**创建日期**: 2026-07-25
