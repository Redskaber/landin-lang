# Stage 6 Gate Review Round 16 (6.16) — resolve/resolver.rs architectural split per §14.4

> **审查日期**: 2026-07-25 | **版本**: v0.13.4 → v0.13.5
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 6.16 单一子阶段（resolve/resolver.rs 按 01-language-specification.md §6.2 拆分）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/01-language-specification.md` §6.2（解析顺序）：

- pass 1: build reduced graph（收集 items + use decls）
- pass 2: finalize imports（解析 use targets）
- pass 3: compute effective visibilities
- pass 4: late resolve crate（解析所有路径表达式）
- pass 5: resolve main
- pass 6-7: check unused imports + report errors
- pass 8: postprocess

**偏差**：实现把所有 25+ Resolver 方法 + lookup_prim_ty + resolve_crate entry 都堆在单一 `resolver.rs`（1131 LOC），违反 §14.4 J2 + J6。

**决策**：按 §6.2 的 pass 阶段聚合为 3 个子模块。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新结构按 01-language-specification.md §6.2 解析阶段划分 |
| J2 | 单一职责 | ✅ | module_build = pass 1-3；path_resolve = pass 4-5；primitives = helper |
| J3 | 单向流动 | ✅ | resolver.rs → {module_build, path_resolve, primitives}，无环 |
| J4 | 编译相关表达完整 | ✅ | module_build（10 个 module/use/vis 函数内聚）；path_resolve（11 个 path/expr 函数内聚） |
| J5 | 阶段划分清晰 | ✅ | 所有新模块在 `src/resolve/` 下，Stage 1 阶段未变 |
| J6 | 科学合理粒度 | ✅ | resolver.rs 154 LOC；子模块 32-577 LOC |

## 拆分执行结果

```
src/resolve/
  mod.rs          (30 LOC)    — crate-level re-exports + 3 子模块声明
  resolver.rs     (154 LOC)   ← Resolver struct + new + resolve + into_errors + helpers + entry (-86.4%)
  error.rs        (36 LOC)    — ResolveError 类型（不变）
  module_tree.rs  (145 LOC)   — ModuleNode 数据结构（不变）
  scope.rs        (174 LOC)   — ScopeStack 数据结构（不变）
  module_build.rs (470 LOC)   ← module tree 构建 + use 解析（§6.2 pass 1-3）
  path_resolve.rs (577 LOC)   ← late resolve 路径解析（§6.2 pass 4-5）
  primitives.rs   (32 LOC)    ← primitive type 查询表
```

**resolver.rs**: 1131 → **154 LOC**（-86.4%，-977 LOC）

## 可见性策略（§16 + §23 合规）

- `Resolver` struct 字段全部 `pub(super)` —— 子模块可读写
- 所有 cursor/helper 方法（`name_to_string` / `path_to_string`）`pub(super)`
- 所有提取的 `resolve_*` / `build_*` / `check_*` 方法 `pub(super)`
- `resolve_crate` 入口点保持 `pub`（不破坏对外 API）
- `into_errors` / `def_visibility` / `current_module` 保持 `pub`
- `lookup_prim_ty` `pub(super)`（resolver.rs + path_resolve.rs 调用）

## §23 API 命名合规

- 所有函数名保留原名（零 churn）
- 模块名遵循 `<noun>` 模式（与既有 `module_tree.rs`、`scope.rs` 风格一致）
- 无新公共符号（纯架构性重组）
- `resolve_crate` 仍是入口

## TD-026 累计进展

新增技术债 TD-026（Stage 6.16 引入）：resolve/resolver.rs 拆分为 3 子模块，已偿还。

| Stage | resolver.rs LOC | Δ |
|-------|----------------|---|
| 6.15 (baseline) | 1131 | — |
| **6.16 (architectural split)** | **154** | **-977 (-86.4%)** |

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 7-module 目录结构，每个模块单一职责，数据流单向 |
| D2 技术债清单 | ✅ TD-026 引入并立即偿还；TD-011/015/017/018/019/022/023/024/025 状态不变 |
| D3 测试覆盖 | ✅ 1881 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 6 架构性拆分继续推进；下一步是 §25.8 完整设计回写 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（行为等价拆分） |
| D7 文档 | ✅ plan-6.16 + gate-review-6.16 + dev-log + api-naming-standard v1.85 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | 完整 §25.8 设计回写（全 docs/lang-design/） | Stage 6 末尾 |
| P2 | TD-015: Region inference | Stage 6+ |
| P3 | TD-018: 用户自定义 trait dyn | Stage 6+ |

---

**审查完成**: 2026-07-25
