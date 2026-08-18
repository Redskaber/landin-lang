# Stage 18.183 — 依赖与基础设施完整能力审查：fat pointer Index projection

> **审查日期**: 2026-08-17
> **审查者**: Super Z (ARCH-A + DEV-A + REV-A + PM-A 联合)
> **基线版本**: v0.450.0 (Stage 18.182)
> **触发条例**: 用户指令 "如果在设计和开发过程中设计的内容需要依赖底层实现和功能时（时机：
>   触发此条例的时机），应当先做依赖与基础设施完整能力审查"
> **Task ID**: stage18.183

---

## 1. 触发场景

### 1.1 任务目标

Stage 18.183: 修复 fat pointer Index projection (`s[0]` for str/切片)。
当前 `s[0]` 直接 codegen 错误 "GEP base pointer is not a vector or a vector of pointers"。

### 1.2 触发条例

用户指令: "如果在设计和开发过程中设计的内容需要依赖底层实现和功能时（时机：触发此条例的时机），
应当先做依赖与基础设施完整能力审查（即，当前项目的基础设施能力和当前任务的前置项（依赖）是否完整
且具备完整完全设计实现当前任务的能力，前置项的设计、开发、测试是否完善（编码、文档），
是否存在其他的依赖性问题等）"

fat pointer Index projection 依赖多项底层能力，必须先审计。

---

## 2. 依赖项审计

### 2.1 MIR 层依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| `ProjectionElem::Index(LocalId)` MIR 表示 | ✅ | src/mir/place.rs:48 |
| Index 的 MIR lower (`lower_expr_to_place`) | ✅ | src/mir/lower/call_lower.rs:73 |
| DCE 正确收集 Index 的 idx_local | ✅ (Stage 18.182 fix) | src/mir/optimization.rs:91 |
| const_prop 不破坏 Index projection | ✅ | 测试验证 arr[i] with constant i |

**结论**: MIR 层依赖完整。

### 2.2 Codegen 层依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| `EmitType::Struct([ptr, i64])` fat pointer 表示 | ✅ | emit_fat_ptr_type |
| `emit_gep_field` GEP 到 struct 字段 | ✅ | AggregateEmitter trait |
| `emit_extractvalue` 从 struct value 提取字段 | ✅ | AggregateEmitter trait |
| `emit_load` 从指针加载值 | ✅ | MemoryEmitter trait |
| `emit_gep_index_ptr` 按索引 GEP 到指针 | ✅ | AggregateEmitter trait |
| `unwrap_fat_ptr_for_index` 辅助函数 | 🟡 有 bug | 见 §3 |
| Index codegen 对 fat pointer 的处理 | 🟡 有 bug | 见 §3 |

**结论**: 底层 emit 能力完整, 但 `unwrap_fat_ptr_for_index` + Index codegen 有 bug。

### 2.3 类型系统依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| `TyKind::Ref(_, _, Str)` &str 类型 | ✅ | mir/ty.rs:112 |
| `TyKind::Ref(_, _, Slice(T))` &[T] 类型 | ✅ | mir/ty.rs:118 |
| `TyKind::Str` bare str 类型 | ✅ | mir/ty.rs:109 |
| `TyKind::Slice(T)` bare [T] 类型 | ✅ | mir/ty.rs:118 |
| `detect_place_storage_type` 正确识别 fat pointer | ✅ | 测试验证 &str field projection |
| `detect_place_type` 正确识别 fat pointer | ✅ | Stage 18.174 fix |

**结论**: 类型系统依赖完整。

### 2.4 测试依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| str::len() intrinsic (Stage 18.173) | ✅ | mir/lower/expr_variants.rs:989 |
| str Field projection (Stage 18.174) | ✅ | fat pointer Field projection 修复 |
| str == / != 比较 | ✅ | __landin_str_eq |
| 数组 [T; N] Index (Stage 18.182) | ✅ | 刚修复 |

**结论**: 测试基础设施完整。

---

## 3. Bug 分析

### 3.1 Bug 1: `unwrap_fat_ptr_for_index` 不加载数据指针

**当前代码** (src/codegen/mir_translation/places.rs:430):
```rust
pub(crate) fn unwrap_fat_ptr_for_index(
    emitter: &mut dyn Emitter,
    base_ptr: &str,
    storage_ty: &EmitType,
) -> (String, Option<EmitType>) {
    match storage_ty {
        EmitType::Struct(fields) if fields.len() == 2 => {
            let is_fat_ptr = fields[0].is_ptr() && fields[1] == EmitType::I64;
            if is_fat_ptr {
                let data_ptr = emitter.emit_gep_field(&base_ptr_owned, storage_ty, 0);
                // ❌ BUG: 只 GEP 到 field 0 的地址, 没有 LOAD 数据指针
                let pointee_ty = fields[0].pointee();
                (data_ptr, Some(pointee_ty))
            } else {
                (base_ptr.to_string(), None)
            }
        }
        _ => (base_ptr.to_string(), None),
    }
}
```

**问题**: `emit_gep_field` 返回 field 0 的 ADDRESS (指向指针的指针),
但后续 `emit_gep_index_ptr` 期望的是数据指针本身。

### 3.2 Bug 2: Index codegen 对 fat pointer 加载了 VALUE 而非用 ADDRESS

**当前代码** (src/codegen/mir_translation/places.rs:692-698):
```rust
let base_ptr = if let PlaceKind::Local(id) = &base.kind {
    let local_ty = mir.local_decls.get(id.0 as usize).map(|ld| ld.ty.clone());
    if let Some(ty) = local_ty {
        if matches!(&ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
            // ❌ 对 &str (fat pointer), 加载了 VALUE { ptr, i64 }
            //    而非使用 alloca pointer (ADDRESS)
            let ptr_ty = detect_place_type(mir, base, layouts);
            codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
        } else {
            // ✅ 对 [T; N] 数组, 使用 alloca pointer
            emitter.local_ptr(id.0).cloned().unwrap_or_else(|| "0".to_string())
        }
    }
    ...
};
```

**问题**: 对 `&str`, `codegen_place_load_typed` 加载了 fat pointer 的 VALUE
(`{ ptr, i64 }`), 然后 `unwrap_fat_ptr_for_index` 试图 GEP 进这个 VALUE
(但 GEP 需要指针, 不是值) → "GEP base pointer is not a vector" 错误。

### 3.3 修复方案

**方案 A** (最小改动):
1. `unwrap_fat_ptr_for_index`: GEP 到 field 0 后, 加 `emit_load` 加载数据指针
2. Index codegen: 对 `Ref(_, _, Str)` / `Ref(_, _, Slice(_))`, 用 alloca pointer
   (不走 `codegen_place_load_typed` 加载 value 的路径)

**方案 B** (更通解): 区分 base_ptr 是 VALUE 还是 ADDRESS, 分别用 extractvalue / gep+load

**选择**: 方案 A — 最小改动, 直接修复 bug, 不改变接口签名。
Per §1.0 原則 6 (通解>特解): fat pointer 的处理统一走 alloca + GEP + load 路径,
与数组 `[T; N]` 的处理一致 (都用 alloca pointer)。

---

## 4. 能力结论

### 4.1 依赖完整性

✅ **所有依赖项完整** — MIR/codegen/类型系统/测试基础设施均就绪。

### 4.2 阻塞项

🟡 **2 个 codegen bug** (unwrap_fat_ptr_for_index + Index codegen),
非基础设施缺失, 是实现 bug。可立即修复。

### 4.3 任务图确认

**不需要重排** — Stage 18.183 可按计划执行, 依赖完整。

---

## 5. 修复计划

### 5.1 修复 unwrap_fat_ptr_for_index (src/codegen/mir_translation/places.rs)

```rust
if is_fat_ptr {
    let field_addr = emitter.emit_gep_field(&base_ptr_owned, storage_ty, 0);
    // Stage 18.183: LOAD the data pointer from field 0's address
    let data_ptr = emitter.emit_load(&fields[0], &field_addr);
    let pointee_ty = fields[0].pointee();
    (data_ptr, Some(pointee_ty))
}
```

### 5.2 修复 Index codegen (src/codegen/mir_translation/places.rs)

```rust
if matches!(&ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
    // Stage 18.183: For fat pointer (&str, &[T]), use alloca pointer
    // (not loaded value) so unwrap_fat_ptr_for_index can GEP+load.
    let is_fat_ref = matches!(&ty.kind,
        crate::mir::ty::TyKind::Ref(_, _, inner)
            if matches!(&inner.kind,
                crate::mir::ty::TyKind::Str
                    | crate::mir::ty::TyKind::Slice(_)
            )
    );
    if is_fat_ref {
        // Fat pointer: use alloca pointer, let unwrap handle GEP+load
        emitter.local_ptr(id.0).cloned().unwrap_or_else(|| "0".to_string())
    } else {
        // Thin pointer (&[T; N]): load the pointer value (unchanged)
        let ptr_ty = detect_place_type(mir, base, layouts);
        codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
    }
}
```

### 5.3 测试计划

- 正向: s[0] for &str, s[1], s[N-1], &[T] slice indexing, byte value correct
- 负向: s[out_of_bounds] (soft — bounds check is TD-ARRAY-BOUNDS-CHECK)
- 集成: str::len() + s[0] 组合, &str + arr[N] 组合

---

## 6. §3.2 验收 (审查 stage, 无代码变更)

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend: 658 lib + 3004 integration = 3662 total, 0 failed

---

## 7. 结论

**依赖与基础设施完整能力审查通过** — Stage 18.183 可按计划执行。
2 个 codegen bug 已定位 (unwrap_fat_ptr_for_index + Index codegen),
修复方案已确定 (方案 A: 最小改动, alloca+GEP+load 统一路径)。
