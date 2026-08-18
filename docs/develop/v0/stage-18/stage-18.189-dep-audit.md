# Stage 18.189 — 依赖与基础设施完整能力审查：Box::new + String::as_str

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + DEV-A + REV-A + PM-A
> **基线版本**: v0.456.0 (Stage 18.188)
> **触发条例**: 用户指令 "设计内容需要依赖底层实现和功能时, 应当先做依赖与基础设施完整能力审查"
> **Task ID**: stage18.189

## 1. 触发场景

### 1.1 任务目标

Per Stage 18.187 deep review + Stage 18.188 plan, 立即可做的 2 个功能:
- `Box::new(x)` — alloc + store + construct (类似 String::from_str)
- `String::as_str()` — 从 String.ptr + len 构造 &str fat pointer

## 2. 依赖项审计

### 2.1 Box::new(x) 依赖

| 依赖项 | 状态 |
|--------|------|
| __landin_alloc (Stage 18.178) | ✅ |
| Box<T> struct 类型 (Stage 18.179) | ✅ |
| Aggregate construction (tuple struct) | ✅ |
| Store through raw pointer (*p = x) | ✅ (Stage 18.178 修复) |
| sizeof(T) 计算 (layouts) | ✅ |
| MIR intrinsic 模式 (String::from_str 先例) | ✅ (Stage 18.185) |

**结论**: 完整, 可立即实现。

### 2.2 String::as_str() 依赖

| 依赖项 | 状态 |
|--------|------|
| String struct field access (s.ptr, s.len) | ✅ |
| &str fat pointer 类型 ({ ptr, i64 }) | ✅ |
| Aggregate construction (tuple with 2 fields) | ✅ |
| MIR intrinsic 模式 (str::len 先例) | ✅ (Stage 18.173) |

**结论**: 完整, 可立即实现。

## 3. 能力结论

✅ **所有依赖项完整** — 2 个功能均可立即实现, 无阻塞。

## 4. 修复计划

### 4.1 Box::new(x) (MIR intrinsic)

在 lower_call_expr 中拦截 `Box::new(x)`:
- 从 x 的类型推导 size (用 layouts)
- Call __landin_alloc(size)
- Store x to alloc'd ptr (*alloc_ptr = x)
- Aggregate Box { alloc_ptr }

### 4.2 String::as_str() (MIR intrinsic)

在 lower_method_call 中拦截 `s.as_str()`:
- 提取 s.ptr (field 0) 和 s.len (field 1)
- 构造 fat pointer Aggregate { ptr, len } 作为 &str
- 返回该 fat pointer

## 5. §3.2 验收 (审查 stage, 无代码变更)

- ✅ cargo check --all-features: 0 errors
- ✅ cargo test --features llvm-backend: 658 lib + 3040 integration = 3698 total, 0 failed

## 6. 结论

**依赖与基础设施完整能力审查通过** — Stage 18.189 可按计划执行。
