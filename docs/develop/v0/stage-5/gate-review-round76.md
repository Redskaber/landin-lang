# Stage 5 Gate Review Round 76 (5.76)

> **审查日期**: 2026-07-24 | **版本**: v0.11.71 → v0.11.72
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## CI/CD

```
cargo clean: clean (543.7 MiB removed)
cargo test: 1586 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 新增 API

| 符号 | 类型 | §23 |
|------|------|-----|
| `MirLowerCtxt::set_dyn_trait_plan` | method (in `mir::lower`) | `<verb>_<noun>_<noun>_<noun>` (setter) ✅ |
| `MirLowerCtxt::dyn_trait_plan` | method (in `mir::lower`) | `<noun>_<noun>_<noun>` (getter, no `get_` prefix per C-GETTER) ✅ |
| `MirLowerCtxt.dyn_trait_plan` | pub field (in `mir::lower`) | `<noun>_<noun>_<noun>` ✅ |

## 设计要点

1. **First mir/lower integration step** — context wiring only, no lowering
   logic changes. Stage 5.77+ will use this field in `HirExprKind::MethodCall`.
2. 设计上明确**不提供 unset 方法** — 一旦 plan 附加，就在 cx 生命周期内存在
   （与 `hir` 字段语义一致）
3. setter 接受 owned `DynTraitMIRPlan`（按值传入），cx 持有所有权
4. getter 返回 `Option<&DynTraitMIRPlan>`（只读引用）
5. §16 合规：plan 由 driver 上游构建（via
   `build_dyn_trait_mir_plan_from_resolver()`），lower 仅读，无循环依赖
6. 11 个新测试覆盖：默认 None、set/get 往返、字段保持、set 两次覆盖、
   空计划、字段隔离、getter 幂等、pub 字段可访问

## 不在本 stage 范围

- ❌ 不修改 `HirExprKind::MethodCall` 分支（Stage 5.77+）
- ❌ 不修改 `lower_hir_body_to_mir_full` 自动设置 plan（Stage 5.78+ 由 driver 接入）
- ❌ 不在 driver 中调用 `set_dyn_trait_plan`（Stage 5.78+）

**5/5 GO → PASS**

---

**审查完成**: 2026-07-24
