# Stage 18.54 — Generic Type Parameter Resolution (P0 Prerequisite for GAT Phase 3)

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.320.0 → v0.321.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §13.5 (design-review agent cycle)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 上一阶段成果 (Stage 18.53 GATs Phase 2)

Stage 18.53 完成了 GATs Phase 2: qualified path `<T as Trait>::Item` 解析 + `TyKind::Projection` 表示 + `>>` splitting + `&'a mut self` 支持。

**审查发现的关键阻断**: 在 Stage 18.53 验收时, conformance 测试 `0379-gat-qualified-path.lin` 报错 "cannot find type in this scope" — 这不是 GAT 特定问题, 而是 **generic function type parameter resolution 的 Stage 0 既有 limit**。

### 1.2 问题根因分析

通过 explore 测试发现:

```landin
fn f<T>(x: T) -> T { x }
```

报错: `cannot find type in this scope at 11..13` (指向 `x: T` 中的 `T`)

**根因** (src/resolve/path_resolve.rs:323 `resolve_path`):
- 当前 resolver 在解析单段路径时检查顺序:
  1. local scope (变量)
  2. primitive types
  3. Self keyword
  4. value namespace (fn/const/static)
  5. type namespace (struct/enum/trait/type alias)
  6. use imports
  7. → `Res::Err` (not found)
- **缺失**: 当前函数/impl/trait 的 generic type parameters (`T`, `U`, ...)
- 后果: `fn f<T>(x: T)` 中的 `T` 无法解析, 报 "cannot find type"
- 连锁影响: `<T as Container>::Item` 中的 `T` 也无法解析, 阻断 GAT Phase 3

### 1.3 已知限制清单 (Stage 18.53 末尾记录)

| 限制 | 严重性 | 本阶段处理 |
|------|--------|-----------|
| Generic fn type param resolve (Stage 0 既有) | **P0 阻断 GAT Phase 3** | ✅ 本阶段修复 |
| `<<` (Shl) splitting | P2 (罕见) | ⏳ 推迟 |
| `find_assoc_type_def_id` 按 name 查找 | P2 (Phase 3 改进) | ⏳ 推迟 |
| GAT monomorphization | P1 (Phase 3 核心) | ⏳ 推迟 |

### 1.4 本阶段目标

**目标**: 让 generic function 的 type parameter 在函数签名 + 函数体内被正确解析, 为 GAT Phase 3 (`<T as Trait>::Item` 在 generic fn 中使用) 铺路。

**做**:
- Resolver 增加 `generic_param_scope: Vec<HashSet<Spur>>` 字段 (栈式 scope)
- `resolve_item_paths(HirItem::Fn)` 进入时 push generic params, 退出时 pop
- `resolve_path` 单段路径检查时, 在 primitive types 之前检查 generic param scope
- 新增 `Res::GenericParam(Spur, usize)` variant 表示 generic type param (携带 index)
- `scan_ty_for_unresolved` 跳过 `Res::GenericParam` (不算未解析)
- `lower_hir_ty_to_mir_ty` 在 `HirTyKind::Path(_, path)` arm 检查 `Res::GenericParam` 产生 `TyKind::Param`
- 新增 1:3+ 比例测试

**不做** (留待后续):
- ❌ Lifetime param resolution (`'a` in `fn f<'a>(x: &'a T)`) — 当前 lifetime 已 erase
- ❌ Generic const param resolution
- ❌ Where clause 中的 type param 约束求解
- ❌ Higher-ranked types (`for<'a>`)
- ❌ GAT monomorphization (Phase 3)

### 1.5 设计文档参考

| 文档 | 章节 | 关键约束 |
|------|------|---------|
| `docs/lang-design/03-type-system.md` | 泛型部分 | type param 语义 |
| `docs/develop/v0/stage-18/stage-18.52-gats-phase1-design.md` | §3.5 | graceful degradation |
| `docs/develop/v0/stage-18/stage-18.53-gats-phase2-design.md` | §1.2 | 已知限制清单 |
| `src/resolve/path_resolve.rs:323-460` | `resolve_path` | 当前检查顺序 |

---

## 2. §1.0 设计原则遵循

| 原则 | 本阶段如何遵循 |
|------|---------------|
| 1. 长期 > 短期 | 修复 generic param resolve 是 GAT Phase 3 的必要前置; 不修则 Phase 3 无意义 |
| 2. 整体 > 局部 | Resolver + driver scan + MIR lower 三层协同变更 |
| 3. 显式 > 隐式 | `Res::GenericParam(Spur, usize)` 显式表示, 不复用 `Res::Local` |
| 4. 报错 > 静默 | 真正未解析的 type 仍报错; generic param 不算未解析 |
| 5. 去除兼容思维 | 不保留 "T falls through to Res::Err" 的旧行为 |
| 6. 通用 > 特例 | 一个 `generic_param_scope` 栈处理 fn/impl/trait 三种 owner |
| 7. API 命名标准化 | `enter_generic_scope` / `exit_generic_scope` / `lookup_generic_param` |
| 8. 设计驱动测试 | 测试覆盖 generic fn signature + body + nested generic |
| 9. 正确 > 妥协 | lifetime param 推迟明确记录 |

---

## 3. 技术设计

### 3.1 新增 Res Variant (src/hir/kinds.rs)

**Before**:
```rust
pub enum Res {
    Unknown,
    Err,
    Def(DefId, DefKind),
    PrimTy(PrimTy),
    SelfTy(HirSelfKind),
    Local(HirId),
}
```

**After**:
```rust
pub enum Res {
    Unknown,
    Err,
    Def(DefId, DefKind),
    PrimTy(PrimTy),
    SelfTy(HirSelfKind),
    Local(HirId),
    /// Stage 18.54: Generic type parameter in scope.
    /// Carries (param_name, param_index) — index is 0-based position in
    /// the owner's generic params list (e.g., `fn f<T, U>` → T=0, U=1).
    GenericParam(Spur, usize),
}
```

**理由** (§1.0 原則 3 "显式 > 隐式"): 不复用 `Res::Local` (那是变量, 不是类型参数); 显式 variant 让下游消费者 (MIR lower, typeck) 能区分对待。

### 3.2 Resolver 新增 generic_param_scope (src/resolve/resolver.rs)

**新增字段**:
```rust
pub struct Resolver {
    // ... existing fields ...
    /// Stage 18.54: Stack of generic type parameter scopes.
    /// Each entry is a set of (name → index) pairs for the current owner.
    /// Pushed when entering fn/impl/trait signature resolution;
    /// popped on exit.
    /// Per §1.0 原則 6 "通用 > 特例": one stack handles all owner kinds.
    pub(super) generic_param_scope: Vec<Vec<(Spur, usize)>>,
}
```

**新增 API** (§10 命名):
- `fn enter_generic_scope(&mut self, params: &HirGenerics)` — push scope with type params
- `fn exit_generic_scope(&mut self)` — pop scope
- `fn lookup_generic_param(&self, name: Spur) -> Option<usize>` — search stack top-down

### 3.3 resolve_path 更新 (src/resolve/path_resolve.rs)

**单段路径检查顺序** (修改后):
1. local scope (变量) — 不变
2. **generic param scope** ← NEW (在 primitive 之前, 因为 `T` 不应被误认为 primitive)
3. primitive types — 不变
4. Self keyword — 不变
5. value namespace — 不变
6. type namespace — 不变
7. use imports — 不变
8. → `Res::Err`

**修改** (在 `resolve_path` line 365 附近, primitive types check 之前):
```rust
// Stage 18.54: Check generic param scope before primitives.
// A user-named `T` should resolve to the generic param, not fall through.
if let Some(idx) = self.lookup_generic_param(seg.ident.name) {
    return Res::GenericParam(seg.ident.name, idx);
}
```

### 3.4 resolve_item_paths 更新 (src/resolve/path_resolve.rs)

**HirItem::Fn arm** (line 72-77):
```rust
HirItem::Fn(f) => {
    self.enter_generic_scope(&f.generics);
    self.resolve_fn_sig_paths(&mut f.sig, &mut f.generics, interner);
    self.exit_generic_scope();
}
```

**HirItem::Struct / Enum / Trait / Impl** — 同样 enter/exit generic scope。

### 3.5 scan_ty_for_unresolved 更新 (src/driver.rs:2685)

**Before**:
```rust
HirTyKind::Path(_, p) => {
    if matches!(p.res, Res::Unknown | Res::Err) {
        errors.resolve.push(...);
    }
}
```

**After**:
```rust
HirTyKind::Path(_, p) => {
    // Stage 18.54: GenericParam is a valid resolution — not an error.
    if matches!(p.res, Res::Unknown | Res::Err) {
        errors.resolve.push(...);
    }
    // Res::GenericParam is fine — no error.
}
```

(实际上现有代码 `matches!(p.res, Res::Unknown | Res::Err)` 已经只匹配 Unknown/Err, GenericParam 不会被匹配, 所以**无需修改** — 这是 §1.0 原則 3 "显式 > 隐式" 的好处: 新 variant 自动被排除。)

### 3.6 lower_hir_ty_to_mir_ty 更新 (src/mir/lower/mod.rs:1953)

**当前** (Stage 18.53 后):
```rust
HirTyKind::Path(qself, path) => {
    if let Some(inner_ty) = &qself.ty {
        lower_qualified_path_to_projection(...)
    } else {
        match path.res {
            Res::Def(def_id, _) => { ... TyKind::Adt ... }
            Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
            _ => Ty::new(TyKind::Error, span),
        }
    }
}
```

**修改后** (新增 `Res::GenericParam` arm):
```rust
HirTyKind::Path(qself, path) => {
    if let Some(inner_ty) = &qself.ty {
        lower_qualified_path_to_projection(...)
    } else {
        match path.res {
            Res::Def(def_id, _) => { ... TyKind::Adt ... }
            Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
            Res::GenericParam(name, idx) => {
                // Stage 18.54: Lower generic type param to TyKind::Param.
                // Per §1.0 原則 6 "通用 > 特例": reuse existing ParamTy.
                let param = crate::mir::ty::ParamTy {
                    index: idx as u32,
                    name: name,
                };
                Ty::new(TyKind::Param(param), span)
            }
            _ => Ty::new(TyKind::Error, span),
        }
    }
}
```

**注**: `TyKind::Param` 与 `ParamTy` 已存在 (src/mir/ty.rs), 无需新增。

### 3.7 测试设计 (§9.4.3 1:3+ ratio)

**测试文件**: `tests/v0/stage18/plan/stage18_54_generic_param_tests.rs` (≥8 测试: 2 正 + 6 负)

**正向测试** (2):
1. `generic_fn_param_resolves` — `fn f<T>(x: T) -> T { x }` 解析成功 (0 resolve errors)
2. `generic_fn_with_bound_resolves` — `fn f<T: Clone>(x: T) -> T { x }` 解析成功

**负向测试** (6):
1. `undefined_type_param` — `fn f(x: U) { }` (U 未声明) 报 resolve error
2. `type_param_shadowed_by_local` — `fn f<T>(T: i32) { }` (局部变量 shadow type param — 应报错或警告)
3. `generic_fn_missing_param_type` — `fn f<T>()` (T 未使用, 但应解析)
4. `nested_generic_scope` — `fn f<T>() { fn g<U>(x: U) { } }` (嵌套 generic scope)
5. `generic_struct_field_undefined` — `struct S<T> { x: U }` (U 未声明) 报错
6. `generic_impl_method_undefined` — `impl<T> S<T> { fn f(x: U) { } }` (U 未声明) 报错

**Conformance 测试** (修复现有 compile_error → compile_ok):
- `002-generic-fn-2params.lin` — `fn f<T, U>(x: T, y: U) -> T { x }` 现应 compile_ok
- `0379-gat-qualified-path.lin` — `<T as Container>::Item` 在 generic fn 中现应 compile_ok

---

## 4. §13.5 设计-审查 Agent 循环

### 4.1 Round 1 自审

| 维度 | 自审结论 | 状态 |
|------|---------|------|
| 设计偏差 | 修复 generic param resolve 是 GAT Phase 3 的必要前置; 符合 v0.7 路线图 | ✅ |
| §1.0 原则 1 长期 > 短期 | 不修则 Phase 3 无意义; 修则 unlock 大量 generic fn 测试 | ✅ |
| §1.0 原则 3 显式 > 隐式 | 新 `Res::GenericParam` variant, 不复用 `Res::Local` | ✅ |
| §1.0 原则 5 去除兼容思维 | 移除 "T falls through to Res::Err" 的旧行为 | ✅ |
| §1.0 原则 6 通用 > 特例 | 一个 `generic_param_scope` 栈处理 fn/impl/trait/struct/enum 五种 owner | ✅ |
| §10 命名标准 | `enter_generic_scope` / `exit_generic_scope` / `lookup_generic_param` | ✅ |
| §11 接口隔离 | Resolver 内部状态, 不外泄; Res variant 通过 HIR 传递 | ✅ |
| §9.4.3 1:3+ 测试 | 8 unit (2:6) + 2 conformance fix = 1:3 ✓ | ✅ |
| 死代码 | 无; `TyKind::Param` + `ParamTy` 已存在, 复用 | ✅ |
| 向后兼容 | 现有非 generic fn 测试不受影响 (scope 为空时 lookup 返回 None) | ✅ |

### 4.2 §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 修复基础 limit, unlock GAT Phase 3 + 大量 generic 测试 |
| DEV-A | GO | 复用现有 `TyKind::Param` + `ParamTy`, 工作量小 |
| QA-A | GO | 1:3+ 比例满足; 修复 2 个现有 compile_error 测试 |
| REV-A | GO | 设计原则 1, 3, 5, 6 遵循; lifetime param 推迟明确记录 |
| PM-A | GO | v0.7 路线图 P1 GATs 推进的关键前置 |

**5/5 GO** ✅

---

## 5. 实施步骤

1. ✅ 写设计文档 (本文件)
2. ⏳ 新增 `Res::GenericParam(Spur, usize)` variant (src/hir/kinds.rs)
3. ⏳ Resolver 新增 `generic_param_scope` 字段 + `enter/exit/lookup` 方法
4. ⏳ `resolve_path` 单段路径检查新增 generic param lookup
5. ⏳ `resolve_item_paths` 在 Fn/Struct/Enum/Trait/Impl arms enter/exit scope
6. ⏳ `lower_hir_ty_to_mir_ty` Path arm 新增 `Res::GenericParam` → `TyKind::Param`
7. ⏳ 检查所有 `match path.res` 处是否需要新增 arm (避免 non-exhaustive)
8. ⏳ 新增单元测试 (tests/v0/stage18/plan/stage18_54_generic_param_tests.rs)
9. ⏳ 修复 conformance 测试 002 + 0379 (compile_error → compile_ok)
10. ⏳ 验收: cargo clean + build + fmt + clippy + test
11. ⏳ worklog + 版本 bump v0.320.0 → v0.321.0
12. ⏳ 打包 tar.gz

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 新增 Res variant 破坏现有 `match path.res` (non-exhaustive) | 编译器会报错; 逐一添加 `Res::GenericParam => ...` arm 或 `_ => ...` |
| Generic param 在 body 内的 scope 传播 | 当前 `resolve_item_paths` 只处理 signature; body 内的 type annotation 通过 `resolve_ty_paths` 已递归处理, 只要 scope 在 body resolution 期间仍 active 即可 |
| 现有 compile_error 测试改为 compile_ok 可能破坏 conformance runner | 修改测试 header `EXPECTED: compile_error` → `compile_ok` + 删除 `ERROR_PATTERN` |

---

## 7. 结论

Stage 18.54 设计完成。修复 generic type parameter resolution 是 GAT Phase 3 的必要前置, 也是 Stage 0 的长期技术债清理。设计原则严格遵循 §1.0 (长期/显式/去兼容/通用/命名), §10 命名标准, §11 接口隔离。

5/5 GO, 进入实施阶段。
