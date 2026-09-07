# Stage 142 开发日志 — TD-PTR-INDEX-CODEGEN-2 修复尝试

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.667.0 (不变 — 修复导致回归，回退) |
| 测试数 | 5822 (898 lib + 4924 integration, 不变) |
| 失败数 | 0 |
| ignored | 12 |

## 5W2H

### WHAT
Stage 142 重新应用 Stage 141 的 codegen 修复（`unwrap_fat_ptr_for_index` Ptr(_) LOAD + array_ty 不 strip Ptr）+ 添加 String 方法。

### 尝试结果
1. 重新应用 codegen 修复 → `Ptr(_)` 分支添加 LOAD → 0 回归！（4924 passed）
2. 添加 String::starts_with/ends_with/contains 到 prelude → String 方法工作！
3. 但 `emit_gep_index_ptr` 使用 `i64` index → 25 个 text IR 测试失败（index 值是 i32）
4. 改 `emit_gep_index_ptr` 为 `i32` → 仍有 25 个测试失败（不同的 type mismatch）
5. 回退所有变更，0 回归

### 根因
- `emit_gep_index_ptr` 的 index 类型需要与实际 MIR local 类型匹配
- 有些 local 是 i32（type defaulting），有些是 i64（usize）
- 需要动态检测 index 类型，而非固定 i32 或 i64

### 新 TD
- TD-PTR-INDEX-GEP-TYPE: `emit_gep_index_ptr` 需要动态使用 index local 的实际类型（i32 或 i64），而非固定类型

### 决策点
- **选回退** — §1.0 原則 9 (正确 > 妥协): 25 回归不可接受
- **依据**: §3.2 红线

### 下一步 (MUV)
Stage 143: 修复 `emit_gep_index_ptr` 动态检测 index 类型 → 重新应用 codegen 修复 + String 方法
