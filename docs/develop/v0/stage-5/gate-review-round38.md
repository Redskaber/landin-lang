# Stage 5 Gate Review Round 38 (5.38)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.38 (stdlib vtable byte size + pointer-width-aware layout)
> **基线版本**: v0.11.33 → v0.11.34
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (911.7 MiB removed)
cargo test: 1172 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 5 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibPointerWidth` | enum | `<Noun><Noun><Noun>` ✅ |
| `StdlibPointerWidth::byte_size` | method | `<noun>_<noun>` ✅ |
| `stdlib_pointer_width_bytes` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_byte_size` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_method_offset` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |

变体命名：`Pointer32` / `Pointer64` (`<Noun><Digits>`) ✅

## 3. 设计要点

1. **指针宽度抽象**：`StdlibPointerWidth` 枚举（Pointer32/Pointer64）让 vtable
   大小计算与目标平台解耦。codegen 在初始化时根据目标三元组选择宽度，
   后续所有 vtable 大小查询都通过这个统一接口。
2. **计算规则**：
   - `vtable_byte_size = slot_count × pointer_width_bytes`
   - `method_offset = slot_index × pointer_width_bytes`
3. **三态返回**（与 Stage 5.37 保持一致）：
   - `Some(0)` — marker trait (registered, no methods, 0-byte vtable)
   - `Some(n)` — trait with n bytes vtable
   - `None` — trait not in registry
4. **§16 自包含**：所有新 API 仅依赖 `StdlibPointerWidth`（stdlib 内部枚举）
   + 已有 `stdlib_vtable_slot_count` / `stdlib_trait_method_index`。不引用
   `codegen::EmitType` 或 `mir::ty`，无循环依赖。
5. **`byte_size()` 是 `const fn`**：可在 const 上下文中使用，方便 codegen
   在编译期计算固定大小。

## 4. 新测试（20 个）

`tests/v0/stage5/plan/stdlib_vtable_size_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_pointer_width_byte_size_32` | Pointer32 → 4 |
| `test_stdlib_pointer_width_byte_size_64` | Pointer64 → 8 |
| `test_stdlib_pointer_width_bytes_free_fn` | free fn 与 method 一致 |
| `test_stdlib_pointer_width_eq` | PartialEq/Eq 派生 |
| `test_stdlib_vtable_byte_size_clone_32` | Clone@32 → 8 |
| `test_stdlib_vtable_byte_size_clone_64` | Clone@64 → 16 |
| `test_stdlib_vtable_byte_size_drop` | Drop → 4/8 |
| `test_stdlib_vtable_byte_size_partial_eq` | PartialEq → 8/16 |
| `test_stdlib_vtable_byte_size_arith` | Add → 4/8 |
| `test_stdlib_vtable_byte_size_marker` | 6 markers → Some(0) at both widths |
| `test_stdlib_vtable_byte_size_unknown` | BogusTrait/From/"" → None |
| `test_stdlib_vtable_method_offset_clone` | Clone::clone@0, clone_from@width |
| `test_stdlib_vtable_method_offset_drop` | Drop::drop@0 |
| `test_stdlib_vtable_method_offset_partial_eq_64` | eq@0, ne@8 |
| `test_stdlib_vtable_method_offset_partial_eq_32` | eq@0, ne@4 |
| `test_stdlib_vtable_method_offset_arith` | Add::add@0, Sub::sub@0 |
| `test_stdlib_vtable_method_offset_marker` | markers → None |
| `test_stdlib_vtable_method_offset_unknown_method` | Clone::bogus → None |
| `test_stdlib_vtable_method_offset_unknown_trait` | Bogus::x → None |
| `test_stdlib_vtable_offset_within_bounds` | 交叉验证 offset < total |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，三态返回与 Stage 5.37 一致
- Tech Lead: GO — 1172 tests, 0 clippy warnings, const fn 优化
- QA: GO — 20 新测试覆盖正/负/边界/markers/交叉验证
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — 全部新 API 遵循 §23

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.39+ (dyn Trait MIR lowering)**: codegen 直接调用
  `stdlib_vtable_byte_size()` 决定 `alloca` 大小；调用
  `stdlib_vtable_method_offset()` 生成 `getelementptr i8, ptr @vtable, i64 offset`。
- **Stage 5.40+ (dyn Trait typeck)**: 验证 method_offset < vtable_byte_size
  (本次测试已交叉验证此不变量)。

---

**审查完成**: 2026-07-23
