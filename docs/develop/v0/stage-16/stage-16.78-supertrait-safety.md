# Stage 16.78 — Task 14 Phase 3: Supertrait Object Safety

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.264.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

Per v0.4 roadmap, Task 14 Phase 3: Supertrait safety (P2, 1-2 stages).

**问题**: `check_trait_object_safety` 只检查直接 trait 的方法，不检查 supertraits。当 `trait Foo: Bar` 且 `Bar` 不是 object-safe 时，`dyn Foo` 也不应合法——因为 vtable 需要包含 `Bar` 的方法。

**目标**: 递归检查所有 supertraits，新增 `SupertraitNotObjectSafe` violation。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿（scope 清晰，无 P0/P1 缺陷）：
- Design v1: `stage-16.78-supertrait-safety-design.md`
- 自审清单 7 项全部通过
- J1-J6 全部满足

## 3. 实现内容

### 3.1 新增 ObjectSafetyViolation variant

```rust
SupertraitNotObjectSafe {
    supertrait: Symbol,
    span: Span,
    violations: Vec<ObjectSafetyViolation>,  // nested for error reporting
}
```

### 3.2 扩展 check_trait_object_safety 签名

**前**: `pub fn check_trait_object_safety(trait_def: &HirTrait) -> Vec<ObjectSafetyViolation>`

**后**: 
```rust
pub fn check_trait_object_safety(
    trait_def: &HirTrait,
    trait_defs: &HashMap<DefId, &HirTrait>,
    interner: &Rodeo,
) -> Vec<ObjectSafetyViolation>
```

**Breaking change**: 公共 API 签名变更。调用点 3 处（driver.rs 2 处 + object_safety.rs tests 10 处）已全部更新。Per §13.3 早期阶段允许。

### 3.3 新增 check_supertraits 递归函数

```rust
fn check_supertraits(
    trait_def: &HirTrait,
    trait_defs: &HashMap<DefId, &HirTrait>,
    visited: &mut HashSet<DefId>,  // 防止循环依赖
    violations: &mut Vec<ObjectSafetyViolation>,
)
```

- 递归遍历 `trait_def.supertraits`
- 对每个 supertrait：检查其方法 + 递归检查其 supertraits
- `visited` set 防止循环依赖（`trait A: B` + `trait B: A`）

### 3.4 嵌套错误消息

```
trait `Foo` is not object-safe: supertrait `Bar` is not object-safe
  └─ trait `Bar` is not object-safe: method `bar` returns `Self`
```

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | safe_trait_with_safe_supertrait | positive | 安全 supertrait → 无 violation |
| 2 | supertrait_self_return | negative | supertrait 有 Self return |
| 3 | supertrait_generic_method | negative | supertrait 有 generic method |
| 4 | supertrait_no_receiver | negative | supertrait 有关联函数 |
| 5 | supertrait_by_value_receiver | negative | supertrait 有 by-value receiver |
| 6 | supertrait_self_in_arg | negative | supertrait 有 Self in arg |
| 7 | transitive_supertrait_not_safe | negative | A: B, B: C, C 不安全 → A 有 violation |
| 8 | circular_supertrait_no_infinite_loop | negative | A: B, B: A → 不死循环 |

**比例**: 1:7 (远超 1:3+ 要求) ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 357 lib + 2494 integration = 2851 unit tests |

## 6. 结论

GO — Task 14 Phase 3 (supertrait safety) 完成：
- 递归 supertrait 检查 ✅
- 循环依赖保护 ✅
- 嵌套错误消息 ✅
- 8 新测试全部通过 ✅
- 1:7 正负比例 ✅

## 7. 后续工作

- Where clause full semantic checking (2-3 stages, P2)
- Improved Error Messages (P3)
- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
