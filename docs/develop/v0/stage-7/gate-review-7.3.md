# Stage 7 Gate Review Round 3 (7.3) — Implied bounds + type tests (TD-015 step 3)

> **审查日期**: 2026-07-25 | **版本**: v0.14.2 → v0.14.3
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 120 unit + 1881 integration = 2001 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §4.6.2（Implied bounds）+ §4.6.4（Type tests）。

## 新增内容

| 类型/方法 | 设计 § | 用途 |
|---------|--------|------|
| `RegionInferenceError::TypeTestFailed` | §4.6.4 | Type test 失败错误 |
| `extract_regions_from_ty(ty)` | §4.6.2 | 从 Ty 递归提取所有 RegionVid |
| `collect_implied_bounds(ref_region, inner_ty, span)` | §4.6.2 | `&'a T` → `T: 'a` 约束收集 |
| `infer_regions()` 扩展 | §4.6.4 | Step 4: type test 验证 |

### Implied bounds 算法（§4.6.2）

`&'a T` 隐含 `T: 'a`。实现：
1. `extract_regions_from_ty(T)` 递归提取 T 中所有 region
2. 对每个 region `r`，添加 `r: 'a` outlives constraint

### Type test 验证（§4.6.4）

在 `infer_regions()` 末尾（Step 4）：
1. 对每个 TypeTest `{ universal_region, ty, span }`
2. `extract_regions_from_ty(ty)` 提取 ty 中所有 region
3. 检查每个 region `r` 是否 outlive `universal_region`（r.points ⊆ ur.points）
4. 失败则报 `TypeTestFailed`

### 单元测试（6 个新增，共 22 个 region_inference 测试）

- `test_extract_regions_from_ref` — `&'a i32` → [vid_a]
- `test_extract_regions_from_nested_ref` — `&'a &'b i32` → [vid_b, vid_a]
- `test_extract_regions_from_non_ref` — `i32` → []
- `test_collect_implied_bounds` — `&'a &'b i32` → 约束 `'b: 'a`
- `test_type_test_passes` — `i32: 'static` 通过
- `test_type_test_fails` — `&'a i32: 'static` 失败（'a escape 'static）

## §23 + §16 合规

- 所有新类型 `pub(crate)`
- 命名遵循 `<verb>_<noun>` / `<noun>_<noun>` 模式
- 1881 原有 tests 零回归

## TD-015 进展

| Step | 状态 | Stage |
|------|------|-------|
| step 1: data structures | ✅ | 7.1 |
| step 2: inference algorithm | ✅ | 7.2 |
| **step 3: implied bounds + type tests** | **✅** | **7.3** |
| step 4: universe + SCC | pending | 7.4 |
| step 5: integrate into borrowck | pending | 7.5 |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
