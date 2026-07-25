# Stage 6.16 开发计划：resolve/resolver.rs 架构性拆分 — 按解析阶段 3 模块

> **阶段**: Stage 6.16
> **版本**: v0.13.4 → v0.13.5
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/01-language-specification.md` | §6.2 解析顺序 | Resolver 设计基线 |

### 1.2 设计意图摘要（01-language-specification.md §6.2）

设计文档把 name resolution 分为 8 pass（MVP 简化为 4 pass）：

1. **Build reduced graph**：收集所有 module 内的 item 名称、use 导入，建立初始符号表
2. **Finalize imports**：解析所有 use 导入的目标
3. **Compute effective visibilities**：计算每个 item 的有效可见性
4. **Late resolve crate**：解析所有路径表达式、类型路径、模式路径
5. **Resolve main**：确定 crate root
6. **Check unused imports**：警告未使用的 use
7. **Report errors**：报告所有 unresolved name
8. **Postprocess**：清理临时数据

MVP 简化为 4 pass：合并 1-3 / 4-5 / 6-7 / 8。

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ Build reduced graph（设计 §6.2 pass 1，实现 `build_module_tree`）
- ✅ Finalize imports（设计 §6.2 pass 2，实现 `resolve_uses`）
- ✅ Late resolve crate（设计 §6.2 pass 4，实现 `resolve_all_paths` + `resolve_expr` + ...）
- ✅ Report errors（设计 §6.2 pass 7，实现 `errors` 字段 + `into_errors`）

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档把 name resolution 分为多个 pass，
  但实现把所有 25+ Resolver 方法 + lookup_prim_ty + resolve_crate entry 都堆在
  单一 `resolver.rs`（1131 LOC），违反 §14.4 J2 + J6。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分粒度？ | 按设计 §6.2 的 pass 阶段聚合为 3 个子模块 | 与设计文档对齐（§14.4 J1） |
| 是否拆分 Resolver struct？ | 不拆，保留在 resolver.rs | 是 resolver 的核心数据结构 |
| 是否拆分 lookup_prim_ty？ | 提取到 `primitives.rs` | 是独立的 primitive type 查询表 |
| `pub` 可见性？ | 现有 pub 函数保留 pub；私有方法改 pub(super) | §16 隔离——resolve 外部接口不变 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 01-language-specification.md §6.2 解析阶段划分：

| 设计文档 pass | 新模块 | 内容 |
|--------------|--------|------|
| §6.2 pass 1-3 (build + imports + vis) | `module_build.rs` | build_module_tree + collect_item_registration + build_child_module + item_def_id + resolve_uses + resolve_use_tree + resolve_use_leaf + resolve_use_glob + lookup_use_path_target + check_visibility |
| §6.2 pass 4-5 (late resolve) | `path_resolve.rs` | resolve_all_paths + resolve_owner_paths + resolve_item_paths + resolve_generics_paths + resolve_ty_paths + resolve_hir_path + resolve_path + resolve_body + collect_pat_bindings + resolve_expr + resolve_block |
| §6.2 helpers | `primitives.rs` | lookup_prim_ty |
| §6.2 entry + struct | `resolver.rs` | Resolver struct + new + resolve + into_errors + name_to_string + path_to_string + def_visibility + current_module + resolve_crate entry |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的职责：
- `module_build.rs` = "构建 module tree + 解析 use 导入（pass 1-3）"
- `path_resolve.rs` = "late resolve：解析所有路径表达式（pass 4-5）"
- `primitives.rs` = "primitive type 查询表"
- `resolver.rs` = "Resolver struct + 入口点"

### 2.3 J3 单向流动 ✅

模块依赖图：

```
resolver.rs (Resolver struct + entry)
  ↓ 调用
module_build.rs (pass 1-3) / path_resolve.rs (pass 4-5) / primitives.rs (helpers)
```

无反向依赖：子模块不调用 Resolver::new / resolve / into_errors。
无循环依赖：所有子模块是叶子模块，只通过 `&mut self` 方法访问 Resolver 字段。

### 2.4 J4 编译相关表达完整 ✅

每个模块的"编译相关概念"在模块内是完整的：
- `module_build.rs`：module tree 构建 + use 解析全部内聚
- `path_resolve.rs`：所有 path/expr/body 解析内聚
- `primitives.rs`：primitive type 表内聚

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/resolve/` 目录下，仍是 Stage 1 阶段。不破坏 §16 阶段隔离。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC | 设计依据 |
|------|---------|---------|
| `resolver.rs` | ~250 | Resolver struct + new + resolve + into_errors + helpers + entry |
| `module_build.rs` | ~430 | 10 个 module/use 解析函数 |
| `path_resolve.rs` | ~450 | 11 个 path/expr 解析函数 |
| `primitives.rs` | ~40 | lookup_prim_ty |
| **总计** | ~1170 | （含模块头注释略增） |

每个模块均在 40-1500 LOC 合理区间。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/resolve/
  mod.rs          (21 LOC, 不变)    — crate-level re-exports
  resolver.rs     (~250 LOC, -78%)  ← Resolver struct + new + resolve + entry + helpers
  error.rs        (36 LOC, 不变)    — ResolveError 类型
  module_tree.rs  (145 LOC, 不变)   — ModuleNode 数据结构
  scope.rs        (174 LOC, 不变)   — ScopeStack 数据结构
  module_build.rs (新, ~430 LOC)    ← module tree 构建 + use 解析（§6.2 pass 1-3）
  path_resolve.rs (新, ~450 LOC)    ← late resolve 路径解析（§6.2 pass 4-5）
  primitives.rs   (新, ~40 LOC)     ← primitive type 查询表
```

### 3.2 可见性策略（与 Stage 6.14/6.15 一致）

- `Resolver` struct 字段保持 `pub(super)` 或私有 + 通过方法访问
- 所有现有 `pub` 函数保持 `pub`（不破坏对外 API）
- 提取的私有方法改 `pub(super)`
- `resolve_crate` 入口点保持 `pub`

### 3.3 §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `module_tree.rs`、`scope.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `resolve_crate` 仍是入口

### 3.4 §16 接口隔离合规

- 子模块通过 `impl Resolver` 方法访问，不直接读字段（除 pub(super) 字段）
- 数据流单向：resolver.rs 入口 → Resolver.resolve → module_build/path_resolve 辅助
- 无跨阶段调用

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Resolver 字段可见性不足 | 中 | 编译失败 | 把字段改 pub(super) 让子模块可访问 |
| impl 跨文件导致方法找不到 | 低 | 编译失败 | 每个子模块独立 `impl Resolver { ... }` |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `resolve/resolver.rs` 降到 ~250 LOC（-78%）
- [ ] 3 个新子模块各自单一职责
- [ ] 文档：plan-6.16.md + gate-review-6.16.md + dev-log + api-naming-standard v1.85 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.4 → v0.13.5

## 6. 后续 Stage 6.17+ 候选

完成本轮后：

- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
