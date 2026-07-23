# Test Plan: Stage 5.57 — TextEmitter::emit_vtable_global Delegation

> **Stage**: 5.57
> **Version**: v0.11.52 → v0.11.53
> **Test file**: `tests/v0/stage5/plan/text_emitter_vtable_delegation_tests.rs`
> **Test count**: 10 new tests (1408 → 1418 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()` (Stage 5.44)
后的正确性。

**关键不变量**：
1. 非 null 路径：委托输出与 free function 输出**逐字节一致**
2. Null 路径：委托后正确 emit `ptr null`（修复 latent bug）
3. 无回归：所有现有 vtable codegen 测试通过

## 2. 覆盖场景

### 2.1 基本委托正确性

- 基本 case（单 symbol）
- 空 symbols → zeroinitializer
- 单 symbol
- 多 symbols

### 2.2 Null 处理 bug 修复

- `"null"` symbol → `ptr null`（NOT `ptr @null`）

### 2.3 无回归

- `emit_vtables()` 在委托后仍正确工作（内部调用 `emit_vtable_global()`）

### 2.4 委托输出 == free function 输出

- `test_text_emitter_vtable_global_delegation_match_free_fn`: 委托输出含 free fn IR

### 2.5 Emitter globals + 返回值 + 真实场景

- globals Vec 含正确条目
- 返回 global_name（无 `@` 前缀）
- 模拟真实场景：S impls Clone + Drop + Display → 3 vtable globals

## 3. 测试统计

- 新增: 10 tests
- 基线: 1408 tests
- 总计: 1418 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.44 (`emit_vtable_global_text`)
  - 现有 `TextEmitter::emit_vtable_global()` (Stage 5.6) — 被修改
  - 现有 `emit_vtables()` (Stage 5.6) — 用于 no-regression 测试
- 下游:
  - Stage 5.58 (TextEmitter::emit_dyn_trait_const delegation)
  - Stage 5.59 (emit_vtables delegation)

## 5. CI/CD 验证

```
cargo clean: clean (945.8 MiB removed)
cargo test: 1418 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
