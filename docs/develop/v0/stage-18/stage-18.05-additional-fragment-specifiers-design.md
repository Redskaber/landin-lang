# Stage 18.05 — macro_rules! Phase 5: Additional Fragment Specifiers

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.291.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

扩展 Stage 18.03 的 fragment 支持范围。Stage 18.03 仅支持 3 个
fragment（`expr`/`ident`/`tt`），Stage 18.05 新增 4 个常用 fragment，
使宏系统的实用性接近 Rust 标准子集。

| Fragment | 用途 | 现状 |
|----------|------|------|
| `expr` | 表达式 | Stage 18.03 ✅ |
| `ident` | 标识符 | Stage 18.03 ✅ |
| `tt` | 单 token tree | Stage 18.03 ✅ |
| `ty` | 类型 | **Stage 18.05 新增** |
| `literal` | 字面量 (int/float/str/char/bool) | **Stage 18.05 新增** |
| `block` | `{ ... }` 块 | **Stage 18.05 新增** |
| `path` | 路径 (a::b::c) | **Stage 18.05 新增** |

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特例" | 一个 `match frag { ... }` 调度所有 fragment，不分散逻辑 |
| §10 命名 | 新增函数遵循 `capture_<fragment>` 模式 |
| §11 接口隔离 | 所有新增函数为 `macro_expand.rs` 内部 `fn`，不暴露 |
| 单一职责 | 每个 `capture_*` 函数只负责一种 fragment |
| 高内聚低耦合 | fragment 调度集中在 `match_pattern` 一处 |
| 避免死代码 | 所有新增 `capture_*` 都被 `match_pattern` 调用 |

## 3. 实现设计

### 3.1 新增 capture 函数

```rust
/// Capture a type: tokens until top-level `,`, `;`, `)`, `}`, or `=>`.
/// Mirrors `capture_expr` but for type position.
fn capture_ty(input: &[Token], idx: &mut usize) -> Vec<Token>;

/// Capture a literal: a single IntLit/FloatLit/StrLit/CharLit/True/False token.
fn capture_literal(input: &[Token], idx: &mut usize) -> Vec<Token>;

/// Capture a block: a balanced `{ ... }` (delimiters included).
fn capture_block(input: &[Token], idx: &mut usize) -> Vec<Token>;

/// Capture a path: `a`, `a::b`, `a::b::c`, ... (segments separated by `::`).
fn capture_path(input: &[Token], idx: &mut usize) -> Vec<Token>;
```

### 3.2 match_pattern 扩展

```rust
let captured = match frag {
    "expr" => capture_expr(input, &mut ii),
    "ident" => capture_ident(input, &mut ii),
    "tt" => capture_tt(input, &mut ii),
    "ty" => capture_ty(input, &mut ii),              // Stage 18.05
    "literal" => capture_literal(input, &mut ii),    // Stage 18.05
    "block" => capture_block(input, &mut ii),        // Stage 18.05
    "path" => capture_path(input, &mut ii),          // Stage 18.05
    _ => return false,
};
```

### 3.3 实现细节

#### capture_ty
- 收集 token 直到遇到 top-level `,` / `;` / `)` / `}` / `=>`
- 跟踪嵌套深度（处理 `<T, U>` 这样的泛型）
- 不消费结束 token（与 capture_expr 一致）

#### capture_literal
- 只看 1 个 token
- 必须是 `IntLit` / `FloatLit` / `StrLit` / `CharLit` / `KwTrue` / `KwFalse`
- 否则返回空 Vec

#### capture_block
- 必须从 `LBrace` 开始
- 收集到匹配的 `RBrace`（包含分隔符本身）
- 否则返回空 Vec

#### capture_path
- 第一个 token 必须是 `Ident` / `RawIdent` / path keyword (`self`/`Self`/`crate`/`super`)
- 后续可选 `:: Ident` 重复
- 收集所有匹配的 token

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | macro_with_ty_fragment | `macro_rules! m { ($t:ty) => { let x: $t; } }` 解析通过 |
| 2 | positive | macro_with_literal_fragment | `macro_rules! m { ($l:literal) => { $l } }` 解析通过 |
| 3 | negative | capture_ty_simple | capture_ty 收集 `i32` 类型 |
| 4 | negative | capture_literal_int | capture_literal 收集 `42` |
| 5 | negative | capture_block_balanced | capture_block 收集 `{ 1; 2 }` |
| 6 | negative | capture_path_segments | capture_path 收集 `a::b::c` |
| 7 | negative | capture_literal_rejects_ident | capture_literal 拒绝 ident |
| 8 | negative | capture_block_rejects_non_brace | capture_block 拒绝非 `{` token |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 479 lib (471 → 479, +8) + 2537 integration = **3,016** total, 0 failures

## 6. 结论

Stage 18.05 完成 macro_rules! Phase 5：Additional Fragment Specifiers。
fragment 支持从 3 个扩展到 7 个（expr/ident/tt/ty/literal/block/path），
覆盖 Rust 常用 macro_rules! 子集。

下一阶段（Stage 18.06）规划：
- 实现 repetition `$(...)*` / `$(...)+` / `$(...)?`
- 或开始 println! 通解化迁移（Phase 6）
