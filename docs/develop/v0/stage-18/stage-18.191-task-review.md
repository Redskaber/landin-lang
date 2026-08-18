# Stage 18.191 — 任务审查：v0.2 P1 后续任务选择与重排

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + PM-A + REV-A
> **基线版本**: v0.458.0 (Stage 18.190)
> **触发条例**: 用户指令 "按计划推进修复" + §17 任务规划排版图
> **Task ID**: stage18.191

## 1. 当前状态

### 1.1 已完成 (Stage 18.177-18.190, 14 stages)

- ✅ Heap alloc infrastructure (__landin_alloc/dealloc/memcpy)
- ✅ Box<T> (struct + Box::new + deref + type coercion fix)
- ✅ Real String (struct + from_str + new + len + as_str)
- ✅ str methods (len + is_empty + as_bytes + Index)
- ✅ format! MVP (literal)
- ✅ Array index codegen fix (P0)
- ✅ Fat pointer Index projection fix (P1)
- ✅ DCE collect_place_locals fix
- ✅ Function redefine bug fix

### 1.2 剩余 TD (from tech-debt-register)

| TD | 描述 | 优先级 | 依赖 |
|----|------|--------|------|
| TD-VEC-MVP | Vec<T> 无实际实现 | P2 | realloc (缺失) |
| TD-STRING-INTRINSICS | String::push_str 未实现 | P2 | realloc (缺失) |
| TD-FORMAT-VARIADIC | format! with {} args | P2 | variadic type inference |
| TD-BOX-AUTO-DROP | Box 无 auto-drop | P2 | drop glue |
| TD-INT-UINT-VAR | i64 literals > i32 max 截断 | P2 | IntOrUintVar in unify table |
| TD-ARRAY-BOUNDS-CHECK | arr[N] 无 OOB 检测 | P2 | bounds check codegen |
| TD-LOC-MACRO-EXPAND | macro_expand.rs 3904 LOC | P3 | 拆分 core matching |
| TD-LOC-DRIVER | driver/mod.rs 2351 LOC | P3 | 拆分 compile_inner |

## 2. 任务选择分析

### 2.1 Vec<T> — 阻塞 (realloc 缺失)

Vec 需要 realloc (动态扩容), 当前只有 alloc + dealloc。
realloc 可以用 alloc + memcpy + dealloc 模拟, 但这不是通解。
**结论**: 需要先实现 realloc 基础设施, 或用 alloc+memcpy+dealloc 临时实现。

### 2.2 String::push_str — 同样阻塞 (realloc)

push_str 需要扩容 buffer, 同 Vec 的 realloc 需求。

### 2.3 TD-INT-UINT-VAR — 可立即做

i64 literals > i32 max 被截断, 因为 lexer/const 始终用 i32 存储。
这是 pre-existing bug, 不依赖新基础设施。
**结论**: 可立即修复, 影响所有 i64 字面量使用。

### 2.4 TD-ARRAY-BOUNDS-CHECK — 可立即做

在 codegen Index projection 中插入 LLVM bounds check (比较 index < len, 超出则 call __landin_panic_bounds_check)。
__landin_panic_bounds_check 已存在 (C wrapper 中)。
**结论**: 可立即修复, 提升安全性。

### 2.5 Box auto-drop — 可立即做

在 drop glue 中, 对 Box 类型自动调用 __landin_dealloc(b.0 as *mut u8)。
drop glue 基础设施已存在 (codegen/drop_glue.rs)。
**结论**: 可立即修复, 消除内存泄漏。

## 3. 任务图重排

### 3.1 新任务图 (按优先级 + 依赖排序)

```
18.191 任务审查 (本 stage)
18.192 TD-INT-UINT-VAR 修复 (i64 literals > i32 max)
18.193 TD-ARRAY-BOUNDS-CHECK (OOB 检测)
18.194 Box auto-drop (drop glue 调用 __landin_dealloc)
18.195 realloc 基础设施 (__landin_realloc runtime stub)
18.196 Vec<T> MVP (new/push/len/pop, 基于 realloc)
18.197 String::push_str (基于 realloc)
18.198 format! variadic (基于类型推导)
18.199 阶段末深度审查 §14.5 D1-D8
```

### 3.2 重排理由

1. TD-INT-UINT-VAR 优先: 影响所有 i64 使用, 是 Box<i64> 的 pre-existing 阻塞
2. TD-ARRAY-BOUNDS-CHECK: 提升安全性, __landin_panic_bounds_check 已存在
3. Box auto-drop: 消除内存泄漏, drop glue 基础设施已就绪
4. realloc: 是 Vec + String::push_str 的共同前置依赖
5. Vec + String::push_str: 依赖 realloc
6. format! variadic: 最低优先级, 依赖类型推导

## 4. 本 stage 范围

本 stage 仅做任务审查 + 文档同步, 不修改编译器代码。
