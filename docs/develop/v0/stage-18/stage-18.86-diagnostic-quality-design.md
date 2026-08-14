# Stage 18.86 — Diagnostic Quality Enhancement (Specific ERROR_PATTERNs)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.353.0 → v0.354.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14
> **Status**: ✅ Complete

## 1. 背景

Stage 18.85 完成了 fuzz 基础设施。审计识别 157 个 conformance 测试使用
泛化 `ERROR_PATTERN: error` — 这些测试无法检测诊断回归 (任何错误都通过)。

本 Stage 将这些泛化模式替换为具体错误模式，提升诊断质量检测能力。

## 2. 修复方案

### 2.1 策略

对每个 `ERROR_PATTERN: error` 的测试：
1. 编译测试源码，捕获 stderr 输出
2. 从 stderr 提取实际错误消息
3. 选择最具体的错误子串作为 ERROR_PATTERN
4. 如果无法确定具体模式，保留泛化模式 (保守，不破坏测试)

### 2.2 常见错误模式映射

| 测试内容 | 期望 ERROR_PATTERN |
|---------|-------------------|
| `let x: i32 = true;` | `mismatched types` |
| `let r1 = &mut x; let r2 = &mut x;` | `cannot borrow` |
| `undefined_fn()` | `cannot find` |
| `let x = 1; x = 2;` | `cannot assign` |
| `S { x: 1 }` (missing field) | `missing field` |
| `S { x: 1, y: 2 }` (extra field) | `no field` |
| `fn f() -> i32 { true }` | `mismatched types` |
| `return 42` in void fn | `mismatched types` |
| `42 = 99;` | `invalid assignment` |
| `42 as bool` | (depends) |

## 3. §6.3 委员会投票

**5/5 GO** ✅
