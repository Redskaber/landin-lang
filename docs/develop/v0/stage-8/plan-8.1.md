# Stage 8.1 开发计划：Lifetime elision 规则实现 + §13.4 设计对齐

> **阶段**: Stage 8.1（Stage 8 / v0.2 首个子阶段）
> **版本**: v0.14.9 → v0.15.0
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/04-ownership-borrowing.md` | §3.1-§3.4 生命周期系统 | Lifetime elision 设计基线 |
| `docs/lang-design/03-type-system.md` | §4 类型推导 | Inference variable 交互 |
| `docs/lang-design/06-mir.md` | §2 顶层结构（Region 类型） | MIR Region 数据结构 |

### 1.2 设计意图摘要（04-ownership-borrowing.md §3.2）

设计文档 §3.2 定义了 lifetime elision 规则（参考 Rust RFC #141）：

1. 每个引用参数的 lifetime 自动分配一个 fresh lifetime `'a`、`'b`、`'c`...
2. 若只有一个输入 lifetime，所有输出引用 lifetime 取 `'a`
3. 若有多个输入 lifetime 但其中一个是 `&self`/`&mut self`，所有输出引用 lifetime 取 self 的 lifetime
4. 否则，输出引用 lifetime 必须显式标注

**边界 case**（§3.2 v1.2 补全）：
1. 嵌套引用 `fn f(x: &Box<&u8>) -> &u8` — elision 不应用
2. `Box<Self>` 方法 — elision 取 self 的 lifetime
3. 泛型类型隐含 lifetime `fn f(x: &Vec<&T>) -> &T` — 隐含 `T: 'a`
4. async fn — elision 不应用（v0.2）
5. 多 lifetime 输入且无 self — 要求显式

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ `Region` enum（`Static` / `Var(RegionVid)` / `Erased`）— 设计 §4.1
- ✅ MIR lower 已处理 `Region::Erased`（所有引用类型使用 Erased）
- ✅ Region inference 基础设施完整（Stage 7.1-7.5, TD-015 CLOSE）
- ✅ Region inference 已集成到 borrowck（Stage 7.5, `run_region_inference`）

#### 已知偏差

- **B1（实现 < 设计）**：lifetime elision 规则未实现。当前 MIR lower 对所有
  引用类型使用 `Region::Erased`，不分配 fresh lifetime，不应用 elision 规则。
  这导致 region inference 当前为 no-op（所有 region 映射到 `'static` vid 0）。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 实现范围？ | 实现 §3.2 规则 1-4（核心 elision），边界 case 1-5 推迟 | MVP 优先核心规则 |
| 在哪里实现？ | 新增 `src/typeck/lifetime_elision.rs` 模块 | §14.4 J2 单一职责 |
| 如何集成？ | driver 在 MIR lower 后、typeck 前调用 | §16 数据流下游 |
| MIR Region 如何变化？ | `Region::Erased` → `Region::Var(fresh_vid)` | 激活 region inference |
| `pub` 可见性？ | 新增类型 `pub(crate)` | §16 隔离 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新模块按 04-ownership-borrowing.md §3.2 设计：
- `lifetime_elision.rs` = lifetime elision 规则实现

### 2.2 J2 单一职责 ✅

`lifetime_elision.rs` = "为函数签名的引用参数分配 fresh lifetime + 应用 elision 规则"

### 2.3 J3 单向流动 ✅

```
driver → MIR lower (Region::Erased)
  → lifetime_elision (Region::Var(fresh_vid))
  → typeck
  → borrowck (region inference activated)
```

### 2.4 J4 编译相关表达完整 ✅

elision 规则 1-4 + fresh lifetime 分配在模块内完整。

### 2.5 J5 阶段划分清晰 ✅

新模块在 `src/typeck/` 下，Stage 2 阶段。

### 2.6 J6 科学合理粒度 ✅

估算 ~300 LOC，合理区间。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/typeck/
  mod.rs          — re-exports
  checker.rs      — TypeChecker
  unify.rs        — UnificationTable
  error.rs        — TypeError
  tables.rs       — typeck 数据表
  predicates.rs   — type 谓词
  lifetime_elision.rs (新, ~300 LOC) ← lifetime elision 规则
```

### 3.2 新增内容

1. **`LifetimeElisionCtxt`** struct — 持有 fresh lifetime 计数器
2. **`elide_lifetimes(fn_sig)`** — 对函数签名应用 elision 规则
3. **`allocate_fresh_lifetime()`** — 分配 fresh `RegionVid`
4. **规则 1-4 实现** — 按设计 §3.2
5. **集成到 driver** — 在 MIR lower 后调用

### 3.3 §23 API 命名合规

- 类型名：`LifetimeElisionCtxt` — `<noun>_<noun>_<noun>` 模式
- 函数名：`elide_lifetimes` / `allocate_fresh_lifetime` — `<verb>_<noun>` 模式
- 模块名：`lifetime_elision` — `<noun>_<noun>` 模式

## 4. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 2042+ tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] 新增 `src/typeck/lifetime_elision.rs` ~300 LOC
- [ ] 测试文件 `tests/v0/stage8/plan/lifetime_elision_tests.rs`
- [ ] 版本 v0.14.9 → v0.15.0

---

**创建日期**: 2026-07-25
