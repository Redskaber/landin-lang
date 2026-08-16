# Stage 18.153 — v0.2 P0 mini-cargo Phase 2: 跨文件 name resolution

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.421.0 (Stage 18.153 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 (重构即架构设计) + §2 原则 4 (报错>静默) + §2 原则 9 (正确>妥协)
> **Complexity**: L2 (resolver 修改 + 新 helper 函数, 2 文件)
> **Task ID**: stage18.153

## 1. 阶段目标

按 v0.2 P0 计划推进 TD-SINGLE-FILE 修复。本 stage 实现 Phase 2: 跨文件 name resolution。

| Phase | 范围 | 状态 |
|-------|------|------|
| Phase 1 | 模块加载器 (`ModuleLoader` + `compile_project`) | ✅ Stage 18.152 |
| Phase 2 | `use` 跨文件 + path 跨模块 name resolution | ✅ 本 stage |
| Phase 3 | `landinc` CLI subcommands (`build`/`run`/`new`) | 后续 stage |
| Phase 4 | `landin.toml` manifest 集成 | 后续 stage |

## 2. 问题分析

### 2.1 Phase 1 的局限

Stage 18.152 实现了模块加载——`mod foo;` 能从磁盘读取 `foo.lin` 并填充 AST。但 **name resolution** 仍不工作:

- `foo::bar()` — 解析器返回 `Res::Def(foo_mod_def_id, Mod)` 而非 `Res::Def(bar_fn_def_id, Fn)`
- `use foo::bar;` — use 解析的 `lookup_use_path_target` 只支持 crate root 级查找

### 2.2 根因

`resolve_path` (path_resolve.rs:719-727) 的多段路径处理:

```rust
// Before (Stage 1.3-18.152):
// For multi-segment paths where the first segment is a module,
// we would walk into the child module. For Stage 1.3, we resolve
// the first segment and return — full multi-level resolution
// (e.g., `std::io::Read`) requires cross-crate resolution which
// is Stage 5+ work.
let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Mod);
return Res::Def(def_id, kind);
```

问题: 任何 `foo::bar()` 都返回模块 `foo` 的 DefId, 而非函数 `bar` 的 DefId。这阻断了跨文件函数调用。

## 3. 修复方案

### 3.1 设计原则

- **通解 > 特解** (§1.0 原則 6): 一个 `resolve_path_in_module` 递归函数处理所有段数 (2-segment, 3-segment, ...)
- **报错 > 静默** (§2 原則 4): 路径无法在子模块内解析时返回 `Res::Err`, 不回退到返回模块 DefId
- **正确 > 妥协** (§2 原則 9): 完整的模块树遍历, 不是 stub

### 3.2 修改: `resolve_path` (path_resolve.rs)

当首段是模块 (DefKind::Mod) 且路径有 ≥2 段时:
1. 查找 `module_tree.child(first_name)` 获取子模块
2. 调用 `resolve_path_in_module(child_mod, &segments[1..], ...)` 递归解析
3. 如果找到 → 返回结果
4. 如果未找到 → 返回 `Res::Err` (报错, 不静默回退)

### 3.3 新增: `resolve_path_in_module` (path_resolve.rs)

```rust
fn resolve_path_in_module(
    module: &ModuleNode,
    segments: &[HirPathSegment],
    def_kinds: &HashMap<DefId, DefKind>,
    _interner: &Rodeo,
) -> Option<Res> {
    if segments.is_empty() { return None; }
    if segments.len() == 1 {
        // 最后一段: 查 value_ns → type_ns → use_imports
        let name = segments[0].ident.name;
        if let Some(def_id) = module.lookup_value(name) { ... }
        if let Some(def_id) = module.lookup_type(name) { ... }
        if let Some(import) = module.lookup_use_import(name) { ... }
        return None;
    }
    // 多段: 首段必须是子模块, 递归
    let first_name = segments[0].ident.name;
    if let Some(child) = module.child(first_name) {
        return Self::resolve_path_in_module(child, &segments[1..], def_kinds, _interner);
    }
    None
}
```

### 3.4 use 解析

`resolve_use_leaf` → `lookup_use_path_target` 已支持 2-segment 路径走 `module_tree.child(mod_name)`。Phase 1 的 `build_module_tree` 已正确构建子模块树 (包括 Loaded 模块的 items)。因此 `use foo::bar;` 无需额外修改——它通过现有的 `lookup_use_path_target` 2-segment 分支工作。

## 4. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 函数 | `resolve_path_in_module` | `<verb>_<noun>_<prep>_<noun>` | ✅ |
| 可见性 | `fn` (private) | resolver 内部 helper | ✅ |

## 5. 接口设计 (§11)

- `resolve_path_in_module` 是 resolver 内部 helper (`fn`, 非 `pub`)
- 不跨阶段调用 — 仅操作 resolver 内部数据结构 (ModuleNode, def_kinds)
- 不修改公共 API — `resolve_crate` 签名不变

## 6. 测试 (§9)

### 6.1 测试矩阵

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_153_cross_file_fn_call` | 正向 | `helper::answer()` 跨文件函数调用 |
| `stage18_153_use_import_from_module` | 正向 | `use helper::answer;` 然后调用 |
| `stage18_153_cross_file_struct` | 正向 | `types::Point { x: 1, y: 2 }` 跨文件结构体 |
| `stage18_153_use_import_struct` | 正向 | `use types::Point;` 然后使用 |
| `stage18_153_nested_module_fn_call` | 正向 | `outer::inner::deep()` 嵌套模块 |
| `stage18_153_inline_mod_cross_file` | 正向 | 内联模块内调用跨文件函数 |
| `stage18_153_call_nonexistent_fn` | 负向 | `helper::nonexistent()` 报错 |
| `stage18_153_use_nonexistent_item` | 负向 | `use helper::nonexistent;` 报错 |

6 positive + 2 negative = 1:0.33 比例 (正向充足, 负向覆盖错误报告)。

### 6.2 测试结果

- ✅ 8/8 通过 (6 positive + 2 negative)
- ✅ 0 回归 (629 lib + 2681 integration = 3310 total, 0 failures)

## 7. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 629 passed, 0 failed
- ✅ cargo test --tests --all-features: 2681 passed (2673 + 8 new), 0 failed
- ✅ 0 TODO/FIXME/HACK

## 8. 简写和缺陷记录

### 8.1 当前简写

**简写 1**: `resolve_path_in_module` 不检查 visibility (跨模块 private 访问)。
- **原因**: `check_visibility` 当前是 stub (Stage 4.12 conservative, 总是 Ok)。
- **修订计划**: v0.2 P2 启用严格 visibility 检查后, 在 `resolve_path_in_module` 中添加 visibility 检查。

**简写 2**: `resolve_use_tree` 的 `Path { prefix, children }` 分支忽略 `prefix`。
- **原因**: `use a::{b, c};` 的 prefix `a` 未被用于限定 `b`/`c` 的查找——children 直接在 crate root 解析。
- **修订计划**: 修改 `resolve_use_tree` 让 `Path` 分支使用 prefix 限定子节点的解析范围。当前 `use helper::answer;` 通过 2-segment `lookup_use_path_target` 工作, 但 `use helper::{a, b};` 可能不正确。

### 8.2 缺陷记录

**缺陷 1**: glob imports (`use foo::*;`) 不递归到子模块。
- **原因**: `resolve_use_glob` 只在 crate root 展开, 不进入子模块。
- **修订计划**: 修改 `resolve_use_glob` 接收目标模块参数, 在指定模块内展开。

**缺陷 2**: 跨模块 private item 访问不报错。
- **原因**: `check_visibility` 是 stub。
- **修订计划**: v0.2 P2 启用严格 visibility 后修复。

## 9. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | 对齐 `01-language-specification.md` §6.2 解析顺序 | ✅ |
| J2 单一职责 | `resolve_path_in_module` 仅负责"在指定模块内解析路径" | ✅ |
| J3 单向流动 | resolve_path → child_mod → resolve_path_in_module (无环) | ✅ |
| J4 编译相关表达完整 | 路径解析逻辑完整在 path_resolve.rs | ✅ |
| J5 阶段划分清晰 | resolver 阶段内修改, 不跨阶段 | ✅ |
| J6 科学合理粒度 | 新增 ~55 LOC helper, 合理 | ✅ |

## 10. Stage Summary

- **Stage 18.153 PASSED** — v0.2 P0 mini-cargo Phase 2: 跨文件 name resolution
- **修改**: `resolve_path` 多段路径走子模块树 + 新增 `resolve_path_in_module` 递归 helper
- **关键修复**: `foo::bar()` 现在正确解析到函数 `bar`, 而非模块 `foo`
- **测试**: 629 lib + 2681 integration (新增 8), 0 failures
- **TD-SINGLE-FILE**: 🟡 Phase 1-2 Resolved (phases 3-4 remain)
- **v0.421.0**: patch bump
- **下一步**: Phase 3 — `landinc` CLI subcommands
