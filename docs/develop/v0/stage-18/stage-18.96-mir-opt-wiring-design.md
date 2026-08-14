# Stage 18.96 — MIR Optimization Wiring

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.363.0 → v0.364.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/lang-design/06-mir.md` §9 MIR 优化 pass — 明确 pass 列表与顺序
- `docs/develop/v0/v0.1-capability-boundaries.md` §2 Code Generation — 已记录限制 "MIR optimization not wired"
- `docs/develop/v0/v0.4-roadmap.md` / `v0.5-roadmap.md` — P1 任务 "MIR optimization wiring"

### 1.2 设计意图摘要

`06-mir.md` §9.3 规定的 pass 顺序为：

```
MIR build → Drop elaboration → Borrow check → Dead store elimination → Const propagation → Jump threading → LLVM IR codegen
```

Stage 17.10/17.13 已实现 `run_dce` 与 `run_const_prop`，但根据 Stage 18.78 P0-D 决策，
**未接入 driver 流水线**，保留为 `#[allow(dead_code)]` + TODO。

v0.1 已发布，v0.2 路线图 P1 明确要求接线。本阶段执行接线。

### 1.3 已实现 / 偏差 / 未实现

| 项目 | 状态 |
|------|------|
| `run_dce` 实现 | ✅ Stage 17.10 |
| `run_const_prop` 实现 | ✅ Stage 17.13 |
| 结构化单元测试 | ✅ Stage 17.10/17.13 (1:3+ 正负比例) |
| driver 接线 | ❌ 本阶段实现 |
| Jump threading | ❌ 推迟至 v0.3（设计文档 §9.2 可选 pass） |

### 1.4 灰区决策

**灰区 1：pass 顺序**
- 设计文档明确：DCE → const_prop
- 实际效果：DCE 先移除明显死代码 → const_prop 在更小 MIR 上做常量传播
- 决策：**遵循设计文档**（§13.1.2 原则 1 "设计文档优先级最高"）

**灰区 2：是否新增 orchestrator 函数**
- 选项 A：driver 直接调用 `run_dce` + `run_const_prop`（两个调用）
- 选项 B：新增 `run_mir_optimizations(&mut mir)` 单一入口（§2.0 原则 6 "通用 > 特例"）
- 决策：**选项 B** — 单一入口封装 pass 顺序，未来添加新 pass 时只改一处

**灰区 3：测试更新策略**
- 旧测试：`compile(); for mir in &mut result.mirs { run_const_prop(mir); run_dce(mir); }`
- 新行为：`compile()` 已自动运行 opt → 手动调用变为二次运行（idempotent）
- 决策：**更新测试为验证 post-opt 状态**（§2.0 原则 5 "去除兼容思维"）

## 2. 任务拆分（MUV）

| ID | 任务 | 验收标准 |
|----|------|---------|
| 18.96.1 | 新增 `run_mir_optimizations(&mut mir)` orchestrator | 函数签名 `pub fn run_mir_optimizations(mir: &mut MirBody)`，按序调用 `run_dce` → `run_const_prop` |
| 18.96.2 | driver 接线 | 在 `writeback_closures(&mut mir)` 之后、`mirs.push(mir)` 之前调用 |
| 18.96.3 | 移除 `#![allow(dead_code)]` | clippy 0 warnings |
| 18.96.4 | 更新 doc comment | 从 "NOT wired" → "Wired at Stage 18.96" |
| 18.96.5 | 更新现有测试 | 移除手动 opt 调用，验证 post-opt 状态 |
| 18.96.6 | 新增 wiring 集成测试 | 1 个 positive（opt 已运行）+ 1 个 negative（idempotent） |
| 18.96.7 | 文档同步 | 设计文档 + RELEASE_NOTES + 能力边界 + worklog + Cargo.toml |

## 3. API 设计（§10 命名标准）

### 3.1 新增 API

```rust
// src/mir/optimization.rs

/// Stage 18.96: Run MIR optimization passes in design-doc order
/// (06-mir.md §9.3): DCE → const_prop.
///
/// Per §2.0 原则 6 "通用 > 特例": single entry point for all opt passes.
/// Per §23: `run_mir_optimizations` follows `<verb>_<noun>` pattern.
/// Per §11: driver (orchestrator) is allowed to call this.
pub fn run_mir_optimizations(mir: &mut MirBody) {
    run_dce(mir);
    run_const_prop(mir);
}
```

### 3.2 命名合规性

- ✅ 自由函数入口（§10.1 规则 1）
- ✅ `<verb>_<noun>` 命名（§10.1 规则 7）
- ✅ 单一真理源 — pass 顺序仅在此函数定义一次（§10.1 规则 5）
- ✅ 无 glob re-export（§10.1 规则 4）

## 4. 接口隔离分析（§11）

### 4.1 调用关系

```
driver::compile()
    ├── mir::lower::lower_hir_body_to_mir_full()  // Stage 2.1
    ├── typeck::TypeChecker::check_mir_body_with_tables()  // Stage 2.2
    ├── borrowck::BorrowChecker::check_mir_body_with_dataflow()  // Stage 2.3
    ├── mir::lower::writeback_type_propagation()  // Stage 15.7
    ├── mir::lower::writeback_closures()  // Stage 15.7
    ├── mir::optimization::run_mir_optimizations()  // ← Stage 18.96 新增
    └── mirs.push(mir)
```

### 4.2 §11 合规性

- ✅ driver 是编排层（§11.6 例外）— 允许调用各阶段入口
- ✅ opt 模块只读/写 MirBody（§16 允许的 MIR 写入）
- ✅ 无跨阶段内部函数调用 — `run_mir_optimizations` 是 `pub` 入口
- ✅ 数据流单向：lower → typeck → borrowck → writeback → opt → codegen

## 5. 测试策略（§9）

### 5.1 现有测试更新

| 测试 | 旧逻辑 | 新逻辑 |
|------|--------|--------|
| `stage17_10_dce_no_dead_code_no_change` | compile + 手动 DCE，before/after 比较 | compile 已自动 DCE，验证 Assign count >= 1（保留 used） |
| `stage17_10_dce_preserves_println` | compile + 手动 DCE | compile 已自动 DCE，验证 MIR 非空 |
| `stage17_13_const_prop_does_not_break` | compile + 手动 const_prop | compile 已自动 const_prop，验证 basic_blocks 非空 |
| `stage17_13_const_prop_handles_empty` | compile + 手动 const_prop | compile 已自动 const_prop |
| `stage17_13_const_prop_then_dce_reduces` | compile + 手动 const_prop+DCE，before/after < | compile 已自动 opt，验证 dead locals 已移除 |
| `stage17_13_const_prop_preserves_used` | compile + 手动 const_prop | compile 已自动 const_prop，验证 basic_blocks 非空 |
| `stage17_13_const_prop_handles_arithmetic` | compile + 手动 const_prop | compile 已自动 const_prop |
| `stage17_13_const_prop_handles_bool` | compile + 手动 const_prop | compile 已自动 const_prop |

### 5.2 新增集成测试

```rust
/// Stage 18.96 positive: compile() automatically runs MIR optimization.
/// Dead locals (x, y, z, _w) should be removed by DCE in the final MIR.
#[test]
fn stage18_96_opt_wired_dead_locals_removed() {
    let src = "fn main() { let x = 1; let y = 2; let z = x + y; let _w = z + 10; println!(\"hello\"); }";
    let result = compile(src);
    assert!(!result.has_errors());
    // After compile(), opt has already run. All 4 dead locals should be DCE'd.
    // Only the println! call should remain (no Assign statements).
    let assign_count: usize = result
        .mirs
        .iter()
        .flat_map(|m| m.basic_blocks.iter())
        .flat_map(|bb| bb.statements.iter())
        .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
        .count();
    assert_eq!(
        assign_count, 0,
        "Dead locals should be DCE'd: got {} Assign statements",
        assign_count
    );
}

/// Stage 18.96 negative: opt is idempotent — running it again is a no-op.
#[test]
fn stage18_96_opt_idempotent() {
    let src = "fn main() { let x = 42; println!(\"{}\", x); }";
    let mut result = compile(src);
    assert!(!result.has_errors());
    // Snapshot the state after compile() (opt already ran).
    let before: usize = result
        .mirs
        .iter()
        .flat_map(|m| m.basic_blocks.iter())
        .flat_map(|bb| bb.statements.iter())
        .count();
    // Run opt again — should be idempotent.
    for mir in &mut result.mirs {
        run_mir_optimizations(mir);
    }
    let after: usize = result
        .mirs
        .iter()
        .flat_map(|m| m.basic_blocks.iter())
        .flat_map(|bb| bb.statements.iter())
        .count();
    assert_eq!(before, after, "Second opt pass should be idempotent");
}
```

## 6. 风险与回滚

### 6.1 风险

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| opt 改变 MIR 语义 → codegen 错误 | 低 | 高 | 全量 conformance 测试（2935 cases）守护 |
| opt 移除 borrowck 需要的信息 | 极低 | 高 | opt 在 borrowck 之后运行，borrowck 已完成分析 |
| 现有测试 before/after 断言失败 | 高 | 低 | 已在 §5.1 列出更新策略 |

### 6.2 回滚策略

- 单点回滚：移除 driver.rs 中的 `run_mir_optimizations(&mut mir)` 调用
- 模块保留：`run_dce` / `run_const_prop` 保持可用（不删除）
- 测试可逆：更新后的测试可还原为手动 opt 调用

## 7. 验收标准（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend` 全绿（lib + integration）
- [x] `python3 tests/conformance/run_all.py` 全绿（2935 cases）
- [x] 文档同步完成（设计文档 + RELEASE_NOTES + 能力边界 + worklog）
- [x] Cargo.toml 版本号 v0.363.0 → v0.364.0
