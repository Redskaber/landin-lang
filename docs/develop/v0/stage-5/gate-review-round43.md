# Stage 5 Gate Review Round 43 (5.43)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.43 (codegen vtable emission helper)
> **基线版本**: v0.11.38 → v0.11.39
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (952.7 MiB removed)
cargo test: 1249 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 1 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `emit_vtable_global_from_emission` | free fn | `<verb>_<noun>_<adj>_<prep>_<noun>` ✅ |

`emit_` 前缀与 codegen 模块其他 free function 一致
（`emit_vtables` / `emit_dyn_trait_ptrs` / `emit_fat_ptr_type`）。

## 3. 设计要点

1. **第一个修改 codegen 的 Stage 5 子阶段**：5.36-5.42 都是纯 stdlib 查询
   API；本轮首次在 `src/codegen/mod.rs` 添加新函数。但**不修改现有 emission
   路径**——`emit_vtables()` + `TextEmitter::emit_vtable_global()` 保持不变。
2. **纯函数 vs trait method**：新函数是 `TextEmitter::emit_vtable_global()` 的
   纯函数对应版本——输入 `&StdlibVtableEmission`，输出 `String`，无需构造
   `Emitter` trait 对象。可在测试中直接调用验证 LLVM IR 文本。
3. **逐字节一致（除 "null" 处理）**：新函数与 `TextEmitter::emit_vtable_global()`
   在非 null 路径上**逐字节一致**（`test_emit_vtable_global_from_emission_match_text_emitter`
   验证）。新函数额外处理 `"null"` 字符串→`ptr null` 字面量（TextEmitter
   当前路径不处理，因为 `emit_vtables()` 只传真实符号）。
4. **"先并行、后委托"策略**：本轮新函数并行存在；Stage 5.44+ 才让
   `TextEmitter::emit_vtable_global()` 委托给这个 free function，消除重复
   的 LLVM IR 格式化逻辑。这种策略让本轮变更可独立审查。
5. **§16 接口隔离**：输入 `&StdlibVtableEmission`（stdlib 内部类型），输出
   `String`。不引用 `mir::ty` / `traits::TraitResolver` / `Emitter` trait，
   无循环依赖。

## 4. 新测试（13 个）

`tests/v0/stage5/plan/codegen_vtable_emission_helper_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_emit_vtable_global_from_emission_clone` | Clone + S + 2 方法 → 完整 IR |
| `test_emit_vtable_global_from_emission_drop` | Drop + S + [drop] → 1 slot |
| `test_emit_vtable_global_from_emission_marker` | Copy + S + [] → zeroinitializer |
| `test_emit_vtable_global_from_emission_partial` | Clone + S + [clone] → null slot |
| `test_emit_vtable_global_from_emission_arith` | Add + Vec + [add] |
| `test_emit_vtable_global_from_emission_format_global_name` | 全局名格式 |
| `test_emit_vtable_global_from_emission_format_array` | `[N x ptr]` 格式 |
| `test_emit_vtable_global_from_emission_format_entries` | `ptr @sym` 格式 |
| `test_emit_vtable_global_from_emission_null_symbol` | "null" → `ptr null` |
| `test_emit_vtable_global_from_emission_empty_marker_zeroinitializer` | marker → zeroinitializer |
| `test_emit_vtable_global_from_emission_match_text_emitter` | **与 TextEmitter 逐字节一致** |
| `test_emit_vtable_global_from_emission_match_text_emitter_marker` | marker 路径交叉验证 |
| `test_emit_vtable_global_from_emission_partial_eq` | PartialEq + [eq] → [ptr @landin_S_eq, ptr null] |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，"先并行后委托"策略降低风险
- Tech Lead: GO — 1249 tests, 0 clippy warnings, 交叉验证测试保证 LLVM IR 一致性
- QA: GO — 13 新测试覆盖正/负/边界/markers/partial/null/codegen-match
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — `emit_vtable_global_from_emission` 遵循 §23 `<verb>_<noun>_<adj>_<prep>_<noun>`

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.44+ (codegen vtable emission refactor)**: 让 `TextEmitter::emit_vtable_global()`
  委托给 `emit_vtable_global_from_emission()`，消除重复的 LLVM IR 格式化逻辑。
- **Stage 5.45+ (dyn Trait MIR lowering)**: MIR lowering 直接调用这个 free function
  生成 vtable 全局文本。

---

**审查完成**: 2026-07-23
