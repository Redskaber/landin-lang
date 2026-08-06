# Stage 16.86 — MonoLayoutKey Clone Elimination (Performance)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.272.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

消除 `lookup_mono_layout` 中的 `TyKind::clone()`，提升 codegen 性能。

## 2. 实现

### 2.1 MonoLayoutMap 结构变更

**前**: `HashMap<MonoLayoutKey, AdtLayout>` — 每次查找需要 `MonoLayoutKey::new(def_id, substs)`，其中 `substs.iter().map(|t| t.kind.clone()).collect()` 克隆所有 `TyKind`。

**后**: `HashMap<DefId, Vec<(Vec<TyKind>, AdtLayout)>>` — 按 `DefId` 分组，每个 `DefId` 对应一个 `Vec`，包含该类型的所有单态化实例。

### 2.2 lookup_mono_layout 优化

不再构造 `MonoLayoutKey`，改为：
1. 按 `DefId` 查找 `Vec`
2. 线性扫描 `Vec`，逐元素比较 `substs[i].kind == stored_kinds[i]`
3. O(n) 其中 n = 该 `DefId` 的单态化实例数（通常 1-3）

这避免了在热路径上克隆可能包含 `Vec<Ty>` / `Box<Ty>` 的 `TyKind`。

### 2.3 build_mono_layouts 更新

使用 `map.entry(def_id).or_default().push((substs_kinds, layout))` 插入。构建路径仍然会克隆 `TyKind`（可接受——每个实例只执行一次）。

## 3. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 415 lib + 2529 integration = 2944 unit tests |

## 4. 结论

GO — MonoLayoutKey clone 消除完成：
- lookup 不再 clone TyKind ✅
- MonoLayoutMap 改为 DefId → Vec 结构 ✅
- 全量回归通过 ✅
