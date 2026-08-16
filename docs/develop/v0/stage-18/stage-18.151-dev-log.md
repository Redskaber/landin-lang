# Stage 18.151 — TD-CODEGEN-RESULT 完整修复 (root-cause)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.419.0 (Stage 18.151 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §12 (最优>最小) + §2.2 原则 4 (报错>静默) + §2.2 原则 9 (正确>妥协)
> **Complexity**: L3 (codegen Result 传播 — 全链路签名变更)
> **Task ID**: stage18.151

## 1. 阶段目标

按 Stage 18.150 完整计划推进 TD-CODEGEN-RESULT 根因修复，同时关闭 3 项关联技术债：

| 技术债 | 原状态 | 本 Stage 修复 |
|--------|--------|---------------|
| TD-CODEGEN-RESULT | Open — v0.2 P2 | ✅ Resolved — 全链路 `CodegenResult<T>` 传播 |
| TD-BINARYOP2-PANIC | Open — v0.2 P2 (依赖 CODEGEN-RESULT) | ✅ Resolved — `panic!()` → `Err(CodegenError)` |
| TD-UNWRAP-CODEGEN-LLVM-MOD | Open — v0.2 P2 (依赖 CODEGEN-RESULT) | ✅ Resolved — `strip_prefix('@').unwrap()` → `if let Some` |

## 2. 设计原则

### 2.1 通解 > 特解 (§1.0 原則 6)

**特解** (rejected): 在 `codegen_function` 顶层 unwrap 内部 Result，pipeline 不变。
- 问题: 违反 §2 原则 9 (正确>妥协)，错误信息丢失，BinaryOp2 仍 panic。

**通解** (adopted): 全链路 `CodegenResult<T>` 传播，从 `codegen_rvalue` 到 `codegen_crate`。
- 优点: 错误信息完整传播到 driver，用户看到清晰诊断；BinaryOp2 panic 消除。

### 2.2 最优 > 最小 (§12)

Stage 18.150 计划 3-phase 拆分 (terminator/function → pipeline/mod/cargo → rvalue/statement)。
**问题**: 3-phase 计划不可编译 — Phase 1 修改 `codegen_function` 签名后，pipeline.rs 的调用必须同步修改，否则编译失败。

**纠正**: 单 Stage 完成全链路修改 (6 source files + 2 caller files + 30 test files + 25 example files = 63 files)，避免中间不可编译状态。

### 2.3 高内聚低耦合 (§13.4 J2)

- `codegen_rvalue` (rvalue.rs) — 单一职责: rvalue → EmitValue，新增 error 路径
- `codegen_statement` (statement.rs) — 单一职责: statement → IR，仅传播 `?`
- `codegen_terminator` (terminator.rs) — 单一职责: terminator → IR，仅传播 `?`
- `codegen_function` (function.rs) — 单一职责: function body → IR，仅传播 `?`
- `run_codegen_pipeline` (pipeline.rs) — 单一职责: pipeline 编排，仅传播 `?`
- `codegen_crate*` (mod.rs) — 公共入口，返回 `CodegenResult<T>`

每个函数仅添加 `?` 传播和 `Ok(...)` 包装，职责不变。

## 3. 修改清单

### 3.1 核心源文件 (6 files)

| 文件 | 函数 | 修改前 | 修改后 |
|------|------|--------|--------|
| `rvalue.rs` | `codegen_rvalue` | `-> EmitValue` | `-> CodegenResult<EmitValue>` |
| `rvalue.rs` | BinaryOp2 arm | `panic!(...)` | `return Err(CodegenError::new(...))` |
| `rvalue.rs` | 7 处 early return | `return X;` | `return Ok(X);` |
| `rvalue.rs` | match 末尾 | `match rv { ... }` | `Ok(match rv { ... })` |
| `statement.rs` | `codegen_statement` | `-> ()` | `-> CodegenResult<()>` |
| `statement.rs` | `emit_printf_call` | `-> ()` | `-> CodegenResult<()>` |
| `statement.rs` | codegen_rvalue call | (无 `?`) | 添加 `?` |
| `statement.rs` | 函数末尾 | `}` | `Ok(()) }` |
| `terminator.rs` | `codegen_terminator` | `-> ()` | `-> CodegenResult<()>` |
| `terminator.rs` | `codegen_print_call` | `-> ()` | `-> CodegenResult<()>` |
| `terminator.rs` | 2 处 `return;` | `return;` | `return Ok(());` |
| `terminator.rs` | `_ => return,` | `_ => return,` | `_ => return Ok(()),` |
| `terminator.rs` | codegen_print_call call | (无 `?`) | 添加 `?` |
| `terminator.rs` | 函数末尾 | `}` | `Ok(()) }` |
| `function.rs` | `codegen_function` | `-> ()` | `-> CodegenResult<()>` |
| `function.rs` | `codegen_from_mir` | `-> ()` | `-> CodegenResult<()>` |
| `function.rs` | `codegen_synthesized_closure_functions` | `-> ()` | `-> CodegenResult<()>` |
| `function.rs` | `codegen_mono_functions` | `-> ()` | `-> CodegenResult<()>` |
| `function.rs` | 4 处 codegen_function call | (无 `?`) | 添加 `?` |
| `function.rs` | codegen_statement/terminator call | (无 `?`) | 添加 `?` |
| `function.rs` | 函数末尾 | `}` | `Ok(()) }` |
| `pipeline.rs` | `run_codegen_pipeline` | `-> ()` | `-> CodegenResult<()>` |
| `pipeline.rs` | 3 处 codegen_from_mir/mono/synth call | (无 `?`) | 添加 `?` |
| `pipeline.rs` | 函数末尾 | `}` | `Ok(()) }` |
| `mod.rs` | `codegen_crate` | `-> String` | `-> CodegenResult<String>` |
| `mod.rs` | `codegen_crate_with_target` | `-> String` | `-> CodegenResult<String>` |
| `mod.rs` | `codegen_crate_to_module` | `-> LLVMSysEmitter` | `-> CodegenResult<LLVMSysEmitter>` |
| `mod.rs` | `codegen_crate_to_module_with_target` | `-> LLVMSysEmitter` | `-> CodegenResult<LLVMSysEmitter>` |

### 3.2 调用方 (2 files)

| 文件 | 修改 |
|------|------|
| `src/cargo.rs` | `Some(codegen_crate(&result))` → `match codegen_crate(&result) { Ok(ir) => Some(ir), Err(e) => { errors.push(...); None } }` |
| `src/bin/main.rs` | 2 处 codegen 调用改为 `match ... { Ok(em) => em, Err(e) => { eprintln!(...); std::process::exit(1); } }` |

### 3.3 测试 + 示例 (31 files, 55 call sites)

使用脚本 `scripts/stage18_151_update_test_codegen_calls.py` 批量替换：
- `codegen_crate(&result)` → `codegen_crate(&result).expect("codegen should succeed for valid test input")`
- `codegen_crate_to_module(&result)` → 同上

测试使用 `.expect()` 因为：
- 测试控制输入，valid input 不应 codegen 失败
- 若 codegen 失败，是 bug 值得 panic
- 符合 §9.4 测试标准 (test setup 可用 `.expect()`)

### 3.4 TD-UNWRAP-CODEGEN-LLVM-MOD 修复

`src/codegen/llvm/mod.rs:378`:
```rust
// Before:
let func_name: String = name.strip_prefix('@').unwrap().to_string();

// After:
if let Some(stripped) = name.strip_prefix('@') {
    let func_name: String = stripped.to_string();
    // ...
}
```

**说明**: 原 `unwrap()` 实际安全（guarded by `name.starts_with('@')` 检查），但 per §1.0 原則 5 (去除兼容思维) 和 §2 原则 9 (正确>妥协)，消除 codegen 中所有 `unwrap()`。

## 4. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 必要性 | BinaryOp2 panic 是用户可见 bug | ✅ 通过 |
| J2 单一职责 | 每个函数仅添加 `?` 传播，职责不变 | ✅ 通过 |
| J3 接口稳定 | 公共 API `codegen_crate` 签名变更 (返回 CodegenResult)，所有调用方已同步更新 | ✅ 通过 |
| J4 测试覆盖 | 全部 3146 测试通过，无回归 | ✅ 通过 |
| J5 文档同步 | dev-log + tech-debt-register + worklog 同步 | ✅ 通过 |
| J6 粒度 | codegen pipeline 整体修改，无法进一步拆分 | ✅ 通过 |

## 5. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 622 passed, 0 failed, 0 ignored
- ✅ cargo test --tests (default): 2524 passed, 0 failed, 2 ignored
- ✅ cargo test --tests --all-features: 2663 passed, 0 failed, 2 ignored
- ✅ 0 TODO/FIXME/HACK in codegen/

## 6. 关闭的技术债

| TD | 原状态 | 新状态 |
|----|--------|--------|
| TD-CODEGEN-RESULT | Open — v0.2 P2 | ✅ Resolved Stage 18.151 |
| TD-BINARYOP2-PANIC | Open — v0.2 P2 | ✅ Resolved Stage 18.151 |
| TD-UNWRAP-CODEGEN-LLVM-MOD | Open — v0.2 P2 | ✅ Resolved Stage 18.151 |

**累计成果**: 25 个 stage (18.125-18.151)，技术债从 22 项降至 18 项 (4 closed in 18.151 + 之前 18 项已关闭)。

## 7. 设计与开发原则记录

### 7.1 简写和缺陷记录

**无简写和缺陷**。本次修改完全遵循原则：
- 通解 > 特解: 全链路 Result 传播 (而非局部 unwrap)
- 高内聚低耦合: 每个函数仅添加 `?` 传播
- 单一职责: 函数职责不变
- 避免死代码: 无未使用函数或类型
- 避免分散内容: 修改集中在 codegen/ 模块

### 7.2 API 命名标准化 (§10)

- `CodegenResult<T>`: 已存在 (Stage 17.01)，本次无新增类型
- `CodegenError::new(message, span)`: 已存在，本次 BinaryOp2 arm 使用 `Span::DUMMY` (Rvalue 无 span 信息)
- 函数命名: 全部保留原名，仅修改返回类型

### 7.3 接口设计 (§11)

- 公共 API (`codegen_crate*`) 返回 `CodegenResult<T>` — 调用方必须处理错误
- 内部 API (`codegen_rvalue` 等) 返回 `CodegenResult<T>` — 内部 `?` 传播
- `cargo.rs` 错误处理: codegen error → `errors.push(format!("codegen error: {}", e))` — 用户可见
- `main.rs` 错误处理: codegen error → `eprintln!` + `std::process::exit(1)` — 用户可见

## 8. Stage Summary

- **Stage 18.151 PASSED** — TD-CODEGEN-RESULT 完整修复 (root-cause)
- **关闭技术债**: TD-CODEGEN-RESULT, TD-BINARYOP2-PANIC, TD-UNWRAP-CODEGEN-LLVM-MOD (3 项)
- **修改范围**: 6 source + 2 caller + 31 test/example = 39 files
- **测试**: 622 lib + 2524/2663 integration, 0 failures
- **v0.419.0**: patch bump (root-cause codegen Result 修复)
- **下一步**: v0.2 P0 mini-cargo 项目系统 (TD-SINGLE-FILE)
