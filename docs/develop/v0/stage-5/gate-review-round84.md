# Stage 5 Gate Review Round 84 (5.84)

> **审查日期**: 2026-07-24 | **版本**: v0.11.79 → v0.11.80
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## CI/CD

```
cargo clean: clean (799.4 MiB removed)
cargo test: 1690 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 新增 API

| 符号 | 类型 | §23 |
|------|------|-----|
| `StdlibTraitMethod.param_kinds` | pub field (in `stdlib`) | `<noun>_<noun>` (plural) ✅ |
| `DynTraitMethodCall.param_kinds` | pub field (in `mir::dyn_trait`) | `<noun>_<noun>` (plural) ✅ |

## 设计要点

1. **Symmetric to Stage 5.82** — return_kind (5.82) + param_kinds (5.84)
   完成 dyn Trait 类型精化的完整覆盖
2. **`&'static [StdlibTypeKind]`** for StdlibTraitMethod — 保持 `Copy` +
   `&'static` 静态表设计；用 `EMPTY_PARAM_KINDS` const 处理零参数方法
3. **`Vec<StdlibTypeKind>`** for DynTraitMethodCall — owned（与现有 String 字段一致）
4. **Breaking change**：DynTraitMethodCall::new() 和 from_fat_ptr() 新增
   param_kinds 参数——所有调用点更新（14 个测试文件 + 1 个 source file + 1 个 struct literal 测试）
5. **codegen 集成**：`codegen_dyn_trait_call` 现在为每个参数精确推断 EmitType
   （self→OpaquePtr, explicit args→param_kinds[i-1]→stdlib_type_kind_to_emit_type）
6. §16 合规：stdlib → mir::dyn_trait → codegen 单向，无循环依赖
7. 14 个新测试覆盖：param_kinds 字段 + codegen 集成 + build 集成

## 不在本 stage 范围

- ❌ 用户自定义 trait 的参数类型（仅 stdlib traits）
- ❌ self 参数类型精化（self 始终是 fat pointer → OpaquePtr）
- ❌ mir/lower 拆分（TD-011, Stage 6）

**5/5 GO → PASS**

---

**审查完成**: 2026-07-24
