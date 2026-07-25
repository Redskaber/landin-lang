# Stage 7 Gate Review Round 2 (7.2) — Region inference 算法 (TD-015 step 2)

> **审查日期**: 2026-07-25 | **版本**: v0.14.1 → v0.14.2
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 114 unit + 1881 integration = 1995 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §4.2（Region inference 不动点迭代算法）。

## §14.4 J1-J6（全过）

新内容扩展 `region_inference.rs`（不新建模块），J1-J6 全部通过。

## 新增内容

| 类型/方法 | 设计 § | 用途 |
|---------|--------|------|
| `PointIndex` (u32) | §4.2 | CFG 点编码 (bb_id << 16 \| stmt_idx) |
| `make_point` / `point_bb` / `point_stmt` | — | 编解码 helper |
| `RegionSet` (Vec<u32>) | §4.2 | sorted point set |
| `RegionInferenceError` (enum) | §4.2 | RegionEscapesUniversal |
| `add_use_point(vid, point)` | §4.2 | 添加 use point |
| `infer_regions()` | §4.2 | **核心算法**：不动点迭代 + universal check |
| `region_points(vid)` | §4.2 | 获取推断结果 |

### 算法实现（§4.2）

```
1. 初始化：每个 region point set = empty
2. 不动点迭代：
   a. 对每个 'sup: 'sub 约束：sup.points = sup.points ∪ sub.points
   b. 对每个 region：添加其 use_points
   c. 重复直到无变化
3. 检查 universal region：
   对每个 universal ur，每个非 universal r：
   r.points ⊆ ur.points？否则报 RegionEscapesUniversal
```

### 单元测试（7 个新增，共 16 个）

- `test_infer_regions_empty` — 空 context
- `test_infer_regions_use_points` — use point 收集
- `test_infer_regions_constraint_propagation` — 约束传播
- `test_infer_regions_universal_escape_detected` — escape 检测
- `test_infer_regions_universal_no_escape` — 无 escape
- `test_point_encoding` — PointIndex 编解码
- `test_infer_regions_fixed_point_convergence` — 链式约束收敛

## §23 + §16 合规

- 所有新类型 `pub(crate)`
- 命名遵循 `<noun>` / `<verb>_<noun>` 模式
- 模块独立，不修改现有 borrowck 代码
- 1881 原有 tests 零回归

## TD-015 进展

| Step | 状态 | Stage |
|------|------|-------|
| step 1: data structures | ✅ | 7.1 |
| **step 2: inference algorithm** | **✅** | **7.2** |
| step 3: implied bounds + type tests | pending | 7.3 |
| step 4: universe + SCC | pending | 7.4 |
| step 5: integrate into borrowck | pending | 7.5 |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
