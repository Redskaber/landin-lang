# Stage 99 开发日志 — TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因分析

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.637.0 (无代码变更) |
| 测试数 | 5585 (898 lib + 4687 integration + 5 stage99 repro) |
| 失败数 | 0 → 0 (基线全绿) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +1 文档 (根因分析) + 5 repro tests |

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/stdlib/prelude.rs` | 无变更 (恢复到 Stage 98 状态) |
| `tests/v0/stage99/plan/prelude_impl_body_repro_tests.rs` | 新建 — 5 个测试验证 user code impl method working |
| `tests/all_tests.rs` | 注册 stage99_prelude_impl_body_repro_tests |
| `docs/develop/v0/stage-99/plan-99.md` | 新建 — 5W2H 根因分析计划 |
| `docs/develop/v0/stage-99/dev-log.md` | 本文件 — 根因分析结果 |
| `docs/tests/v0/stage-99/plan/prelude-impl-body-repro-tests.md` | 新建 — 测试计划 |
| `docs/develop/v0/tech-debt-register.md` | 升级 TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因描述 |

## 5W2H 根因分析

### WHAT (现象)
当 prelude 中加入 `impl Debug for i32 { fn fmt(&self) -> String { String::from_str("debug_i32") } }` 后, cargo test 中约 8-20 个 integration test 非确定性地 SIGSEGV/SIGABRT (signal 11/6)。

### WHY (根因)
**根因链** (Layer 1 → Layer 4):

**Layer 1: prelude generic methods 的 Param type 未解析**

prelude 中 generic methods (`impl<T> Option<T> { fn map }`, `impl<T> Box<T> { fn new }`, `impl<T> Vec<T> { fn push }` 等) 在 codegen 阶段被作为"未实例化"的 generic function emit。它们的 MIR 中包含 `TyKind::Param(ParamTy { index: 0, name: Spur(N) })` 类型，code 时无法解析。

**Layer 2: mir_type_to_emit_type 对 Param type fallback 到 i32**

`src/codegen/emitter/mod.rs:392` 中:
```rust
TyKind::Adt(_) | TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) | ... => {
    eprintln!("warning: unresolved type kind ... falling back to i32 ...");
    EmitType::I32
}
```

这是 defense-in-depth warning, 但实际产生了**不正确的 LLVM IR** — 例如 GEP 用 i32 替代 `{ ptr, i64, i64 }` (String, 24 bytes)。

**Layer 3: 加 Debug impl 触发 LLVM module 全局状态累积**

加 `impl Debug for i32` 后，每次 `compile()` 调用都为 prelude 中所有 items 生成 LLVM IR，包括:
- `@.vtable.Debug.i32` 全局 (引用 `@landin_Debug_i32_fmt`)
- `@.dynptr.Debug.i32` 全局 (引用 `@.data.i32` + `@.vtable.Debug.i32`)
- `landin_Debug_i32_fmt` 函数定义 (调用 `landin_String_from_str`)

LLVM module 中的全局变量数量增加，LLVM 内部的 type table + global symbol table 累积。某些 .lin 文件 codegen 时, LLVM 在 verify 或 emit 阶段触发 SIGSEGV/SIGABRT。

**Layer 4: cargo test 子进程的 LLVM context 不释放**

`src/codegen/llvm/mod.rs:797` 中 `Drop for LLVMSysEmitter` 故意不释放 LLVMContext + Module (注释: "caller may still want to extract the module")。这导致 cargo test 进程中累积多个 LLVMContext, 加速 LLVM 全局状态累积, 触发 crash。

### WHO (影响)
- 影响: 所有 prelude trait impl 的 codegen 稳定性
- 阻断: 无法添加 Debug/PartialOrd impls (Stage 97 移除)

### WHEN (触发条件)
1. prelude 中存在 trait impl method body
2. body 调用 prelude 中 generic methods (String::from_str, Option::map, Box::new, Vec::push 等)
3. cargo test 进程中累积足够多次 `compile()` 调用
4. LLVM module 全局变量数量超过某个阈值

### WHERE (代码位置)
- `src/codegen/emitter/mod.rs:275-403` — `mir_type_to_emit_type` Param/Never fallback 到 i32
- `src/codegen/mir_translation/types.rs:255` — `_with_layouts` variant fallback 到 bare variant
- `src/codegen/llvm/mod.rs:797-811` — `Drop for LLVMSysEmitter` 不释放 context
- `src/stdlib/prelude.rs` — generic methods (Option/Box/Vec/String) 定义

### HOW (复现步骤)
1. 在 prelude.rs Debug trait 后加 `impl Debug for i32 { fn fmt(&self) -> String { String::from_str("debug_i32") } }`
2. `cargo build --release --features llvm-backend --bin landin-stage0`
3. `RUST_TEST_THREADS=1 cargo test --release --features llvm-backend --test all_tests`
4. 观察: 8-20 个 integration test SIGSEGV/SIGABRT, 失败非确定

### HOW MUCH (影响范围)
- v0.637.0 基线 0 failures (prelude Debug impl body 未加)
- 加 Debug impl 触发 ~12 个非确定 SIGSEGV
- 阻断 Stage 99 完整推进: Debug + PartialOrd impls 无法重新添加

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协)

### 决策 1: 不在 Stage 99 实施修复，只做根因分析

**选择**: 根因分析 + Stage 100+ 规划，不在 Stage 99 实施代码修复。

**理由** (§1.0 原则 9 正确>妥协):
- 根因涉及 monomorphization pass + LLVM context lifecycle + Param type fallback 多个层面
- 完整修复需要 4-stage 渐进重构 (见"修复路径")
- 单 stage 修复不完整会引入更多 bug
- 用户指示: "遇依赖缺失停止阉割版，转而分析根因"

### 决策 2: 不修改 LLVMSysEmitter::Drop

**选择**: 保持 Drop 不释放 context + module。

**替代方案 (拒绝)**: Drop 中释放 context + module。

**理由**:
- Drop 中释放 context 后, `to_module()` 返回的 raw pointer 变 dangling
- 测试: Drop 释放后 cargo test 失败数从 12 增加到 20 (更糟)
- 正确修复需要重新设计 emitter ownership (Stage 100+ 任务)

### 决策 3: 保留 stage99 repro tests

**选择**: 保留 5 个 stage99 repro tests (验证 user code impl method working)。

**理由** (§1.0 原则 8 设计驱动测试):
- 这些 tests 验证 v0.637.0 中 user code impl method returning String/struct 工作正常
- 为 Stage 100+ 修复提供 baseline 回归测试
- 5 个 tests 全过 (基线全绿)

## 修复路径 (Stage 100+)

### Stage 100: monomorphization pass 跳过 prelude generic function
- 在 `collect_mono_items` 中跳过 prelude items (DefId > user_item_count)
- 只为具体类型实例化的 generic function 生成 LLVM IR
- 预期: 减少 Param warnings 数量 (从 1360 降到 ~100)

### Stage 101: 修复 mir_type_to_emit_type Param fallback
- 把 Param/Infer/Never fallback 从 i32 改为 Error (per §1.0 原则 4 报错>静默)
- 添加 CodegenError 类型, codegen 失败时返回 Err
- 预期: 任何未解析类型显式报错, 不产生错误 LLVM IR

### Stage 102: LLVMSysEmitter ownership 重构
- 拆分 LLVMSysEmitter 为 Builder + Module 两个 owned 类型
- Builder Drop 释放 builder, Module Drop 释放 module + context
- 预期: LLVM context 正确释放, 无资源累积

### Stage 103: 重新添加 Debug + PartialOrd impls
- 完成 Stage 100-102 后, 重新加 Debug::fmt (returning String) + PartialOrd::partial_cmp (returning Option<i32>) impls
- 预期: 全绿, 触发原 crash 的 .lin 文件正常 codegen

## §3.2 验收清单

- [x] `cargo fmt --check` ✓
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [x] `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures)
- [x] `cargo test --release --features llvm-backend --test all_tests` ✓ (4687 tests, 0 failures, 9 ignored)
- [x] `cargo test --release --features llvm-backend --test all_tests stage99` ✓ (5 tests, 0 failures)

## 关键产出

1. **根因分析报告** (本文件) — 完整 5W2H + 4-layer 根因链
2. **Stage 100-103 修复路径规划** — 4-stage 渐进重构计划
3. **5 个 stage99 repro tests** — 验证 user code impl method 工作正常
4. **TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 升级** — 在 tech-debt-register.md 中细化根因描述

## 下一步

- **Stage 100**: monomorphization pass 跳过 prelude generic function (P2 修复)
- **Stage 101-103**: Param fallback 修复 + Emitter ownership 重构 + Debug impls 重新添加
