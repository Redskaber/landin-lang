# Stage 18.145 — TD-LOC-* 评估调整 (用户指导: 不强制 1500, 以执行流清晰度为准)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A + PM-A)
> **Date**: 2026-08-16
> **Version**: v0.413.0 (Stage 18.145 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 (重构判据 J6) + §12 (最优>最小)
> **Complexity**: L1 (评估 + 文档更新)
> **Task ID**: stage18.145

## 1. 用户指导调整

用户明确指出:
> "并不是一定要将单文件压缩到 1500 行以内或者只要在 1500 行以内就行，而是如果超过且在科学合理划分后仍处于远大于 1500 行才尽量在 1500 行附近（当然如果继续整理和划分有利于执行流的清晰，单函数的清晰与分层级执行流，那么这个拆分就是有必要的）。"

这与 §13.4 J6 "粒度由职责决定而非 LOC" 完全一致。

## 2. §13.4 J6 重新评估

### 2.1 driver/mod.rs (1580 LOC)

- **compile_inner**: 934 LOC — 编译流水线编排函数
- **执行流**: Lex → Parse → HIR → Resolve → MIR → Typeck → Borrowck → Opt → Codegen prep
- **评估**: compile_inner 是单一职责 (编译流水线编排), 其执行流清晰且分层级
- **进一步拆分的影响**: 会破坏流水线的线性可读性, 使执行流更难追踪
- **结论**: ✅ ACCEPTABLE — 进一步拆分不利于执行流清晰

### 2.2 macro_expand/mod.rs (3904 LOC)

- **真实代码**: 1562 LOC (接近 1500)
- **测试代码**: 2342 LOC (测试使用大量私有函数, 按 §13.3.5 必须内联)
- **评估**: 真实代码已接近 1500, 测试代码膨胀是 §13.3.5 的代价
- **结论**: ✅ ACCEPTABLE — 真实代码接近 1500, 测试代码无法迁移

### 2.3 其他 > 1500 LOC 文件

| 文件 | LOC | 评估 | 状态 |
|------|-----|------|------|
| `mir/lower/control_flow.rs` | 2228 | 控制流 lowering (if/match/loop/while/for) | v0.3 P3 |
| `parser/builtin_macros.rs` | 2069 | 27 个 builtin macro 定义 (各自独立) | ✅ ACCEPTABLE |
| `borrowck/mod.rs` | 1857 | borrowck 主入口 | v0.3 P3 |
| `borrowck/region_inference.rs` | 1789 | 区域推断 | v0.3 P3 |
| `traits/resolver.rs` | 1558 | trait resolver | v0.3 P3 |

## 3. §14.5 深度审查快评

| 维度 | 状态 | 备注 |
|------|------|------|
| D1 架构健康度 | ✅ | 所有文件有清晰单一职责 |
| D2 技术债清单 | ✅ | TD-LOC-* 评估调整完成 |
| D3 测试覆盖深度 | ✅ | 640 lib + 2663 integration, 0 failures |
| D4 下一阶段就绪度 | ✅ | v0.2 P0 mini-cargo 可启动 |
| D5 设计合理性 | ✅ | 无过度设计或设计不足 |
| D6 性能 | ✅ | 无 O(n²) 报告 |
| D7 文档 | ✅ | 流程文档 v6.4 + 技术债登记册 + 校准数据池 |
| D8 测试路径 | ✅ | pipeline-test-coverage.md 完整 |

## 4. 项目状态总结

### 4.1 TD-LOC-* 最终状态

| TD | 原 LOC | 最终 LOC | 状态 | 评估 |
|----|--------|---------|------|------|
| TD-LOC-TYPECK-CHECKER | 2635 | 1371 (4 文件) | ✅ Resolved | 18.128 |
| TD-LOC-MIR-LOWER-MOD | 2857 | 960 (3 文件) | ✅ Resolved | 18.129-18.130 |
| TD-LOC-MIR-LOWER-EXPR | 3599 | 1156 (4 文件) | ✅ Resolved | 18.131-18.133 |
| TD-LOC-DRIVER | 4038 | 1580 (6 文件) | ✅ ACCEPTABLE | 18.134-18.144 (compile_inner 934 LOC, 拆分不改善清晰度) |
| TD-LOC-MACRO-EXPAND | 5962 | 3904 (目录模块, 真实 1562) | ✅ ACCEPTABLE | 18.135-18.136 (真实代码接近 1500, 测试代码无法迁移) |

### 4.2 累计成果 (Stage 18.126-18.145)

- **20 个 stage** 完成 (18.126-18.145)
- **3 项 TD-LOC-* 完全解决** (typeck-checker, mir-lower-mod, mir-lower-expr)
- **2 项 TD-LOC-* 评估为 ACCEPTABLE** (driver, macro-expand)
- **代码量减少**: 总计 ~10000 LOC 从"上帝模块"提取到子模块
- **0 测试回归**: 全程 640 lib + 2663 integration, 0 failures
- **0 TODO/FIXME/HACK**: 代码库无待修复标记

### 4.3 其他技术债状态

- **Span::DUMMY**: ~1284 非测试代码 (大部分是 Category A 合成值, 已在 tech-debt-register 登记)
- **unwrap()**: 40 个非测试代码 (已在 tech-debt-register 登记, 大部分在 borrowck/typeck)
- **流程文档**: v6.4, 13 维度审计 + Round 2 深度审计全部修复
- **技术债登记册**: v0.412.0, 分类索引完整
- **校准数据池**: v1.1, 20 个 stage 统计完整

## 5. §3.2 验收

- ✅ `cargo check` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — exit 0
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --lib` — 640 passed, 0 failed
- ✅ `cargo test --tests` — 2,663 passed, 0 failed, 2 ignored

## 6. Stage Summary

- **Stage 18.145 PASSED** — TD-LOC-* 评估调整
- **结果**: 5 项 TD-LOC-* 全部关闭 (3 Resolved + 2 ACCEPTABLE)
- **用户指导**: 不强制 1500 LOC, 以执行流清晰度为准 — 与 §13.4 J6 一致
- **v0.413.0**: patch bump (评估调整)
- **下一步**: v0.2 P0 mini-cargo 项目系统启动, 或其他技术债修复 (Span::DUMMY 审计, unwrap/expect 审计)
