# Plan 18.152 — v0.2 P0 mini-cargo Phase 1: 多文件模块加载器

> **Author**: redskaber (ARCH-A + PM-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.420.0 (Stage 18.152 plan)
> **Process**: docs/stage-committee-process.md v6.4 §13.1 (设计对齐) + §13.4 (重构即架构设计)
> **Complexity**: L3 (新增 driver 子系统 + AST 变更 + 新公共 API)
> **Task ID**: stage18.152

## 1. 设计文档对齐 (§13.1)

### 1.1 对应设计文档

- `docs/lang-design/10-toolchain.md` §3.3 项目布局 — 定义 `landin.toml` + `src/` + 子模块文件结构
- `docs/lang-design/10-toolchain.md` §3.4 依赖解析 — 简化版 semver (本 stage 不实现依赖解析)
- `docs/lang-design/05-ast.md` — AST `ModDecl` 已支持 `Loaded` variant (文件加载占位)
- `docs/lang-design/06-mir.md` §9 — `compile()` 公共 API 契约

### 1.2 设计意图摘要

Per `10-toolchain.md` §3.3，Landin 项目布局:
```
myapp/
├── landin.toml
└── src/
    ├── main.lin         # 二进制入口
    ├── module1.lin      # 子模块 (mod module1;)
    └── module1/
        └── submodule.lin # 嵌套模块 (mod module1::submodule;)
```

`mod foo;` 声明指示编译器从磁盘加载 `foo.lin` 或 `foo/mod.lin`。当前实现只把 `Loaded` 标记为占位，**不加载文件**——这是 TD-SINGLE-FILE 的核心阻塞点。

### 1.3 已实现 / 偏差项 / 未实现项

| 项 | 状态 | 说明 |
|----|------|------|
| `mod foo { ... }` 内联模块 | ✅ 已实现 | `ModDecl::Inline` + `HirModKind::Inline` |
| `mod foo;` 文件加载声明 | 🟡 偏差 | 解析支持，但文件**不加载**，`HirModKind::Loaded` 是空占位 |
| `use foo::bar;` 跨模块引用 | 🟡 偏差 | 解析支持，但 name resolution **不跨文件** |
| `landin.toml` manifest 解析 | ✅ 已实现 | `cargo.rs::ProjectManifest::parse_manifest` (Stage 5.24) |
| `landinc` CLI subcommands | ❌ 未实现 | 当前 `bin/main.rs` 是单文件 CLI，无 `build`/`run`/`new` 子命令 |
| 依赖解析 (semver) | ❌ 未实现 | 推迟到 v0.2 P1 |
| registry 拉取 | ❌ 未实现 | 推迟到 v0.3+ |

### 1.4 本 stage 范围 (Phase 1: 模块加载器)

**In scope**:
- 实现 `ModuleLoader` 递归加载 `mod foo;` 对应的 `foo.lin` / `foo/mod.lin`
- 新增公共 API `compile_project(entry_path: &Path) -> CompileResult`
- 修改 AST `ModDecl::Loaded` 携带加载后的 items (向后兼容)
- 修改 HIR lowering 把 `Loaded` 的 items 展开到 HIR (与 `Inline` 统一路径)
- e2e 测试：多文件项目能编译通过

**Out of scope (后续 stage)**:
- `use` 跨文件 name resolution (Stage 18.153)
- `landinc` CLI subcommands (Stage 18.154)
- `landin.toml` 完整支持 (Stage 18.155)
- 依赖解析 (v0.2 P1)

### 1.5 灰区决策

**灰区 1**: `ModDecl::Loaded` 是否保留原 span 还是改为加载后文件的 span？
- **决策**: 保留原 `mod foo;` 声明的 span (用于错误定位到声明处)，加载的 items 各自带自己的 span。
- **理由**: 用户写 `mod foo;` 时，如果 `foo.lin` 不存在，错误应指向声明处；如果 `foo.lin` 内部有语法错误，错误应指向 `foo.lin` 内部。

**灰区 2**: 模块路径解析规则 — `foo.lin` 优先还是 `foo/mod.lin` 优先？
- **决策**: `foo.lin` 优先 (Rust 语义)。
- **理由**: 与 Rust `mod foo;` 一致；`foo.lin` 是单文件模块，`foo/mod.lin` 是目录模块的入口。

**灰区 3**: 循环模块依赖如何处理？
- **决策**: ModuleLoader 维护 `visited: HashSet<PathBuf>`，遇到已访问路径报错 `circular module dependency`。
- **理由**: 循环依赖是用户错误，应明确报错而非无限递归 (§2 原则 4 报错>静默)。

## 2. 架构设计 (§13.4 J1-J6)

### 2.1 新结构

```
src/driver/
├── mod.rs                  # 新增 compile_project() 公共 API
├── module_loader.rs        # 新增: ModuleLoader + load_module_tree
├── ... (existing files)
```

### 2.2 J1 架构设计对齐

- `10-toolchain.md` §3.3 定义项目布局 → `ModuleLoader` 实现该布局的文件解析
- `05-ast.md` `ModDecl::Loaded` 已定义 → 本 stage 填充其语义 (加载文件)
- ✅ 通过

### 2.3 J2 单一职责

- `ModuleLoader`: 仅负责"从 AST `mod foo;` 声明解析出文件路径并读取内容"
- `compile_project`: 仅负责"编排 lex → parse → module_load → lower → ... → codegen"
- 每个函数一句话能描述
- ✅ 通过

### 2.4 J3 单向流动

```
compile_project(path)
  → read entry file
  → lex + parse entry file → AST::Crate
  → ModuleLoader::load_module_tree(ast, entry_dir)
       → for each ModDecl::Loaded:
            → resolve path (foo.lin or foo/mod.lin)
            → read file → lex + parse → recursive load
            → merge items into ModDecl::Loaded(items)
  → lower_crate(merged_ast) → existing pipeline
```

无环依赖。✅ 通过

### 2.5 J4 编译相关表达完整

- 模块加载逻辑 (路径解析 + 文件 IO + 递归) 全部在 `module_loader.rs`
- AST/HIR 不变 (除了 `ModDecl::Loaded` 携带 items)
- ✅ 通过

### 2.6 J5 阶段划分清晰

- `ModuleLoader` 在 driver 层 (parse 之后, lower 之前)
- 不跨阶段调用 (不直接调 codegen/typeck)
- ✅ 通过

### 2.7 J6 科学合理粒度

- 预估 `module_loader.rs` ~250 LOC
- `compile_project` ~80 LOC (复用 `compile_inner`)
- 在合理区间
- ✅ 通过

## 3. API 命名 (§10)

| 新增 | 命名 | 模式 |
|------|------|------|
| 类型 | `ModuleLoader` | `<Noun>Loader` (-er 后缀, 上下文类型) |
| 方法 | `ModuleLoader::new()` | `<verb>` 构造 |
| 方法 | `ModuleLoader::load_module_tree()` | `<verb>_<noun>_<noun>` |
| 函数 | `compile_project(entry_path: &Path) -> CompileResult` | `<verb>_<noun>` (入口函数模式) |
| 错误 | `ModuleLoadError` | `<Noun>LoadError` (Error 后缀) |

## 4. 接口设计 (§11)

### 4.1 公共 API

```rust
// src/driver/mod.rs
pub fn compile_project(entry_path: &Path) -> CompileResult;

// src/driver/module_loader.rs
pub struct ModuleLoader {
    visited: HashSet<PathBuf>,
    interner: Rodeo,  // 共享 interner
}

impl ModuleLoader {
    pub fn new() -> Self;
    pub fn load_module_tree(&mut self, krate: &mut ast::Crate, base_dir: &Path) -> Vec<ModuleLoadError>;
}
```

### 4.2 AST 变更 (向后兼容)

```rust
// Before:
pub enum ModDecl {
    Inline { ident, items, span },
    Loaded { ident, span },  // 空占位
}

// After:
pub enum ModDecl {
    Inline { ident, items, span },
    Loaded { ident, items, span },  // 携带加载后的 items (初始为空, ModuleLoader 填充)
}
```

`items: Vec<Item>` 新增字段。HIR lowering 把 `Loaded(items)` 展开为 `HirModKind::Inline(hir_items)` (统一路径)。

### 4.3 HIR 变更

`HirModKind::Loaded` 保留为占位 (语义: "声明但未加载")，但实际 `compile_project` 路径下 `Loaded` 永远不会被 HIR 看到 (因为 ModuleLoader 已把 items 填充并降级为 Inline-like 处理)。

**简化**: 直接把 `ModDecl::Loaded` 在 HIR lowering 中按 `Inline` 处理 (items 已经被 ModuleLoader 填充)。`HirModKind::Loaded` 保留用于"声明但 ModuleLoader 未运行"的场景 (例如 `compile(src)` 单文件路径)。

## 5. 测试计划 (§9)

### 5.1 测试矩阵

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `test_compile_project_single_file` | 正向 | 单文件项目能编译 |
| `test_compile_project_mod_file` | 正向 | `mod foo;` 加载 `foo.lin` |
| `test_compile_project_mod_dir` | 正向 | `mod foo;` 加载 `foo/mod.lin` |
| `test_compile_project_nested_mod` | 正向 | 嵌套 `mod a::b;` 加载 `a/b.lin` |
| `test_compile_project_mod_not_found` | 负向 | `mod foo;` 但 `foo.lin` 不存在 → 报错 |
| `test_compile_project_circular_mod` | 负向 | `a.lin: mod b;` + `b.lin: mod a;` → 报错 |
| `test_compile_project_inline_mod_unchanged` | 回归 | 内联 `mod foo { ... }` 仍正常 |

### 5.2 测试基础设施

测试用临时目录 (tempfile crate 或 std::env::temp_dir) 创建多文件项目结构，避免污染源码树。

## 6. 验收标准

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 622+ passed (新增 ~5 个 lib test)
- ✅ cargo test --tests --all-features: 2663+ passed (新增 ~5 个 integration test)
- ✅ 0 TODO/FIXME/HACK
- ✅ TD-SINGLE-FILE: Partial Resolved (Phase 1 of 4)

## 7. 简写和缺陷记录

### 7.1 当前简写

**简写 1**: `HirModKind::Loaded` 在 `compile(src)` 单文件路径下保留为空占位。
- **原因**: `compile(src)` 不接收文件路径，无法加载外部模块。
- **修订计划**: Stage 18.154 实现 `landinc build` CLI 时，统一走 `compile_project(path)` 路径，`compile(src)` 仅供单文件测试/REPL 使用。

**简写 2**: `ModuleLoader` 不解析 `#[path = "..."]` 属性 (Rust 支持自定义模块路径)。
- **原因**: Landin 暂未实现 attributes 系统 (v0.2 P2)。
- **修订计划**: 等 attributes 系统就绪后 (v0.2 P2)，扩展 `ModuleLoader` 读取 `#[path]`。

### 7.2 缺陷记录

**缺陷 1**: `compile_project` 不处理 `landin.toml` manifest — 仅接收入口文件路径。
- **原因**: 本 stage 聚焦模块加载，manifest 解析已有 `ProjectManifest::parse_manifest` 但未集成。
- **修订计划**: Stage 18.155 集成 manifest，`compile_project` 升级为 `compile_manifest(manifest: &ProjectManifest)`。

**缺陷 2**: 错误 span 跨文件时，`Span` 类型无法表达文件 ID。
- **原因**: 当前 `Span = (u32, u32)` 只有 byte range，无 file_id。
- **修订计划**: v0.2 P2 引入 `SourceMap` + `FileId`，`Span` 升级为 `{ file_id: FileId, range: Range<u32> }`。本 stage 用 `Span::DUMMY` 标记跨文件错误，错误消息中包含文件路径。
