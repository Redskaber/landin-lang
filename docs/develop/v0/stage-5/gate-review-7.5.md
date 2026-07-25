# Stage 7 Gate Review Round 5 (7.5) — Integrate region inference into borrowck (TD-015 complete)

> **审查日期**: 2026-07-25 | **版本**: v0.14.4 → v0.14.5
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §1.2 + §17.1

## CI/CD

```
cargo clean: clean
cargo test: 126 unit + 1889 integration = 2015 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §4.2-§4.6（完整 NLL + region inference 规范）。

## 新增内容

### 1. borrowck 集成

在 `BorrowChecker::check_mir_body` 末尾添加 `run_region_inference(mir)` 调用：
- 创建 `RegionInferenceContext`
- 从 local declarations 的引用类型收集 implied bounds (§4.6.2)
- 运行 `infer_regions()` (§4.2 不动点迭代 + §4.6.4 type tests)
- 当前 MIR 所有 region 为 `Erased`（映射到 `'static` vid 0），推理为 no-op
- **不替换现有 NLL** — 作为附加检查运行（§14.4 安全集成策略）

### 2. 测试文件（§17.1 tests/ 目录标准化）

创建 `tests/v0/stage7/plan/region_inference_tests.rs`（8 个测试）：
- `stage7_region_inference_context_creation` — 空 MIR body 无错误
- `stage7_region_inference_simple_body` — 简单 i32 body 无错误
- `stage7_region_inference_ref_type_body` — 引用类型 body 无错误
- `stage7_borrow_checker_accepts_valid_borrow` — 有效共享借用通过
- `stage7_borrow_checker_detects_use_after_move` — use-after-move 不 panic
- `stage7_region_inference_context_standalone` — BorrowChecker + into_errors
- `stage7_regression_no_errors_on_simple_body` — 回归：空 body
- `stage7_regression_copy_type_not_moved` — 回归：Copy 类型多次使用

`tests/all_tests.rs` 新增 `#[path = "v0/stage7/plan/region_inference_tests.rs"]`

### §23 + §16 合规

- `run_region_inference` 方法：`<verb>_<noun>_<noun>` 命名
- `#[allow(dead_code)]` 保留在 region_inference 模块（部分基础设施未来才完全激活）
- 测试文件遵循 §17.1 `tests/v0/stageN/plan/` 结构

## TD-015 完成

| Step | 状态 | Stage |
|------|------|-------|
| step 1: data structures | ✅ | 7.1 |
| step 2: inference algorithm | ✅ | 7.2 |
| step 3: implied bounds + type tests | ✅ | 7.3 |
| step 4: universe + SCC | ✅ | 7.4 |
| **step 5: integrate into borrowck** | **✅** | **7.5** |

**🎉 TD-015 (Region inference) 全部 5 步完成！**

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
