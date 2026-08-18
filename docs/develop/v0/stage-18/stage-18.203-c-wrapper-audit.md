# Stage 18.203 — C Wrapper Dependency Audit (设计审查)

> **Date**: 2026-08-17
> **Reviewer**: Super Z (main) — ARCH-A + REV-A + PM-A
> **Task ID**: stage18.203-audit
> **触发条例**: 用户指令 — "结合项目设计原则，是否应该过多的依赖 C wrapper，是否符合高内聚低耦合，是否通过 c wrapper 会引入额外依赖，是否会对项目最终的自举产生影响，是否是一种偷懒，当前实现中是否符合项目的真实设计要求"
> **依据**: docs/stage-committee-process.md v6.4 §1.0 §10 §11 §12 §13.4 §17.6 + docs/lang-design/07-codegen.md §4-§5 §13 + docs/lang-design/08-bootstrap-strategy.md §1-§2

## 1. 审查范围

本审计检查 Stage 18.177-18.202 引入的 C runtime helper 模式是否符合 Landin 设计原则。
覆盖以下 C 函数（src/codegen/runtime.rs）：

| C helper | Stage | 类型 |
|----------|-------|------|
| `__landin_alloc` | 18.178 | 原语 (Primitive) |
| `__landin_dealloc` | 18.178 | 原语 |
| `__landin_realloc` | 18.194 | 原语 (libc realloc wrapper) |
| `__landin_memcpy` | 18.185 | 原语 (libc memcpy wrapper) |
| `__landin_oom_abort` | 18.178 | 原语 (panic) |
| `__landin_panic_bounds_check` | design | 原语 (panic) |
| `__landin_panic_overflow` | design | 原语 (panic) |
| `__landin_panic_div_by_zero` | design | 原语 (panic) |
| `__landin_vec_push` | 18.197 | **复合 (Compound)** |
| `__landin_vec_get` | 18.200 | **复合 (Compound)** |
| `__landin_string_push_str` | 18.198 | **复合 (Compound)** |
| `__landin_format_variadic` | 18.202 | **复合 (Compound)** |

## 2. 设计文档基线（§13.1 设计对齐）

### 2.1 07-codegen.md §4-§5（明确允许的原语）

> `__landin_panic_bounds_check(index, len)` — 打印 "index out of bounds" 后 abort
> `__landin_panic_overflow(op, lhs, rhs)` — 打印溢出信息后 abort
> `__landin_panic_div_by_zero()` — 打印 "division by zero" 后 abort
> MVP 全部 panic 直接 `abort()`，不做 unwind。
>
> MVP 链接 libc 的 `malloc` / `free`：
> 编译器在 `__landin_alloc` / `__landin_dealloc` 调用中替换为用户的 allocator 方法。

### 2.2 07-codegen.md §13.2（C wrapper 仅用于 C++ ABI 互操作）

> MVP 不支持 C++ ABI（如 name mangling、vtable 布局、异常）。需通过 C wrapper 中转。

### 2.3 08-bootstrap-strategy.md §1-§2（v0.1 不自举，v0.3 自举）

> v0.1（20-40 月）：交付 stage 0 编译器（Rust 实现），可编译第三方 Landin 程序，**不要求自举**
> v0.3（31-64 月）：完成自举（stage 1 用 Landin 重写 + stage 2 验证）

### 2.4 §11.1（接口隔离核心原则）

> 项目阶段之间通过明确的数据契约交互，禁止跨阶段直接调用内部接口。
> 每个阶段（lexer → parser → HIR → MIR → typeck → borrowck → codegen）是一个"管道节点"。
> 节点之间应该：1. 高内聚；2. 低耦合；3. 可替换；4. 组合优于继承。

### 2.5 §12.1（最优 > 最小核心原则）

> 当面对"最小改动"与"最优架构"二选一时，选最优架构。
> 最小方案的"省下的工作量"是短期收益，但累积的"潜在问题复杂度"是长期负债。

## 3. 审查结论

### 3.1 "是否过多依赖 C wrapper？"

**结论：部分过度。**

**符合设计的部分（原语类，ENDORS）：**
- `__landin_alloc` / `__landin_dealloc` / `__landin_realloc` / `__landin_memcpy` — 这些是
  libc 的薄包装，设计文档 §5.2 明确说明 "MVP 链接 libc 的 malloc/free"。Landin v0.3
  自举后这些会变成 Landin 标准库的 `extern "C"` 函数声明，不需要重新实现。
- `__landin_panic_*` / `__landin_oom_abort` — 设计文档 §4 明确说明这些是 stage-0 的
  panic runtime，会在 stage 1 (Landin 自举) 重写为 Landin `panic_handler`。

**不符合设计的部分（复合类，OVERUSE — TD-C-WRAPPER-OVERUSE）：**
- `__landin_vec_push` / `__landin_vec_get` / `__landin_string_push_str` / `__landin_format_variadic` —
  这些把"复合操作逻辑"放进 C runtime，绕过了 MIR 层。

### 3.2 "是否符合高内聚低耦合？"

**结论：违反 §11 接口隔离。**

复合 C helper 违反 §11.1 的"高内聚"原则：
- Vec::push 的逻辑（grow cap + store val + inc len）应该内聚在 MIR 层（作为 intrinsic
  展开），但当前实现把这些逻辑放在 C runtime，codegen 只负责调用。
- 这导致 codegen 阶段需要"知道" `__landin_vec_push` 的 C 实现细节（参数顺序、字段偏移），
  这是 §11.3 第 3 项禁止的"下游阶段依赖上游的内部实现细节"。

具体违反点：
- `src/codegen/runtime.rs:198` `__landin_vec_push` 直接读 Vec 字段（offset 0=ptr, 8=len, 16=cap）
- `src/mir/lower/expr_variants.rs:1977` MIR lower 也读相同字段偏移 — 两处必须保持一致
- 这是隐式数据契约（不是显式 MIR 数据结构），违反 §11.5 "数据下沉"原则

### 3.3 "是否引入额外依赖？"

**结论：是。**

每新增一个复合 C helper：
1. 新增一个 C 函数（src/codegen/runtime.rs）
2. 新增一个 DefId 注册（u32::MAX - N）
3. 新增一个 fn_sigs_map 条目（src/codegen/llvm/function_sigs.rs）
4. 新增一个 variadic check 例外（src/codegen/llvm/mod.rs + aggregate.rs）
5. 新增一个 MIR intrinsic lower 函数（expr_variants.rs）

这种"5 处协调"的耦合本身就是设计 smell — 真正的最优方案是 MIR 层用现有原语
（Alloc/Copy/BinOp/Branch）组合实现，不需要新 C 函数。

### 3.4 "是否对自举产生影响？"

**结论：间接影响。**

v0.1 不要求自举（per 08-bootstrap-strategy.md §1.3），所以复合 C helper 在 v0.1 阶段不阻塞。

但 v0.3 自举时需要重写这些复合 C helper 为 Landin 实现：
- 选项 A: 用 Landin 标准库重写（如 `Vec::push` 用 `Vec::resize` + `ptr::write` 组合）
- 选项 B: MIR 层 intrinsic 展开（在 Landin 编译器内实现"Vec::push 等价 MIR 序列"）

选项 A 更符合 Landin 设计哲学（§1.3 "拒绝语言层特判"）— Vec::push 不应该是编译器特例，
而是普通标准库方法。当前 C helper 模式把 Vec::push 变成了"编译器特判的 intrinsic"，
违背了 §1.3。

### 3.5 "是否是一种偷懒？"

**结论：部分是。**

复合 C helper 模式的优势是"快速 MVP"——4 行 C 代码 + 30 行 MIR lower = 一个可工作的
intrinsic。但代价是：
- 增加了 v0.3 自举时的迁移成本（每个 C helper 要么重写为 Landin，要么展开为 MIR）
- 违反 §11 接口隔离（codegen 依赖 C runtime 内部细节）
- 违反 §1.3 "拒绝特判"（Vec::push 变成特例）
- 违反 §12.1 最优 > 最小（短期 MVP，长期负债）

按照 §12.3 "何时仍可选最小方案"：
- ✅ "最优方案依赖未就绪的前置条件" — 当前 MIR 缺少必要的 intrinsic ops
  （Alloc/Copy/Branch 组合），所以复合 C helper 是"前置未就绪"的最小方案
- ⚠️ 但必须在 worklog 和 dev-log 记录"待 v0.2/v0.3 修复根因"
- ⚠️ 必须在 gate review 列入 limitation 表

本审计文档就是该记录。

### 3.6 "当前实现中是否符合项目的真实设计要求？"

**结论：部分符合。**

符合的部分：
- 原语 C helper（alloc/dealloc/panic）完全符合 07-codegen.md §4-§5 设计
- v0.1 不自举原则允许复合 C helper 作为 MVP（per 08-bootstrap-strategy.md §1.3）

不符合的部分：
- 复合 C helper 违反 §11 接口隔离（codegen 依赖 C runtime 内部细节）
- 复合 C helper 违反 §1.3 "拒绝特判"（Vec::push 等变成特例）
- 复合 C helper 违反 §12.1 最优 > 最小（应记录为技术债）
- 当前没有 limitation 表跟踪这些 C helper 的迁移计划

## 4. 修复计划（迁移路径）

### 4.1 短期 (v0.1, 当前阶段) — 接受并记录

- 接受当前复合 C helper 作为 MVP 简化（per §12.3 第 3 项 "前置未就绪"）
- 创建 TD-C-WRAPPER-OVERUSE 跟踪（已添加到 tech-debt-register.md §2.6）
- 在每个 C helper 处添加 `// TODO: replace with MIR intrinsic (TD-C-WRAPPER-OVERUSE)` 注释

### 4.2 中期 (v0.2 Phase 2) — MIR intrinsic ops

- 添加 MIR 原语操作：`Rvalue::Alloc(size) -> ptr`, `Rvalue::Load(ptr) -> val`,
  `Statement::Store(ptr, val)`, `Terminator::Branch(cond, then, else)`
- 将复合 C helper 重写为 MIR intrinsic 展开（在 MIR lower 阶段把 Vec::call(Vec::push)
  展开为 Alloc+Copy+BinOp+Branch 的 MIR 序列）
- codegen 只翻译 MIR，不再调用复合 C helper

### 4.3 长期 (v0.3 自举) — Landin 标准库

- Vec/String 等容器在标准库中用 Landin 实现
- 编译器 intrinsic 只保留必要的原语（Alloc/Dealloc/Copy/Panic）
- 自举时这些原语作为 `extern "C"` 声明，由 stage-0 编译器生成

## 5. §13.4 J1-J6 判据检查

| # | 判据 | 当前状态 | 修复方向 |
|---|------|---------|---------|
| J1 | 架构设计对齐 | ⚠️ 复合 C helper 未在 07-codegen.md 设计 | §4.2 计划补 §14.3 "Compound ops via MIR intrinsics" 章节 |
| J2 | 单一职责 | ⚠️ runtime.rs 同时承担原语 + 复合操作 | §4.2 拆分: `runtime_primitives.rs` (原语) + MIR intrinsics (复合) |
| J3 | 单向流动 | ✅ codegen 只读 MIR，不反向修改 | 保持 |
| J4 | 编译相关表达完整 | ⚠️ Vec 字段偏移在 C + MIR lower 两处定义 | §4.2 用 MIR Place::Projection 表达字段访问 |
| J5 | 阶段划分清晰 | ⚠️ codegen 阶段依赖 runtime 阶段细节 | §4.2 移除 codegen → runtime 耦合 |
| J6 | 科学合理粒度 | ✅ runtime.rs 529 LOC，合理 | 保持 |

## 6. §6 缺陷分级

| 严重度 | 判定 |
|--------|------|
| P0 | 无（不影响 soundness，不阻塞 v0.1） |
| P1 | 无（不影响 v0.1 发布） |
| P2 | ✅ TD-C-WRAPPER-OVERUSE — 影响 v0.3 自举工作量，需 v0.2 修复 |
| P3 | 无风格问题 |

## 7. 结论与建议

1. **当前复合 C helper 模式是 v0.1 阶段合理的 MVP 简化**（per §12.3 第 3 项）
2. **但必须在 tech-debt-register 中跟踪迁移计划**（已通过 TD-C-WRAPPER-OVERUSE 完成）
3. **v0.2 Phase 2 必须启动 MIR intrinsic 重构**（避免技术债累积）
4. **v0.3 自举前必须完成迁移**（否则 stage-1 Landin 编译器无法生成等价代码）
5. **Stage 18.203 的 elem_size 统一推导修复不引入新的复合 C helper**——
   只是用 `compute_type_size_with_fallback` 替换 3 处硬编码 size 表，
   没有新增 C runtime 函数。符合"不再扩大 C wrapper 范围"原则。

## 8. 后续 stage 任务规划（更新到 §17 排版图）

```
18.201 任务审查 ✅
18.202 format! variadic ✅
18.203 elem_size 统一推导 (TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE) ✅
18.204 deep review §14.5 D1-D8 (close chain)
18.205 (新) C wrapper audit 记录到 design docs (本审计文档)
--- v0.2 Phase 2+ ---
v0.2.1 MIR intrinsic ops 设计 (Alloc/Load/Store/Branch)
v0.2.2 Vec::push MIR intrinsic 展开 (替换 __landin_vec_push)
v0.2.3 Vec::get MIR intrinsic 展开 (替换 __landin_vec_get)
v0.2.4 String::push_str MIR intrinsic 展开 (替换 __landin_string_push_str)
v0.2.5 format! variadic MIR intrinsic 展开 (替换 __landin_format_variadic)
v0.2.6 typeck generic instantiation (TD-TYPECK-GENERIC-INST)
--- v0.3 自举 ---
v0.3.x stage-1 Landin 重写（C helpers → Landin stdlib）
```
