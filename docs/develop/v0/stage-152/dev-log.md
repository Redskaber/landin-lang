# Stage 152 开发日志 — TD-OPTION-AND-THEN-I32-MISMATCH 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.675.0 → v0.676.0 |
| 测试数 | 6002 → 6002 (不变 — 修复回归, 恢复 stage40 测试) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
修复 Stage 151 引入的 I64 fallback 导致 prelude 泛型方法 (Option/Result map/and_then) 在 i32 类型上返回 `{i32,i64}` 但调用者期望 `{i32,i32}` 的类型不匹配.

### WHY
Stage 151 在 `adt_layout_to_emit_type` 中对 `Param(N)` 类型使用 I64 fallback. 这对 i64 payload 正确 (Iterator sum 返回 15), 但对 i32 payload 错误 (and_then 返回 `{i32,i64}` 但调用者期望 `{i32,i32}` → bitcast store → 垃圾值).

### HOW
1. Revert Stage 151 的 I64 fallback — 恢复到标准 `mir_type_to_emit_type_with_layouts_and_mono` (对 Param 返回 I32).
2. 新增 `substitute_adt_layout` 函数 — 在 `mir_type_to_emit_type_with_layouts_and_mono` 中, 当 `Adt(def_id, substs)` 的 substs 是具体类型 (非 Param) 时, 用 `substitute` 替换 AdtLayout 的 `variant_payloads` 中的 `Param(N)` → 具体类型. 这处理了 prelude 泛型函数 (如 `Option::and_then<U>`) 中 crate-level AdtLayout 含 Param 但函数 substs 是具体的情况.
3. stage40 测试恢复到原始 i32 版本.

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. 选 substitute_adt_layout 不选 I64 fallback — §1.0 原則 6 (通解: 正确替换 Param, 不猜测类型宽度)
2. 选在 mir_type_to_emit_type_with_layouts_and_mono 中替换 不选在 adt_layout_to_emit_type 中替换 — §1.0 原則 10 (substs from Adt type 是唯一可信数据源, AdtLayout 不携带 substs)

### 发现的新 TD
- TD-CALL-DEST-TYPE-SUBSTS: `call_dest_type` 使用 `fn_sigs.get(&did).output` (带 Param) 而非特化后的签名 → loc_6 类型错误 (i32 而非 i64). 但 substitute_adt_layout 在 Adt 层面修复了 AdtLayout, 所以 loc_6 通过 Adt 的 substs 正确解析. 剩余问题是 Iterator sum (i64) 仍有垃圾值 — 这是 `call_dest_type` 未特化 sig.output 的问题. P3, v0.16+.

### 下一步
Stage 153: TD-CALL-DEST-TYPE-SUBSTS (修复 call_dest_type 使用特化签名) 或 TD-STDLIB-ITERATOR
