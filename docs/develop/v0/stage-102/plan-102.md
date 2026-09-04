# Stage 102 开发计划 — LLVMSysEmitter ownership 重构 (Layer 3+4)

> **阶段**: v0.10 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 修复 - Layer 3+4)
> **TD**: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+) — Stage 99 RCA Layer 3+4 修复
> **复杂度**: L2 (聚焦: LLVMSysEmitter::Drop 释放 module + context; 验证 ownership 安全)
> **版本基线**: v0.640.0 (Stage 101 Layer 2 partial, 5599 tests)
> **目标版本**: v0.641.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 1) `LLVMSysEmitter::Drop` 释放 module + context (Layer 4); 2) 验证 `to_module()` 调用者均在 Drop 前使用 (ownership 安全) |
| **WHY** | Layer 3+4 根因: Drop 不释放 module + context → LLVM 资源累积 → cargo test 多次 compile() 后 SIGSEGV/SIGABRT. 释放后消除累积, 与 rustc `LLVMContext` 释放模式一致. |
| **WHO** | ARCH-A 设计; DEV-A 实施; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 102 完成 → 进入 Stage 103 (重新添加 Debug + PartialOrd impls) |
| **WHERE** | `src/codegen/llvm/mod.rs:797-811` (Drop impl); 调用者: `src/bin/main.rs`, `src/bin/landinc.rs`, `src/codegen/llvm/tests.rs` |
| **HOW** | 1) Drop 中 LLVMDisposeModule + LLVMContextDispose; 2) 验证所有 `to_module()` 调用在 Drop 前; 3) 验证 `to_object_file` 在 Drop 前调用 |
| **HOW MUCH** | 1 src 文件 (Drop impl ~10 LOC), 0 测试文件新增 (复用 stage99-101 回归测试) |

## 二、对齐设计文档 (§13.1 / §8.4.5)

### docs/llvm/backend-architecture.md 对齐
LLVM C API ownership: `LLVMModuleCreateWithNameInContext` 创建的 module 由 `LLVMDisposeModule` 释放; `LLVMContextCreate` 创建的 context 由 `LLVMContextDispose` 释放。当前 Landin Drop 只释放 builder, 不释放 module + context → 资源泄漏。

### docs/llvm/execution-pipeline.md 对齐
execution pipeline: codegen_crate_to_module → run_codegen_pipeline → to_object_file → Drop. 当前 Drop 不释放 → 跨 compile() 调用累积。

### Rust 设计对齐
rustc_codegen_llvm: `LLVMContext` 在 `llvm::LLVMContext::new` 中创建, 在 Drop 中释放. 每个 `compile()` 调用创建独立 context, 编译完成后释放. Landin 应一致。

## 三、决策点 (§12 最优>最小, §1.0 原则 1 内存安全决不能妥协)

### 决策 1: Drop 释放 module + context

**选择**: 在 Drop 中添加 `LLVMDisposeModule` + `LLVMContextDispose` (在 builder 之后)。

**替代方案 (拒绝)**:
- ❌ 保持现状 (不释放) — 治症不治根, LLVM 资源累积
- ❌ 拆分 LLVMSysEmitter 为 Builder + Module 两个类型 — 过度设计, 单一 Drop 已足够

**理由** (§1.0 原则 1 内存安全决不能妥协, §12 最优>最小):
- 释放 module + context 是 LLVM C API 的标准 ownership 模式
- 验证所有 `to_module()` 调用均在 Drop 前 (调用者: tests.rs, main.rs, landinc.rs)
- 与 rustc 设计一致

### 决策 2: 不拆分 LLVMSysEmitter 类型

**选择**: 保持单一 LLVMSysEmitter 类型, 仅修改 Drop impl。

**替代方案 (拒绝)**:
- ❌ 拆分为 Builder + Module 两类型 — 复杂度过高, 实际不需要
- ❌ 用 `Rc<LLVMContext>` 共享 context — 不需要, 每 compile() 独立 context

**理由** (§12 最优>最小, §1.0 原则 6 通解>特解):
- 单一 Drop 修复 Layer 4 根因
- 验证调用者安全后即可实施
- 避免 over-engineering

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 102.1 | 验证所有 `to_module()` + `to_object_file()` 调用均在 Drop 前 | 调用者安全 |
| 102.2 | 修改 Drop impl 添加 LLVMDisposeModule + LLVMContextDispose | 编译通过 |
| 102.3 | §3.2 验收 (lib + all_tests 全绿) | fmt/clippy/test 全绿 |
| 102.4 | 验证 LLVM 资源不再累积 (cargo test 多次跑不 crash) | 稳定 |
| 102.5 | 添加 stage102 测试 (LLVMSysEmitter Drop 验证) | 1:3+ 比例 |
| 102.6 | 更新 worklog + tech-debt + calibration-data + matrix + README + RELEASE_NOTES | 文档同步 |
| 102.7 | 打包 v0.641.0 + 更新 web download page | 完整交付 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- [ ] `cargo test --release --features llvm-backend --lib` ✓ (898+ tests, 0 failures)
- [ ] `cargo test --release --features llvm-backend --test all_tests` ✓ (5606+ tests, 0 failures, 9 ignored)
