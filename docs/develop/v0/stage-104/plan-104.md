# Stage 104 开发计划 — TD-MONO-INFER: type inference back-propagation for FnDef substs

> **阶段**: v0.11 (TD-MONO-INFER 修复 — type inference back-propagation)
> **TD**: TD-MONO-INFER (P3, v0.11+) — Stage 101 发现
> **复杂度**: L3 (跨模块: mir/lower/expr_variants + hir/generics + mir/substitute)
> **版本基线**: v0.642.0 (Stage 103 Layer 3 partial, 5613 tests)
> **目标版本**: v0.643.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 在 `lower_call_expr` 中, 当 func 是 FnDef 且 substs 为空时, 从 fn_sigs 获取 generic_params + sig inputs, 对每个 (sig_input[i], arg_ty[i]) pair 调用 `collect_param_bindings` 推断 substs, 然后更新 FnDef 类型的 substs |
| **WHY** | TD-MONO-INFER 根因: `Box::new(42i32)` 无 turbofish → `lower_path_generic_args` 返回空 substs → FnDef(def_id, []) → codegen_operand fallback 到 generic def name → generic def body 必须 emit (Param fallback to i32 → 错误 LLVM IR → SIGSEGV) |
| **WHO** | ARCH-A 设计; DEV-A 实施; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 104 完成 → 进入 Stage 105 (重新添加 Debug + PartialOrd impls) |
| **WHERE** | `src/mir/lower/expr_variants.rs:271-917 lower_call_expr`; 复用 `src/mir/lower/expr_operand.rs:1962 collect_param_bindings` + `src/hir/generics.rs:128 find_generics_for_fn_owner` |
| **HOW** | 1) 在 lower_call_expr 创建 Call terminator 前, 检查 func_local 的 FnDef substs; 2) 如果空, 查 fn_sigs 获取 sig.inputs; 3) 查 HIR 获取 generic_params; 4) 对每个 (sig_input[i], arg_operand_ty[i]) pair collect_param_bindings; 5) 构建 inferred substs; 6) 更新 func_local 的 FnDef 类型 substs |
| **HOW MUCH** | 1 src 文件 (~60 LOC) + 1 测试文件 (~80 LOC) |

## 二、对齐设计文档 (§13.1 / §8.4.5)

### Rust 设计对齐
rustc typeck: generic function call 的 substs 通过 type inference 推断 — 从 arg types 反向推断 generic param types。Landin 当前 MIR lower 不做这个推断, 导致 FnDef substs 为空。

### docs/graph/mir/data-flow.md 对齐
data flow: MIR lower (产生 FnDef 空 substs) → writeback (不处理 FnDef substs) → codegen (fallback 到 generic def name). Stage 104 在 MIR lower 阶段填充 substs, 修复 data flow。

## 三、决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

### 决策 1: 在 lower_call_expr 中推断 substs

**选择**: 在 `lower_call_expr` 中, 当 func 是 FnDef 且 substs 为空时, 从 arg types 推断 substs。

**替代方案 (拒绝)**:
- ❌ 在 writeback 中推断 — writeback 不访问 HIR, 无法获取 generic_params
- ❌ 在 codegen 中推断 — 违反 §16 (codegen 不访问 HIR)
- ❌ 修改 lower_path_generic_args — 该函数只看 path turbofish, 不应有 arg context

**理由** (§1.0 原则 6 通解>特解, §12 最优>最小):
- lower_call_expr 有 arg context + fn_sigs + HIR 访问 — 正确位置
- 复用现有 collect_param_bindings (DRY)
- 与 struct literal field type inference (Stage 18.376) 一致

### 决策 2: 复用 collect_param_bindings

**选择**: 复用 `collect_param_bindings` (expr_operand.rs:1962)。

**理由** (§1.0 原则 6 通解>特解):
- collect_param_bindings 已处理 Param/Adt/RawPtr/Ref/Slice/Array/Tuple 递归
- Stage 18.376 为 struct literal field type inference 创建, 现复用于 fn call arg inference
- 一个函数处理所有 type inference 场景

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 104.1 | 5W2H 根因分析 + 设计修复方案 | 方案明确 |
| 104.2 | 在 lower_call_expr 中添加 FnDef substs 推断 | 编译通过 |
| 104.3 | 添加 stage104 测试 (1:3+ 正负比例) | cargo test 全绿 |
| 104.4 | 加 Debug impl 验证修复效果 (100 次跑 0 SIGSEGV) | 完全修复 |
| 104.5 | §3.2 验收 + 文档同步 + 打包 | 完整交付 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- [ ] `cargo test --release --features llvm-backend --lib` ✓ (898+ tests, 0 failures)
- [ ] `cargo test --release --features llvm-backend --test all_tests` ✓ (5620+ tests, 0 failures, 9 ignored)
- [ ] 加 Debug impl 后 cargo test 100 次跑 0 SIGSEGV
