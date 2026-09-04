# Stage 95 开发计划 — PartialEq + Eq traits (无 supertrait)

> **阶段**: v0.9 (Prelude Trait Coverage Wave)
> **TD**: TD-PRELUDE-TRAIT-COVERAGE 续
> **复杂度**: L2 (~15 行 prelude.rs + 4 tests + docs sync)
> **版本基线**: v0.633.0 (Stage 94, 5568 tests)
> **目标版本**: v0.634.0

## 一、5W2H 启动分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 添加 `PartialEq<Rhs>` + `Eq` traits 到 prelude (无 supertrait) |
| **WHY** | Rust prelude 有 PartialEq+Eq — `==` 比较的基础。Stage 94 发现 `Eq: PartialEq<Self>` 导致 object safety 测试失败，本阶段采用无 supertrait 方案 |
| **WHO** | DEV-A 主导；ARCH-A 评估语义偏差 |
| **WHEN** | Stage 95 完成 → 进入 Stage 96 (Ord) |
| **WHERE** | `src/stdlib/prelude.rs` (Default 后追加) + `tests/v0/stage95/plan/partial_eq_eq_trait_tests.rs` |
| **HOW** | `trait PartialEq<Rhs> { fn eq(&self, other: &Rhs) -> bool; }` + `trait Eq {}` (无 supertrait) + 4 primitive PartialEq impls + 4 marker Eq impls |
| **HOW MUCH** | ~15 LOC + 4 测试。零回归 (5568→5572) |

## 二、决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协)

### 决策 1: Eq 不带 supertrait

**情境**: Rust 标准库 `trait Eq: PartialEq<Self> {}` — supertrait 强制要求 impl Eq 必须先 impl PartialEq。

**选择**: Landin `trait Eq {}` (无 supertrait)。

**理由**:
- Landin **没有 automatic trait resolution** — supertrait 仅影响 object safety 分析。
- 不带 supertrait 避免干扰 object safety 测试 (`stage16_78_supertrait*`)。
- 用户独立 impl PartialEq + Eq — 与 Landin 当前 v0.9 MVP 一致。
- `==` operator 重载推迟到 v0.10+ (需要 operator overloading)。

**未来路径**: v0.10+ 实现 automatic trait resolution 后，可重新引入 supertrait。

## 三、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 95.1 | 添加 PartialEq trait 声明 + 4 primitive impls | impl 编译通过 |
| 95.2 | 添加 Eq trait 声明 (无 supertrait) + 4 marker impls | marker 通过 |
| 95.3 | 添加 4 测试 (1 positive + 3 negative) | cargo test 全绿 |
| 95.4 | §3.2 验收 + worklog 更新 | fmt/clippy/test 全绿 |

## 四、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (5572 tests, 0 failures, 9 ignored)
