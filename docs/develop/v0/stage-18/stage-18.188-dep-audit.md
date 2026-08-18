# Stage 18.188 — 依赖与基础设施完整能力审查：String::new + String::as_str + Box::new

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + DEV-A + REV-A + PM-A
> **基线版本**: v0.455.0 (Stage 18.187)
> **触发条例**: 用户指令 "设计内容需要依赖底层实现和功能时, 应当先做依赖与基础设施完整能力审查"
> **Task ID**: stage18.188

## 1. 触发场景

### 1.1 任务目标

Per Stage 18.187 deep review plan, 立即可做的 3 个功能:
- `String::new()` — trivial: String { ptr: null, len: 0, cap: 0 }
- `String::as_str()` — 从 String.ptr + len 构造 &str fat pointer
- `Box::new(x)` — alloc + store + construct (类似 String::from_str)

### 1.2 触发条例

这 3 个功能依赖底层基础设施 (alloc, memcpy, fat pointer 构造), 必须先审计。

## 2. 依赖项审计

### 2.1 String::new() 依赖

| 依赖项 | 状态 |
|--------|------|
| String struct 类型 (Stage 18.180) | ✅ |
| Aggregate construction (struct literal) | ✅ |
| Null pointer constant (0 as *mut u8) | ✅ |
| Integer constant (0i64) | ✅ |

**结论**: 完整, 可立即实现。

### 2.2 String::as_str() 依赖

| 依赖项 | 状态 |
|--------|------|
| String struct field access (s.ptr, s.len) | ✅ |
| &str fat pointer 类型 ({ ptr, i64 }) | ✅ |
| Aggregate construction (tuple/struct with 2 fields) | ✅ |
| MIR intrinsic pattern (str::len precedent) | ✅ |

**结论**: 完整, 可立即实现。需新增 MIR intrinsic (类似 str::len)。

### 2.3 Box::new(x) 依赖

| 依赖项 | 状态 |
|--------|------|
| __landin_alloc (Stage 18.178) | ✅ |
| Box<T> struct 类型 (Stage 18.179) | ✅ |
| Aggregate construction (tuple struct) | ✅ |
| Store through raw pointer (*p = x) | ✅ (Stage 18.178 修复) |
| sizeof(T) 计算 | 🟡 需 type-aware alloc |

**sizeof 问题**: `Box::new(x)` 需要 alloc(sizeof(T))。当前 __landin_alloc(size: i64)
需要调用方提供 size。MIR lower 需要从 T 的类型推导 size。

**MVP 方案**: 用固定 size (按类型 hardcoded) 或从 layouts 推导。
Per §1.0 原則 6 (通解>特例): 用 layouts 推导, 不 hardcode。

**结论**: 完整 (layouts 已支持), 可实现。

## 3. 能力结论

### 3.1 依赖完整性

✅ **所有依赖项完整** — 3 个功能均可立即实现。

### 3.2 阻塞项

无阻塞项。

### 3.3 任务图确认

**不需要重排** — Stage 18.188 可按计划执行, 范围: 3 个功能。

## 4. 修复计划

### 4.1 String::new() (prelude impl)

添加到 prelude:
```landin
impl String {
    fn new() -> String { String { ptr: 0 as *mut u8, len: 0, cap: 0 } }
}
```

### 4.2 String::as_str() (MIR intrinsic)

在 lower_method_call 中拦截 `s.as_str()`:
- 从 String.ptr (field 0) + String.len (field 1) 构造 &str fat pointer
- 用 AggregateKind::Tuple (因为 &str 是 { ptr, i64 } struct)

### 4.3 Box::new(x) (MIR intrinsic)

在 lower_call_expr 中拦截 `Box::new(x)`:
- 从 x 的类型推导 size (用 layouts)
- Call __landin_alloc(size)
- Store x to alloc'd ptr
- Aggregate Box { ptr }

## 5. §3.2 验收 (审查 stage, 无代码变更)

- ✅ cargo check --all-features: 0 errors
- ✅ cargo test --features llvm-backend: 658 lib + 3035 integration = 3693 total, 0 failed

## 6. 结论

**依赖与基础设施完整能力审查通过** — Stage 18.188 可按计划执行。
3 个功能 (String::new, String::as_str, Box::new) 均可立即实现, 无阻塞。
