# Stage 140 开发日志 — TD-STR-FAT-PTR-LAYOUT-MISMATCH 部分修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.667.0 (不变 — 部分修复，prelude 回退) |
| 测试数 | 5822 (898 lib + 4924 integration, 不变) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 140 修复 TD-STR-FAT-PTR-LAYOUT-MISMATCH — &str fat pointer field access (.ptr, .len)。

### 完成的修复
1. **`src/mir/lower/field_resolution.rs`**: `resolve_field_index` — 当 receiver 是 &str (Ref to Str)，映射 .ptr→field 0, .len→field 1
2. **`src/mir/lower/field_resolution.rs`**: `resolve_field_type` — 为 &str 的 field 0 返回 RawPtr(Mut, U8)，field 1 返回 Uint(Usize)
3. **`src/codegen/mir_translation/places.rs`**: `unwrap_fat_ptr_for_index` — 添加 Ptr(_) 和 OpaquePtr 分支，返回 Some(pointee_ty)
4. **`src/codegen/mir_translation/places.rs`**: `detect_place_storage_type` OpaquePtr 分支 — 对非 Local base 直接返回 raw_ty

### 未完成
- String::starts_with/ends_with/contains 方法体被 LLVM module verification failed 阻断
- 根因: codegen 的 Index projection 对 RawPtr 生成错误的 GEP (i32 0, i32 idx 而非 i32 idx)
- `unwrap_fat_ptr_for_index` 的 OpaquePtr 分支需要 LOAD 指针值，但存在双重 load 问题
- 回退 prelude 变更，保留 field_resolution + places.rs 修复（它们是正确的架构改进）

### 新发现 TD
- TD-PTR-INDEX-CODEGEN: RawPtr 索引的 codegen GEP 生成错误 — `unwrap_fat_ptr_for_index` 的 OpaquePtr 分支需要正确的 load + GEP 路径

### §3.2 验收
- fmt clean, 0 clippy warnings
- 898 lib + 4924 integration = 5822 tests, 0 failures, 12 ignored

## Stage Summary
- Stage 140 PASSED — &str field access 部分修复（MIR lower 完成，codegen 待 TD-PTR-INDEX-CODEGEN）
- 0 回归 (5822 tests, 不变)
- v0.667.0 (不变)
