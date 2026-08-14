# Stage 18.75 — P0 Error System Fixes

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.342.0 → v0.343.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.74 深度审计识别了 6 项 P0 正确性缺陷。本 Stage 修复其中 5 项；
Param unify 不安全项经重新评估为 Stage 0 已知限制 (需 v0.2 单态化)，
降级为 P1 延后处理。

## 2. P0 修复项

| P0 # | 描述 | 修复方案 |
|------|------|---------|
| P0-1 | CompileErrors 缺 lower/codegen 字段 | 添加字段 + 更新 is_empty/total_count/to_diagnostics |
| P0-2 | to_diagnostics 不迭代 macro_errors | 添加 macro_errors 迭代循环 |
| P0-3 | ErrorCode 缺 Codegen/Macro | 添加 ErrorCode::Codegen (E700) + ErrorCode::Macro (E800) |
| P0-4 | 30+ CString::new().unwrap() | 替换为 cstr() 缓存辅助函数 |
| P0-5 | BinaryOp2 静默返回 "0" | 推送 typeck error 而非静默编译 |

### 2.1 P0-6 重新评估: Param unify

审计指出 `Param` 与任何类型 unify 是不安全的。但代码注释明确说明这是
Stage 0 的有意设计 — 在没有单态化的情况下，Param 需要作为"任意类型"
处理才能让泛型代码编译通过。修复此问题需要 v0.2 的完整单态化基础设施，
超出 Stage 18.75 范围。**降级为 P1，记录在 Stage 18.76 中。**

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | 错误显式收集到 CompileErrors，不静默丢弃 |
| 4 报错 > 静默 | codegen/macro 错误必须到达用户 |
| 6 通用 > 特例 | 统一的 to_diagnostics 覆盖所有错误类型 |
| 9 正确 > 妥协 | CString unwrap 替换为安全路径 |

### 3.2 P0-1: CompileErrors 添加 lower + codegen 字段

**File**: `src/driver.rs`

```rust
pub struct CompileErrors {
    pub lex: Vec<LexError>,
    pub parse: Vec<ParseError>,
    pub lower: Vec<LowerError>,      // NEW
    pub resolve: Vec<ResolveError>,
    pub typeck: Vec<TypeError>,
    pub borrowck: Vec<BorrowError>,
    pub trait_errors: Vec<TraitError>,
    pub macro_errors: Vec<MacroError>,
    pub codegen: Vec<CodegenError>,  // NEW
}
```

更新: `is_empty()`, `total_count()`, `has_fatal()`, `to_diagnostics_with_resolver()`

### 3.3 P0-2: to_diagnostics 迭代 macro_errors

**File**: `src/driver.rs`

在 `to_diagnostics_with_resolver` 中 trait_errors 循环后添加:
```rust
for e in &self.macro_errors {
    diags.push(
        DiagnosticBuilder::error(&e.message, e.span)
            .with_code(ErrorCode::Macro.to_string())
            .build(),
    );
}
// NEW: codegen errors
for e in &self.codegen {
    diags.push(
        DiagnosticBuilder::error(&e.message, e.span)
            .with_code(ErrorCode::Codegen.to_string())
            .build(),
    );
}
// NEW: lower errors
for e in &self.lower {
    diags.push(
        DiagnosticBuilder::error(&e.message, e.span)
            .with_code(ErrorCode::Lower.to_string())
            .build(),
    );
}
```

### 3.4 P0-3: ErrorCode 添加 Codegen + Macro

**File**: `src/diagnostics/mod.rs`

```rust
pub enum ErrorCode {
    Lex,        // E001
    Parse,      // E100
    Lower,      // E200
    Resolve,    // E300
    Type,       // E400
    Borrow,     // E500
    Trait,      // E600
    Codegen,    // E700  NEW
    Macro,      // E800  NEW
    Internal,   // E900
}
```

### 3.5 P0-4: CString::new().unwrap() 替换

**File**: `src/codegen/llvm/*.rs`

将所有 `CString::new("literal").unwrap()` 替换为 `cstr("literal")`
(已有的缓存辅助函数，返回 `*const c_char`)。

对于需要 owned `CString` 的场景 (如 `LLVMBuildRet` 需要传递 `*const c_char`)，
使用 `cstr()` 即可 (LLVM C API 接受 `*const c_char`)。

### 3.6 P0-5: BinaryOp2 静默 "0" 修复

**File**: `src/codegen/rvalue.rs`

将 `Rvalue::BinaryOp2(_, _, _) => "0".to_string()` 替换为推送 typeck error。

## 4. 测试

- 新增 5 个 Rust 单元测试验证每个 P0 修复
- 全量 conformance 测试验证无回归

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P0 正确性缺陷修复 |
| REV-A | GO | 静默错误丢失是最严重问题 |
| DEV-A | GO | 实现简洁 |
| QA-A | GO | 5 新测试 + 全量回归 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
