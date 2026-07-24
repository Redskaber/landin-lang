# Stage 5 Gate Review Round 77 (5.77)

> **审查日期**: 2026-07-24 | **版本**: v0.11.72 → v0.11.73
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## CI/CD

```
cargo clean: clean (544.8 MiB removed)
cargo test: 1598 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 新增 API

| 符号 | 类型 | §23 |
|------|------|-----|
| `find_dyn_trait_method_call_in_plan_by_method` | free fn (in `mir`) | `find_<noun>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` ✅ |

## 设计要点

1. **Fuzzy lookup variant of Stage 5.75** — looks up by method_name only
   (no trait/type required)
2. 适用场景：MIR lower 阶段处理 `receiver.method(args)`，HIR 层只暴露
   `method.name`，receiver 的具体 dyn Trait 类型未知（typeck 职责）
3. First-match-wins 语义——当多个 method_call 共享 method_name 时，
   返回第一项。设计权衡：lower 阶段无法消歧，调用方需接受候选
4. §16 合规：纯只读，数据流在 `mir::dyn_trait` 内部
5. `_by_method` 后缀遵循 Rust API guidelines 的字段过滤命名约定
   （如 `iter_by`、`get_by`）
6. 12 个新测试覆盖：空 plan、单/多匹配、字段不匹配、大小写、跨 trait
   first-wins、跨 type first-wins、与 5.75 一致性、无副作用

## 与 5.75 / 5.76 的关系

| Stage | API | 用途 |
|-------|-----|------|
| 5.75 | `find_dyn_trait_method_call_in_plan` (精确) | 调用方已知 (trait, type, method) |
| 5.76 | `MirLowerCtxt::set_dyn_trait_plan` / `dyn_trait_plan()` | cx 接线 |
| 5.77 | `find_dyn_trait_method_call_in_plan_by_method` (模糊) | 调用方只知道 method_name |

Stage 5.78+ 将在 `mir/lower/` 的 `HirExprKind::MethodCall` 分支同时使用
5.76 的 cx 字段 + 5.77 的模糊查询。

**5/5 GO → PASS**

---

**审查完成**: 2026-07-24
