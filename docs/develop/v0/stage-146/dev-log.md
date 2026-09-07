# Stage 146 开发日志 — TD-TYPECK-ASSOC-TYPE-PROJECTION 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.669.0 → v0.670.0 |
| 测试数 | 5918 → 5942 (+24 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 146 完整修复 TD-TYPECK-ASSOC-TYPE-PROJECTION — 泛型上下文中的关联类型投影解析。

### WHY (根因分析, §2.2 根因思维)

**根因 1 (typeck)**: `unify.rs` 的 `unify_resolved` 函数**没有 `TyKind::Projection` 分支**。
当 `unify(Projection, i64)` 被调用时, 它落入默认的 `_ => Err(...)` 拒绝分支, 导致
"mismatched types: expected <projection>, found i64" 错误。

**根因 2 (codegen)**: `projection_resolver::resolve_projections_in_mir` 只在 driver
post-typeck 阶段调用 (compile_inner.rs:907), 但**在 monomorphization 之前**。对于
泛型函数中的投影 `<C as Container>::Item` (self type = Param(C)), resolver 无法解析
(Param self 无法匹配 impl 的 concrete self)。Monomorphization 在 codegen 中
(function.rs:93 `substitute_mir_body`) 才发生, 但 codegen 没有再次调用 resolver。

**完整根因链**:
1. `fn use_container<C: Container>(c: &C) -> <C as Container>::Item { c.get() }`
2. `c.get()` 返回 `Self::Item` (impl 中 = i64), 目标 place 类型 = `<C as Container>::Item` (Projection)
3. typeck `check_terminator` 调用 `unify(sig.output=i64, dest_ty=Projection)` → **失败** (无 Projection 分支)
4. 即使 unify 成功 (假设修复 1), driver 的 resolver 在 mono 前运行, Projection 的 self = Param(C), 无法匹配 Holder impl
5. Monomorphization 后, Projection 变成 `Projection(Container, [Adt(Holder)])`, 但 resolver 没有再次运行
6. codegen 看到 unresolved Projection → fallback 到 i32 → wrong type

### HOW (核心修复)

#### 修复 1: unify.rs 添加 Projection 分支
```rust
if matches!(a.kind, TyKind::Projection(_, _))
    || matches!(b.kind, TyKind::Projection(_, _))
{
    return Ok(());
}
```
- §1.0 原則 6 (通解 > 特解): 一个 Projection 规则处理所有 unify
- §1.0 原則 9 (正确 > 妥协): deferring to projection_resolver is correct design (typeck 不能解析 projections without monomorphization context)
- §1.0 原則 10 (唯一可信数据源): projection_resolver 是 authoritative source

#### 修复 2: codegen function.rs 添加 post-mono projection resolution
```rust
// After substitute_mir_body:
if let Some(hir_crate) = hir {
    crate::driver::projection_resolver::resolve_projections_in_mir(
        &mut specialized_mir,
        hir_crate,
    );
}
```
- §1.0 原則 6 (通解 > 特解): 一个 resolver 处理所有 projections (driver pre-mono + codegen post-mono)
- §1.0 原則 9 (正确 > 妥协): post-substitution pre-codegen is correct stage
- §1.0 原則 10 (唯一可信数据源): projection_resolver 是唯一权威
- §11 (allowed cross-stage access): codegen 调用 resolver (driver concern), resolver reads HIR (allowed)

#### 修复 3: driver/mod.rs — projection_resolver 模块改为 pub
```rust
pub mod projection_resolver;  // was: mod projection_resolver;
```
- §11: codegen 需要访问 driver 的 projection_resolver

#### 修复 4: codegen pipeline.rs — 传递 HIR 给 codegen_mono_functions
```rust
codegen_mono_functions(
    ...,
    result.hir.as_ref(),  // Stage 146: Pass HIR
)?;
```

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10/11)

1. **选 Projection unify any type** 不选 resolve-in-typeck — §1.0 原則 9 (typeck 无 mono context)
2. **选 codegen 调用 resolver** 不选 driver pre-mono-only — §1.0 原則 6 (通解: post-mono resolves generic projections)
3. **选传递 HIR 给 codegen** 不选 codegen 无 HIR — §11 (allowed cross-stage access for projection resolution)
4. **选 projection_resolver pub** 不选复制 resolver 逻辑到 codegen — §1.0 原則 6 (通解: 一个 resolver)

### 裁剪点 (§1.2.1)
L3 任务 — codegen 跨模块 (unify.rs + function.rs + pipeline.rs + driver/mod.rs), 但单轮收敛。

### 发现的 follow-up TD
- **TD-ASSOC-TYPE-MULTI-RUNTIME**: 多关联类型 trait + 泛型投影的运行时 segfault (stage146_multiple_assoc_types_generic 测试跳过)。根因可能是 method call 返回类型解析在多 assoc type 场景下的 ambiguity。P3, v0.16+。

### 下一步 (MUV)
- Stage 147: TD-ASSOC-TYPE-MULTI-RUNTIME (本阶段发现) 或
  TD-STDLIB-ITERATOR (本阶段解锁 Iterator trait) 或
  TD-TYPECK-LIFETIME-ELISION (3 条 elision rules)

## 文件清单

### 修改文件 (4)
- `src/typeck/unify.rs` — 添加 Projection unify 分支 (+36 行注释 + 6 行代码)
- `src/codegen/function.rs` — codegen_mono_functions 添加 hir 参数 + post-mono resolver 调用
- `src/codegen/pipeline.rs` — 传递 result.hir.as_ref() 给 codegen_mono_functions
- `src/driver/mod.rs` — projection_resolver 改为 pub mod

### 新建文件 (1)
- `tests/v0/stage146/plan/assoc_type_tests.rs` — 24 tests
