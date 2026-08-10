# Stage 18.01 — v0.6 Roadmap + macro_rules! System Phase 1 (Design)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.288.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

1. 创建 v0.6 roadmap
2. 设计 macro_rules! 系统 Phase 1: macro 定义解析

## 2. v0.6 Roadmap

详见 `docs/develop/v0/v0.6-roadmap.md`。

核心任务：
- P1: macro_rules! 系统 (6-8 stages) — println! 通解化前置条件
- P2: GATs (4-6 stages)
- P2: Incremental Compilation (4-6 stages)
- P3: Cross-compilation (2-3 stages)

## 3. macro_rules! Phase 1 设计

### 3.1 目标

解析 `macro_rules! name { ... }` 语法，将 macro 定义存储在 HIR 中。

### 3.2 语法

```landin
macro_rules! say_hello {
    () => { println!("hello"); };
    ($name:expr) => { println!("hello, {}", $name); };
}
```

### 3.3 AST 结构

```rust
// src/ast/kinds.rs
pub enum Item {
    // ... existing variants ...
    /// Stage 18: macro_rules! definition
    MacroRules {
        name: Symbol,
        rules: Vec<MacroRule>,
        span: Span,
    },
}

pub struct MacroRule {
    pub pattern: TokenTree,
    pub body: TokenTree,
    pub span: Span,
}
```

### 3.4 实现范围

Phase 1 仅解析语法，不实现展开。展开在 Phase 2。

## 4. 验收

- v0.6-roadmap.md 创建
- 本设计文档创建
- 全量测试通过

## 5. 结论

v0.6 roadmap 规划完成。macro_rules! Phase 1 设计完成，下一 stage 实现。
