# Stage 5.22 开发计划：driver validation integration

> **阶段**: Stage 5.22
> **版本**: v0.11.19 → v0.11.20
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1
> **来源**: Deep review r70 action item (P2)

## 1. 目标

将 `validate_impls()`（Stage 5.20）接入 driver，使 coherence 和
completeness 错误自动报告给用户。

## 2. 设计

### 2.1 `CompileErrors.trait_errors` 字段

新增 `trait_errors: Vec<String>` 字段，存储人类可读的 trait 验证错误消息。

### 2.2 driver `collect()` 后调用 `validate_impls()`

在 `trait_resolver.collect()` 后调用 `validate_impls()`，将 coherence
errors 和 incomplete impls 格式化为字符串存入 `errors.trait_errors`。

### 2.3 命名标准化

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `CompileErrors.trait_errors` | `<noun>_errors` | 与 `lex`/`parse`/`typeck`/`borrowck` 一致 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1016 → 1023, +7 ✅）
4. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-23
