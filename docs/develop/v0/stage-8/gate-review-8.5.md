# Stage 8 Gate Review Round 5 (8.5) — async/await foundation (§10)

> **审查日期**: 2026-07-25 | **版本**: v0.15.3 → v0.15.4
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 146 unit + 1945 integration = 2091 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `12-roadmap.md` §4.1 (v0.2: async fn + Future + async/await)。

## 新增内容

### 1. AST: `Expr::Await` + `Expr::Async` (src/ast/kinds.rs)

| Variant | Syntax | MVP 行为 |
|---------|--------|---------|
| `Await { expr, span }` | `await expr` | 同步求值 expr |
| `Async { block, span }` | `async { block }` | 同步执行 block |

### 2. HIR: `HirExprKind::Await` + `HirExprKind::Async` (src/hir/kinds.rs)

### 3. Parser: `KwAsync` + `KwAwait` 分支 (src/parser/expr.rs)

- `async { block }` → `Expr::Async`
- `await expr` → `Expr::Await`
- 已加入 `is_expr_start` lookahead

### 4. HIR lowering: async/await → 同步求值 (src/hir/lower/body.rs)

### 5. MIR lowering: async/await → 同步求值 (src/mir/lower/expr_operand.rs)

### 6. Resolve: async/await 路径解析 (src/resolve/path_resolve.rs)

### 7. Closure capture: async/await 捕获收集 (src/mir/lower/closure_capture.rs)

### 8. `src/ast/async_marker.rs` — AsyncMarker 工具类型

### 测试

- 3 个单元测试 (async_marker.rs 内联)
- 5 个集成测试 (tests/v0/stage8/plan/async_await_tests.rs)

## v0.2 路线图

| 优先级 | 行动 | 状态 |
|--------|------|------|
| P1 | Lifetime elision (§3.2) | ✅ Stage 8.1 |
| P2 | Object safety (§2.3) | ✅ Stage 8.2 |
| P2 | extern "C" ABI (§13.2) | ✅ Stage 8.3 |
| P2 | Drop elaboration (§5) | ✅ Stage 8.4 |
| P3 | async/await (§10) | ✅ Stage 8.5 |

**🎉 v0.2 路线图全部 5 项完成！**

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
