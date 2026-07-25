# Stage 7.2 开发计划：Region inference 算法 (TD-015 step 2)

> **阶段**: Stage 7.2
> **版本**: v0.14.1 → v0.14.2
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4

## 1. §13.4 设计对齐

### 1.1 对应设计文档

`docs/lang-design/04-ownership-borrowing.md` §4.2 阶段 2（Region inference）。

### 1.2 算法（§4.2 不动点迭代）

```
algorithm region_inference:
    # 初始化：每个 region = 空 set
    for r in all_regions: r.points = {}

    # 不动点迭代
    changed = true
    while changed:
        changed = false
        for (r1, r2) in constraints:  # r1: r2 表示 r1 ⊇ r2
            new = r1.points ∪ r2.points
            if new != r1.points:
                r.points = new
                changed = true
        for r in regions:
            for use_point in r.use_points:
                if use_point not in r.points:
                    r.points = r.points ∪ {use_point}
                    changed = true

    # 检查 universal region
    for ur in universal_regions:
        for r in non_universal_regions:
            if r.points ⊄ ur.points:
                report_error(ur, r)
```

复杂度 O(R² × P)，R=region 数，P=CFG 点数。

### 1.3 本阶段决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| PointIndex 表示？ | `u32`（bb_id << 16 | stmt_idx） | 简单，足够 MVP |
| RegionSet 表示？ | `Vec<u32>`（sorted point set） | 简单，避免 BitSet 依赖 |
| 算法集成？ | 独立方法 `infer_regions()`，不自动调用 | Stage 7.5 集成到 borrowck |
| use_points 来源？ | 提供 `add_use_point(vid, point)` API | Stage 7.5 由 borrowck 填充 |
| 错误报告？ | `RegionInferenceError` 类型 | 不直接 panic |

## 2. §14.4 J1-J6 判据

| # | 判据 | 状态 |
|---|------|------|
| J1 | 对齐 §4.2 算法 | ✅ |
| J2 | 单一职责（inference 算法） | ✅ |
| J3 | 单向流动（context.infer_regions() → 结果） | ✅ |
| J4 | 算法完整（初始化 + 不动点 + universal check） | ✅ |
| J5 | 在 src/borrowck/ 下 | ✅ |
| J6 | 估算 +200 LOC | ✅ |

## 3. 执行计划

在 `region_inference.rs` 中新增：
1. `PointIndex` type alias (`u32`)
2. `RegionSet` type (`Vec<u32>` sorted)
3. `RegionInferenceError` enum
4. `add_use_point(vid, point)` 方法
5. `infer_regions()` 方法（不动点迭代 + universal check）
6. `region_points(vid)` getter
7. 单元测试

## 4. 验收

- [ ] cargo clean + cargo test — 1890+ tests 全过
- [ ] cargo fmt + clippy — clean
- [ ] 版本 v0.14.1 → v0.14.2

---

**创建日期**: 2026-07-25
