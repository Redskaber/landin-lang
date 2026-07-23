# Stage 5.28 开发计划：stdlib alloc layer

> **阶段**: Stage 5.28
> **版本**: v0.11.24 → v0.11.25
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

扩展 stdlib 到 `alloc` 层——添加堆分配集合类型（Box/Vec/String/HashMap/...）
和格式化/智能指针 trait（Display/Debug/Deref/Default/Hash）。

## 2. 设计

### 2.1 新增常量

| 常量 | 内容 | 数量 |
|------|------|------|
| `STDLIB_ALLOC_TYPES` | Box/Vec/String/HashMap/BTreeMap/HashSet/BTreeSet/Rc/Arc/Cell/RefCell/LinkedList/VecDeque | 13 |
| `STDLIB_ALLOC_TRAITS` | Display/Debug/Write/Formatter/Deref/DerefMut/Default/Hash | 8 |

### 2.2 扩展现有 API

- `all_stdlib_type_names()` → 包含 core + alloc types
- `all_stdlib_trait_names()` → 包含 marker + ops + convert + iter + alloc traits
- `register_stdlib()` → 注册 alloc types + alloc traits
- `StdlibPrelude` → 自动包含 alloc items

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1049 → 1058, +9 ✅）
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
