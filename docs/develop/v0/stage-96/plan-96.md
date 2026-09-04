# Stage 96 开发计划 — Ord trait (marker only)

> **阶段**: v0.9 (Prelude Trait Coverage Wave)
> **TD**: TD-PRELUDE-TRAIT-COVERAGE 续
> **复杂度**: L2 (~10 行 prelude.rs + 4 tests + docs sync)
> **版本基线**: v0.634.0 (Stage 95, 5572 tests)
> **目标版本**: v0.635.0

## 一、5W2H 启动分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 添加 `Ord` trait 到 prelude (marker only, no method body) |
| **WHY** | Rust prelude 有 Ord — total ordering trait。但 Debug + PartialOrd 的 impl bodies (含 if/match 返回 String/Option) 触发 codegen SIGSEGV (新 TD-PRELUDE-IMPL-CODEGEN-CRASH)。本阶段先验证 marker trait 无回归 |
| **WHO** | DEV-A 主导；ARCH-A 评估新 TD |
| **WHEN** | Stage 96 完成 → 进入 Stage 97 (TD 根因调查) |
| **WHERE** | `src/stdlib/prelude.rs` (Eq 后追加) + `tests/v0/stage96/plan/ord_trait_tests.rs` |
| **HOW** | `trait Ord {}` + 4 marker impls (i32/i64/bool/usize) |
| **HOW MUCH** | ~10 LOC + 4 测试。零回归 (5572→5576) |

## 二、关键发现 (新 TD)

### 新 TD: TD-PRELUDE-TRAIT-IMPL-CODEGEN-CRASH (P3, v0.10+)

**发现**: Debug::fmt body (returning String) 和 PartialOrd::partial_cmp body (returning Option<i32>) 触发 codegen SIGSEGV。

**根因初判**: codegen 对 prelude impl method body 的处理不完整 — 任何含复杂控制流 (if/match) 返回 String/Option 的 impl body 都会 crash。

**临时方案**: 只添加 marker traits (无 body), 等 Stage 97 调查根因。

## 三、决策点 (§12 最优>最小)

### 决策 1: 只添加 Ord (marker), 不添加 Debug/PartialOrd

**选择**: Ord marker only。

**理由**:
- 先验证 marker trait 无回归。
- Debug/PartialOrd 需 impl body, 触发 TD-PRELUDE-TRAIT-IMPL-CODEGEN-CRASH。
- 不阉割推进 — Stage 97 转向根因调查。

## 四、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (5576 tests, 0 failures, 9 ignored)
