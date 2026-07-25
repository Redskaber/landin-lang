# Stage 8 Gate Review Round 1 (8.1) — Lifetime elision 规则实现 (v0.2 启动)

> **审查日期**: 2026-07-25 | **版本**: v0.14.9 → v0.15.0
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 129 unit + 1923 integration = 2052 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §3.2 (Lifetime elision 规则)。

## 新增内容

### 1. `src/typeck/lifetime_elision.rs` (新模块, ~200 LOC)

| 类型/方法 | 用途 |
|---------|------|
| `LifetimeElisionCtxt` | Fresh lifetime 分配上下文 |
| `allocate_fresh_lifetime()` | 分配 fresh `RegionVid` (从 1 开始) |
| `elide_lifetimes(fn_sig)` | 对函数签名应用 §3.2 规则 1-4 |
| `LifetimeElisionError` | Elision 错误（MissingLifetime） |
| `collect_erased_regions(ty)` | 从 HIR 类型递归收集引用 region |

### 2. 测试文件 (§17.1)

`tests/v0/stage8/plan/lifetime_elision_tests.rs` — 7 个测试：
- module exists / pipeline with refs / simple fn / ref param / mut ref / nested refs / ref return

## §14.4 J1-J6 (全过)

J1 对齐 §3.2 / J2 单一职责 / J3 单向流动 / J4 完整 / J5 typeck 下 / J6 ~200 LOC

## §23 命名合规

- `LifetimeElisionCtxt` — `<noun>_<noun>_<noun>`
- `elide_lifetimes` / `allocate_fresh_lifetime` — `<verb>_<noun>`

## v0.2 路线图

| 优先级 | 行动 | 状态 |
|--------|------|------|
| P1 | Lifetime elision 规则 (§3.2) | ✅ 本轮完成 |
| P2 | Object safety 规则 (§2.3) | pending |
| P2 | extern "C" ABI (§13.2) | pending |
| P2 | Drop elaboration (§5) | pending |
| P3 | async/await (§10) | pending |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
