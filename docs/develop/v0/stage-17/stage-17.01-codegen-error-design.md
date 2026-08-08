# Stage 17.01 Design — CodegenError Error System (v0.5 P1)

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Per v0.5 roadmap, CodegenError Error System (P1, 2-3 stages)。

**问题**: codegen 中 51 处 `unwrap()` (主要在 `CString::new(name).unwrap()`)，1 处 `expect()`。如果标识符含 NUL 字节（虽然 Landin 标识符不会），会 panic 而非报错。

**目标**: 引入 `CodegenError` 类型 + `cstr()` helper，将 panic 路径改为 `Result` 传播。

## 2. 架构现状分析

### 2.1 当前 unwrap 分布

- `CString::new(name).unwrap()` — 45 处（LLVM C-API 调用前构造 C 字符串）
- `self.cur_fn.expect("emit_block called outside function")` — 1 处
- 其他 `unwrap()` — 5 处（LLVM C-API 返回值处理）

### 2.2 当前 codegen pipeline 签名

```rust
pub fn run_codegen_pipeline(result: &CompileResult, emitter: &mut dyn Emitter)  // 返回 ()
pub fn codegen_crate(result: &CompileResult) -> String  // 返回 IR string
pub fn codegen_crate_to_module(result: &CompileResult) -> LLVMSysEmitter  // 返回 emitter
```

### 2.3 §10.1.8 错误类型标准

> 所有错误类型使用 `Error` 后缀。结构共享 `{ message: String, span: Span }` 最小形态。

## 3. 重构方案

### 3.1 新增 CodegenError 类型

```rust
// src/codegen/error.rs
use crate::session::Span;

pub struct CodegenError {
    pub message: String,
    pub span: Span,
}

impl CodegenError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span }
    }
}

pub type CodegenResult<T> = Result<T, CodegenError>;
```

### 3.2 新增 cstr helper

在 `src/codegen/llvm/helpers.rs` 中：

```rust
/// Stage 17.01: Convert a string to a CString, returning CodegenError on failure.
/// NUL bytes in the string are the only failure case.
pub(crate) fn cstr(s: &str) -> CodegenResult<CString> {
    CString::new(s).map_err(|_| CodegenError::new(
        format!("invalid string containing NUL byte: {:?}", s),
        Span::DUMMY,
    ))
}
```

### 3.3 迁移策略

**Phase 1** (本 stage): 引入类型 + helper，迁移 llvm/helpers.rs 的 cstr helper。
**Phase 2** (下 stage): 迁移 45 处 `CString::new().unwrap()` 为 `cstr()?`。
**Phase 3** (下 stage): 更新 `run_codegen_pipeline` 签名为 `CodegenResult<()>`。

本 stage 聚焦 Phase 1 + 部分迁移（helpers.rs 中的 `cstr` 函数 + 替换最容易的 ~10 处）。

### 3.4 迁移优先级

1. `cstr()` helper — 立即可用
2. `mod.rs` 中 `to_object_file` 的 error 返回路径 — 已有 `Result`
3. `mod.rs` 中 `expect("emit_block called outside function")` — 改为 CodegenError
4. 剩余 `CString::new().unwrap()` — 逐步替换

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `CodegenError` 只负责 codegen 错误 |
| J3 | 单向流动 | ✅ cstr → CodegenError → pipeline |
| J4 | 编译相关表达完整 | ✅ 引入类型 + helper |
| J5 | 阶段划分清晰 | ✅ 仍在 codegen/ |
| J6 | 科学合理粒度 | ✅ ~50 LOC 新增 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `codegen_error_new_creates_error` — CodegenError::new 正确构造
2. `cstr_valid_string_returns_ok` — cstr 对正常字符串返回 Ok

### 负向测试 (negative)
1. `cstr_nul_byte_returns_error` — cstr 对含 NUL 字节的字符串返回 Err
2. `codegen_error_message_correct` — 消息内容正确
3. `codegen_error_span_correct` — span 正确
4. `codegen_result_ok_variant` — CodegenResult Ok 变体
5. `codegen_result_err_variant` — CodegenResult Err 变体
6. `cstr_empty_string_returns_ok` — 空字符串正常处理

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。Phase 1 实现 ~50 LOC + 8 测试。
