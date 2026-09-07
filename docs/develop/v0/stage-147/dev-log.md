# Stage 147 开发日志 — TD-ASSOC-TYPE-MULTI-RUNTIME 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.670.0 → v0.671.0 |
| 测试数 | 5942 → 5960 (+18 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 147 完整修复 TD-ASSOC-TYPE-MULTI-RUNTIME — 多关联类型 trait + 泛型投影的运行时 segfault。

### WHY (根因分析, §2.2 根因思维)

**根因**: Bodyless trait 方法 (e.g., `fn key(&self) -> Self::Key;` 在有多个方法的 trait 中) 使用 `fresh_hir_id()`, 这共享 trait 的 owner DefId。这导致 `TraitMethodResolutionMap` key 冲突: `(trait_def_id, type_name)` 对同一 trait 的所有方法相同, 第二个 `insert` 覆盖第一个。`kv.key()` 解析到 `Entry::value` (最后插入的 entry)。

### HOW (核心修复)

#### 修复 1: hir/lower/item.rs — bodyless trait 方法获得唯一 DefId
```rust
// Before: fresh_hir_id() (shares trait's DefId)
// After: enter_owner() + store_owner() (unique DefId)
```
- §1.0 原則 6 (通解 > 特解): 一个 enter_owner/exit_owner 路径处理所有 trait 方法 (bodied + bodyless)

#### 修复 2: resolve/module_build.rs — 跳过 trait 方法在模块注册中
```rust
if trait_method_def_ids.contains(def_id) {
    self.def_kinds.insert(*def_id, DefKind::Fn);
    continue;
}
```
- §12 (最优 > 最小): same pattern as impl methods (Stage 14.42)

#### 修复 3: driver/driver_validations.rs — mir_ty_kinds_compatible 添加 Projection 分支
```rust
(TyKind::Projection(_, _), _) | (_, TyKind::Projection(_, _)) => true,
```
- §1.0 原則 6/9/10 (mirrors unify.rs Stage 146 Projection rule)

### 决策点 (§12 最优 > 最小)
1. 选 bodyless 方法获得唯一 DefId 不选修改 map key — §1.0 原則 6 (通解: 所有 trait 方法同一路径)
2. 选跳过模块注册不选不调用 store_owner — §1.0 原則 9 (DefId 必须有, 注册可跳过)

### 下一步 (MUV)
- Stage 148: TD-STDLIB-ITERATOR (本阶段 + Stage 146 解锁 Iterator trait) 或
  TD-TYPECK-LIFETIME-ELISION

## 文件清单
### 修改文件 (5)
- `src/hir/lower/item.rs` — bodyless trait 方法使用 enter_owner
- `src/resolve/module_build.rs` — 跳过 trait 方法在模块注册 + trait_method_def_ids 集合
- `src/driver/driver_validations.rs` — mir_ty_kinds_compatible 添加 Projection 分支
- `tests/v0/stage1/plan/hir_lowering_tests.rs` — 更新 owner_count 预期 (1→2) + trait 查找
- `tests/v0/stage146/plan/assoc_type_tests.rs` — 启用之前跳过的 multi-assoc generic 测试

### 新建文件 (1)
- `tests/v0/stage147/plan/multi_assoc_type_tests.rs` — 18 tests
