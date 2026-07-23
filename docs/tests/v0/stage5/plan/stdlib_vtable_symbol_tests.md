# Test Plan: Stage 5.40 — Stdlib Vtable Symbol Name Planner

> **Stage**: 5.40
> **Version**: v0.11.35 → v0.11.36
> **Test file**: `tests/v0/stage5/plan/stdlib_vtable_symbol_tests.rs`
> **Test count**: 16 new tests (1190 → 1206 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `stdlib_vtable_global_name()` + `stdlib_dynptr_global_name()` +
`stdlib_data_global_name()` + `stdlib_impl_method_symbol()` +
`stdlib_vtable_method_symbols()` 的正确性。

**关键不变量**：生成的字符串必须与 codegen 当前 `format!` 输出**逐字节一致**
——Stage 5.41+ 重构 codegen 时行为等价。

## 2. 覆盖场景

### 2.1 单字符串生成

- `stdlib_vtable_global_name("Foo", "S")` → `.vtable.Foo.S`
- `stdlib_dynptr_global_name("Foo", "S")` → `.dynptr.Foo.S`
- `stdlib_data_global_name("S")` → `.data.S`
- `stdlib_impl_method_symbol("S", "bar")` → `landin_S_bar`
- 多部分名字: `landin_MyType_my_method`

### 2.2 vtable_method_symbols（组合查询）

- Clone + S + [clone, clone_from] → `[landin_S_clone, landin_S_clone_from]`
- Clone + S + [clone] → `[landin_S_clone, null]` (clone_from not provided)
- Drop + S + [drop] → `[landin_S_drop]`
- PartialEq + S + [eq] → `[landin_S_eq, null]` (ne missing)
- Add + Vec + [add] → `[landin_Vec_add]`
- Copy + S + [] → `[]` (marker, empty vtable)
- BogusTrait/From/"" → None
- 顺序 = slot_index 升序
- 多余 provided 名静默忽略

### 2.3 codegen 一致性交叉验证

- `test_stdlib_vtable_global_name_match_codegen`:
  `stdlib_vtable_global_name(t, T) == format!(".vtable.{t}.{T}")`
- `test_stdlib_vtable_method_symbols_match_codegen_format`:
  每个 provided entry == `format!("landin_{type}_{method}")`

## 3. 测试统计

- 新增: 16 tests
- 基线: 1190 tests
- 总计: 1206 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.36 (`StdlibTraitMethod` + `stdlib_trait_methods()`)
  - Stage 5.39 (`stdlib_vtable_plan()`)
- 下游:
  - Stage 5.41+ (codegen vtable emission refactor) — codegen 将用这些
    函数替换 inline `format!` 调用
  - Stage 5.42+ (dyn Trait MIR lowering) — MIR lowering 调用
    `stdlib_vtable_method_symbols()` 获取完整符号列表

## 5. CI/CD 验证

```
cargo clean: clean (921.7 MiB removed)
cargo test: 1206 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
