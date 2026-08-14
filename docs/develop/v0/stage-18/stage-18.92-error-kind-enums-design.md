# Stage 18.92 — Error Type Kind Enums (LexError/ParseError/LowerError/CodegenError/MacroError)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.359.0 → v0.360.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14
> **Status**: ✅ Complete

## 1. 背景

Stage 18.74 审计识别 5 个错误类型缺 Kind enum (仅 String message)。
Stage 18.91 推荐为 v0.2 核心基础。本 Stage 为 5 个错误类型添加 Kind enum。

已有 Kind enum 的错误类型 (Stage 18.58):
- ✅ ResolveErrorKind (8 variants)
- ✅ TypeErrorKind (6 variants)
- ✅ BorrowErrorKind (9 variants)

缺 Kind enum 的错误类型 (本 Stage 修复):
- ❌ LexError
- ❌ ParseError
- ❌ LowerError
- ❌ CodegenError
- ❌ MacroError

## 2. 设计方案

### 2.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | Kind enum 显式分类, 不依赖字符串匹配 |
| 6 通用 > 特例 | 统一的 Kind enum 模式 (与 Resolve/Type/Borrow 一致) |

### 2.2 Kind enum 设计

每个错误类型添加 `kind` 字段 + 对应 Kind enum:

**LexErrorKind** (lexer 错误分类):
- `UnterminatedString` — 未闭合字符串
- `UnterminatedChar` — 未闭合字符
- `UnterminatedBlockComment` — 未闭合块注释
- `InvalidEscape` — 无效转义序列
- `UnexpectedChar` — 意外字符
- `InvalidNumber` — 无效数字
- `Generic` — 向后兼容

**ParseErrorKind** (parser 错误分类):
- `UnexpectedToken` — 意外 token
- `MissingToken` — 缺少 token
- `InvalidExpression` — 无效表达式
- `InvalidStatement` — 无效语句
- `InvalidType` — 无效类型
- `InvalidItem` — 无效 item
- `Generic` — 向后兼容

**LowerErrorKind / CodegenErrorKind / MacroErrorKind**: 类似模式

### 2.3 向后兼容策略

- `new(message, span)` 构造器保留, kind 默认为 `Generic`
- 新增 `with_kind(kind, message, span)` 构造器
- 现有代码无需修改 (使用 `new` 时 kind = Generic)

## 3. §6.3 委员会投票

**5/5 GO** ✅
