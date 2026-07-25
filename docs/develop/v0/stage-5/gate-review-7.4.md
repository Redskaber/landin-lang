# Stage 7 Gate Review Round 4 (7.4) — Universe tracking + SCC compression (TD-015 step 4)

> **审查日期**: 2026-07-25 | **版本**: v0.14.3 → v0.14.4
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 126 unit + 1881 integration = 2007 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §4.6.3（Universe 机制）+ §4.6.5（SCC 压缩）。

## 新增内容

| 类型/方法 | 设计 § | 用途 |
|---------|--------|------|
| `SccId` (struct) | §4.6.5 | SCC 标识符 |
| `UniverseEscapeError` (struct) | §4.6.3 | Universe escape 错误 |
| `region_universe(vid)` | §4.6.3 | 获取 region 所属 universe |
| `check_universe_escapes()` | §4.6.3 | 检查 universe escape（soundness） |
| `compute_sccs()` | §4.6.5 | Tarjan SCC 算法（O(V+E)） |

### Universe tracking（§4.6.3）

HRTB `for<'a>` 创建新 universe。每个 inference region 属于一个 universe。
`check_universe_escapes()` 验证高 universe 的 region 不会被约束为 outlive 低
universe 的 region（防止变量捕获导致的 unsound）。

### SCC compression（§4.6.5）

使用 Tarjan 算法计算约束图的强连通分量。同一 SCC 中的 region 互相 outlive，
可以压缩为单个节点，避免 O(R²×P) 退化为指数复杂度。

### 单元测试（6 个新增，共 28 个 region_inference 测试）

- `test_region_universe` — 获取 region 所属 universe
- `test_check_universe_escapes_no_violation` — 同 universe 无 escape
- `test_check_universe_escapes_detected` — 跨 universe escape 检测
- `test_scc_no_constraints` — 无约束 → 每个 region 独立 SCC
- `test_scc_mutual_constraints` — 互相约束 → 同一 SCC
- `test_scc_chain` — 链式约束 → 独立 SCC

## TD-015 进展

| Step | 状态 | Stage |
|------|------|-------|
| step 1: data structures | ✅ | 7.1 |
| step 2: inference algorithm | ✅ | 7.2 |
| step 3: implied bounds + type tests | ✅ | 7.3 |
| **step 4: universe + SCC** | **✅** | **7.4** |
| step 5: integrate into borrowck | pending | 7.5 |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
