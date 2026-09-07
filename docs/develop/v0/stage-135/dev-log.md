# Stage 135 开发日志 — 系统性架构审查

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.665.0 (不变 — 审查任务，无代码变更) |
| 测试数 | 898 lib (不变) |
| 失败数 | 0 |
| ignored | 0 (lib only) |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 135 对比 Rust 各编译管道/阶段/架构/模型，系统性全面审查 Landin 当前项目的缺陷和遗漏 + 审查自举能力+可拓展性差距 + 检索 bug/缺陷/特解转通解。

### WHY (审查动机)
v0.14 阶段 builtin macro TD 已全部处理完毕 (Stage 131-134)。需要系统性审查确定 v0.15+ 的工作优先级，确保项目朝着自举和可拓展方向前进。

### HOW (审查方法)
对比 Rust 编译管道 (lex → parse → AST → HIR → resolve → typeck → borrowck → MIR → monomorphize → codegen → link)，逐阶段审查 Landin 当前实现。

### 审查范围 (14 维度)
1. Lexer (Token 系统) — 缺 raw/byte literals
2. Parser — 缺 GAT/const generics/async/impl Trait return 等
3. HIR — 缺 GATs + const generics
4. Resolver — 缺 multi-crate + visibility
5. Typeck — 缺 assoc type projection + HRTB
6. Borrowck — 缺 NLL + two-phase borrows
7. MIR — 缺 optimizations + async lowering
8. Monomorphization — 基本完整
9. Codegen — 缺 debug info + opt levels
10. Trait System — 缺 specialization + auto traits
11. Stdlib/Prelude — MVP 级别
12. Macro System — 缺 proc macros
13. Self-Hosting — 距离自举非常远 (50+ stages)
14. Extensibility — 缺 plugin/multi-backend/incremental/LSP

### 发现: ~50+ 项 TD
详见 `docs/develop/v0/tech-debt-register.md` — "P3 — v0.15+ 系统性架构审查发现 (Stage 135)" 节。

## 决策点 (为何选此审查方法)
- **选逐阶段对比 Rust** — 不选随机审计 — §1.0 原则 6 (通解 > 特例): 系统性对比确保无遗漏
- **选 14 维度审查** — 不选单一维度 — §13.4 J4 (编译相关表达完整): 覆盖整个编译管道

## 裁剪点
- 无 — L3 审查任务，全流程执行

## 下一步 (下一 MUV)
基于审查结果，v0.15 优先级建议:
1. TD-TYPECK-ASSOC-TYPE-PROJECTION (关联类型投影) — 解锁 Iterator trait + 多个下游 TD
2. TD-PARSE-IMPL-TRAIT-RETURN (impl Trait 返回) — 解锁现代 API 风格
3. TD-STDLIB-ITERATOR (Iterator trait) — 最常用的 trait
4. TD-TRAIT-BLANKET-IMPL (blanket impl) — 解锁通用 trait impl
5. TD-PARSE-EXTERN-BLOCK (extern block) — 解锁 FFI 支持
