# Stage 6.17 开发计划：mir/lower/expr_operand.rs 按表达式类别细拆

> **阶段**: Stage 6.17
> **版本**: v0.13.5 → v0.13.6
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

| 设计文档 | 章节 | 用途 |
|---------|------|------|
| `docs/lang-design/05-ast.md` | §8 表达式定义 | 表达式分类基线 |
| `docs/lang-design/06-mir.md` | §8 MIR 构建算法 | MIR lowering 设计 |

### 1.2 设计意图摘要（05-ast.md §8）

设计文档把表达式（`enum Expr`）按语义分为 8 个类别：
1. **字面量 + 路径**：Lit / Path
2. **块**：Block
3. **调用**：Call / MethodCall
4. **字段 + 索引**：Field / Index
5. **一元 + 二元 + 赋值**：Unary / Binary / Assign
6. **借用 + 类型转换 + 错误传播**：AddrOf / Cast / Try / Deref
7. **控制流**：If / Match / Loop / While / For
8. **闭包 + Return/Break/Continue**：Closure / Return / Break / Continue
9. **聚合字面量**：Struct / Array / Repeat / Tuple / Range / MacroCall / Unsafe

### 1.3 当前实现 vs 设计文档

#### 已对齐项

- ✅ 所有表达式类别都已实现
- ✅ 控制流表达式（If/Match/Block）委托给 control_flow.rs
- ✅ Deref 委托给 control_flow::lower_deref_expr

#### 已知偏差

- **B3 实现 ≠ 设计（结构层面）**：设计文档把表达式分为 8+ 语义类别，
  但实现把 `lower_expr_to_operand`（1046 LOC 的巨型 match）+ 3 个辅助函数
  都堆在单一 `expr_operand.rs`（1275 LOC），违反 §14.4 J2 + J6。

### 1.4 本阶段灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| 拆分策略？ | 把巨型 match 的复杂分支提取为独立函数，按表达式类别分组到子模块 | Rust match 不能跨文件，但可以把每个分支的 body 提取为函数 |
| 模块组织？ | 创建 `expr/` 子目录，按 5 个表达式类别组织 | 与 05-ast.md §8 语义分类对齐 |
| 是否拆分 lower_expr_to_place / build_dyn_trait_call_terminator / resolve_enum_variant？ | 不拆，保留在 expr_operand.rs | 它们是单一职责的小函数 |
| `pub` 可见性？ | 提取的函数 `pub(super)` 或 `pub(crate)` | §16 隔离 |

## 2. §14.4 J1-J6 判据检查

### 2.1 J1 架构设计对齐 ✅

新结构按 05-ast.md §8 表达式语义分类划分：

| 设计文档 §8 类别 | 新模块 | 提取的函数 |
|----------------|--------|----------|
| 字面量 + 路径 | `expr/primary.rs` | lower_lit_expr / lower_path_expr |
| 调用 | `expr/call.rs` | lower_call_expr / lower_method_call_expr |
| 聚合字面量 | `expr/aggregate.rs` | lower_tuple_expr / lower_array_expr / lower_repeat_expr / lower_struct_expr / lower_range_expr / lower_macro_call_expr / lower_closure_expr |
| 控制 + 跳转 | `expr/control.rs` | lower_return_expr / lower_loop_expr / lower_while_expr / lower_for_expr / lower_break_expr / lower_continue_expr / lower_try_expr / lower_unsafe_expr |
| 运算 + 借用 | `expr/ops.rs` | lower_binary_expr / lower_unary_expr / lower_field_expr / lower_index_expr / lower_addr_of_expr / lower_cast_expr / lower_assign_expr / lower_unit_expr |

### 2.2 J2 单一职责 ✅

每个新模块承担且仅承担一个明确的表达式类别职责。

### 2.3 J3 单向流动 ✅

```
expr_operand.rs (lower_expr_to_operand dispatcher)
  ↓ 调用
expr/primary.rs / expr/call.rs / expr/aggregate.rs / expr/control.rs / expr/ops.rs
```

无反向依赖。

### 2.4 J4 编译相关表达完整 ✅

每个模块的表达式类别在模块内是完整的。

### 2.5 J5 阶段划分清晰 ✅

所有新模块仍在 `src/mir/lower/` 目录下，Stage 2 阶段。

### 2.6 J6 科学合理粒度 ✅

拆分后 LOC 分布（估算）：

| 模块 | 估算 LOC |
|------|---------|
| `expr_operand.rs` | ~300（dispatcher + lower_expr_to_place + build_dyn_trait_call_terminator + resolve_enum_variant） |
| `expr/primary.rs` | ~200 |
| `expr/call.rs` | ~200 |
| `expr/aggregate.rs` | ~250 |
| `expr/control.rs` | ~200 |
| `expr/ops.rs` | ~250 |

每个模块均在 200-300 LOC 合理区间。

## 3. 拆分方案

### 3.1 目标组织结构

```
src/mir/lower/
  expr_operand.rs    (~300 LOC, -76%) ← dispatcher + 3 辅助函数
  expr/
    mod.rs           — re-exports
    primary.rs       ← Lit + Path
    call.rs          ← Call + MethodCall
    aggregate.rs     ← Tuple + Unit + Array + Repeat + Struct + Range + MacroCall + Closure
    control.rs       ← Return + Loop + While + For + Break + Continue + Try + Unsafe
    ops.rs           ← Binary + Unary + Field + Index + AddrOf + Cast + Assign
```

### 3.2 提取策略

每个 match 分支的 body 提取为独立函数：
```rust
// 原：
HirExprKind::Lit(lit) => {
    // 50 行实现
}

// 拆分后：
HirExprKind::Lit(lit) => expr::primary::lower_lit_expr(cx, lit, expr.span),

// 在 expr/primary.rs:
pub(super) fn lower_lit_expr(cx: &mut MirLowerCtxt, lit: &HirLitKind, span: Span) -> LocalId {
    // 50 行实现
}
```

### 3.3 §23 API 命名合规

- 所有函数名遵循 `lower_<kind>_expr` 模式
- 模块名遵循 05-ast.md §8 类别名
- 无新公共符号（提取的函数都是 `pub(super)`）

### 3.4 §16 接口隔离合规

- 子模块通过 `&mut MirLowerCtxt` 公开 API 交互
- 数据流单向：expr_operand.rs → expr/* 子模块

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 提取函数时遗漏局部变量 | 中 | 编译失败 | 仔细检查每个分支的变量捕获 |
| match 分支共享辅助函数 | 中 | 编译失败 | 共享辅助函数留在 expr_operand.rs 或提取到 expr/mod.rs |
| 1881 测试回归 | 低 | 测试失败 | 行为等价拆分，逐模块迁移 + cargo test 验证 |

## 5. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] `expr_operand.rs` 降到 ~300 LOC（-76%）
- [ ] 5 个新子模块各自单一职责
- [ ] 文档：plan-6.17.md + gate-review-6.17.md + dev-log + api-naming-standard v1.86 + RELEASE_NOTES + README + worklog
- [ ] 版本 v0.13.5 → v0.13.6

## 6. 后续 Stage 6.18+ 候选

完成本轮后：

- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
