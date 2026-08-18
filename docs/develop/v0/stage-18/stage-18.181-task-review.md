# Stage 18.181 — 任务审查：基础类型完整性审计 (str / 数组 / 原语 / fat pointer)

> **审查日期**: 2026-08-17
> **审查者**: Super Z (ARCH-A + DEV-A + REV-A + PM-A 联合)
> **基线版本**: v0.448.0 (Stage 18.180)
> **触发条例**: 用户指令 "在设计实现 heap 上的内容之前，检查当前的基础 types 是否已经完整"
>   + docs/stage-committee-process.md §2.1 "任务规划排版图" + §17 任务审查
> **Task ID**: stage18.181

---

## 1. 触发场景

### 1.1 用户指令

> 在设计实现 heap 上的内容之前，检查当前的基础 types 是否已经完整
> （如， 你在计划 String 时，就应该考虑 str 设计支持得怎么样，是否完整，
> 是否需要重排任务图等问题）

### 1.2 客观事实

已完成 heap-allocated 类型的前 3 个 stage:
- Stage 18.178: heap alloc infrastructure (__landin_alloc / __landin_dealloc)
- Stage 18.179: Box<T> MVP (tuple struct wrapper)
- Stage 18.180: real String type (struct { ptr, len, cap })

原计划 Stage 18.181 = String intrinsics (from_str/push_str/len/as_str)。
但在推进前，必须按用户指令审计基础类型完整性。

### 1.3 触发条件判定

- **任务依赖缺陷**: String intrinsics 依赖 &str 完整性 (from_str 需读取 &str 内容)
- **基础类型完整性未审计**: str 方法支持度未知
- **触发条例生效**: 必须先做任务审查

---

## 2. 基础类型完整性审计

### 2.1 str 类型审计

#### 2.1.1 已支持 (✅)

| 功能 | 测试 | 状态 |
|------|------|------|
| `let s: &str = "hello"` 字面量绑定 | manual test | ✅ |
| `s.len()` 返回字节数 | test_str_methods.lin → "5" | ✅ |
| `s == t` 字符串相等比较 | test_str_eq.lin → "false true true" | ✅ |
| `s != t` 字符串不等比较 | 同上 | ✅ |
| `println!("{}", s)` 打印 | 多处测试 | ✅ |
| `println!("{}", "literal")` 字面量打印 | 多处测试 | ✅ |

#### 2.1.2 未支持 / 部分支持 (❌ / 🟡)

| 功能 | 测试结果 | 严重性 |
|------|---------|--------|
| `s.is_empty()` | 编译通过，但运行时 segfault (EXIT=1, 无输出) | 🔴 P1 |
| `s.as_bytes()` | 编译通过，但运行时 segfault | 🔴 P1 |
| `s[0]` 字节索引 | codegen 错误 "GEP base pointer is not a vector" | 🔴 P1 |
| `s + t` 字符串拼接 | 编译错误 "cannot apply arithmetic to &str" | 🟡 P2 (设计如此, 需 String + format!) |
| `s.to_string()` | 编译通过但运行时 segfault | 🔴 P1 |
| `s.chars()` 字符迭代 | 未实现 (没有 chars 类型) | 🟡 P2 |
| `s.split(sep)` 分割 | 未实现 | 🟡 P2 |
| `s.trim()` 去空白 | 未实现 | 🟡 P2 |
| `s.starts_with(p)` / `ends_with(p)` | 未实现 | 🟡 P2 |
| `s.contains(p)` | 未实现 | 🟡 P2 |

**关键发现**: str 类型基础功能不完整 — `is_empty`/`as_bytes`/`to_string` 编译通过
但运行时 segfault，`s[0]` 直接 codegen 错误。这些都是 String intrinsics 的前置依赖。

### 2.2 数组类型审计 ([T; N])

#### 2.2.1 已支持 (✅)

| 功能 | 测试 | 状态 |
|------|------|------|
| `let arr = [1, 2, 3]` 字面量构造 | test_array_simple.lin | ✅ |
| `arr[0]` 索引访问 (单次) | test_arr_simple2.lin → "10" | ✅ |
| `let mut arr = [...]; arr[0] = 10` 可变索引赋值 | conformance test 016 | ✅ |
| 多元素类型 (i32, u8, struct) | 多处 | ✅ |

#### 2.2.2 未支持 / 有 bug (❌ / 🟡)

| 功能 | 测试结果 | 严重性 |
|------|---------|--------|
| `arr[1]` 返回错误值 (返回 arr[0]) | test_arr_simple2.lin: arr=[10,20,30], arr[1]→"10" (应为 20) | 🔴 P0 |
| `arr[2]` 返回 0 (out of bounds 未触发 panic) | test_arr02.lin: arr[2]→"0" (应为 30) | 🔴 P0 |
| `println!("{} {} {}", arr[0], arr[1], arr[2])` 多索引打印 | test_array_print3.lin: segfault | 🔴 P0 |
| `println!("{} {}", arr[0], arr[2])` 2 索引打印 | test_array_2vals.lin → "949522175 1" (垃圾值) | 🔴 P0 |

**关键发现**: 数组索引 `arr[N]` 有严重 codegen bug — `arr[1]` 返回 `arr[0]` 的值，
`arr[2]` 返回 0 (out-of-bounds 未检测)。这是 P0 阻塞项，影响所有依赖数组的基础类型。

### 2.3 原语类型审计

#### 2.3.1 已支持 (✅)

| 类型 | 算术 | 比较 | 打印 | 转换 |
|------|------|------|------|------|
| i8/i16/i32/i64/i128 | ✅ | ✅ | ✅ | ✅ (cast) |
| u8/u16/u32/u64/u128 | ✅ | ✅ | ✅ (Stage 18.179 zext fix) | ✅ |
| f32/f64 | ✅ | ✅ | ✅ | ✅ |
| bool | ✅ | ✅ | ✅ ("true"/"false") | ✅ |
| char | 🟡 | ✅ | 🟡 | 🟡 |
| isize/usize | ✅ | ✅ | ✅ | ✅ |

#### 2.3.2 未支持 / 部分 (🟡)

| 功能 | 状态 | 严重性 |
|------|------|--------|
| i8 溢出 (100+50 → -106) | ✅ 设计如此 (wrap) | — |
| `i32::MAX` / `i32::MIN` 常量 | 🟡 未实现 | P2 |
| `size_of::<T>()` | 🟡 未实现 | P2 |
| `char` 类型的 println | 🟡 未测试 | P2 |

**关键发现**: 原语类型基本完整，仅缺常量 (MAX/MIN) 和 size_of，可推迟。

### 2.4 fat pointer 审计 (&str, &[T])

#### 2.4.1 已支持 (✅)

| 功能 | 状态 |
|------|------|
| &str fat pointer { ptr, i64 } | ✅ |
| &[T; N] 数组引用 | ✅ |
| fat pointer Field projection (FieldId(0)=ptr, FieldId(1)=len) | ✅ (Stage 18.174 fix) |
| fat pointer 通过 Deref 加载 | ✅ (Stage 18.178 RawPtr fix) |

#### 2.4.2 未支持 (🟡)

| 功能 | 状态 | 严重性 |
|------|------|--------|
| fat pointer Index projection (s[0]) | ❌ codegen 错误 | P1 |
| fat pointer 跨函数传递 | 🟡 未完整测试 | P2 |

**关键发现**: fat pointer 基础 OK, 但 Index projection (s[0]) 不工作 — 这阻塞了
str 字节索引和 &[T] 切片索引。

### 2.5 tuple 类型审计

#### 2.5.1 已支持 (✅)

| 功能 | 状态 |
|------|------|
| `(a, b)` 构造 | ✅ |
| `t.0` / `t.1` 字段访问 | ✅ |
| tuple 模式匹配 `let (a, b) = t` | ✅ |
| 嵌套 tuple `((a, b), c)` | ✅ |

**关键发现**: tuple 完整。

### 2.6 struct 类型审计

#### 2.6.1 已支持 (✅)

| 功能 | 状态 |
|------|------|
| `struct S { x: i32, y: i32 }` 定义 | ✅ |
| `S { x: 1, y: 2 }` 字面量构造 | ✅ |
| `s.x` / `s.y` 字段访问 | ✅ |
| `s.x = 10` 字段赋值 | ✅ |
| tuple struct `struct S(T)` | ✅ |
| `S(v)` tuple struct 构造 | ✅ |
| 嵌套 struct | ✅ |
| struct 模式匹配 | ✅ |

#### 2.6.2 未支持 (🟡)

| 功能 | 状态 | 严重性 |
|------|------|--------|
| `struct Update` `S { x: 1, ..base }` | 🟡 parser 支持但 codegen 未完整 | P2 |
| struct Default trait | 🟡 未实现 | P2 |

**关键发现**: struct 完整。

### 2.7 enum 类型审计

#### 2.7.1 已支持 (✅)

| 功能 | 状态 |
|------|------|
| `enum E { A, B(T), C { x: i32 } }` 定义 | ✅ |
| `E::A` / `E::B(v)` 构造 | ✅ |
| `match e { ... }` 模式匹配 | ✅ |
| variant constructor (不带前缀) | ✅ (Stage 18.167) |
| Option/Result 内置类型 | ✅ (Stage 18.165-18.168) |

**关键发现**: enum 完整。

---

## 3. 审计结论

### 3.1 严重阻塞项 (P0)

**数组索引 codegen bug**: `arr[N]` 返回错误值。
- `arr[1]` 返回 `arr[0]` (索引偏移 bug)
- `arr[2]` 返回 0 (out-of-bounds 未检测)
- 多索引表达式 segfault

这是 P0 阻塞项, 影响所有依赖数组的基础类型 (String bytes, &[T], Vec 实现)。
**必须先修复再继续 heap 类型开发**。

### 3.2 重要阻塞项 (P1)

**str 方法运行时 segfault**: `is_empty`/`as_bytes`/`to_string` 编译通过但运行时崩溃。
- 这些是 String intrinsics 的前置依赖
- `from_str` 需要遍历 &str 字节, 依赖 `as_bytes` 工作正常
- `push_str` 需要计算 source 长度, 依赖 `len` (OK) 和字节读取 (broken)

**fat pointer Index projection**: `s[0]` 直接 codegen 错误。
- 这是 str/切片字节索引的基础
- 也是 Vec 实现的基础 (Vec 内部用 Index projection)

### 3.3 可推迟项 (P2)

- str 高级方法 (chars/split/trim/starts_with/ends_with/contains)
- 原语常量 (i32::MAX/MIN)
- size_of::<T>()
- struct Update 语法
- str + 拼接 (依赖 String + format!)

### 3.4 任务图重排结论

**原任务图 (Stage 18.177 重排)**:
```
18.181 String intrinsics (from_str/push_str/len/as_str)
18.182 format! 宏
18.183 阶段末深度审查
```

**新任务图 (重排)**:
```
18.181 任务审查 (本 stage) — 基础类型审计 + 重排
18.182 数组索引 codegen 修复 (P0)
  → arr[N] 索引偏移 bug + out-of-bounds 检测
18.183 fat pointer Index projection (P1)
  → s[0] / bytes[N] 字节索引
18.184 str 方法运行时修复 (P1)
  → is_empty / as_bytes / to_string 运行时不再 segfault
18.185 String intrinsics (原 18.181)
  → from_str / push_str / len / as_str
18.186 format! 宏 (原 18.182)
18.187 阶段末深度审查 §14.5 D1-D8
```

### 3.5 重排理由

1. **依赖正确性**: String intrinsics 依赖 str 方法 (as_bytes/len) 工作正常
2. **基础先行**: 数组索引是 str/Vec/format! 的共同基础, 必须先修
3. **避免累积偏差**: 不在 broken base 上堆叠新功能 (Stage 18.177 教训)
4. **整体性完整修复**: 数组 + fat pointer + str 方法是一组关联 bug, 一起修

---

## 4. 简写与缺陷记录

### 4.1 TD-ARRAY-INDEX-CODEGEN (新增, P0)

| 字段 | 值 |
|------|---|
| **ID** | TD-ARRAY-INDEX-CODEGEN |
| **优先级** | P0 |
| **简写内容** | 数组索引 `arr[N]` codegen 有偏移 bug: arr[1] 返回 arr[0], arr[2] 返回 0 (OOB 未检测) |
| **根因** | 待 Stage 18.182 调查 (推测 codegen Index projection 的 GEP offset 计算错误) |
| **影响** | 所有数组访问, 阻塞 String/Vec/format! |
| **修复计划** | Stage 18.182: 修复 codegen Index projection + 添加 OOB bounds check |
| **状态** | 🟡 Active — Stage 18.182 修复 |

### 4.2 TD-STR-METHODS-RUNTIME (新增, P1)

| 字段 | 值 |
|------|---|
| **ID** | TD-STR-METHODS-RUNTIME |
| **优先级** | P1 |
| **简写内容** | str 的 is_empty/as_bytes/to_string 编译通过但运行时 segfault |
| **根因** | 待 Stage 18.184 调查 (推测 codegen 未实现这些方法的 intrinsic) |
| **影响** | String intrinsics 的前置依赖 |
| **修复计划** | Stage 18.184: 实现这些方法的 MIR intrinsic + codegen |
| **状态** | 🟡 Active — Stage 18.184 修复 |

### 4.3 TD-FAT-PTR-INDEX-PROJ (新增, P1)

| 字段 | 值 |
|------|---|
| **ID** | TD-FAT-PTR-INDEX-PROJ |
| **优先级** | P1 |
| **简写内容** | fat pointer (str/切片) 的 Index projection `s[0]` 直接 codegen 错误 |
| **根因** | 待 Stage 18.183 调查 (推测 codegen 未处理 fat pointer 的 Index) |
| **影响** | str 字节索引, &[T] 切片索引, Vec 实现 |
| **修复计划** | Stage 18.183: codegen 添加 fat pointer Index projection 支持 |
| **状态** | 🟡 Active — Stage 18.183 修复 |

---

## 5. 本 stage (18.181) 范围

### 5.1 范围声明

本 stage **仅做任务审查 + 文档同步**, 不修改编译器代码。理由:
1. 任务审查是流程性活动 (§17 + 用户指令触发)
2. 修复工作量大 (数组 + fat pointer + str 方法 = 3+ stages)
3. 每个 stage 需独立验收 + 测试 + 打包

### 5.2 本 stage 交付物

1. `docs/develop/v0/stage-18/stage-18.181-task-review.md` (本文件)
2. `docs/develop/v0/tech-debt-register.md` 新增 3 个 TD 条目 (P0+P1+P1)
3. worklog 追加本 stage 记录

### 5.3 下一 stage 启动条件

- ✅ 本任务审查完成
- ✅ tech-debt-register 更新完成
- ✅ 任务图重排完成

---

## 6. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings (无代码变更)
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 2996 passed
- **Total**: 3654 tests, 0 failures

---

## 7. 结论

**任务审查通过** — 基础类型完整性审计发现 1 个 P0 (数组索引) + 2 个 P1
(str 方法 + fat pointer Index) 阻塞项, 必须先修复再继续 String intrinsics。

**关键决策**:
1. 任务图重排: 原 18.181 (String intrinsics) 推迟到 18.185
2. 新增 3 个 stage (18.182-18.184) 修复基础类型 bug
3. 不在 broken base 上堆叠新功能 (Stage 18.177 教训)
4. 整体性完整修复: 数组 + fat pointer + str 方法是一组关联 bug

**用户指令达成**:
- ✅ "检查当前的基础 types 是否已经完整" — 审计完成
- ✅ "你在计划 String 时，就应该考虑 str 设计支持得怎么样" — 已考虑 (str 方法 broken)
- ✅ "是否需要重排任务图等问题" — 已重排
