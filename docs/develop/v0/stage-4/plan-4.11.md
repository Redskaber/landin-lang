# Stage 4.11 开发计划：性能基准套件 + ADR 文档

> **阶段**: Stage 4.11
> **版本**: v0.9.7 → v0.9.8
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.17 §17.3 时期 1

## 1. 目标

1. 创建性能基准测试套件（`benches/`）— 满足深度审查 R37 QA 条件
2. 创建架构决策记录（ADR）文档 — 满足深度审查 R37 D7 条件

## 2. 背景

深度审查 R37 (Stage 3.69) 的 GO-WITH-CONDITIONS 条件：
- **条件 1**：添加性能基准套件（QA-A 条件项）
- **条件 2**：创建 ADR 文档（D7 文档与知识传承）
- **条件 3**：审视 HirParam 重复（已在 Stage 3.65 接受为设计决策）

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.11-a | 创建 `benches/compile_bench.rs` — 编译流水线基准测试 | L2 |
| 4.11-b | 在 Cargo.toml 添加 `[[bench]]` target + criterion dev-dependency | L1 |
| 4.11-c | 创建 `docs/develop/v0/architecture-decisions.md` (ADR) | L2 |
| 4.11-d | 记录关键设计决策：HirParam 重复、Emitter trait、L1 PHI、check_visibility | L1 |

## 4. 基准测试设计

由于环境无 criterion 依赖，采用轻量级方案：
- 使用 `std::time::Instant` 手动计时
- 基准测试编译各种规模的 Landin 程序
- 输出编译时间到 stdout（不依赖外部 crate）
- 测试文件在 `benches/` 目录，通过 `[[bench]]` 注册

## 5. ADR 内容

记录以下关键设计决策：
1. **ADR-001**: HirParam 重复（Stage 1.1 接受）
2. **ADR-002**: Emitter trait 36 方法（Stage 3.59 记录）
3. **ADR-003**: L1 PHI 优化 — 依赖 LLVM mem2reg（Stage 4.2 关闭）
4. **ADR-004**: 可见性强制检查 — same-crate 访问（Stage 4.3）
5. **ADR-005**: 闭包捕获 — Copy 模式（Stage 4.7）

## 6. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets -- -D warnings` 通过
3. `cargo fmt --check` 通过
4. `benches/` 目录存在且可运行
5. ADR 文档完整
6. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
