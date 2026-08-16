# Stage 18.152 — v0.2 P0 mini-cargo Phase 1: 多文件模块加载器

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.420.0 (Stage 18.152 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.1 (设计对齐) + §13.4 (重构即架构设计) + §12 (最优>最小)
> **Complexity**: L3 (新增 driver 子系统 + AST 变更 + 新公共 API)
> **Task ID**: stage18.152

## 1. 阶段目标

按 v0.2 P0 计划推进 TD-SINGLE-FILE 修复。本 stage 实现 Phase 1: 多文件模块加载器。

| Phase | 范围 | 状态 |
|-------|------|------|
| Phase 1 | 模块加载器 (`ModuleLoader` + `compile_project`) | ✅ 本 stage |
| Phase 2 | `use` 跨文件 name resolution | 后续 stage |
| Phase 3 | `landinc` CLI subcommands (`build`/`run`/`new`) | 后续 stage |
| Phase 4 | `landin.toml` manifest 集成 | 后续 stage |

## 2. 设计文档对齐 (§13.1)

### 2.1 对应设计文档

- `docs/lang-design/10-toolchain.md` §3.3 项目布局 — 定义 `landin.toml` + `src/` + 子模块文件结构
- `docs/lang-design/05-ast.md` — AST `ModDecl` 已支持 `Loaded` variant

### 2.2 设计意图

Per `10-toolchain.md` §3.3，`mod foo;` 声明指示编译器从磁盘加载 `foo.lin` 或 `foo/mod.lin`。当前实现只把 `Loaded` 标记为占位，**不加载文件**——这是 TD-SINGLE-FILE 的核心阻塞点。

### 2.3 已实现 / 偏差项 / 未实现项

| 项 | 状态 |
|----|------|
| `mod foo { ... }` 内联模块 | ✅ 已实现 |
| `mod foo;` 文件加载声明 | ✅ Phase 1 Resolved (ModuleLoader) |
| `use foo::bar;` 跨模块引用 | 🟡 Phase 2 (待实现) |
| `landinc` CLI subcommands | ❌ Phase 3 |
| `landin.toml` 完整支持 | ❌ Phase 4 |

## 3. 架构设计 (§13.4 J1-J6)

### 3.1 新结构

```
src/driver/
├── mod.rs                  # 新增 compile_project() 公共 API + compile_inner 重构
├── module_loader.rs        # 新增: ModuleLoader + load_module_tree (340 LOC)
├── ... (existing files)
```

### 3.2 J1-J6 评估

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | 对齐 `10-toolchain.md` §3.3 项目布局 | ✅ |
| J2 单一职责 | `ModuleLoader` 仅负责文件加载; `compile_project` 仅编排 | ✅ |
| J3 单向流动 | parse → module_load → hir_lower (无环) | ✅ |
| J4 编译相关表达完整 | 模块加载逻辑全在 `module_loader.rs` | ✅ |
| J5 阶段划分清晰 | driver 层 (parse 后, hir_lower 前) | ✅ |
| J6 科学合理粒度 | module_loader.rs ~340 LOC, 合理 | ✅ |

## 4. 实现细节

### 4.1 AST 变更 (向后兼容)

```rust
// Before:
pub enum ModDecl {
    Inline { ident, items, span },
    Loaded { ident, span },  // 空占位
}

// After:
pub enum ModDecl {
    Inline { ident, items, span },
    Loaded { ident, items, span },  // items 由 ModuleLoader 填充
}
```

### 4.2 HIR Lowering 统一路径

`ModDecl::Loaded` 和 `Inline` 现在共用 lowering 路径:
- `Loaded` 且 items 非空 → `HirModKind::Inline(hir_items)` (统一处理)
- `Loaded` 且 items 为空 (单文件 `compile(src)` 路径) → `HirModKind::Loaded` (占位)

Per §1.0 原則 6 (通解>特例): 一个 lowering 路径处理两种 AST variant。

### 4.3 新公共 API

```rust
// src/driver/mod.rs
pub fn compile_project(entry_path: &Path) -> CompileResult;

// src/driver/module_loader.rs
pub struct ModuleLoader { visited: HashSet<PathBuf> }
impl ModuleLoader {
    pub fn new() -> Self;
    pub fn load_module_tree(&mut self, krate: &mut Crate, base_dir: &Path, interner: &mut Rodeo) -> Vec<ModuleLoadError>;
}

pub struct ModuleLoadError { message: String, span: Span, path: Option<PathBuf> }
```

### 4.4 compile_inner 重构

`compile_inner` 新增 `entry_path: Option<&Path>` 参数:
- `None`: 单文件模式 (legacy `compile(src)`)
- `Some(path)`: 多文件模式 (`compile_project`), ModuleLoader 在 parse 后运行

Per §1.0 原則 6 (通解>特例): 一个 `compile_inner` 处理两种模式, 参数化控制。

### 4.5 模块路径解析规则

For `mod foo;` in `/proj/src/main.lin`:
1. Try `/proj/src/foo.lin` (single-file module) — 优先
2. Else try `/proj/src/foo/mod.lin` (directory module)
3. Else report `ModuleLoadError` (file not found)

`foo.lin` takes precedence over `foo/mod.lin` (Rust semantics).

### 4.6 循环依赖检测

`ModuleLoader` 维护 `visited: HashSet<PathBuf>` (canonicalized paths)。遇到已访问路径报错 `circular module dependency`，而非无限递归。

Per §2 原则 4 (报错>静默): 循环依赖是用户错误，应明确报错。

## 5. 测试 (§9)

### 5.1 Lib 测试 (7 个, in `module_loader.rs`)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_152_single_file_no_mod_decls` | 正向 | 单文件无 mod 声明 |
| `stage18_152_inline_mod_unchanged` | 回归 | 内联 mod 不触发文件 IO |
| `stage18_152_module_loader_missing_file` | 负向 | 文件不存在报错 |
| `stage18_152_module_loader_loads_file` | 正向 | 加载 `foo.lin` |
| `stage18_152_module_loader_loads_dir` | 正向 | 加载 `foo/mod.lin` |
| `stage18_152_module_loader_circular_dep` | 负向 | 循环依赖检测 |
| `stage18_152_module_loader_nested` | 正向 | 嵌套模块递归加载 |

### 5.2 集成测试 (10 个, in `stage18_152_module_loader_tests.rs`)

7 positive + 3 negative, 1:2.3 比例 (超过 §9.4.3 的 1:3 要求)。

## 6. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 629 passed (622 + 7 new), 0 failed
- ✅ cargo test --tests --all-features: 2673 passed (2663 + 10 new), 0 failed
- ✅ 0 TODO/FIXME/HACK in new code

## 7. 简写和缺陷记录

### 7.1 当前简写

**简写 1**: `HirModKind::Loaded` 在 `compile(src)` 单文件路径下保留为空占位。
- **原因**: `compile(src)` 不接收文件路径，无法加载外部模块。
- **修订计划**: Phase 3 实现 `landinc build` CLI 时，统一走 `compile_project(path)` 路径。

**简写 2**: `ModuleLoader` 不解析 `#[path = "..."]` 属性。
- **原因**: Landin 暂未实现 attributes 系统 (v0.2 P2)。
- **修订计划**: 等 attributes 系统就绪后扩展。

**简写 3**: 模块加载错误通过 `LowerError` 传递 (非专用错误类型)。
- **原因**: `CompileErrors` 没有 `ModuleLoadError` 字段。
- **修订计划**: Phase 2 添加 `CompileErrors.module_load: Vec<ModuleLoadError>` 字段。

### 7.2 缺陷记录

**缺陷 1**: `compile_project` 不处理 `landin.toml` manifest。
- **原因**: 本 stage 聚焦模块加载，manifest 解析已有但未集成。
- **修订计划**: Phase 4 集成 manifest。

**缺陷 2**: 错误 span 跨文件时，`Span` 类型无法表达文件 ID。
- **原因**: 当前 `Span = (u32, u32)` 只有 byte range，无 file_id。
- **修订计划**: v0.2 P2 引入 `SourceMap` + `FileId`。

**缺陷 3**: `compile_inner` 签名变更 (新增 `entry_path` 参数)，所有内部调用点需更新。
- **原因**: 重构 `compile_inner` 以支持两种模式。
- **修订计划**: 已完成 — `compile()` 和 `compile_no_opt()` 传 `None`, `compile_project()` 传 `Some(path)`。

## 8. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 类型 | `ModuleLoader` | `<Noun>Loader` (-er 后缀) | ✅ |
| 类型 | `ModuleLoadError` | `<Noun>LoadError` (Error 后缀) | ✅ |
| 方法 | `ModuleLoader::new()` | `<verb>` | ✅ |
| 方法 | `ModuleLoader::load_module_tree()` | `<verb>_<noun>_<noun>` | ✅ |
| 函数 | `compile_project(entry_path)` | `<verb>_<noun>` (入口函数) | ✅ |

## 9. 接口设计 (§11)

- `ModuleLoader` 在 driver 层 (parse 后, hir_lower 前)
- 不跨阶段调用 (不直接调 codegen/typeck)
- `compile_project` 是公共 API，调用方处理 `CompileResult`
- `ModuleLoader`/`ModuleLoadError` 通过 `lib.rs` re-export 公开

## 10. Stage Summary

- **Stage 18.152 PASSED** — v0.2 P0 mini-cargo Phase 1: 多文件模块加载器
- **新增**: `src/driver/module_loader.rs` (340 LOC) + `compile_project()` 公共 API
- **修改**: AST `ModDecl::Loaded` 携带 items + HIR lowering 统一路径 + `compile_inner` 重构
- **测试**: 629 lib + 2673 integration, 0 failures (新增 17 个测试)
- **TD-SINGLE-FILE**: 🟡 Phase 1 Resolved (phases 2-4 remain)
- **v0.420.0**: minor bump (新公共 API `compile_project` + `ModuleLoader`)
- **下一步**: Phase 2 — `use` 跨文件 name resolution
