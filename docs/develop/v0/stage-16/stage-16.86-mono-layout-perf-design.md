# Stage 16.86 Design — MonoLayoutKey Clone Elimination (Performance)

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Per v0.4 roadmap, Performance Optimization (P3).

**问题**: `MonoLayoutKey::new(def_id, substs)` 每次 lookup 都 clone 所有 `TyKind`：
```rust
pub fn new(def_id: DefId, substs: &SubstsRef) -> Self {
    let substs = substs.iter().map(|t| t.kind.clone()).collect();  // 昂贵！
    MonoLayoutKey { def_id, substs }
}
```

`TyKind` 含 `Vec<Ty>` (Tuple/Adt/FnDef/Closure/Projection)、`Box<Ty>` (Array/Slice/Ref/Ptr)，clone 成本高。`lookup_mono_layout` 在 codegen 热路径上被调用（每个 Adt 类型访问都调一次）。

**目标**: 消除 lookup 时的 clone，用引用或哈希代替。

## 2. 架构现状分析

### 2.1 MonoLayoutKey 结构

```rust
pub struct MonoLayoutKey {
    pub def_id: DefId,
    pub substs: Vec<TyKind>,  // owned, requires clone
}
```

### 2.2 lookup_mono_layout 调用

```rust
pub fn lookup_mono_layout(def_id, substs, mono_layouts) -> Option<&AdtLayout> {
    let key = MonoLayoutKey::new(def_id, substs);  // clone!
    map.get(&key)  // HashMap lookup
}
```

### 2.3 MonoLayoutKey 使用点

- `build_mono_layouts`: insert 时用 `MonoLayoutKey::new`（一次性，可接受）
- `lookup_mono_layout`: 每次 codegen 类型访问（热路径，需优化）

## 3. 重构方案

### 3.1 新增 lookup_mono_layout_by_ref

避免构造 `MonoLayoutKey`，直接用 `(DefId, &[Ty])` 做 HashMap lookup。

Rust HashMap 支持通过 `&Q where K: Borrow<Q>` 查找。但 `MonoLayoutKey` 含 `Vec<TyKind>`，`Borrow` 不能从 `&[Ty]` 得到。

**方案 A**: 使用 `HashMap<(DefId, Vec<TyKind>), AdtLayout>` + 每次构造 key（当前，有 clone）
**方案 B**: 新增 `lookup_mono_layout_by_ref` 用临时 key + 线性扫描（O(n) 但无 clone）
**方案 C**: 预计算哈希，用 `HashMap<u64, (MonoLayoutKey, AdtLayout)>` + 二次验证

**最优方案**: 改用 `BTreeMap` 或改 key 结构为可借用的形式。

实际上，最简单且最优的方案是：**缓存 MonoLayoutKey 的哈希值**，避免每次重新哈希。但这不解决 clone 问题。

**真正最优**: 把 `MonoLayoutKey` 的 `substs: Vec<TyKind>` 改为 `substs: Vec<Ty>`（存完整 Ty 而非 TyKind），然后用 `substs.as_slice()` 作为 `Borrow<[Ty]>` 的 target。但 `Ty` 也含 `TyKind`，clone 同样昂贵。

**最终方案**: 使用 `HashSet` + 自定义 `Hash` + `Eq` 的引用版本。具体：

1. 新增 `MonoLayoutKeyRef<'a>` — 含 `def_id: DefId, substs: &'a [Ty]`
2. `MonoLayoutKey` 实现 `Borrow<MonoLayoutKeyRef>` — 但这需要 `MonoLayoutKey.substs` 也是 `&[Ty]`...

**实际可行的简单方案**：在 `lookup_mono_layout` 中，如果 `substs` 长度为 1（最常见情况——单泛型参数），直接用 `substs[0].kind` 构造 key，避免 `Vec` 分配。但这只是部分优化。

**最终决定**: 引入 `lookup_mono_layout_fast` 函数，对于常见的 `Adt(def_id, [single_subst])` 情况，用栈上 `[Ty; 1]` 数组避免 `Vec` 分配。对于多 substs 情况，回退到 `MonoLayoutKey::new`。

但这还是 clone。让我重新思考。

**真正最优方案（§12 最优 > 最小）**: 把 `MonoLayoutMap` 从 `HashMap<MonoLayoutKey, AdtLayout>` 改为 `HashMap<DefId, Vec<(SubstsRef, AdtLayout)>>`，lookup 时线性扫描 substs（通常只有 1-3 个 monomorphization per type）。这避免了 clone，因为 `SubstsRef` 是 `Vec<Ty>` 可以直接比较 `&[Ty]`。

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 monomorphize 设计一致 |
| J2 | 单一职责 | ✅ `lookup_mono_layout` 只负责查找 |
| J3 | 单向流动 | ✅ build → insert → lookup |
| J4 | 编译相关表达完整 | ✅ 保留 MonoLayoutKey 用于 insert |
| J5 | 阶段划分清晰 | ✅ 仍在 mir/monomorphize/ |
| J6 | 科学合理粒度 | ✅ ~50 LOC |

## 5. 测试计划

性能优化不需新功能测试，只需回归测试通过。但按 §9.4.3，仍需 1:3+ 比例测试。

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅

## 7. 结论

定稿 — 用 `HashMap<DefId, Vec<(SubstsRef, AdtLayout)>>` 替代 `HashMap<MonoLayoutKey, AdtLayout>`，消除 lookup clone。
