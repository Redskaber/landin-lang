# Stage 94 开发计划 — TD-PRELUDE-TRAIT-COVERAGE partial: Default trait

> **阶段**: v0.9 (Prelude Trait Coverage Wave)
> **TD**: TD-PRELUDE-TRAIT-COVERAGE (Stage 93 审查新发现)
> **复杂度**: L2 (~20 行 prelude.rs + 4 tests + docs sync)
> **版本基线**: v0.632.0 (Stage 92, 5564 tests)
> **目标版本**: v0.633.0

## 一、5W2H 启动分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 添加 `Default` trait 到 prelude (trait 声明 + i32/i64/bool/usize impls) |
| **WHY** | Rust prelude 有 `Default` — 缺失导致用户无法 `T::default()`。这是 v0.9 trait 系统阶段的最小推进，先验证无回归再扩展到 PartialEq/Eq |
| **WHO** | DEV-A 主导；ARCH-A 评估 object safety 影响 |
| **WHEN** | Stage 94 完成 → 进入 Stage 95 (PartialEq+Eq) |
| **WHERE** | `src/stdlib/prelude.rs` (Drop trait 后追加) + `tests/v0/stage94/plan/default_trait_tests.rs` |
| **HOW** | 在 Drop trait 后添加 `trait Default { fn default() -> Self; }` + 4 个 primitive impls |
| **HOW MUCH** | ~20 LOC + 4 测试。零回归 (5564→5568) |

## 二、对齐与决策

### 设计对齐 (§13.1)
- 已查 `docs/develop/v0/stage-93/architecture-audit-report.md` (TD-PRELUDE-TRAIT-COVERAGE P3 v0.9+)
- 已查 `src/stdlib/prelude.rs` (Drop trait 在 line 464-484, Default 应放其后)
- 参考 Rust prelude trait 列表 (std::default::Default)

### 决策点 (§12 最优>最小)
1. **选择"只添加 Default (不含 PartialEq/Eq)"** 而非"一次添加全部 3 个 trait"
   - **理由**: 先验证 Default 无回归，再添加 PartialEq/Eq。
   - PartialEq/Eq 有 supertrait (`Eq: PartialEq<Self>`) 影响 object safety 分析 — Stage 94 实测发现 `Eq: PartialEq<Self>` 导致 2 个 lib test 失败 (`stage16_78_supertrait*`)。需先修复 object safety 分析才能添加 Eq。

### 裁剪点 (§1.2.1)
- L2 → §7.3 门审查替代 §14.5 深度审查

## 三、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 94.1 | 添加 `Default` trait 声明到 prelude | trait 声明存在 |
| 94.2 | 添加 4 个 primitive impls (i32/i64/bool/usize) | impl 编译通过 |
| 94.3 | 添加 4 个测试 (1 positive + 3 negative) | cargo test 全绿 |
| 94.4 | §3.2 验收 + worklog + tech-debt-register 更新 | fmt/clippy/test 全绿 |

## 四、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (5568 tests, 0 failures, 9 ignored)

## 五、下游影响

- **TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION**: `Default::default()` 调用需要 turbofish path 解析才能工作 (目前 `Default::default()` MIR lower 解析错误)
- **TD-PRELUDE-METHOD-COVERAGE**: 后续扩展 prelude 方法覆盖
