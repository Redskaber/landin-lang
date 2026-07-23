# Test Plan: Stage 5.43 — Codegen Vtable Emission Helper

> **Stage**: 5.43
> **Version**: v0.11.38 → v0.11.39
> **Test file**: `tests/v0/stage5/plan/codegen_vtable_emission_helper_tests.rs`
> **Test count**: 13 new tests (1236 → 1249 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_vtable_global_from_emission()` free function 的正确性。

**关键不变量**：输出与 `TextEmitter::emit_vtable_global()` 在非 null 路径上
**逐字节一致**——`test_emit_vtable_global_from_emission_match_text_emitter`
+ `test_emit_vtable_global_from_emission_match_text_emitter_marker` 显式
交叉验证（构造 TextEmitter，调用 trait method，断言 free fn 输出出现在
TextEmitter 输出中）。

## 2. 覆盖场景

### 2.1 基本 emission

- Clone + S + [clone, clone_from] → 完整 2-slot IR
- Drop + S + [drop] → 1-slot IR
- Copy + S + [] → marker, zeroinitializer
- Clone + S + [clone] → 2-slot IR with `ptr null` for missing clone_from
- Add + Vec + [add] → 1-slot arith IR
- PartialEq + S + [eq] → 2-slot IR with `ptr null` for missing ne

### 2.2 格式组件

- 全局名格式 `.vtable.<trait>.<type>`
- 数组类型 `[N x ptr]`
- 条目格式 `ptr @sym`
- "null" 字符串 → `ptr null` 字面量（无 `@` 前缀）
- marker → `zeroinitializer`（无数组类型）

### 2.3 codegen 一致性交叉验证

- `test_emit_vtable_global_from_emission_match_text_emitter`:
  构造 `StdlibVtableEmission` 手动（含真实符号），同时调用 free fn 和
  `TextEmitter::emit_vtable_global()`，断言 free fn 输出出现在 TextEmitter
  的 `output_with_globals()` 中。
- `test_emit_vtable_global_from_emission_match_text_emitter_marker`:
  marker 路径交叉验证（zeroinitializer）。

## 3. 测试统计

- 新增: 13 tests
- 基线: 1236 tests
- 总计: 1249 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.41 (`StdlibVtableEmission` + `stdlib_vtable_emission()`)
  - 现有 `TextEmitter::emit_vtable_global()` (Stage 5.6)
- 下游:
  - Stage 5.44+ (codegen vtable emission refactor) — `TextEmitter::emit_vtable_global()`
    将委托给这个 free function
  - Stage 5.45+ (dyn Trait MIR lowering) — 直接调用 free function

## 5. CI/CD 验证

```
cargo clean: clean (952.7 MiB removed)
cargo test: 1249 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
