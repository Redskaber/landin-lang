# Stage 5 Gate Review Round 40 (5.40)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.40 (stdlib vtable symbol name planner)
> **基线版本**: v0.11.35 → v0.11.36
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (921.7 MiB removed)
cargo test: 1206 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 5 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `stdlib_vtable_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` ✅ |
| `stdlib_dynptr_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` ✅ |
| `stdlib_data_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` ✅ |
| `stdlib_impl_method_symbol` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_method_symbols` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |

## 3. 设计要点

1. **严格复现 codegen 现有命名约定**（逐字节一致）：
   - `stdlib_vtable_global_name(t, T)` = `format!(".vtable.{t}.{T}")` ↔ codegen `mod.rs:145`
   - `stdlib_dynptr_global_name(t, T)` = `format!(".dynptr.{t}.{T}")` ↔ codegen `mod.rs:184`
   - `stdlib_data_global_name(T)` = `format!(".data.{T}")` ↔ codegen `text_emitter.rs:565`
   - `stdlib_impl_method_symbol(T, m)` = `format!("landin_{T}_{m}")` ↔ `resolver.rs:235`
   - 测试 `test_stdlib_vtable_global_name_match_codegen_format` + 
     `test_stdlib_vtable_method_symbols_match_codegen_format` 显式交叉验证

2. **`stdlib_vtable_method_symbols` 组合 Stage 5.39 plan + impl symbol**：
   - 遍历 `stdlib_vtable_plan(trait, provided)` entries
   - `provided=true` → `stdlib_impl_method_symbol(type, method_name)`
   - `provided=false` → `"null"` 字符串（codegen 直接 emit 为 null pointer）
   - 未注册 trait → `None`
   - 这是 codegen emit `@.vtable.<trait>.<type>` 全局时所需方法的完整字符串列表

3. **§16 自包含**：所有新 API 输入 `&str`，输出 `String` / `Vec<String>`，
   不引用 `codegen::EmitType` / `mir::ty` / `traits::TraitResolver`，无循环依赖。
   纯函数，可在任意阶段调用。

4. **为 Stage 5.41+ codegen 重构做准备**：Stage 5.41+ 将把 codegen 内的
   `format!` 调用替换为对这些函数的调用——行为等价但字符串格式化逻辑
   集中到 stdlib，便于未来调整命名约定（例如加入 module path 前缀）。

## 4. 新测试（16 个）

`tests/v0/stage5/plan/stdlib_vtable_symbol_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_global_name` | `.vtable.Foo.S` / `.vtable.Display.Vec` |
| `test_stdlib_vtable_global_name_match_codegen` | 与 codegen `format!` 一致 |
| `test_stdlib_dynptr_global_name` | `.dynptr.Foo.S` |
| `test_stdlib_data_global_name` | `.data.S` / `.data.Vec` |
| `test_stdlib_impl_method_symbol` | `landin_S_bar` / `landin_Vec_push` |
| `test_stdlib_impl_method_symbol_multi_part` | `landin_MyType_my_method` |
| `test_stdlib_vtable_method_symbols_clone_complete` | Clone + S + 2 方法 → 2 symbols |
| `test_stdlib_vtable_method_symbols_clone_partial` | Clone + S + [clone] → [landin_S_clone, null] |
| `test_stdlib_vtable_method_symbols_drop` | Drop + S + [drop] → 1 symbol |
| `test_stdlib_vtable_method_symbols_partial_eq` | PartialEq + S + [eq] → [landin_S_eq, null] |
| `test_stdlib_vtable_method_symbols_marker` | Copy + S + [] → 空 Vec |
| `test_stdlib_vtable_method_symbols_unknown_trait` | BogusTrait/From/"" → None |
| `test_stdlib_vtable_method_symbols_ordered` | 顺序 = slot_index 升序 |
| `test_stdlib_vtable_method_symbols_match_codegen_format` | 字符串与 codegen format! 一致 |
| `test_stdlib_vtable_method_symbols_arith` | Add + Vec + [add] → [landin_Vec_add] |
| `test_stdlib_vtable_method_symbols_extra_ignored` | 多余 provided 名静默忽略 |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，严格复现 codegen 命名，为 5.41 重构铺路
- Tech Lead: GO — 1206 tests, 0 clippy warnings, 16 新测试含交叉验证
- QA: GO — 覆盖正/负/边界/markers/arith/extra-names/codegen-format-match
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — 全部新 API 遵循 §23

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.41+ (codegen vtable emission refactor)**: 替换 codegen 内的
  `format!` 调用为 `stdlib_vtable_global_name()` /
  `stdlib_dynptr_global_name()` / `stdlib_impl_method_symbol()` /
  `stdlib_vtable_method_symbols()`，行为等价但字符串逻辑集中。
- **Stage 5.42+ (dyn Trait MIR lowering)**: MIR lowering 调用
  `stdlib_vtable_method_symbols()` 获取方法符号列表，构造 vtable 常量。

---

**审查完成**: 2026-07-23
