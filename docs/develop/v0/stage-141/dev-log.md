# Stage 141 开发日志 — TD-PTR-INDEX-CODEGEN 修复 + TD-STR-FAT-PTR-LAYOUT-MISMATCH 最终修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.667.0 (不变 — 修复导致 25 回归，回退) |
| 测试数 | 5822 (898 lib + 4924 integration, 不变) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 141 尝试修复 TD-PTR-INDEX-CODEGEN — RawPtr 索引的 codegen GEP 生成错误。

### 根因链 (5W2H 深度分析)
1. `self.ptr[i]` where `self.ptr` is `*mut u8` (String struct field 0)
2. MIR lower: `Projection(Local(self), Field(0))` → `Index(idx)`
3. codegen: `base_ptr = compute_place_address` → alloca address of field 0
4. `array_ty = detect_place_storage_type` → `Ptr(I8)` (because RawPtr(Mut, U8) → Ptr(I8))
5. **OLD BUG**: `array_ty` match stripped `Ptr(inner)` to `inner` (I8) → `unwrap_fat_ptr_for_index(I8)` → `_ => (base_ptr, None)` → `emit_gep_index(I8)` → `GEP i8, ptr, i32 0, i32 idx` (WRONG — has leading 0)
6. **FIX 1**: Don't strip Ptr → `unwrap_fat_ptr_for_index(Ptr(I8))` → `Ptr(_) => (base_ptr, Some(I8))` → `emit_gep_index_ptr(base_ptr, I8, idx)` → `GEP i8, ptr base_ptr, i64 idx` (CORRECT format but WRONG base)
7. **FIX 2**: `Ptr(_)` also needs LOAD — `emit_load(Ptr(I8), base_ptr)` → loads pointer value → `GEP i8, ptr loaded_ptr, i64 idx` (FULLY CORRECT!)
8. **REGRESSION**: Fix 2 changes TextEmitter IR output → 25 text IR tests fail (GEP format changed from `i32 0, i32 idx` to `i64 idx`)
9. **DECISION**: Revert all changes (§1.0 原則 9: 正确 > 妥协 — don't ship regression)

### 完成的根因分析 (保留在 worklog 中供 Stage 142 使用)
- `unwrap_fat_ptr_for_index` 的 `Ptr(_)` 分支需要 LOAD 指针值再 GEP
- 但修复后 TextEmitter IR 测试需要更新（25 个测试的 expected IR 改变）
- 回退所有变更，0 回归 (5822/0/12)

### 新发现 TD
- TD-PTR-INDEX-CODEGEN-2: 需要同时修复 codegen + 更新 25 个 text IR 测试

### 决策点
- **选回退而非保留部分修复** — §1.0 原則 9 (正确 > 妥协): 25 个回归不可接受
- **依据**: §3.2 红线 — 0 failures required

### 下一步 (MUV)
Stage 142: 重新应用 codegen 修复 + 批量更新 25 个受影响的 text IR 测试
