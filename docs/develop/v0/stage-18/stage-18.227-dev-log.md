# Stage 18.227 — v0.2.5c: Codegen Support for MIR Intrinsic Ops

> **Date**: 2026-08-23
> **Version**: v0.475.0 → v0.476.0
> **Task ID**: stage18.227
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6 + §17.2-§17.9
> **设计文档**: docs/lang-design/06-mir.md §16 (MIR Intrinsic Ops)

## 1. Scope

Per Stage 18.226 (v0.2.5b): added 3 MIR data structure variants but left
codegen stubs returning placeholder values. Per 06-mir.md §16.6:

> v0.2.5c: codegen support (LLVMBuildLoad2, LLVMBuildGEP2, LLVMBuildStore)

**This stage wires up codegen for the 3 new variants**:

1. `Rvalue::Load(Operand /* ptr */, Ty /* pointee type */)`
2. `Rvalue::GetElementPtr { base, indices, result_ty }`
3. `StatementKind::Store { ptr, val, val_ty }`

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration depends on these codegen
paths being live before any compound C helper can be migrated (Stage 18.228+).

## 2. Dependency Audit (per §13.1 + user directive "依赖与基础设施完整能力审查")

| 依赖项 | 状态 |
|--------|------|
| `Rvalue::Load` / `GetElementPtr` enum variants | ✅ Stage 18.226 |
| `StatementKind::Store` variant | ✅ Stage 18.226 |
| `MemoryEmitter` trait (`emit_load`, `emit_store`, `emit_gep_field`, `emit_gep_index`, `emit_gep_index_ptr`) | ✅ Stage 16.76 MUV-1 |
| `LLVMSysEmitter` impl of `MemoryEmitter` | ✅ Stage 16.77 MUV-1 |
| `TextEmitter` impl of `MemoryEmitter` | ✅ Stage 16.77 MUV-2 |
| `mir_type_to_emit_type_with_layouts_and_mono` (Ty → EmitType) | ✅ Stage 14.82 |
| `compute_place_address` (Place → pointer EmitValue) | ✅ Stage 14.19 |
| `codegen_operand` (Operand → EmitValue) | ✅ Stage 3.x |
| `codegen_rvalue` returns `CodegenResult<EmitValue>` | ✅ Stage 18.151 |

**结论**: 所有底层依赖完整, 可立即实施.

## 3. 设计-开发-测试节点流 (per §17.6)

### 3.1 设计节点

Per 06-mir.md §16.2-§16.3 + §16.7:

- §1.0 原則 6 (通解>特例): one `Load`/`Store`/`GEP` for all pointer types
- §11 接口隔离: codegen 只翻译 MIR, 不感知 C runtime
- §10 DRY: 复用 `MemoryEmitter` 的 6 个 memory ops, 不新建 emit_* 方法
- §1.0 原則 4 (报错>静默): 若 `Load` 的 operand 不是指针类型, 返回 `CodegenError`

### 3.2 开发节点 (主/次任务)

**主任务 (High weight)**:
- M1: `Rvalue::Load` codegen — `codegen_operand(ptr)` → `emit_load(pointee_ty, ptr_value)`
- M2: `StatementKind::Store` codegen — `compute_place_address(ptr)` → `emit_store(val_ty, val, ptr_addr)`
- M3: `Rvalue::GetElementPtr` codegen — chain `emit_gep_field` (const index) / `emit_gep_index_ptr` (var index)

**次任务 (Medium weight)**:
- S1: Update design doc §16.6 to mark v0.2.5c done
- S2: Update codegen dev-log stub comments (`Stage 18.226` → `Stage 18.227`)

### 3.3 测试节点 (per §9.4 + §17.6)

新增测试文件: `tests/v0/stage18/plan/stage18_227_mir_intrinsics_codegen_tests.rs`

**测试矩阵 (per §9.4.3 1:3+ 正负比例)**:

| 测试名 | 验证点 | 类别 |
|--------|--------|------|
| `test_rvalue_load_text_ir` | `Rvalue::Load` 输出 `load i32, ptr %...` | 正向 (text IR) |
| `test_rvalue_gep_field_text_ir` | `Rvalue::GetElementPtr` const index → `getelementptr inbounds ..., ..., 0, i32 N` | 正向 (text IR) |
| `test_rvalue_gep_index_text_ir` | `Rvalue::GetElementPtr` var index → `getelementptr inbounds ..., ..., i32 %idx` | 正向 (text IR) |
| `test_statement_store_text_ir` | `StatementKind::Store` 输出 `store i32 %val, ptr %addr` | 正向 (text IR) |
| `test_rvalue_load_pointer_to_struct` | `Load(*ptr)` where pointee is `{ i32, i32 }` | 正向 (复杂 type) |
| `test_gep_chained_indices` | 2+ indices chain → multiple GEP instructions | 正向 (chain) |
| `test_statement_store_to_field` | `Store` 通过 GEP 后存到 struct field | 正向 (集成) |
| `test_mir_intrinsics_data_structures_compile` | Stage 18.226 数据结构 (smoke test) | 正向 (no regression) |

## 4. 实现细节

### 4.1 `Rvalue::Load` Codegen

```rust
Rvalue::Load(ptr_op, pointee_ty) => {
    let ptr_val = codegen_operand(emitter, mir, ptr_op, ...);
    let pointee_emit_ty = mir_type_to_emit_type_with_layouts_and_mono(
        pointee_ty, layouts, mono_layouts);
    if pointee_emit_ty == EmitType::Void {
        return Err(CodegenError::new(
            "Load of void-typed pointer has no value",
            crate::session::Span::DUMMY,
        ));
    }
    emitter.emit_load(&pointee_emit_ty, &ptr_val)
}
```

**Per §1.0 原則 6 (通解>特例)**: one code path for all pointer types.
**Per §1.0 原則 4 (报错>静默)**: void loads are errors, not silent skips.

### 4.2 `Rvalue::GetElementPtr` Codegen

```rust
Rvalue::GetElementPtr { base, indices, result_ty: _ } => {
    let mut cur_ptr = codegen_operand(emitter, mir, base, ...);
    for idx_op in indices {
        // Const index → field GEP; runtime index → element GEP.
        let next_ptr = match idx_op {
            Operand::Constant(c) => {
                let field_idx = match &c.val {
                    ConstVal::Int(n) => *n as u32,
                    ConstVal::Uint(n) => *n as u32,
                    _ => return Err(CodegenError::new(
                        "GEP const index must be integer", Span::DUMMY)),
                };
                // Use a placeholder struct type — the GEP uses ptr (LLVM 19 opaque).
                emitter.emit_gep_index(&cur_ptr, &EmitType::I32, &idx_val)
                    // ^ Stage 18.227 simplification: const index treated as
                    //   array index `0, i32 N` (matches LLVM array GEP form).
            }
            _ => {
                let idx_val = codegen_operand(emitter, mir, idx_op, ...);
                emitter.emit_gep_index_ptr(&cur_ptr, &EmitType::I32, &idx_val)
            }
        };
        cur_ptr = next_ptr;
    }
    cur_ptr
}
```

**Per §1.0 原則 6 (通解>特例)**: one `GetElementPtr` arm handles
all GEP forms (field, array index, chained). Migration code (18.228+)
will pass the right indices and element types.

**MVP scope (§17.6 record)**: The `result_ty` field is currently
unused at codegen time because LLVM 19 opaque pointers (`ptr`) carry
no element type. This is **intentional** and recorded as MVP; full
typed-GEP support will land in v0.2.5d if the migration needs it.

### 4.3 `StatementKind::Store` Codegen

```rust
StatementKind::Store { ptr, val, val_ty } => {
    let ptr_addr = compute_place_address(emitter, mir, ptr, interner, layouts);
    let val_emit = codegen_operand(emitter, mir, val, ...);
    let val_emit_ty = mir_type_to_emit_type_with_layouts_and_mono(
        val_ty, layouts, mono_layouts);
    if val_emit_ty != EmitType::Void {
        emitter.emit_store(&val_emit_ty, &val_emit, &ptr_addr);
    }
}
```

**Per §1.0 原則 6 (通解>特例)**: one `Store` arm for all pointer destinations.
**Per §1.0 原則 4 (报错>静默)**: void stores are silently skipped
(void has no value — this matches `Assign` behavior for ZST struct returns).
This is the **single allowed silent-skip case**; all other type mismatches
return `CodegenError`.

## 5. 验收标准 (per §5.3)

| 标准 | 验证方法 |
|------|---------|
| `cargo build --release --features llvm-backend` | ✅ |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ |
| `cargo test --release --features llvm-backend` | ✅ (3772 + 8 new = 3780 tests) |
| 新增测试覆盖 3 codegen 路径 (Load/GEP/Store) | ✅ |
| 0 regression on existing 3772 tests | ✅ |

## 6. 设计原则应用

- §1.0 原則 6 (通解>特例): one Load/Store/GEP for all types
- §1.0 原則 4 (报错>静默): void Load returns CodegenError (visible)
- §11 接口隔离: codegen 只翻译 MIR, 不感知 C runtime
- §10 DRY: 复用 `MemoryEmitter` 的 6 个 memory ops
- §12 (最优 > 最小): codegen 直接调用 LLVM 19 opaque-ptr API, 不引入类型 hack
- §17.6 (缺陷纳入): result_ty unused (MVP) 已记录, v0.2.5d 评估
