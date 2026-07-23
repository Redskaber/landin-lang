# Test Plan: Stage 5.37 — Stdlib Vtable Slot Layout

> **Stage**: 5.37
> **Version**: v0.11.32 → v0.11.33
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_layout_tests.rs`
> **Test count**: 22 new tests (1130 → 1152 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibVtableSlot` + `stdlib_trait_method_index()` +
`stdlib_vtable_layout()` + `stdlib_vtable_slot_count()` +
`is_stdlib_marker_trait()` + `stdlib_traits_with_vtable()` 的正确性。

## 2. 覆盖场景

### 2.1 method_index 查询

- Clone::clone@0, clone_from@1
- Drop::drop@0
- PartialEq::eq@0, ne@1
- Add::add@0, Sub::sub@0 (每个 arith trait 独立 slot 0)
- 未注册 trait (BogusTrait/From/"") → None
- 已知 trait 未知方法 (Clone::bogus, Clone::next, Add::sub) → None
- Markers (Copy/Send/Sync/Sized/Unpin/Eq) → None (无 slot)

### 2.2 vtable_layout 查询

- Clone 完整布局（2 slots, names + indices 正确）
- Drop 完整布局（1 slot）
- Marker 布局为空 Vec（非 None）
- 未注册 trait → None
- 重复查询返回相同顺序（deterministic）
- Add/Sub 各自方法名正确（per-op const table 验证）

### 2.3 vtable_slot_count

- Clone=2, Drop=1, Default=1, PartialEq=2, Add=1, Iterator=1
- Markers: Copy=0, Send=0, Eq=0
- 未注册: BogusTrait=None, ""=None

### 2.4 is_stdlib_marker_trait

- True: Copy/Send/Sync/Sized/Unpin/Eq (6 markers)
- False: Clone/Drop/Default/Add/Iterator
- False: BogusTrait/From/"" (未注册，不算 marker)

### 2.5 stdlib_traits_with_vtable

- 包含 Clone/Drop/Add/Iterator
- 不包含任何 marker (Copy/Send/Sync/Sized/Unpin/Eq)
- 总数 ≥ 20（13 core + 2 I/O + 2 unary + 10 binary + 10 assign = 37）

### 2.6 StdlibVtableSlot struct

- 字段访问：slot_index + method (含 method.name, method.self_kind)
- PartialEq/Eq 派生：相同 slot 相等，不同 slot 不等

## 3. 测试统计

- 新增: 22 tests
- 基线: 1130 tests
- 总计: 1152 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游: Stage 5.36 (`StdlibTraitMethod` + `stdlib_trait_methods()`)
- 下游: Stage 5.38+ (dyn Trait MIR lowering) — 将使用
  `stdlib_vtable_layout()` 决定 vtable 全局 element count，
  `stdlib_trait_method_index()` 计算 method call 字节偏移

## 5. CI/CD 验证

```
cargo clean: clean (907.8 MiB removed)
cargo test: 1152 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
