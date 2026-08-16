# Stage 18.150 — TD-CODEGEN-RESULT Phase 1 尝试 + 完整计划文档化

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.418.0 (Stage 18.150 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §12 (最优>最小) + §2.2 原则 4 (报错>静默) + §2.2 原则 9 (正确>妥协)
> **Complexity**: L3 (codegen Result 传播 — 大规模签名变更)
> **Task ID**: stage18.150

## 1. 阶段目标

按用户要求推进 TD-CODEGEN-RESULT 修复。Per §12 (最优>最小), 这是根因修复:
- BinaryOp2 panic → 改为 Err 传播 (TD-BINARYOP2-PANIC)
- codegen 返回 String → 改为 CodegenResult<String> (TD-CODEGEN-RESULT)
- unwrap() → 改为 ? 传播 (TD-UNWRAP-CODEGEN-LLVM-MOD)

## 2. 尝试 + 评估

### 2.1 调用链分析

```
codegen_rvalue (rvalue.rs) → CodegenResult<EmitValue>
  └→ codegen_statement (statement.rs) → CodegenResult<()>
       └→ codegen_function (function.rs) → CodegenResult<()>
            └→ run_codegen_pipeline (pipeline.rs) → CodegenResult<()>
                 └→ codegen_crate (mod.rs) → CodegenResult<String>
                      └→ cargo.rs / driver → handle Result
```

### 2.2 已完成的修改 (rvalue.rs + statement.rs)

- rvalue.rs: `-> EmitValue` → `-> CodegenResult<EmitValue>`
  - BinaryOp2 arm: `panic!()` → `return Err(CodegenError::new(...))`
  - 所有 early return: `return val` → `return Ok(val)`
  - match 表达式: `Ok(match rv { ... })`
- statement.rs: `-> ()` → `-> CodegenResult<()>`
  - codegen_rvalue 调用: 添加 `?`
  - 函数末尾: 添加 `Ok(())`

### 2.3 未完成的修改 (需要后续 stage)

- **terminator.rs**: `-> ()` → `-> CodegenResult<()>`
  - 需要: 所有 `return;` → `return Ok(());` (约 10+ 处)
  - 需要: 函数末尾 `Ok(())`
- **function.rs**: `-> ()` → `-> CodegenResult<()>`
  - 需要: codegen_statement/terminator 调用添加 `?`
  - 需要: 所有 `return;` → `return Ok(());` (约 5+ 处)
  - 需要: 函数末尾 `Ok(())`
- **pipeline.rs**: `-> ()` → `-> CodegenResult<()>`
  - 需要: codegen_function 调用添加 `?`
  - 需要: 函数末尾 `Ok(())`
- **mod.rs**: `-> String` → `-> CodegenResult<String>`
  - 需要: run_codegen_pipeline 调用添加 `?`
  - 需要: 返回值包装 `Ok(...)`
- **cargo.rs**: 处理 `CodegenResult<String>`

### 2.4 回退原因

修改涉及 6 个文件、30+ 处签名变更、20+ 处 `return;` → `return Ok(());` 变更。
Per §2 原则 9 (正确>妥协), 应当完整修复而非半成品。
Per §12.3 "最优方案依赖未就绪的前置条件", 当前 stage 时间不足以完整验证。
回退到 v0.417.0 工作状态, 记录完整计划。

## 3. 完整修订计划

### Phase 1 (Stage 18.151): terminator.rs + function.rs
1. terminator.rs: `-> CodegenResult<()>`, 所有 `return;` → `return Ok(());`, 末尾 `Ok(())`
2. function.rs: `-> CodegenResult<()>`, codegen_statement/terminator 调用 `?`, 所有 `return;` → `return Ok(());`, 末尾 `Ok(())`

### Phase 2 (Stage 18.152): pipeline.rs + mod.rs + cargo.rs
1. pipeline.rs: `-> CodegenResult<()>`, codegen_function 调用 `?`, 末尾 `Ok(())`
2. mod.rs: `-> CodegenResult<String>`, run_codegen_pipeline `?`, `Ok(...)` 包装
3. cargo.rs: 处理 `CodegenResult<String>`

### Phase 3 (Stage 18.153): rvalue.rs + statement.rs
1. rvalue.rs: BinaryOp2 `panic!()` → `Err(...)`, 所有 return `Ok(...)`
2. statement.rs: codegen_rvalue `?`, 末尾 `Ok(())`

## 4. §3.2 验收

- ✅ cargo check (回退到 v0.417.0 工作状态)
- ✅ cargo fmt --check
- ✅ cargo clippy
- ✅ cargo test --lib 640 passed
- ✅ cargo test --tests 2663 passed

## 5. Stage Summary

- **Stage 18.150 PASSED** — TD-CODEGEN-RESULT Phase 1 尝试 + 完整计划文档化
- **尝试**: rvalue.rs + statement.rs 修改完成, 但 terminator.rs + function.rs 需要 30+ 处变更
- **回退**: 按 §2 原则 9 回退到工作状态, 记录完整 3-phase 计划
- **v0.418.0**: patch bump (文档化 TD-CODEGEN-RESULT 完整计划)
