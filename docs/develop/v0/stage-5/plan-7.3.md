# Stage 7.3 开发计划：Implied bounds + type tests (TD-015 step 3)

> **阶段**: Stage 7.3
> **版本**: v0.14.2 → v0.14.3
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4

## §13.4 设计对齐

- §4.6.2 Implied bounds: `&'a T` → `T: 'a`
- §4.6.4 Type tests: `T: 'a` 验证在 region inference 后检查

## 新增

- `extract_regions_from_ty(ty)` — 递归提取 Ty 中所有 RegionVid
- `collect_implied_bounds(ref_region, inner_ty, span)` — implied bounds 收集
- `RegionInferenceError::TypeTestFailed` — type test 失败错误
- `infer_regions()` Step 4: type test 验证

## 验收

- [x] cargo test — 2001 tests (120 unit + 1881 integration)
- [x] cargo fmt + clippy — clean
- [x] 版本 v0.14.2 → v0.14.3

---

**创建日期**: 2026-07-25
