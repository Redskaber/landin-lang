# Stage 16.76 Design v2 — Codegen Pipeline Refactoring

> **Author**: ARCH-A (Design Agent)
> **Date**: 2026-08-05
> **Version**: design-v2 (responds to review-v1)
> **Status**: ✅ Final (定稿 with limitations) — review-v2 confirmed all P1 resolved; 1 P2 (MUV-1 commit granularity) is implementation-phase concern, not design flaw.
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环)

## 0. v1 → v2 修订摘要

按 review-v1 修订建议汇总（§4）逐项处理：

| # | 优先级 | 问题 | v2 处理 |
|---|--------|------|---------|
| 1 | P1 | 修正方法数 36 → 39 | ✅ §1.1 全文 39 + 修复 mod.rs L60-62 + emitter.rs doc |
| 2 | P1 | MUV-1 增加"与 16.38 关系"+"迁移步骤" | ✅ §3.1.0 + §3.1.1 |
| 3 | P1 | MUV-3 重写为 types/layouts/places/stdlib | ✅ §3.3（含 places.rs 733 LOC） |
| 4 | P1 | CodegenError 改为 `{message, span}` | ✅ §3.4（MUV-4 已删除，理由见该节） |
| 5 | P2 | MUV-1 改为 6 子 trait | ✅ §3.1（ModuleEmitter/FunctionEmitter/ArithmeticEmitter/MemoryEmitter/AggregateEmitter/LocalStateEmitter） |
| 6 | P2 | MUV-4 改为完整改造或完全删除 | ✅ §3.4 完全删除（推迟 v0.4+） |
| 7 | P2 | MUV-3 J1 对齐重写 | ✅ §3.3 标注 07-codegen.md 章节号 |
| 8 | P2 | 修正"不破坏现有调用"声明 | ✅ §3.1.2 区分调用者 vs 实现者 |
| 9 | P3 | `fn_sigs.rs` 重命名 | ✅ §3.2 移入 `llvm/function_sigs.rs` |
| 10 | P3 | LOC 估计修正 | ✅ §4.1 附实测起止行号 |
| 11 | P3 | 修复 mod.rs L60-62 过期注释 | ✅ §3.1.4 |
| 12 | P3 | 修复 mod.rs L504-506 误导注释 | ✅ §3.2（set_fn_sigs 移入 llvm/function_sigs.rs 后注释自然消除） |

## 1. 设计目标

按用户要求"胆大心细优化重构 codegen 编译管道"，本阶段聚焦：

1. **真正抽象出 codegen**：把 `Emitter` trait（**39 methods**，2 implementations）按职责拆分为 6 个子 trait
2. **真正组织 llvm 和 text**：明确 backend 实现的边界与共享代码
3. **数据结构选型与架构设计**：审视 `EmitType`、`EmitValue` 是否最优（review-v1 §3.1 确认 EmitType 当前形态已合理，不重新设计）
4. **编译流水线组织**：审视 `run_codegen_pipeline` 的 6 步是否清晰
5. **API 接口设计**：§10 命名标准化在 codegen 的应用

### 1.1 事实修正（review-v1 P1-1）

**Emitter trait 实际方法数 = 39**（review-v1 实测，本设计核实无误）：

| 分组 | 方法数 | 方法列表 |
|------|--------|---------|
| Module-level | 5 | `emit_header`, `emit_declare`, `emit_string_global`, `emit_vtable_global`, `emit_dyn_trait_const` |
| Function scope | 30 | `emit_function_begin`, `emit_function_end`, `emit_block`, `emit_ret`, `emit_unreachable`, `emit_br`, `emit_br_cond`, `emit_switch`, `emit_const`, `emit_binop`, `emit_unop`, `emit_alloca`, `emit_store`, `emit_load`, `emit_call`, `emit_dyn_trait_method_call`, `emit_icmp`, `emit_fcmp`, `emit_and`, `emit_or`, `emit_zext`, `emit_cast`, `emit_select`, `emit_gep_field`, `emit_gep_index`, `emit_gep_index_ptr`, `emit_phi`, `emit_insertvalue`, `emit_extractvalue`, `emit_checked_binop` |
| Local state | 4 | `set_local_ptr`, `get_local_ptr`, `set_local`, `get_local` |
| **Total** | **39** | |

**实现数 = 2**：`TextEmitter`（text/mod.rs L167-815, 648 LOC）+ `LLVMSysEmitter`（llvm/mod.rs L546-1825, 1279 LOC）。

`mod.rs` L60-62 过期注释将在 MUV-1 中修复为：

```rust
//! - **Emitter trait split**: 39 methods, 2 implementations (TextEmitter + LLVMSysEmitter).
//!   Stage 16.76 split into 6 sub-traits (ModuleEmitter, FunctionEmitter,
//!   ArithmeticEmitter, MemoryEmitter, AggregateEmitter, LocalStateEmitter)
//!   per §13.4 J2 single responsibility.
```

## 2. 架构现状分析

### 2.1 当前 codegen 模块结构

```
src/codegen/
├── mod.rs               (931 LOC)  — 入口 + pipeline + drop_glue + per-function + fn_sigs_map
├── mir_translation.rs   (1144 LOC) — MIR→EmitType 翻译 + 7 个 place codegen 函数
├── emitter.rs           (490 LOC)  — Emitter trait (39 methods) + EmitType + 辅助函数
├── text/mod.rs          (841 LOC)  — TextEmitter impl (impl Emitter 块 648 LOC)
├── llvm/mod.rs          (2133 LOC) — LLVMSysEmitter impl (impl Emitter 块 1279 LOC)
├── operand.rs           (243 LOC)  — codegen_operand
├── rvalue.rs            (529 LOC)  — codegen_rvalue
├── statement.rs         (449 LOC)  — codegen_statement
├── terminator.rs        (593 LOC)  — codegen_terminator
├── dyn_trait_emit.rs    (294 LOC)  — dyn trait 文本辅助
└── trait_dispatch/                 — vtable/dynptr/orchestrator
    ├── mod.rs           (57 LOC)
    ├── vtable.rs        (349 LOC)
    ├── dynptr.rs        (268 LOC)
    └── orchestrator.rs  (415 LOC)
```

**总计**: 8735 LOC

### 2.2 已识别的架构问题

| # | 问题 | 严重度 | 来源 |
|---|------|--------|------|
| AP-1 | **Emitter trait 臃肿**：39 methods 集中在单 trait，违反 §13.4 J2 单一职责 | P2 (architecture debt) | mod.rs L60-62 注释 + Stage 16.38 历史 |
| AP-2 | **mod.rs 职责混合**：含入口+pipeline+drop_glue+per-function+helper+fn_sigs_map | P2 | 本审查 |
| AP-3 | **mir_translation.rs 过大**：1144 LOC，混合类型翻译 + 733 LOC place codegen | P2 | review-v1 P1-3 |
| AP-4 | **emit_drop_glue_functions 过长**：单函数 ~235 LOC (L256-490)，嵌套深 | P3 (readability) | mod.rs |
| AP-5 | **codegen_function 过长**：单函数 ~170 LOC (L699-872)，含 ret_ty 计算+params+loop | P3 (readability) | mod.rs |
| AP-6 | **mod.rs L60-62 + L504-506 过期/误导注释** | P3 | review-v1 P3-3, P3-4 |

### 2.3 已识别的优势（保留）

| # | 优势 | 来源 |
|---|------|------|
| ST-1 | `run_codegen_pipeline` 已经是统一入口（Stage 16.37） | mod.rs L151 |
| ST-2 | TextEmitter + LLVMSysEmitter 都实现 Emitter trait | emitter.rs L93 |
| ST-3 | §11 接口隔离已合规（codegen 是纯 MIR 消费者） | mod.rs L4-9 |
| ST-4 | trait_dispatch 已分子模块（vtable/dynptr/orchestrator） | trait_dispatch/ |
| ST-5 | codegen_{operand,rvalue,statement,terminator} 已分文件 | codegen/*.rs |

## 3. 重构方案（方案 B，6 子 trait + 4 模块拆分）

### 3.1 MUV-1: Emitter trait 拆分为 6 子 trait

#### 3.1.0 与 Stage 16.38 的关系（review-v1 P1-2）

**Stage 16.38 历史**：曾尝试 `ModuleEmitter + FunctionEmitter` 2-trait 拆分，因 `impl Emitter for TextEmitter`（648 LOC）/ `impl Emitter for LLVMSysEmitter`（1279 LOC）的方法在 impl 块中**物理交叉分布**（module-level 方法在 function-scope 方法之后），需要 ~1000 LOC 跨文件迁移，被判定为"high risk"而 defer。

**v2 与 16.38 的关系**：
- 16.38 是 2-trait 方案，因迁移成本 defer；
- v2 是 **6-trait 方案**，迁移成本 ≥ 16.38（更多切分点，~1927 LOC 重排）；
- 选 6-trait 而非 2-trait 的理由：v1 的 `ValueEmitter` 22 方法是新的"fat trait"，违反 §13.4 J2；6-trait 让每个 trait 单一职责清晰，长期收益是后续添加第三 backend 时各 trait 独立演进。
- **现在不再 defer 的理由**：(1) v0.3+ 已完成，是 §13.2 阶段切换期，允许破坏性变更；(2) Stage 16.76 是 dedicated refactoring stage，不是 feature stage，可承受 ~1927 LOC 重排；(3) 8000+ 测试作为回归守护网，可及时发现迁移错误。

#### 3.1.1 迁移步骤（review-v1 P1-2）

```text
Step 1: 创建 6 个子 trait 文件
  src/codegen/emitter/
  ├── mod.rs              — Emitter super-trait + blanket impl + re-exports
  ├── module.rs           — ModuleEmitter (5 methods)
  ├── function.rs         — FunctionEmitter (8 methods)
  ├── arithmetic.rs       — ArithmeticEmitter (11 methods)
  ├── memory.rs           — MemoryEmitter (6 methods)
  ├── aggregate.rs        — AggregateEmitter (5 methods)
  └── local_state.rs      — LocalStateEmitter (4 methods)

Step 2: 把 emitter.rs 中 39 个方法签名按职责分到 6 个子 trait 文件
  - 不移动方法体（方法体在 TextEmitter/LLVMSysEmitter 的 impl 块中）
  - 只移动 trait 定义

Step 3: 删除 text/mod.rs 中 `impl Emitter for TextEmitter`（L167-815, 648 LOC）
  - 按 6 个子 trait 重新切分为 6 个 impl 块（同一文件内）
  - 方法签名不变，只是物理位置重排

Step 4: 删除 llvm/mod.rs 中 `impl Emitter for LLVMSysEmitter`（L546-1825, 1279 LOC）
  - 同样按 6 个子 trait 切分为 6 个 impl 块

Step 5: 在 emitter/mod.rs 加入 blanket impl
  impl<T> Emitter for T where
      T: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
       + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}

Step 6: 修复 mod.rs L60-62 过期注释（review-v1 P3-3）

Step 7: 验证 14 处 `&mut dyn Emitter` 调用点全部编译通过
  - mod.rs (5 处)
  - statement.rs, rvalue.rs, terminator.rs, operand.rs (5 处)
  - mir_translation.rs (4 处)
  - trait_dispatch/ (6 处)

Step 8: 为每个子 trait 增加 compile-time trait satisfaction 测试
  let _: &dyn ModuleEmitter = &TextEmitter::new();
  let _: &dyn FunctionEmitter = &TextEmitter::new();
  // ... 6 个子 trait × 2 backend = 12 个类型断言
```

#### 3.1.2 破坏性变更声明（review-v1 P2-4）

**对调用者（callers）**：不破坏。14 处 `&mut dyn Emitter` 调用点全部继续工作（super-trait 模式保 `dyn Emitter` 可用）。

**对实现者（implementers）**：**破坏性变更**。`lib.rs` L425-438 把 `Emitter` 作为 public API re-export：
```rust
pub use codegen::{..., Emitter, ...};
```
任何外部 `impl Emitter for MyBackend` 在 v2 拆分后会编译失败——必须改为 6 个独立 impl 块。

Per §13.3 早期阶段允许破坏性变更，但需在 `RELEASE_NOTES.md` 标注：
> **breaking**: `Emitter` trait split into 6 sub-traits (ModuleEmitter + FunctionEmitter + ArithmeticEmitter + MemoryEmitter + AggregateEmitter + LocalStateEmitter). External backends must now implement 6 sub-traits instead of 1 Emitter trait. The blanket `impl<T: ...> Emitter for T` preserves `dyn Emitter` compatibility for callers.

#### 3.1.3 6 子 trait 定义

```rust
// src/codegen/emitter/module.rs
pub trait ModuleEmitter {
    fn emit_header(&mut self);
    fn emit_declare(&mut self, signature: &str);
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;
    fn emit_vtable_global(&mut self, name: &str, methods: &[String]) -> EmitValue;
    fn emit_dyn_trait_const(&mut self, name: &str, data: &str, vtable: &str) -> EmitValue;
}

// src/codegen/emitter/function.rs
pub trait FunctionEmitter {
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType);
    fn emit_function_end(&mut self);
    fn emit_block(&mut self, label: &str);
    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>);
    fn emit_unreachable(&mut self);
    fn emit_br(&mut self, label: &str);
    fn emit_br_cond(&mut self, cond: &EmitValue, then_l: &str, else_l: &str);
    fn emit_switch(&mut self, discr: &EmitValue, ty: &EmitType,
                   cases: &[(i128, String)], default: &str);
}

// src/codegen/emitter/arithmetic.rs
pub trait ArithmeticEmitter {
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue;
    fn emit_binop(&mut self, op: BinOp, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue;
    fn emit_icmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_fcmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;
    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;
    fn emit_select(&mut self, ty: &EmitType, cond: &EmitValue,
                   true_val: &EmitValue, false_val: &EmitValue) -> EmitValue;
    fn emit_checked_binop(&mut self, op: BinOp, ty: &EmitType,
                          lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
}

// src/codegen/emitter/memory.rs
pub trait MemoryEmitter {
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue;
    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue);
    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue;
    fn emit_gep_field(&mut self, base: &EmitValue, struct_ty: &EmitType, field_index: u32) -> EmitValue;
    fn emit_gep_index(&mut self, base: &EmitValue, array_ty: &EmitType, index: &EmitValue) -> EmitValue;
    fn emit_gep_index_ptr(&mut self, base: &EmitValue, elem_ty: &EmitType, index: &EmitValue) -> EmitValue;
}

// src/codegen/emitter/aggregate.rs
pub trait AggregateEmitter {
    fn emit_phi(&mut self, ty: &EmitType, incoming: &[(EmitValue, String)]) -> EmitValue;
    fn emit_insertvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue,
                        val_ty: &EmitType, val: &EmitValue, index: u32) -> EmitValue;
    fn emit_extractvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue, index: u32) -> EmitValue;
    fn emit_call(&mut self, fn_name: &str, args: &[(EmitType, &EmitValue)],
                 ret_ty: &EmitType) -> EmitValue;
    fn emit_dyn_trait_method_call(&mut self, dynptr_symbol: &str, slot_index: u32,
                                  args: &[(EmitType, &EmitValue)],
                                  ret_ty: &EmitType) -> EmitValue;
}

// src/codegen/emitter/local_state.rs
pub trait LocalStateEmitter {
    fn set_local_ptr(&mut self, id: u32, ptr: EmitValue);
    fn get_local_ptr(&self, id: u32) -> Option<&EmitValue>;
    fn set_local(&mut self, id: u32, val: EmitValue);
    fn get_local(&self, id: u32) -> Option<&EmitValue>;
}

// src/codegen/emitter/mod.rs
pub trait Emitter: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
                 + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}
impl<T> Emitter for T where
    T: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
     + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}

// Re-exports for backward compatibility
pub use aggregate::*;
pub use arithmetic::*;
pub use function::*;
pub use local_state::*;
pub use memory::*;
pub use module::*;
```

#### 3.1.4 J1-J6 检查

| # | 判据 | 通过条件 | v2 满足情况 |
|---|------|---------|-------------|
| J1 | 架构设计对齐 | 与 07-codegen.md §4 (MIR→LLVM IR 映射) 一致 | ✅ ModuleEmitter ↔ §4 模块级，FunctionEmitter ↔ §4 函数级，ArithmeticEmitter/MemoryEmitter/AggregateEmitter ↔ §4 各类 IR 指令 |
| J2 | 单一职责 | 每个 trait 用一句话能描述 | ✅ ModuleEmitter="module-level globals & declares"，FunctionEmitter="function scope & control flow"，ArithmeticEmitter="compute value from operands"，MemoryEmitter="stack & pointer arithmetic"，AggregateEmitter="aggregate construction & calls"，LocalStateEmitter="local value/ptr mapping" |
| J3 | 单向流动 | 子 trait 间无环依赖 | ✅ 6 子 trait 互相独立，Emitter super-trait 依赖所有子 trait |
| J4 | 编译相关表达完整 | Emission 概念完整保留 | ✅ 39 methods 100% 归属 6 子 trait |
| J5 | 阶段划分清晰 | 仍在 codegen 阶段 | ✅ 不破坏 §11 |
| J6 | 科学合理粒度 | 各 trait 方法数合理 | ✅ 5/8/11/6/5/4，最大 11 (ArithmeticEmitter) 远低于"fat trait"阈值 |

### 3.2 MUV-2: mod.rs 职责拆分

把 mod.rs 931 LOC 拆为：

```
src/codegen/
├── mod.rs               (~80 LOC)  — 入口 + re-exports (实测 mod.rs L1-128 + L585-612 ≈ 80 LOC 实质内容)
├── pipeline.rs          (~70 LOC)  — run_codegen_pipeline (实测 L151-217 = 67 LOC)
├── function.rs          (~316 LOC) — codegen_function (174 LOC L699-872) + codegen_from_mir (27 LOC L586-612) + codegen_synthesized_closure_functions (62 LOC L634-695) + get_call_dest_type (53 LOC L879-932)
├── drop_glue.rs         (~235 LOC) — emit_drop_glue_functions (L256-490)
└── llvm/function_sigs.rs (~47 LOC) — build_fn_sigs_map (L524-570, LLVM-only)
```

**J1-J6 检查**：
- J1 ✅ 与 07-codegen.md §1 总体流程对齐（pipeline ↔ §1 流程，function ↔ §4 函数级，drop_glue ↔ §6 Drop glue）
- J2 ✅ pipeline/function/drop_glue/function_sigs 各自单一职责
- J3 ✅ mod.rs → pipeline → function → drop_glue（单向）
- J4 ✅ 每个 codegen 概念在模块内完整
- J5 ✅ 仍在 codegen 阶段
- J6 ✅ 各文件 LOC 合理（最大 function.rs 316 LOC，远低于 1500 阈值）

**`fn_sigs.rs` 重命名说明**（review-v1 P3-1）：移入 `llvm/function_sigs.rs`，LLVM-only 性质由目录结构表达，文件名用完整单词 `function_sigs` 符合 §10 命名标准。

**`set_fn_sigs` 注释修复**（review-v1 P3-4）：把 `set_fn_sigs` 移入 `llvm/function_sigs.rs` 后，`mod.rs` L504-506 的"trait-based hook"误导注释自然消除——改为"LLVM-specific pre-pipeline setup (LLVMSysEmitter only); the pipeline itself remains backend-agnostic via &mut dyn Emitter"。

### 3.3 MUV-3: mir_translation.rs 拆分为 4 模块

**review-v1 P1-3 修正**：v1 完全遗漏了 733 LOC 的 place codegen 逻辑。v2 重写为按 07-codegen.md 章节对齐的 4 模块拆分：

```
src/codegen/mir_translation/
├── mod.rs               (~80 LOC)  — re-exports + 共享 imports
├── types.rs             (~250 LOC) — mir_type_to_emit_type_with_layouts (L50-201, 151 LOC)
│                                    + mir_type_to_emit_type_with_layouts_and_mono (L202-280, 79 LOC)
│                                    （对应 07-codegen.md §2.1-§2.3：基本类型 + 复合类型 + Layout 计算）
├── layouts.rs           (~80 LOC)  — adt_layout_to_emit_type (L281-344, 64 LOC)
│                                    + 未来 niche 优化扩展点（§2.4）
│                                    （对应 07-codegen.md §2.3-§2.4）
├── places.rs            (~780 LOC) — detect_place_storage_type (L368-469, 102 LOC)
│                                    + detect_place_type (L470-572, 103 LOC)
│                                    + compute_place_address (L573-775, 203 LOC)
│                                    + unwrap_fat_ptr_for_index (L776-798, 23 LOC)
│                                    + codegen_place_load_typed (L799-1082, 284 LOC)
│                                    + codegen_place_load (L1083-1100, 18 LOC)
│                                    + detect_operand_type (L1101-1144, 44 LOC)
│                                    （对应 07-codegen.md §4.4 Place 投影映射）
└── stdlib.rs            (~30 LOC)  — stdlib_type_kind_to_emit_type (L345-367, 23 LOC)
                                     （跨章节辅助：stdlib type kind → EmitType）
```

**J1-J6 检查**（review-v1 P2-3 修正）：

| # | 判据 | v2 满足情况 |
|---|------|-------------|
| J1 | 架构设计对齐 | ✅ types.rs ↔ 07-codegen.md §2.1-§2.3，layouts.rs ↔ §2.3-§2.4，places.rs ↔ §4.4，stdlib.rs 跨章节辅助——每个文件对应明确章节号 |
| J2 | 单一职责 | ✅ types="MIR Ty → EmitType"，layouts="AdtLayout → EmitType"，places="Place 投影地址计算 + load"，stdlib="StdlibTypeKind → EmitType" |
| J3 | 单向流动 | ✅ mod.rs 依赖所有子模块，子模块间无环 |
| J4 | 编译相关表达完整 | ✅ 类型翻译 + place codegen 概念完整保留 |
| J5 | 阶段划分清晰 | ✅ 仍在 codegen 阶段 |
| J6 | 科学合理粒度 | ✅ 250/80/780/30 LOC，最大 places.rs 780 LOC（合理，因 place codegen 是单一职责的完整集合） |

### 3.4 MUV-4: ~~CodegenError 类型~~ — 完全删除，推迟 v0.4+

**review-v1 P2-2 修正**：v1 的"引入类型但不改造路径"是半成品，违反 §13.3 第 5 条"一步到位"。

**v2 决定**：完全删除 MUV-4，把 CodegenError 推迟到 v0.4+ 阶段。

**理由**：
1. codegen 路径中 ~40 处 `unwrap()` 集中在 `llvm/mod.rs` CString 构造——改造工作量小但需要修改 `run_codegen_pipeline` 返回类型 + `codegen_crate` / `codegen_crate_to_module` 公开 API 签名，影响面广。
2. 当前 codegen panic 路径已存在 16+ stage 未引发生产问题（Landin 标识符不含 NUL 字节），无 soundness 风险。
3. 本阶段聚焦 MUV-1/2/3 三类结构性重构，已完成 ~3000 LOC 重排（MUV-1 1927 LOC + MUV-2 730 LOC + MUV-3 1140 LOC），不宜再叠加错误系统改造。
4. CodegenError 改造应作为 v0.4+ 的独立 stage，包含：(a) `CodegenError { message: String, span: Span }` 类型（符合 §10.1.8），(b) `run_codegen_pipeline` 返回 `CodegenResult<()>`，(c) ~40 处 unwrap 改造，(d) 公开 API 签名变更 + RELEASE_NOTES 标注。

**review-v1 P1-4 处理**：MUV-4 删除后，CodegenError 字段形态问题自动消解（不在本阶段引入）。

## 4. 重构执行计划

### 4.1 MUV 顺序（按风险递增，先低后高）

| MUV | 估计 LOC 变动 | 风险 | 实测依据 |
|-----|--------------|------|---------|
| MUV-3 mir_translation 拆分 | ~1140 LOC 移动 | 低（纯文件移动，无 trait 改动） | L50-1144 |
| MUV-2 mod.rs 拆分 | ~730 LOC 移动 | 低（纯文件移动，无 trait 改动） | L151-217, L256-490, L524-570, L586-612, L634-695, L699-872, L879-932 |
| MUV-1 Emitter trait 拆分 | ~1927 LOC 重排 + ~200 LOC 新增 | 中（trait 重排，影响 14 处调用点） | text/mod.rs L167-815 (648 LOC) + llvm/mod.rs L546-1825 (1279 LOC) |

**为什么这个顺序**：
- MUV-3 先做：纯文件移动，无逻辑改动，最低风险；完成后 mir_translation.rs 从 1144 LOC 降到 mod.rs ~80 LOC + 4 子模块。
- MUV-2 次做：纯文件移动，无逻辑改动，低风险；完成后 mod.rs 从 931 LOC 降到 ~80 LOC + 4 子模块。
- MUV-1 最后做：trait 重排，需删除 2 个 manual impl 块（1927 LOC）并按 6 子 trait 重新切分；前两个 MUV 完成后，mod.rs 已是 ~80 LOC 干净入口，trait 拆分影响面清晰。

### 4.2 每个 MUV 的验收

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- worklog 记录 ✅

### 4.3 回滚策略

- 每个 MUV 单独 commit，便于 git revert
- 备份当前 codegen/ 到 `/tmp/codegen-backup-v0.261.0/`（防意外）

## 5. 与 §15 项目图管理同步

重构后更新：
- `docs/graph/stage/stage-3/codegen-data-flow.md` — 新模块结构图（版本 v2.0）
- `docs/graph/design/08-codegen-flow.md` — 设计层 codegen 流程图（更新版本号）

## 6. 与 §14.5 深度审查的关系

本重构计划本身是 §14.5 D1（架构健康度）+ D5（设计合理性）的行动项。完成后：
- D1 改善：Emitter trait 不再臃肿，6 子 trait 单一职责清晰
- D5 改善：mod.rs 不再混合职责，mir_translation.rs 拆为 4 模块按 07-codegen.md 章节对齐

## 7. 风险盲点（review-v1 §1 R-1 至 R-5 全部识别）

| # | 风险 | v2 缓解措施 |
|---|------|------------|
| R-1 | Emitter 是 public API（lib.rs L437），拆分对外部实现者是破坏性变更 | §3.1.2 明确声明 breaking change，RELEASE_NOTES.md 标注 |
| R-2 | blanket impl 与现有 manual impl 冲突 | §3.1.1 Step 3-4：先删除 manual impl 块，再加 blanket impl |
| R-3 | `dyn Emitter` 在 14 处调用点使用 | §3.1.1 Step 7：逐一编译验证；super-trait 模式保 dyn 兼容（所有子 trait object-safe） |
| R-4 | Stage 16.38 已留下"documentation groups"妥协方案 | §3.1.0 说明现在不再 defer 的 3 个理由 |
| R-5 | `set_fn_sigs` 是 LLVM-specific inherent method，非 trait hook | §3.2：移入 `llvm/function_sigs.rs`，mod.rs L504-506 注释改为"LLVM-specific pre-pipeline setup" |

## 8. 待 Review Agent (REV-A) 第 2 轮审查事项

请 REV-A 重点审查 v2 是否：

1. **P1 全部修复**：4 个 P1 缺陷是否在 v2 中完全解决？
2. **P2-1 (6 子 trait) 是否合理**：6 子 trait 划分是否真的符合 §13.4 J2？是否有更好的划分？
3. **P2-2 (MUV-4 删除) 是否可接受**：完全删除 MUV-4 推迟 v0.4+ 是否符合 §12 最优 > 最小？
4. **P1-3 (places.rs 780 LOC) 是否过大**：单一文件 780 LOC 是否违反 §13.4 J6？是否应进一步拆分？
5. **MUV 执行顺序是否合理**：先 MUV-3 → MUV-2 → MUV-1 的递增风险顺序是否最优？
6. **R-1 至 R-5 缓解措施是否充分**：每个风险的缓解措施是否可执行？
7. **是否有新引入的设计缺陷**：v2 修订过程中是否引入了 v1 没有的新问题？
