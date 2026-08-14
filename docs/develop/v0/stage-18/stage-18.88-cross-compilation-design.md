# Stage 18.88 — Cross-Compilation Foundation (Target Triple Configuration)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.355.0 → v0.356.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14
> **Status**: ✅ Complete

## 1. 背景

v0.7 路线图 P2 交叉编译 (2-3 stages)。当前 target triple 硬编码为
`x86_64-unknown-linux-gnu`，不支持其他平台。

## 2. 修复方案

### 2.1 TargetTriple 类型

新增 `src/codegen/target.rs`:
```rust
pub struct TargetTriple {
    triple: String,
    data_layout: String,
}

impl TargetTriple {
    pub fn x86_64_linux() -> Self { ... }
    pub fn aarch64_linux() -> Self { ... }
    pub fn from_str(triple: &str) -> Self { ... }
    pub fn triple(&self) -> &str { ... }
    pub fn data_layout(&self) -> &str { ... }
}
```

### 2.2 Emitter 接受 TargetTriple

- `TextEmitter::new()` → `TextEmitter::with_target(target)`
- `LLVMSysEmitter::new()` → `LLVMSysEmitter::with_target(target)`
- `emit_header` 使用 `self.target` 而非硬编码字符串

### 2.3 CLI 支持

- `--target <triple>` CLI 参数
- 默认: `x86_64-unknown-linux-gnu`

## 3. §6.3 委员会投票

**5/5 GO** ✅
