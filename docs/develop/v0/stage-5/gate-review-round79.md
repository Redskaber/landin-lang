# Stage 5 Gate Review Round 79 (5.79)

> **审查日期**: 2026-07-24 | **版本**: v0.11.74 → v0.11.75
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## CI/CD

```
cargo clean: clean (778.8 MiB removed)
cargo test: 1626 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 新增 API

| 符号 | 类型 | §23 |
|------|------|-----|
| `emit_dyn_trait_method_call` | Emitter trait method + TextEmitter impl | `<verb>_<noun>_<noun>_<noun>_<noun>` ✅ |
| `codegen_dyn_trait_call` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>` ✅ |

## 设计要点

1. **FIRST codegen integration** — `Terminator::Call` 分支顶部检测
   `Const{ty: Error, val: Int(index)}` marker，匹配时 dispatch 到
   dyn Trait path
2. **四指令 LLVM IR 序列**：
   - `getelementptr { ptr, ptr }, ptr @<dynptr_symbol>, i32 0, i32 1`
     — 取 dynptr 的 vtable 指针字段
   - `load ptr, ptr %gep` — 加载 vtable 指针
   - `load ptr, ptr %vtable, i32 <slot_index>` — 加载方法函数指针
   - `call <ret_ty> %method_fn(<args>)` — 间接调用
3. **三重 marker 检测条件**（保证向后兼容）：
   - `func` 是 `Operand::Constant`
   - `c.ty.kind` 是 `TyKind::Error`
   - `c.val` 是 `ConstVal::Int(idx)` 且 `idx < mir.dyn_trait_calls.len()`
4. 不匹配时回退到原 legacy direct-call 路径——所有 1611 个已有测试不变通过
5. §16 合规：MIR 通过 `dyn_trait_calls` side-table 携带所有 dyn Trait 信息，
   codegen 不查询 HIR/TraitResolver
6. 15 个新测试覆盖：emitter 基本/IR 指令/dynptr 引用/slot_index/void ret/
   与 direct call 区分/codegen_dyn_trait_call 多场景/marker shape/多索引/IR 格式

## 与 5.78 的关系

| Stage | 角色 |
|-------|------|
| 5.78 | mir/lower 写入 `mir.dyn_trait_calls` + Const marker |
| 5.79 | codegen 读取 side-table + 翻译 marker 为 vtable indirect call IR |

5.78 + 5.79 一起构成完整的 dyn Trait MIR lowering → codegen pipeline。

**5/5 GO → PASS**

---

**审查完成**: 2026-07-24
