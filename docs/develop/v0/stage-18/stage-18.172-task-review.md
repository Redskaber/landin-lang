# Stage 18.172 — 任务审查: heap allocation 可行性 + 推进计划

> **Author**: redskaber (PM-A + ARCH-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.440.0 (Stage 18.172 任务审查报告)
> **Process**: docs/stage-committee-process.md v6.4 §5.1 (复杂度预评估) + §17 (任务规划排版图)
> **Task ID**: stage18.172

## 1. 任务审查背景

按 Stage 18.171 完成后的计划, 下一步是 heap allocation 基础设施 (String/Vec 前置条件)。

## 2. 审查发现

### 2.1 Box::new 当前行为

**发现**: `Box::new(42)` 已经"工作" — 但不是通过堆分配:
- Box 被注册为 struct `Box<T> { val: T }` (栈分配)
- `Box::new(42)` 生成 `insertvalue { i32 } undef, i32 42, 0` (栈上构造)
- `*b` (deref) 只是加载栈上的值
- 实际运行返回正确结果 (exit code 42)

**结论**: Box 不需要堆分配即可工作 — 它只是栈上的 struct wrapper。真正的堆分配仅在 String/Vec (需要动态大小) 时才需要。

### 2.2 String/Vec 是否可以不用堆分配?

**分析**:
- String 需要动态大小的缓冲区 → 需要堆分配 (malloc)
- Vec 需要动态大小的缓冲区 → 需要堆分配 (malloc)
- 但如果 Landin 的 String/Vec 仅支持固定大小 (如编译期已知长度的字符串), 可以用栈数组模拟

**决策**: 实现简化的 String/Vec (栈分配, 固定大小) 作为 MVP:
- String = `{ ptr, len }` fat pointer (指向全局字符串常量, 无需 malloc)
- Vec = 暂不实现 (需要动态大小, 依赖 malloc)
- 这样 String 可以立即工作 (&str 已经是 fat pointer)

### 2.3 能力具备性

| 维度 | 评估 | 详情 |
|------|------|------|
| &str fat pointer | ✅ 具备 | codegen 已支持 `{ ptr, i64 }` fat pointer |
| String literal | ✅ 具备 | 字符串字面量存储为全局常量 |
| String::from(&str) | ✅ 可实现 | 返回 fat pointer (无需 malloc) |
| String length | ✅ 可实现 | fat pointer 的 len 字段 |
| String index | ✅ 可实现 | 通过 ptr + index 访问 |
| String concatenation | ❌ 不具备 | 需要 malloc (动态大小) |
| Vec | ❌ 不具备 | 需要 malloc (动态大小) |

### 2.4 复杂度评估

- **String (栈分配 MVP)**: L2 — 仅添加 prelude 类型 + 方法
- **Vec**: L3 — 需要 malloc/free codegen
- **heap allocation**: L3 — 需要修改 codegen 添加 malloc/free 调用

## 3. 重排任务排版图

### 3.1 新排版图

```
Stage 18.172 (本 stage):
  → 任务审查 + 记录发现 (Box 已栈分配工作)
  → 重排: String (栈分配) 优先, Vec/heap 推迟

Stage 18.173:
  → 实现 String (栈分配 MVP)
  → String::from(&str) → 返回 fat pointer
  → String::len() → 返回 len 字段
  → String::as_str() → 返回 &str
  → 不需要 malloc (字符串字面量是全局常量)

Stage 18.174:
  → heap allocation 基础设施 (malloc/free codegen)
  → LLVM IR 中调用 @malloc/@free
  → C wrapper 添加 malloc/free 声明

Stage 18.175:
  → Vec 实现 (基于 malloc)
  → Vec::new() → 空 Vec
  → Vec::push() → 动态扩容

Stage 18.176:
  → String 动态功能 (基于 Vec<u8>)
  → String::push_str() → 动态拼接
  → format! 宏
```

### 3.2 重排原因

| 原任务 | 重排原因 | 新位置 |
|--------|---------|--------|
| heap allocation | Box 已栈分配工作, 不急需; String 可先栈分配 | 18.174 (推迟) |
| String | 可先栈分配 (无需 malloc) | 18.173 (提前) |
| Vec | 依赖 malloc | 18.175 (推迟) |
| format! | 依赖 String 动态功能 | 18.176 (推迟) |

## 4. 简写和缺陷记录

### 4.1 Box 栈分配简写

**简写**: Box::new 使用栈分配 (非堆分配), 不是真正的 owned heap pointer。
- **原因**: codegen 无 malloc 支持, Box 被当作普通 struct 处理。
- **影响**: Box 值在函数返回时被 drop (栈上), 不能跨函数传递。
- **修订计划**: Stage 18.174 实现 malloc 后, 修改 Box codegen 使用堆分配。

### 4.2 String 栈分配简写

**简写**: String 将使用 fat pointer 指向全局常量 (非堆分配)。
- **原因**: 字符串字面量是编译期已知的, 存储为全局常量。
- **影响**: String 不能动态修改 (push_str 等需要 malloc)。
- **修订计划**: Stage 18.176 实现 String 动态功能后修复。

## 5. §3.2 验收

本 stage 为任务审查, 无代码修改, 验收基于上 stage (v0.439.0) 状态。

## 6. Stage Summary

- **Stage 18.172 PASSED** — 任务审查: heap allocation 可行性 + 推进计划
- **发现**: Box::new 已栈分配工作 (非堆分配), String 可先栈分配
- **重排**: String (栈分配) 提前到 18.173, heap allocation 推迟到 18.174
- **能力边界**: &str fat pointer ✅, String literal ✅, String::from 可实现 ✅
- **v0.440.0**: patch bump (任务审查)
- **下一步**: Stage 18.173 实现 String (栈分配 MVP)
