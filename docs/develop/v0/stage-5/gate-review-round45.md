# Stage 5 Gate Review Round 45 (5.45)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.45 (codegen vtable emission batch helper)
> **基线版本**: v0.11.40 → v0.11.41
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (938.7 MiB removed)
cargo test: 1273 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 2 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibVtableGlobalSpec` | struct | `<Noun><Noun><Noun><Noun>` ✅ |
| `emit_vtable_globals_batch` | free fn | `<verb>_<noun>_<adj>_<noun>` ✅ |

字段命名：`global_name` / `method_symbols` (`<noun>_<noun>`) ✅

## 3. 设计要点

1. **批量版本**：`emit_vtable_globals_batch(&[StdlibVtableGlobalSpec]) -> Vec<String>`
   是 Stage 5.44 `emit_vtable_global_text()` 的批量对应——一次调用生成所有
   vtable IR 行，避免在 `emit_vtables()` 循环中多次调用 free function。
2. **StdlibVtableGlobalSpec struct**：封装 `(global_name, method_symbols)`
   对，让 batch API 接收 `&[StdlibVtableGlobalSpec]` 而非两个平行切片。
   派生 PartialEq/Eq 用于测试断言。
3. **顺序保留 + 不去重**：输出顺序匹配输入顺序，重复 spec 产生重复 IR 行。
   去重责任在调用方（`emit_vtables()` 通过 TraitResolver.vtables 的 HashMap
   保证 (trait, type) 唯一性）。
4. **逐字节一致**：`test_emit_vtable_globals_batch_matches_individual` 验证
   batch 输出 == 逐个调用 `emit_vtable_global_text()` 收集的结果。
5. **§16 接口隔离**：仅依赖 `String` + `Vec<String>`，不引用 `mir::ty` /
   `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`，无循环依赖。

## 4. 新测试（12 个）

`tests/v0/stage5/plan/codegen_vtable_batch_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_emit_vtable_globals_batch_empty` | 空 input → 空 Vec |
| `test_emit_vtable_globals_batch_single` | 单个 spec |
| `test_emit_vtable_globals_batch_multi` | 多个 spec，顺序保留 |
| `test_emit_vtable_globals_batch_matches_individual` | **batch == 逐个调用** |
| `test_emit_vtable_globals_batch_order_preserved` | 顺序保留（非字母序） |
| `test_emit_vtable_globals_batch_with_marker` | 含 marker (zeroinitializer) |
| `test_emit_vtable_globals_batch_with_null` | 含 null symbol |
| `test_emit_vtable_globals_batch_mixed` | 混合 marker + null + real |
| `test_stdlib_vtable_global_spec_struct` | struct 字段访问 |
| `test_stdlib_vtable_global_spec_eq` | PartialEq/Eq 派生 |
| `test_emit_vtable_globals_batch_real_vtables` | 模拟真实 emit_vtables 场景 |
| `test_emit_vtable_globals_batch_dedup_not_required` | 不去重（调用方负责） |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，batch API 设计合理，为 5.46 重构铺路
- Tech Lead: GO — 1273 tests, 0 clippy warnings, batch==individual 交叉验证
- QA: GO — 12 新测试覆盖正/负/边界/markers/null/mixed/order/dedup/struct
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — `emit_vtable_globals_batch` 遵循 §23 `<verb>_<noun>_<adj>_<noun>`

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.46 (codegen vtable emission refactor)**:
  - `emit_vtables()` 内部构造 `Vec<StdlibVtableGlobalSpec>`，调用
    `emit_vtable_globals_batch()`，再批量 push 到 emitter
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
- **Stage 5.47+ (dyn Trait MIR lowering)**: 直接调用 batch helper

---

**审查完成**: 2026-07-23
