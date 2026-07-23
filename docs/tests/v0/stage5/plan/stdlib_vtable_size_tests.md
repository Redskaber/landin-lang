# Test Plan: Stage 5.38 — Stdlib Vtable Byte Size + Pointer-Width Layout

> **Stage**: 5.38
> **Version**: v0.11.33 → v0.11.34
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_size_tests.rs`
> **Test count**: 20 new tests (1152 → 1172 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibPointerWidth` + `byte_size()` + `stdlib_pointer_width_bytes()` +
`stdlib_vtable_byte_size()` + `stdlib_vtable_method_offset()` 的正确性。

## 2. 覆盖场景

### 2.1 StdlibPointerWidth + byte_size

- Pointer32 → 4 bytes
- Pointer64 → 8 bytes
- `stdlib_pointer_width_bytes()` free fn 与 method 形式返回一致
- `StdlibPointerWidth` 派生 PartialEq/Eq

### 2.2 stdlib_vtable_byte_size

- Clone@32 → 8 (2×4), Clone@64 → 16 (2×8)
- Drop → 4/8
- PartialEq → 8/16
- Add (single method) → 4/8
- Markers (Copy/Send/Sync/Sized/Unpin/Eq) → Some(0) at both widths
- Unknown (BogusTrait/From/"") → None

### 2.3 stdlib_vtable_method_offset

- Clone::clone@0, clone_from@4 (32bit) / @8 (64bit)
- Drop::drop@0
- PartialEq::eq@0, ne@4 (32bit) / @8 (64bit)
- Add::add@0, Sub::sub@0 (each trait has own slot 0)
- Markers → None (no slots)
- Known trait + unknown method → None (Clone::bogus, Add::sub)
- Unknown trait → None (Bogus::x, From::from, "")

### 2.4 交叉验证（不变量检查）

- `test_stdlib_vtable_offset_within_bounds`: 对 7 个 (trait, method) 对 ×
  2 个宽度，验证 `method_offset < vtable_byte_size` 当两者都为 Some。
  这是 typeck 在 Stage 5.40+ 需要验证的核心安全不变量。

## 3. 测试统计

- 新增: 20 tests
- 基线: 1152 tests
- 总计: 1172 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游: Stage 5.37 (`stdlib_vtable_slot_count` + `stdlib_trait_method_index`)
- 下游: Stage 5.39+ (dyn Trait MIR lowering) — codegen 将使用
  `stdlib_vtable_byte_size()` 决定 `alloca` 大小，
  `stdlib_vtable_method_offset()` 生成 `getelementptr` 偏移

## 5. CI/CD 验证

```
cargo clean: clean (911.7 MiB removed)
cargo test: 1172 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
