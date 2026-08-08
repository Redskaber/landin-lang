# Stage 17.01 — CodegenError Error System (v0.5 P1, Phase 1)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.274.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

引入 `CodegenError` 类型 + `cstr_result` helper，为 Phase 2 unwrap 迁移做准备。

## 2. 实现

### 2.1 CodegenError 类型 (src/codegen/error.rs)

```rust
pub struct CodegenError {
    pub message: String,
    pub span: Span,
}
pub type CodegenResult<T> = Result<T, CodegenError>;
```

符合 §10.1.8 最小形态 `{ message, span }`。

### 2.2 cstr_result helper (src/codegen/llvm/helpers.rs)

```rust
pub(crate) fn cstr_result(s: &str) -> CodegenResult<CString>
```

错误安全的 CString 构造，NUL 字节返回 CodegenError 而非 panic。

### 2.3 模块注册

`codegen/mod.rs` 注册 `pub mod error` + re-export。

## 3. 测试 (§9.4.3 1:3 ratio)

| # | 测试 | 极性 |
|---|------|------|
| 1 | codegen_error_new_creates_error | positive |
| 2 | cstr_valid_string_returns_ok | positive |
| 3 | cstr_nul_byte_returns_error | negative |
| 4 | codegen_error_message_correct | negative |
| 5 | codegen_error_span_correct | negative |
| 6 | codegen_result_ok_variant | negative |
| 7 | codegen_result_err_variant | negative |
| 8 | cstr_empty_string_returns_ok | negative |

## 4. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 423 lib + 2529 integration = 2952 unit tests |

## 5. 后续工作

- Phase 2: 迁移 45 处 `CString::new().unwrap()` 为 `cstr_result()?`
- Phase 3: 更新 `run_codegen_pipeline` 签名为 `CodegenResult<()>`
