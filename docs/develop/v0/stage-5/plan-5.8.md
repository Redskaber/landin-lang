# Stage 5.8 开发计划：标准 trait 注册表（stdlib MVP）

> **阶段**: Stage 5.8
> **版本**: v0.11.6 → v0.11.7
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.19 §17.3 时期 1

## 1. 目标

让编译器自动识别标准库 trait（Copy, Clone, Drop, Sized, Send, Sync 等），
无需用户定义 `trait Copy {}`。这是 stdlib MVP 的基础——编译器需要知道
这些 trait 的存在，才能正确执行 Copy 检测、trait 约束求解等。

## 2. 背景

Stage 5.4 的 `is_copy()` 通过 `interner.get("Copy")` 查找 Copy trait 的
Spur，但只在用户定义了 `trait Copy {}` 时才有效。这意味着用户必须写
`trait Copy {}` 才能让 `impl Copy for S` 生效——不符合 Rust 语义（Copy
是标准库 trait，编译器应自动识别）。

Stage 5.8 添加 `BuiltinTraits` 注册表，在 `collect()` 前自动注册标准
trait，使编译器无需用户定义即可识别它们。

## 3. 设计

### 3.1 `BUILTIN_TRAIT_NAMES` 常量

```rust
pub const BUILTIN_TRAIT_NAMES: &[&str] = &[
    "Copy", "Clone", "Drop", "Sized", "Send", "Sync", "Unpin", "Fn", "FnMut", "FnOnce",
];
```

### 3.2 `BUILTIN_DEF_ID_BASE` 常量

```rust
pub const BUILTIN_DEF_ID_BASE: u32 = u32::MAX;
```

内置 trait 获得 `DefId(u32::MAX - N)` 范围的保留 ID，与用户定义项
（从 0 开始）永不冲突。

### 3.3 `TraitResolver.builtin_traits` 字段

```rust
pub builtin_traits: HashMap<Spur, DefId>,
```

映射内置 trait 名 → 保留 DefId。

### 3.4 `register_builtin_traits(&mut Rodeo)` 方法

在 `collect()` 前由 driver 调用（需要 `&mut Rodeo` intern 名字）：
- 对每个 `BUILTIN_TRAIT_NAMES` 条目：intern 名字 + 分配保留 DefId
- 注册到 `builtin_traits` + `trait_by_name`（用 `entry().or_insert()`
  避免覆盖用户定义）+ `type_by_def_id`

### 3.5 查询方法

- `is_builtin_trait(name: Spur) -> bool` — 判断是否内置 trait
- `find_builtin_trait(name: Spur) -> Option<DefId>` — 获取内置 DefId

### 3.6 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `BUILTIN_TRAIT_NAMES` | SCREAMING_SNAKE_CASE 常量 | 标准 Rust 常量命名 |
| `BUILTIN_DEF_ID_BASE` | SCREAMING_SNAKE_CASE 常量 | 同上 |
| `TraitResolver::register_builtin_traits` | snake_case 方法 | 动作动词 |
| `TraitResolver::is_builtin_trait` | `is_` 前缀查询 | 布尔查询 |
| `TraitResolver::find_builtin_trait` | `find_` 前缀查询 | 与 `find_trait` 一致 |

## 4. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.8-a | `BUILTIN_TRAIT_NAMES` + `BUILTIN_DEF_ID_BASE` 常量 | L1 |
| 5.8-b | `TraitResolver.builtin_traits` 字段 | L1 |
| 5.8-c | `register_builtin_traits(&mut Rodeo)` 方法 | L2 |
| 5.8-d | `is_builtin_trait` + `find_builtin_trait` 查询方法 | L1 |
| 5.8-e | driver 调用 `register_builtin_traits` | L1 |
| 5.8-f | lib.rs re-export 常量 | L1 |
| 5.8-g | 测试 `builtin_traits_tests.rs` (5 用例) | L1 |
| 5.8-h | Cargo.toml 版本 + all_tests.rs 模块注册 | L1 |

## 5. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（926 → 931, +5 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规：codegen/typeck 仍是纯消费者 ✅
6. API 命名遵循 api-naming-standard §3 ✅

---

**创建日期**: 2026-07-22
