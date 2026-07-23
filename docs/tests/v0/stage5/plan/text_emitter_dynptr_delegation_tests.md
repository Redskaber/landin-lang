# Test Plan: Stage 5.58 — TextEmitter::emit_dyn_trait_const Delegation

> **Stage**: 5.58
> **Version**: v0.11.53 → v0.11.54
> **Test file**: `tests/v0/stage5/plan/text_emitter_dynptr_delegation_tests.rs`
> **Test count**: 10 new tests (1418 → 1428 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()`
(Stage 5.48) 后的正确性。

**关键不变量**：
1. 所有路径：委托输出与 free function 输出**逐字节一致**
2. 无回归：所有现有 dynptr codegen 测试通过

## 2. 覆盖场景

### 2.1 基本委托正确性

- 基本 case
- 格式验证（完整 IR 行）
- Foo + S 例子

### 2.2 无回归

- `emit_dyn_trait_ptrs()` 在委托后仍正确工作（内部调用 `emit_dyn_trait_const()`）

### 2.3 委托输出 == free function 输出

### 2.4 Emitter globals + 返回值 + symbol 正确性

### 2.5 真实场景 + 多个 dynptr globals

## 3. 测试统计

- 新增: 10 tests
- 基线: 1418 tests
- 总计: 1428 tests
- 2 ignored (pre-existing)

## 4. CI/CD 验证

```
cargo clean: clean (926.7 MiB removed)
cargo test: 1428 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
