# Stage 8.6 开发计划: §25.8 设计回写 + §25 深度审查 (Stage 8 收尾)

> **阶段**: Stage 8.6 (Stage 8 收尾里程碑)
> **版本**: v0.15.4 → v0.15.5
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.21 §25.8 (阶段末尾设计回写) + §25 (深度审查) + §17.1 + §1.2

## 1. 背景

Stage 8.5 完成 v0.2 路线图最后一项 (async/await foundation)。本轮作为 Stage 8
收尾，执行 §25.8 完整设计回写 + §25 深度审查，确认 v0.2 路线图全部完成且架构
健康度足以进入下一大阶段 (v0.1 conformance / v0.3 自举)。

## 2. §25.8 设计回写计划

### 2.1 待更新设计文档 (4 份)

| 文档 | 新增章节 | 内容 |
|------|---------|------|
| `03-type-system.md` | +§12 | 5 项 v0.2 特性状态更新 (lifetime elision / object safety / extern C / drop / async-await) |
| `04-ownership-borrowing.md` | +§13 | lifetime elision (§3.2 RFC #141) + drop elaboration (§5) 实现状态 |
| `05-ast.md` | +§14 | Await/Async 表达式 variant 补写 (B4: 文档遗漏) |
| `07-codegen.md` | +§15 | extern "C" ABI (§13.2) 实现状态更新 |

### 2.2 偏差分类

- **B1 (设计有/实现无)**: 0 (Stage 8 全部新功能已实现)
- **B3 (设计描述不精确)**: 0
- **B4 (文档遗漏实现)**: 1 (05-ast.md 未记录 Await/Async variant)
- **可重构**: 0 (无重构机会)

## 3. §25 深度审查计划

### 3.1 七维度审查范围

| 维度 | 审查内容 |
|------|---------|
| D1 架构健康度 | 50+ 模块, 所有文件 < 1500 LOC, 数据流单向 |
| D2 技术债清单 | 仅 TD-019 OPEN (用户指示暂不拆) |
| D3 测试覆盖深度 | 2100 tests, v0.2 features covered |
| D4 下一阶段就绪度 | v0.1 conformance / v0.3 自举 |
| D5 设计合理性 | §3.2/§2.3/§13.2/§5/§10 alignment |
| D6 性能与可扩展性 | 无 O(n²) 算法 |
| D7 文档与知识传承 | 5 plan + 5 gate + dev-log + worklog |

### 3.2 输出

- `docs/develop/v0/stage-8/deep-review-stage8-r181.md`
- `tests/v0/stage8/plan/deep_review_tests.rs` (9 verification tests)

## 4. 验收标准

- `cargo clean && cargo test`: 2100 passed (0 failed)
- `cargo fmt --check`: clean
- `cargo clippy --all-targets`: 0 warnings, 0 errors
- §25.8 4 docs updated
- §25 deep-review 5/5 GO → PASS

## 5. 版本

- Cargo.toml: 0.15.4 → 0.15.5
- api-naming-standard.md: v2.01 → v2.02

---

**创建日期**: 2026-07-25
**完成日期**: 2026-07-25
