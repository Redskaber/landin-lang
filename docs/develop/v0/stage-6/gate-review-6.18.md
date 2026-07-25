# Stage 6 Gate Review Round 18 (6.18) — Stage 6 收尾：§25.8 完整设计回写 + 重构阶段告一段落

> **审查日期**: 2026-07-25 | **版本**: v0.13.6 → v0.14.0
> **流程**: stage-committee-process.md v3.21 §25.8（阶段末尾设计回写协议）+ §1.2 验收
> **审查范围**: Stage 6.18 单一子阶段（Stage 6 收尾里程碑）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 用户指示

> 像这种重构之后的收益不够时不需要现状去重构它，所以回退你对
> expr_operand.rs 的重构（当前不需要）；并且明确当前重构阶段
> 已经告一段落（可以接下来内容，继续重构只会收益不成正比）。

依据该指示：

1. **回退 Stage 6.17**（expr_operand.rs 子模块提取）— 已执行
   - 删除 `place.rs` / `dyn_call.rs` / `enum_variant.rs`
   - 恢复 `expr_operand.rs` 到 1275 LOC（Stage 6.16 状态）
   - 恢复 `mod.rs` re-exports
   - 1881 tests pass（行为等价回退）
2. **宣布架构性重构阶段告一段落** — Stage 6.1-6.16 已完成 47 模块拆分
3. **进入 Stage 6 收尾阶段** — 执行 §25.8 完整设计回写

## §25.8 完整设计回写

依据 v3.21 §25.8，Stage 6 末尾必须对照 `docs/lang-design/` 与项目实际实现，
识别 B1-B4 偏差并回写设计文档。

### 已回写文档（Stage 6.11）

| 文档 | 回写内容 |
|------|---------|
| `06-mir.md` | ✅ §14 实现状态（B1/B3/B4 偏差清单 + dyn Trait lowering 算法补写） |
| `07-codegen.md` | ✅ §14 实现扩展（Trait dispatch codegen 子系统补写） |

### 本阶段回写文档（Stage 6.18）

| 文档 | 回写内容 | 偏差类型 |
|------|---------|---------|
| `01-language-specification.md` | ✅ §13 实现状态（§6 名称解析 + §7 模块系统） | B1/B3/B4 |
| `02-grammar.md` | ✅ §5 实现状态（§1 词法 + §2-§3 语法） | B4 |
| `03-type-system.md` | ✅ §10 实现状态（§4 类型推导 + §5 trait resolution + §7-§8） | B1/B3 |
| `04-ownership-borrowing.md` | ✅ §11 实现状态（§2 借用 + §3 生命周期 + §4 NLL + §5 drop + §6 诊断 + §8 disjoint captures） | B1/B3 |
| `05-ast.md` | ✅ §13 实现状态（§2-§8 AST + §12 HIR vs AST） | B3/B4 |
| `09-stdlib.md` | ✅ §11 实现状态（stdlib 整体 + trait method 查询 API + vtable 布局） | B1/B3/B4 |

### 偏差汇总

| 偏差类型 | 数量 | 典型示例 |
|---------|------|---------|
| B1（实现 < 设计） | ~20 | region inference / two-phase borrows / drop check / orphan rule / async/await / extern "C" / alloc 层 / std 层 |
| B3（实现 ≠ 设计，实现更简化） | ~10 | visibility permissive / NLL 合并 / lifetime elision / trait resolution 简化 / subtyping 简化 |
| B4（设计灰区，实现已做） | ~8 | dyn Trait lowering / trait method 查询 API / vtable 布局 / HIR 扩展 |

**B1 处理**：全部推迟到 v0.2+（MVP 不需要）。
**B3 处理**：当前简化版满足 MVP，接受为临时偏差，v0.2+ 完善。
**B4 处理**：已在设计文档中补写。

## Stage 6 架构性拆分总览（最终）

| Phase | Modules | Largest file LOC (before → after) |
|-------|---------|-----------------------------------|
| mir/lower | 7 | mod.rs 3346 → 772 (-76.9%) |
| codegen | 5 | mod.rs 2461 → 1050 (-57.3%) |
| stdlib | 3 | (single file → 3 modules) |
| parser | 8 | parser.rs 3112 → 263 (-91.5%) |
| lexer | 6 | reader.rs 1537 → 349 (-77.3%) |
| borrowck | 6 | mod.rs 1452 → 1146 (-21%) |
| typeck | 5 | checker.rs 1320 → 1160 (-12%) |
| resolve | 7 | resolver.rs 1131 → 154 (-86.4%) |
| **Total** | **47** | All < 1300 LOC |

**Stage 6.17（expr_operand 子模块提取）已回退** — 用户判断收益不足。
`expr_operand.rs` 保持 1275 LOC（含 1046 LOC 巨型 match，TD-019 仍 OPEN，
待收益足够时再考虑）。

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 47-module 架构稳定，重构阶段告一段落 |
| D2 技术债清单 | ✅ TD-019 仍 OPEN（待收益足够）；TD-015/TD-018 推迟 v0.2+ |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 7 候选（TD-015 Region inference / TD-018 用户自定义 trait dyn） |
| D5 设计合理性 | ✅ §25.8 完整设计回写完成（8 份设计文档） |
| D6 性能 | ✅ 无性能影响 |
| D7 文档 | ✅ plan-6.18 + gate-review-6.18 + dev-log + api-naming-standard v1.87 + RELEASE_NOTES + README + worklog + 6 份设计文档回写 |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | TD-015: Region inference | Stage 7+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 7+ |
| P3 | TD-019: expr_operand 巨型 match 细拆（当收益足够时） | Stage 7+ |
| P2 | v0.2 特性: async/await / extern "C" / unwind / drop elaboration | v0.2 |

---

**审查完成**: 2026-07-25

**Stage 6 收尾里程碑达成** — 架构性重构阶段告一段落，§25.8 完整设计回写完成。
