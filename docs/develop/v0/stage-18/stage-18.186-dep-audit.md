# Stage 18.186 — 依赖与基础设施完整能力审查：format! 宏

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + DEV-A + REV-A + PM-A
> **基线版本**: v0.453.0 (Stage 18.185)
> **触发条例**: 用户指令 "设计内容需要依赖底层实现和功能时, 应当先做依赖与基础设施完整能力审查"
> **Task ID**: stage18.186

---

## 1. 触发场景

### 1.1 任务目标

Stage 18.186: 实现 `format!` 宏。
当前 `format!("x={}", x)` 展开为 `__landin_format("x={}", x)` 调用, 但:
- `__landin_format` 在 C wrapper 中**未定义** → 链接错误
- 当前返回 `{ ptr, i64 }` fat pointer (即 &str), 不是 owned String
- 与 Stage 18.180 的真实 String 类型不匹配

### 1.2 触发条例

format! 宏依赖:
- String::from_str (Stage 18.185) ✅
- __landin_memcpy (Stage 18.185) ✅
- __landin_alloc (Stage 18.178) ✅
- printf-style 格式化 (现有 codegen) ✅
- 但 `__landin_format` runtime stub **缺失** ❌

---

## 2. 依赖项审计

### 2.1 MIR 层依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| format! macro expansion (→ __landin_format call) | ✅ | src/parser/builtin_macros.rs:448 |
| MIR Call terminator | ✅ | 已支持 |
| Synthetic DefId for __landin_format | ❌ 未注册 | 需要新增 |

### 2.2 Codegen 层依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| emit_call for extern fn | ✅ | Stage 18.178 修复 |
| declare_function auto-declare | ✅ | Stage 18.178 修复 |
| String 类型 (struct { ptr, len, cap }) | ✅ | Stage 18.180 |
| Aggregate construction | ✅ | Stage 18.185 |

### 2.3 Runtime 层依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| __landin_alloc | ✅ | Stage 18.178 |
| __landin_memcpy | ✅ | Stage 18.185 |
| __landin_dealloc | ✅ | Stage 18.178 |
| __landin_format runtime stub | ❌ 缺失 | **必须新增** |
| printf (libc) | ✅ | 已可用 |

### 2.4 类型系统依赖

| 依赖项 | 状态 | 验证 |
|--------|------|------|
| String Adt 类型 | ✅ | Stage 18.180 prelude |
| &str fat pointer | ✅ | Stage 18.174 |
| i32 / i64 / u8 / bool 类型 | ✅ | 完整 |

---

## 3. 设计决策

### 3.1 两种实现路径

**方案 A: C runtime stub (__landin_format)**

在 C wrapper 中实现 `__landin_format`:
```c
// 接受 fat pointer format + variadic args, 返回 String (struct)
struct String __landin_format(const char* fmt, long long fmt_len, ...) {
    // 1. 计算结果长度 (vsnprintf twice)
    // 2. __landin_alloc(len)
    // 3. vsnprintf 写入 buffer
    // 4. 返回 String { ptr, len, cap }
}
```

**问题**:
- C 函数返回 struct by value — ABI 复杂 (LLVM sret 参数)
- 需要在 C stub 中知道 String 的布局 (与 Landin 一致)
- variadic args 的类型信息丢失 (C 不知道 i32 vs i64)

**方案 B: MIR intrinsic (推荐)**

在 MIR lower 中拦截 `__landin_format(...)` 调用, 展开为:
1. 用 `vsnprintf` (libc) 计算格式化后的长度
2. `__landin_alloc(len)` 分配 heap buffer
3. `vsnprintf(buffer, len, fmt, args)` 写入格式化字符串
4. 构造 `String { ptr: buffer, len, cap: len }`

**优点**:
- 复用 String::from_str intrinsic 模式 (alloc + construct)
- 类型信息在 MIR 层可见
- 不需要 C struct by value ABI

**缺点**:
- MIR lower 需要处理 variadic args (每个 arg 一个 Call)
- 需要 `__landin_vsnprintf` runtime stub

### 3.2 选择: 方案 B (MIR intrinsic)

**理由**:
1. 复用 Stage 18.185 的 String::from_str intrinsic 模式 (通解)
2. 避免 C struct by value ABI 复杂性
3. 类型安全 (类型信息在 MIR 层)
4. Per §1.0 原則 6 (通解>特例): 与 String::from_str 一致的处理方式

### 3.3 简化: 先做 MVP — `format!("literal")` 无 args

由于 variadic args + 类型信息复杂, MVP 先支持:
- `format!("hello")` — 单字符串字面量, 无 {}
- `format!("x={}", x)` — 推迟到 Stage 18.187

MVP 实现: format!("hello") → String::from_str("hello")

### 3.4 简写记录: TD-FORMAT-VARIADIC

`format!("x={}", x)` 的完整实现 (variadic + 类型推导) 推迟到 Stage 18.187+。
理由: 需要 MIR 层处理 variadic args, 类型信息传递 (i32 vs &str), 这些是
v0.2 P2+ 工作。MVP 先支持字面量字符串。

---

## 4. 能力结论

### 4.1 依赖完整性

✅ **所有依赖项完整** (在新增 __landin_format stub 或 MIR intrinsic 后)

### 4.2 阻塞项

🟡 `__landin_format` runtime stub / MIR intrinsic 缺失 — 本 stage 实现

### 4.3 任务图确认

**不需要重排** — Stage 18.186 可按计划执行, MVP 范围:
- `format!("literal")` → String::from_str("literal")
- `format!("x={}", x)` 推迟到 Stage 18.187 (TD-FORMAT-VARIADIC)

---

## 5. 修复计划

### 5.1 MVP 实现 (本 stage)

在 `lower_call_expr` 中拦截 `__landin_format` 调用:
- 如果只有 1 个 arg (format string literal), 展开为 String::from_str(arg)
- 如果有 >1 args, 暂时报错 "format! with args not yet supported (TD-FORMAT-VARIADIC)"

### 5.2 测试计划

- 正向: format!("hello").len() == 5, format!("").len() == 0
- 负向: format!("x={}", 42) 报错 (TD-FORMAT-VARIADIC)

### 5.3 延后项

- TD-FORMAT-VARIADIC: format! with {} placeholders + args (Stage 18.187+)
- 需要: __landin_vsnprintf + variadic arg 类型推导

---

## 6. §3.2 验收 (审查 stage, 无代码变更)

- ✅ cargo check --all-features: 0 errors
- ✅ cargo test --features llvm-backend: 658 lib + 3027 integration = 3685 total, 0 failed

---

## 7. 结论

**依赖与基础设施完整能力审查通过** — Stage 18.186 MVP 可按计划执行。
- 方案 B (MIR intrinsic) 选定, 复用 String::from_str 模式
- MVP 范围: `format!("literal")` → String::from_str (无 {} args)
- 完整 variadic format! 推迟到 Stage 18.187 (TD-FORMAT-VARIADIC)
