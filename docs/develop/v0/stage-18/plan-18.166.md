# Plan 18.166 — Variant Constructor 支持 (不带前缀的 Some/None/Ok/Err)

> **Author**: redskaber (PM-A + ARCH-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.434.0 (Stage 18.166 plan)
> **Process**: docs/stage-committee-process.md v6.4 §5.1 (复杂度预评估) + §13.4 (重构即架构设计)
> **Complexity**: L2 (resolver 修改)
> **Task ID**: stage18.166

## 1. 任务审查 (§5.1 复杂度预评估)

### 1.1 能力具备性

| 维度 | 评估 | 详情 |
|------|------|------|
| Enum variant 解析 | ✅ 部分 | `Color::Red` 工作但 `Red` 不工作 |
| Value namespace | ✅ 具备 | module_tree 有 value_ns |
| Prelude 注入 | ✅ 具备 | Stage 18.165 已注入 Option/Result |
| Variant 注册 | ❌ 不具备 | enum variant 未注册到 value namespace |

### 1.2 阻塞项

**无阻塞项** — 只需在 module_build.rs 注册 enum variant 到 value namespace。

### 1.3 复杂度评估

- **代码变动量**: ~30 LOC (module_build.rs 注册 variant)
- **依赖风险**: 低 (仅修改 build_module_tree, 不影响其他)
- **历史缺陷密度**: 低

**复杂度等级**: L2 (常规业务逻辑增改)

## 2. 设计方案

### 2.1 方案: 注册 enum variant 到 value namespace

在 `build_module_tree` 中, 遍历 enum 的 variants, 将每个 variant 名称注册到 value namespace:

```rust
HirItem::Enum(e) => {
    registrations.push((def_id, DefKind::Enum, e.ident.name));
    self.def_kinds.insert(def_id, DefKind::Enum);
    // Stage 18.166: Register variant names to value namespace
    for variant in &e.variants {
        // Variant is a constructor function — register as DefKind::Fn
        // with a synthetic DefId (reuse enum's DefId for now)
    }
}
```

### 2.2 灰区决策

**灰区1**: variant 的 DefKind 是什么?
- **决策**: `DefKind::Fn` (variant constructor 本质是函数)
- **原因**: MIR lower 已处理 `Color::Red` 为 enum variant access, 这里只是让 resolver 能找到名称

**灰区2**: variant 的 DefId 是什么?
- **决策**: 复用 enum 的 DefId (简化, 无需新分配)
- **原因**: variant 不是独立 owner, 是 enum 的一部分

## 3. 测试计划

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `test_some_no_prefix` | 正向 | `Some(42)` 编译通过 |
| `test_none_no_prefix` | 正向 | `None` 编译通过 |
| `test_ok_no_prefix` | 正向 | `Ok(42)` 编译通过 |
| `test_err_no_prefix` | 正向 | `Err("error")` 编译通过 |
| `test_option_match_no_prefix` | 正向 | `match x { Some(v) => v, None => 0 }` |
| `test_result_match_no_prefix` | 正向 | `match x { Ok(v) => v, Err(e) => 0 }` |
| `test_user_enum_no_prefix` | 正向 | 用户自定义 enum variant 也不带前缀 |
