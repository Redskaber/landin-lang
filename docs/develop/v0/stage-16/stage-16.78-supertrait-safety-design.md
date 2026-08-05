# Stage 16.78 Design — Task 14 Phase 3: Supertrait Object Safety

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Per v0.4 roadmap, Task 14 Phase 3: Supertrait safety (P2, 1-2 stages).

**当前状态**: `check_trait_object_safety` 只检查直接 trait 的方法是否 object-safe，不检查 supertraits。

**问题**: 当 `trait Foo: Bar` 且 `Bar` 不是 object-safe 时，`dyn Foo` 也不应该合法——因为 vtable 需要包含 `Bar` 的方法，而 `Bar` 的方法可能违反 object safety 规则。

**目标**: 扩展 `check_trait_object_safety` 递归检查所有 supertraits，新增 `SupertraitNotObjectSafe` violation 类型。

## 2. 架构现状分析

### 2.1 当前 object_safety.rs 结构

```rust
pub enum ObjectSafetyViolation {
    SelfReturn { method, span },
    SelfInArg { method, arg_idx, span },
    GenericMethod { method, span },
    NoReceiver { method, span },
    ByValueReceiver { method, span },
}

pub fn check_trait_object_safety(trait_def: &HirTrait) -> Vec<ObjectSafetyViolation>
fn check_method(f: &HirFn, violations: &mut Vec<ObjectSafetyViolation>)
fn ty_contains_self(ty: &HirTy) -> bool
```

### 2.2 当前 driver.rs 集成

`check_object_safety_for_dyn_trait_usage` 扫描所有 `HirTyKind::TraitObject`，对每个 trait 调用 `check_trait_object_safety`。

### 2.3 TraitResolver 已有 supertraits 数据

```rust
// src/traits/resolver.rs L26-27
pub struct TraitInfo {
    pub supertraits: Vec<Spur>,  // supertrait name spurs
    ...
}
```

但 `check_trait_object_safety` 只接收 `&HirTrait`，不接收 `TraitResolver`——无法查找 supertraits。

## 3. 重构方案

### 3.1 新增 ObjectSafetyViolation variant

```rust
/// A supertrait of this trait is not object-safe.
SupertraitNotObjectSafe {
    /// The name of the non-object-safe supertrait.
    supertrait: Symbol,
    /// The span of the supertrait bound in the trait definition.
    span: Span,
    /// The violations found in the supertrait (nested for error reporting).
    violations: Vec<ObjectSafetyViolation>,
},
```

### 3.2 扩展 check_trait_object_safety 签名

**前**: `pub fn check_trait_object_safety(trait_def: &HirTrait) -> Vec<ObjectSafetyViolation>`

**后**: 
```rust
pub fn check_trait_object_safety(
    trait_def: &HirTrait,
    resolver: &TraitResolver,
    trait_defs: &HashMap<DefId, &HirTrait>,
    interner: &Rodeo,
) -> Vec<ObjectSafetyViolation>
```

**理由**:
- 需要访问 supertrait 的 `HirTrait` 定义来递归检查
- `resolver` 提供 supertrait name → DefId 映射
- `trait_defs` 提供 DefId → &HirTrait 映射
- `interner` 提供 Symbol → string 解析（错误消息用）

**Breaking change**: 公共 API 签名变更。调用点：
- `src/driver.rs` `check_object_safety_for_dyn_trait_usage` (2 处)
- `src/driver.rs` `check_trait_object_ty` (1 处)
- `src/traits/object_safety.rs` 测试 (5+ 处)

Per §13.3 早期阶段允许破坏性变更。

### 3.3 递归检查 supertraits

```rust
fn check_supertraits(
    trait_def: &HirTrait,
    resolver: &TraitResolver,
    trait_defs: &HashMap<DefId, &HirTrait>,
    interner: &Rodeo,
    visited: &mut HashSet<DefId>,  // 防止环依赖
    violations: &mut Vec<ObjectSafetyViolation>,
) {
    // 从 HirTrait.supertraits (Vec<HirTypeBound>) 解析 supertrait DefIds
    for bound in &trait_def.supertraits {
        if let HirTypeBound::Trait(tc) = bound {
            if let Res::Def(supertrait_def_id, _) = tc.path.res {
                // 防止环依赖
                if visited.contains(&supertrait_def_id) {
                    continue;
                }
                visited.insert(supertrait_def_id);

                // 查找 supertrait 的 HirTrait 定义
                if let Some(supertrait_def) = trait_defs.get(&supertrait_def_id) {
                    // 递归检查 supertrait 的方法
                    let mut super_violations = Vec::new();
                    for item in &supertrait_def.items {
                        if let HirTraitItem::Fn(f) = item {
                            check_method(f, &mut super_violations);
                        }
                    }

                    // 递归检查 supertrait 的 supertraits
                    check_supertraits(
                        supertrait_def,
                        resolver,
                        trait_defs,
                        interner,
                        visited,
                        &mut super_violations,
                    );

                    if !super_violations.is_empty() {
                        violations.push(ObjectSafetyViolation::SupertraitNotObjectSafe {
                            supertrait: supertrait_def.ident.name,
                            span: tc.span,
                            violations: super_violations,
                        });
                    }
                }
            }
        }
    }
}
```

### 3.4 更新 check_trait_object_safety 主函数

```rust
pub fn check_trait_object_safety(
    trait_def: &HirTrait,
    _resolver: &TraitResolver,
    trait_defs: &HashMap<DefId, &HirTrait>,
    interner: &Rodeo,
) -> Vec<ObjectSafetyViolation> {
    let mut violations = Vec::new();

    // 检查直接 trait 的方法
    for item in &trait_def.items {
        if let HirTraitItem::Fn(f) = item {
            check_method(f, &mut violations);
        }
    }

    // 检查 supertraits（递归）
    let mut visited = HashSet::new();
    visited.insert(trait_def.def_id);  // 防止自引用
    check_supertraits(trait_def, _resolver, trait_defs, interner, &mut visited, &mut violations);

    violations
}
```

### 3.5 更新 error_message 处理嵌套 violations

```rust
ObjectSafetyViolation::SupertraitNotObjectSafe { supertrait, violations, .. } => {
    let supertrait_str = interner.try_resolve(supertrait).unwrap_or("<unknown>");
    let mut msg = format!(
        "trait `{}` is not object-safe: supertrait `{}` is not object-safe",
        trait_name, supertrait_str
    );
    // Append nested violations for detailed error reporting
    for v in violations {
        msg.push_str("\n  └─ ");
        msg.push_str(&v.error_message(&supertrait_str, interner));
    }
    msg
}
```

### 3.6 更新 span() 方法

```rust
ObjectSafetyViolation::SupertraitNotObjectSafe { span, .. } => *span,
```

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 RFC #255 object safety 规则一致 |
| J2 | 单一职责 | ✅ `check_supertraits` 专责 supertrait 递归检查 |
| J3 | 单向流动 | ✅ check_trait_object_safety → check_supertraits → check_method 单向 |
| J4 | 编译相关表达完整 | ✅ 新增 SupertraitNotObjectSafe variant 完整表达 |
| J5 | 阶段划分清晰 | ✅ 仍在 traits/ 模块 |
| J6 | 科学合理粒度 | ✅ 新增 ~80 LOC |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `stage16_78_safe_trait_with_safe_supertrait` — `trait Bar { fn bar(&self); } trait Foo: Bar { fn foo(&self); }` + `dyn Foo` → 无 violation

### 负向测试 (negative, ≥3)
1. `stage16_78_supertrait_self_return` — supertrait 有 Self return → SupertraitNotObjectSafe
2. `stage16_78_supertrait_generic_method` — supertrait 有 generic method → SupertraitNotObjectSafe
3. `stage16_78_supertrait_no_receiver` — supertrait 有关联函数 → SupertraitNotObjectSafe
4. `stage16_78_supertrait_by_value_receiver` — supertrait 有 by-value receiver → SupertraitNotObjectSafe
5. `stage16_78_supertrait_self_in_arg` — supertrait 有 Self in arg → SupertraitNotObjectSafe
6. `stage16_78_transitive_supertrait_not_safe` — A: B, B: C, C not safe → A 有 SupertraitNotObjectSafe
7. `stage16_78_circular_supertrait_no_infinite_loop` — A: B, B: A (循环) → 不死循环

比例: 1:7 = 1:7 (远超 1:3+) ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅
- worklog 记录 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~80 LOC + 8 测试。
