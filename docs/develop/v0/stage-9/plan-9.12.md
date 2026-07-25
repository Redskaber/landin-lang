# Stage 9.12 开发计划: §25 deep review + v0.1 release candidate

> **阶段**: Stage 9.12 (Stage 9 收尾 — v0.1 release candidate)
> **版本**: v0.16.10 → v0.17.0 (v0.1 RC — minor bump for release candidate)
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §25 (深度审查) + §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.11 完成 conformance 547 → 599 (realistic programs, 99.8%). Stage 9.12
是 Stage 9 的收尾阶段, 目标是:
1. 添加最后 1 个 conformance test 达到 600 目标
2. 执行 §25 七维度深度审查 (D1-D7)
3. 宣布 v0.1 release candidate

## 2. §25 深度审查计划 (七维度)

### D1 架构健康度
- 源代码: ~90 files, ~32,000 LOC
- 模块组织: 50+ modules, 单一职责, 数据流单向
- §14.4 J1-J6 判据

### D2 技术债清单
- TD-019 (expr_operand 巨型 match) — 用户 hold
- 其他 TD 全部 CLOSED

### D3 测试覆盖深度
- Rust tests: ~2225
- Conformance tests: 600 (target met!)
- 测试矩阵全覆盖

### D4 下一阶段就绪度
- v0.1 release gate: Stage 0 完整 + conformance 通过 ✅
- v0.3 bootstrap: Stage 1 重写规划 (远期)

### D5 设计合理性
- 19 份设计文档全部已通过 §25.8 同步
- conformance suite 作为可执行规范

### D6 性能与可扩展性
- 无 O(n²) 算法
- Lexer/Parser O(n) 线性

### D7 文档与知识传承
- §17.1/§17.2/§17.3/§18.4 全合规
- 9 个 stage 目录 (stage-0 到 stage-9)
- worklog.md 完整

## 3. 最后 1 个 conformance test

创建 `tests/conformance/00-parse/10-realistic/v0.1_milestone.lin` — 一个综合性的
realistic program, 验证 v0.1 所有核心特性组合正确解析。

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2225+ tests pass
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 600 passed (599 + 1 new)
- ✅ §25 deep review 5/5 GO → PASS
- ✅ v0.1 release candidate 宣布

## 5. 版本

- Cargo.toml: 0.16.10 → 0.17.0 (v0.1 RC — minor bump)
- api-naming-standard.md: v2.14 → v2.15

---

**创建日期**: 2026-07-26
