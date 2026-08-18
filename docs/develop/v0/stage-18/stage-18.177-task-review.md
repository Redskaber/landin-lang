# Stage 18.177 — 任务审查：String 设计缺陷 + heap allocation 任务图重排

> **审查日期**: 2026-08-17
> **审查者**: Super Z (ARCH-A + DEV-A + REV-A + PM-A 联合)
> **基线版本**: v0.444.0 (Stage 18.176)
> **触发条例**: docs/stage-committee-process.md §2.1 "任务规划排版图" + 用户指令
>   "如果在开始选择处理的任务时遇到任务依赖缺陷及环境等任务阻塞问题时（时机：此条例触发时机），
>    应当先做任务审查"
> **Task ID**: stage18.177

---

## 1. 触发场景

### 1.1 用户对当前 String 设计的批评

> 当前的 String 明显是不符合 设计要求的（如果临时过渡，那么可以忍受），rust 中 String 是
> 用于分配在 heap 上的内存，而 str 才是 stack 上

### 1.2 客观事实

Stage 18.176 实现了 `String` 类型，方式为：

```rust
// src/resolve/primitives.rs
"String" => PrimTy::Str,  // String 作为 &str 的别名
```

这与设计文档 `docs/lang-design/09-stdlib.md` §3.4 严重不一致：

```landin
// 设计文档定义：
pub struct String {
    vec: Vec<u8>,   // 由 Vec<u8> 支撑的堆分配类型
}
```

### 1.3 触发条件判定

- **任务依赖缺陷**: String (real) → Vec → RawVec → heap allocation (malloc/free codegen)
- **环境阻塞**: 当前 codegen 无 malloc/free 支持
- **结论**: 实现真正的 String 被阻塞在 heap allocation 基础设施缺失上
- **触发条例生效**: 必须先做任务审查

---

## 2. 当前能力盘点

### 2.1 已具备的能力

| 能力 | 状态 | 位置 |
|------|------|------|
| `emit_call` 通用函数调用 | ✅ | `src/codegen/llvm/aggregate.rs:15` |
| `declare_function` 外部符号声明 | ✅ | `src/codegen/llvm/mod.rs:513` |
| C wrapper 已 `#include <stdlib.h>` | ✅ | `src/codegen/runtime.rs:39` (malloc/free 可用) |
| MIR intrinsic 拦截模式 | ✅ | `src/mir/lower/expr_variants.rs:989` (str::len 先例) |
| `Drop` terminator + drop glue 生成 | ✅ | `src/codegen/drop_glue.rs` |
| `TyKind::Adt` 结构体类型 | ✅ | `src/mir/ty.rs` |
| Prelude 源码注入 | ✅ | `src/stdlib/prelude.rs` |

### 2.2 缺失的能力（需新建）

| 能力 | 缺失原因 | 修复路径 |
|------|---------|---------|
| `malloc(size)` 调用 emission | codegen 无 extern 调用约定 | 直接 `emit_call("malloc", ...)` — 现有 `declare_function` 会自动声明 |
| `free(ptr)` 调用 emission | 同上 | 同上 |
| `Box<T>` 类型系统支持 | 无 Box 类型定义 | prelude 注入 `struct Box<T>(*mut T)` + intrinsics |
| `Box::new(x)` MIR lower 拦截 | 无 intrinsic 处理 | 在 `expr_variants.rs` 中拦截 `Box::new(x)` → MIR `malloc + store` |
| `Box<T>` Drop glue 自动调用 `free` | drop glue 仅生成 ADT 递归 drop | 扩展 drop glue：Box 类型时 emit `call void @free(ptr)` |
| `Vec<T>` 类型 + 方法 | 无 | prelude 注入 + Box<[T]> 支撑 |
| `String` 真实实现 (owned Vec<u8>) | 当前是 &str 别名 | prelude 注入 `struct String { vec: Vec<u8> }` + 方法 |

### 2.3 能力结论

**heap allocation 基础设施可立即实现**：所有底层原语已就绪（`emit_call`/`declare_function`/C `<stdlib.h>`），仅需在 MIR lower + codegen 层增加 Box/malloc/free 的语义拦截。**无环境阻塞**。

---

## 3. 简写与缺陷记录（必须随开发/设计文档登记）

### 3.1 TD-STRING-AS-STR-ALIAS（新增）

| 字段 | 值 |
|------|---|
| **ID** | TD-STRING-AS-STR-ALIAS |
| **优先级** | P2 |
| **引入版本** | v0.444.0 (Stage 18.176) |
| **简写内容** | `String` 实现为 `&str` 别名（`PrimTy::Str`），而非设计文档定义的 owned `Vec<u8>` 堆分配类型 |
| **简写原因** | Stage 18.175 深度审查后急于提供 String 可用性，跳过 heap allocation 基础设施直接做 MVP |
| **设计偏差** | `docs/lang-design/09-stdlib.md` §3.4 明确要求 `pub struct String { vec: Vec<u8> }` |
| **影响** | (1) `String` 不是 owned 类型，无法 push_str/extend (2) 与 Rust 语义不一致 (3) 用户预期落空 |
| **修复计划** | Stage 18.181: 用真实 `String { vec: Vec<u8> }` 替换 `&str` 别名 |
| **依赖** | TD-HEAP-ALLOC (Stage 18.178) + TD-VEC-MVP (Stage 18.180) |
| **状态** | 🟡 Active — Stage 18.176 已记录, Stage 18.181 修复 |

### 3.2 TD-HEAP-ALLOC（新增）

| 字段 | 值 |
|------|---|
| **ID** | TD-HEAP-ALLOC |
| **优先级** | P2 |
| **简写内容** | codegen 无 malloc/free 调用支持，阻碍所有 heap-allocated 类型 (Box/Vec/String/Rc/Arc) |
| **修复计划** | Stage 18.178: codegen 添加 malloc/free extern 调用 + Box<T> intrinsic + drop glue |
| **状态** | 🟡 Active — Stage 18.178 修复 |

### 3.3 TD-VEC-MVP（新增）

| 字段 | 值 |
|------|---|
| **ID** | TD-VEC-MVP |
| **优先级** | P2 |
| **简写内容** | `Vec<T>` 在 stdlib 注册表中作为名字存在，但无实际类型 + 方法实现 |
| **修复计划** | Stage 18.180: prelude 注入 `struct Vec<T> { ptr: *mut T, len: usize, cap: usize }` + new/push/len/pop 方法 |
| **依赖** | TD-HEAP-ALLOC (Stage 18.178) |
| **状态** | 🟡 Active — Stage 18.180 修复 |

---

## 4. 任务图重排（§17 任务规划排版图）

### 4.1 旧任务图（Stage 18.175 推进计划）

```
18.175 深度审查 GO
  ↓
18.176 String 栈分配 MVP (&str 别名)        ← 简写！
  ↓
18.177 heap allocation 基础设施 (未排期)
  ↓
18.178 Vec 实现 (未排期)
  ↓
18.179 String 动态功能 (未排期)
  ↓
18.180 format! 宏 (未排期)
```

### 4.2 新任务图（重排）

```
18.175 深度审查 GO
  ↓
18.176 String 栈分配 MVP (&str 别名)        ← 已完成（标记为 TD-STRING-AS-STR-ALIAS）
  ↓
18.177 任务审查 (本 stage) — 记录缺陷 + 重排任务图
  ↓
18.178 heap allocation 基础设施 + Box<T> MVP
  │   - codegen 添加 malloc/free extern 调用
  │   - prelude 注入 struct Box<T>(*mut T)
  │   - MIR lower 拦截 Box::new(x) → malloc + store
  │   - drop glue 自动调用 free
  ↓
18.179 Box<T> 完整支持 (Deref/DerefMut/Drop trait 接入)
  ↓
18.180 Vec<T> MVP (struct Vec<T>{ptr,len,cap} + new/push/len/pop)
  ↓
18.181 真实 String (owned Vec<u8>) — 替换 &str 别名 (修复 TD-STRING-AS-STR-ALIAS)
  │   - prelude 注入 struct String { vec: Vec<u8> }
  │   - String::new/from_str/push_str/push/len/as_str
  │   - Display trait 接入
  │   - 移除 PrimTy::Str 对 "String" 的别名映射
  ↓
18.182 format! 宏 (基于真实 String)
  ↓
18.183 阶段末深度审查 §14.5 D1-D8
```

### 4.3 关键依赖约束

| 任务 | 依赖 | 阻塞理由 |
|------|------|---------|
| 18.179 Box | 18.178 heap alloc | Box::new 需要 malloc |
| 18.180 Vec | 18.179 Box | Vec 内部用 Box<[T]> 或直接 malloc |
| 18.181 String (real) | 18.180 Vec | String = Vec<u8> 包装 |
| 18.182 format! | 18.181 String (real) | format! 需要动态 String 拼接 |

### 4.4 与原计划差异说明

| 差异点 | 原计划 | 新计划 | 理由 |
|--------|-------|-------|------|
| String 出现位置 | 18.176 (栈分配 MVP) | 18.181 (真实堆分配) | 原 MVP 违反设计，必须延后到 heap/Box/Vec 就绪后 |
| Box 新增 | 未排期 | 18.178 + 18.179 | Box 是最简单的 owned heap 类型，是 Vec/String 的前置 |
| 阶段数 | 4 (176-179) | 7 (176-182) | 完整修复需要更多阶段，避免再次简写 |

---

## 5. 设计文档同步

### 5.1 `docs/lang-design/09-stdlib.md` §3.4 更新

需在 §3.4 String 节首添加 MVP 偏差说明：

```markdown
> **MVP 偏差说明（Stage 18.176-18.181）**:
>
> 当前编译器实现 (v0.444.0) 中 `String` 是 `&str` 的别名（栈分配 fat pointer），
> 而非此处定义的 owned `Vec<u8>` 堆分配类型。这是 Stage 18.176 为快速提供
> String 可用性的临时过渡，记录为 TD-STRING-AS-STR-ALIAS。
>
> 真实 `String { vec: Vec<u8> }` 实现计划在 Stage 18.181 完成（依赖
> Stage 18.178 heap allocation 基础设施 + Stage 18.180 Vec MVP）。
> 详见 `docs/develop/v0/stage-18/stage-18.177-task-review.md`。
```

### 5.2 `docs/develop/v0/tech-debt-register.md` 更新

- §2.6 Standard Library 新增 TD-STRING-AS-STR-ALIAS, TD-HEAP-ALLOC, TD-VEC-MVP
- §4.1 By Severity 表格更新

---

## 6. 同类型/依赖关系整体性完整修复方案

### 6.1 问题性质

用户指出 "同类型错误或者存在依赖关系的应该考虑整体性完整修复（避免存在缺漏和遗失）"。
heap allocation 链路是一个完整依赖图：

```
heap alloc → Box → Vec → String → format!
```

任何中间节点的简写都会导致下游全部偏差。因此本次任务审查的核心结论是：

**不接受局部修复** — 必须从 heap allocation 基础设施开始，逐层完整实现直到 String (real)。

### 6.2 完整修复路径

| Stage | 工作内容 | 验收标准 |
|-------|---------|---------|
| 18.178 | heap alloc 基础设施 + Box MVP | `let b = Box::new(42); *b` 在 codegen 中正确生成 malloc + load |
| 18.179 | Box trait 接入 | Deref/DerefMut 自动 deref；Drop 自动 free；通过 borrowck |
| 18.180 | Vec MVP | `let mut v = Vec::new(); v.push(1); v.push(2); v.len()` 返回 2 |
| 18.181 | 真实 String | `let mut s = String::from_str("a"); s.push_str("b"); s.len()` 返回 2 |
| 18.182 | format! 宏 | `format!("{}-{}", a, b)` 返回拼接 String |

### 6.3 验收测试矩阵

每个 stage 必须同时通过：
- 正向：功能可用 (exit code 正确)
- 负向：误用报错 (类型不匹配、越界、空指针)
- 集成：与现有 Option/Result/print!/match 等组合使用
- 内存：valgrind 无 leak（v0.2 P2，本阶段不强求）

---

## 7. 本 stage (18.177) 范围

### 7.1 范围声明

本 stage **仅做任务审查 + 文档同步**，不修改编译器代码。理由：

1. 任务审查是流程性活动（§17 + 用户指令触发）
2. 实现工作量大（heap alloc + Box + Vec + String + format! = 5+ stages）
3. 每个实现 stage 需独立验收 + 测试 + 打包
4. 拆分可避免单个 stage 过大（>500 LOC = L3 全流程）

### 7.2 本 stage 交付物

1. `docs/develop/v0/stage-18/stage-18.177-task-review.md` (本文件)
2. `docs/lang-design/09-stdlib.md` §3.4 MVP 偏差说明 (更新)
3. `docs/develop/v0/tech-debt-register.md` 新增 3 个 TD 条目 (更新)
4. worklog 追加本 stage 记录

### 7.3 下一 stage 启动条件

- ✅ 本任务审查完成
- ✅ 设计文档同步完成
- ✅ tech-debt-register 更新完成
- ✅ 用户确认推进方向（隐式：用户已要求"按计划推进修复"）

---

## 8. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings (无代码变更)
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend: 656 lib + 2967 integration = 3623 total, 0 failed

---

## 9. 结论

**任务审查通过** — 当前 String=&str 简写已正式记录为 TD-STRING-AS-STR-ALIAS，完整的 heap
allocation → Box → Vec → String (real) → format! 任务图已重排，下一 stage 18.178 将开始
heap allocation 基础设施实现。

**关键决策**:
1. 不接受局部修复 — heap allocation 链路必须完整实现
2. String=&str 简写作为临时过渡保留，但在 Stage 18.181 必须替换为真实实现
3. 任务图重排后阶段数从 4 增至 7，但避免了再次简写造成的累积偏差
