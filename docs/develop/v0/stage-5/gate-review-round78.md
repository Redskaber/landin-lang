# Stage 5 Gate Review Round 78 (5.78)

> **审查日期**: 2026-07-24 | **版本**: v0.11.73 → v0.11.74
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## CI/CD

```
cargo clean: clean (545.5 MiB removed)
cargo test: 1611 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 新增 API

| 符号 | 类型 | §23 |
|------|------|-----|
| `build_dyn_trait_call_terminator` | free fn (in `mir::lower`) | `<verb>_<noun>_<noun>_<noun>_<noun>` ✅ |
| `MirBody.dyn_trait_calls` | pub field (in `mir::body`) | `<noun>_<noun>_<noun>` ✅ |

## 设计要点

1. **FIRST real mir/lower integration** — `HirExprKind::MethodCall` 分支
   首次使用 `cx.dyn_trait_plan()` + `find_dyn_trait_method_call_in_plan_by_method()`
2. **Side-table pattern** (§16-compliant): `MirBody.dyn_trait_calls: Vec<DynTraitMethodCall>`
   记录所有 dyn Trait 方法调用信息。`Terminator::Call` 的 `func` 用
   `Const{ty: Error, val: Int(index)}` 作为 marker——`index` 是 side-table
   条目索引。Codegen (Stage 5.79+) 读取此 side-table 翻译为 vtable indirect call。
3. **Borrow checker 处理**：先把 matched `DynTraitMethodCall` clone 出
   immutable borrow scope，再 mutable borrow `cx` 调用 helper
4. **Backward-compatible**：当 `cx.dyn_trait_plan()` 为 None 或 method 不匹配
   时，自动回退到原有 placeholder 路径——所有现有测试不受影响
5. §16 合规：data flow `mir::dyn_trait` → `mir::lower` → `mir::body` → codegen
   单向，无循环依赖
6. 13 个新测试覆盖：helper 构造、side-table 索引、call info 保持、
   args 顺序、destination、target None、func ty Error、无 plan 回退、
   匹配 plan 记录、多调用索引唯一、method_name 完整保留

## 不在本 stage 范围

- ❌ codegen 实际翻译 dyn Trait Const marker 为 vtable indirect call
  （Stage 5.79+）
- ❌ 在 driver 中自动调用 `set_dyn_trait_plan`（Stage 5.80+）
- ❌ MethodCall 的非 dyn Trait 路径（struct/enum inherent methods）

**5/5 GO → PASS**

---

**审查完成**: 2026-07-24
