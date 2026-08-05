# Stage 16.76 Design Review v1 — Codegen Pipeline Refactoring

> **Author**: REV-A (Review Agent via Plan subagent)
> **Date**: 2026-08-05
> **Version**: review-v1
> **Status**: Complete
> **Reviewed**: design-v1 by ARCH-A
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环) + §6 (缺陷分级) + §10/§11/§12/§13.4

---

## 1. 总体评估

**总体合理性**：方案 B（中度重构）的方向正确——确实需要解决 `Emitter` trait 臃肿、`mod.rs` 职责混合、`mir_translation.rs` 过大这三类架构债。4 个 MUV 的拆分粒度（trait 拆 / mod.rs 拆 / mir_translation 拆 / 错误类型）在概念层面是合理的。

**是否最优**：**否**。方案 B 在三个关键 MUV 上存在设计缺陷：

1. **MUV-1 Emitter trait 拆分**：未引用 Stage 16.38 已有的失败尝试（`docs/develop/v0/stage-16/stage-16.38-emitter-trait-split-attempt.md`），未说明 v1 如何克服 16.38 已识别的"~1000 LOC 跨文件方法迁移"阻断点；`ValueEmitter` 单 trait 容纳 22 个方法，比原 trait 更臃肿，违反 §13.4 J2 单一职责。
2. **MUV-3 mir_translation 拆分**：设计显然未实际通读 `mir_translation.rs`——该文件 1144 LOC 中有 **733 LOC（64%）是 place codegen 逻辑**（7 个函数：`detect_place_storage_type`/`detect_place_type`/`compute_place_address`/`unwrap_fat_ptr_for_index`/`codegen_place_load_typed`/`codegen_place_load`/`detect_operand_type`），而设计提议的 `types.rs`/`layouts.rs`/`stdlib.rs` 三个模块合起来只覆盖 ~317 LOC（28%），剩余 733 LOC 无家可归。
3. **MUV-4 CodegenError**：只引入类型不改造路径，违反 §13.3 第 5 条"一步到位"，制造死代码（§1.0 原則 5）；同时 `CodegenError` 字段形态违反 §10.1.8（未用 `Span`，自造 `CodegenLocation`）。

**风险识别充分性**：**不充分**。设计 §7 列出的 7 项待审查事项体现了一定的自省，但未识别以下 5 个隐藏风险：

- R-1：`Emitter` trait 在 `lib.rs` L437 是 **public API**（"third-party LLVM-IR backends can implement `Emitter`"），拆分对外部实现者是破坏性变更，但设计声称"不破坏现有调用"——只对调用者成立，对实现者不成立。
- R-2：blanke impl `impl<T> Emitter for T where T: ...` 与现有 `impl Emitter for TextEmitter` / `impl Emitter for LLVMSysEmitter` **会冲突**——必须删除两个手动 impl 块并按 4 个子 trait 重新切分，这是 MUV-1 真正的工作量（~1279 LOC 重排 in `llvm/mod.rs` + ~648 LOC in `text/mod.rs`）。
- R-3：`dyn Emitter` 在 14 个调用点使用（mod.rs 5 处、statement/rvalue/terminator/operand 5 处、mir_translation 4 处、trait_dispatch 6 处）——super-trait 模式可保 dyn 兼容，但需逐一编译验证。
- R-4：Stage 16.38 已留下"documentation groups"作为妥协方案，emitter.rs L82-91 明确记录"physical split is deferred"——v1 设计未引用此历史决策，未说明为何现在不再 defer。
- R-5：`set_fn_sigs`（mod.rs L512）是 `LLVMSysEmitter` 的具体类型方法，不是 trait 方法——mod.rs L505 注释"trait-based hook"是误导性遗留注释，设计 MUV-2 沿用 `fn_sigs.rs` 命名但未澄清其 LLVM-only 性质。

**结论**：v1 设计**未通过审查**，需 v2 修订。具体缺陷与校准建议见 §2、§3。

---

## 2. 缺陷清单 (按 §6 分级)

### P0 设计缺陷（必须修复 — 会导致编译器 panic / soundness hole / 数据丢失）

**无。**

本次设计为纯结构性重构，不涉及类型推导、借用检查、代码生成语义，无 soundness 风险。所有缺陷集中在 API 设计 / 接口合规 / 设计完整性层面，归入 P1/P2/P3。

---

### P1 设计缺陷（必须修复 — API 错误 / 接口违反 §11 / 命名违反 §10）

#### P1-1: Emitter trait 方法数事实错误（36 → 39）

**位置**：design-v1 §1 第 13 行、§2.2 AP-1 表格、§3.2 MUV-1 代码块上方注释、emitter.rs L60-62 遗留注释。

**事实核查**：
```
emitter.rs L97-279 实际方法数 = 39
  Module-level  : 5 (emit_header/emit_declare/emit_string_global/emit_vtable_global/emit_dyn_trait_const)
  Function scope: 30 (emit_function_begin..emit_checked_binop)
  Local state   : 4 (set_local_ptr/get_local_ptr/set_local/get_local)
  Total         = 39
```

设计文档 4 处声称"36 methods"，与实际计数 **39** 不符。该错误继承自 `mod.rs` L60 的过期注释（"Emitter trait bloat: 36 methods, 1 implementation"），该注释本身还错误声称"1 implementation"（实际为 2：TextEmitter + LLVMSysEmitter）。

Stage 16.38 文档 (`stage-16.38-emitter-trait-split-attempt.md` §2) 已正确记录"39 methods"。

**校准建议**：
- v2 设计稿、`mod.rs` L60-62 注释、emitter.rs L60 doc-comment 三处统一改为 **"39 methods"**。
- `mod.rs` L60 同步修正"1 implementation" → "2 implementations (TextEmitter + LLVMSysEmitter)"。
- 设计稿在引用方法数时，需同时给出按职责分组的明细（5 + 30 + 4 = 39），避免再次出现笼统数字误差。

---

#### P1-2: MUV-1 未引用 Stage 16.38 失败尝试，未说明如何克服阻断点

**位置**：design-v1 §3.2 MUV-1（L81-127）、§7.6 风险盲点。

**事实核查**：Stage 16.38 已尝试过 `ModuleEmitter + FunctionEmitter` super-trait 拆分，明确记录阻断原因为：

> "Rust does not allow multiple `impl` blocks for the same trait on the same type. The current impl blocks have module-level and function-scoped methods interleaved (e.g., `emit_string_global` appears after `emit_checked_binop` in both `text/mod.rs` and `llvm/mod.rs`). To split, all module-level methods would need to be physically moved to a contiguous block, and all function-scoped methods to another. This is a ~1000-line code movement across two files, with high risk of introducing bugs."

`text/mod.rs` 实测：`impl Emitter for TextEmitter` 块（L167-815）中，`emit_string_global`（L724）/`emit_vtable_global`（L758）/`emit_dyn_trait_const`（L778）位于 `emit_checked_binop`（L677）之后，确实交叉分布。`llvm/mod.rs` 同样：`emit_string_global`（L1636）等位于 `emit_checked_binop`（L1548）之后。

设计 v1 §3.2 MUV-1 直接给出 `pub trait Emitter: ModuleEmitter + FunctionEmitter + ValueEmitter + LocalStateEmitter {}` + `impl<T> Emitter for T where T: ...` 的代码片段，但：

1. 未引用 Stage 16.38 文档——读者无法知道这是一次"重启"而非"首次"尝试。
2. 未说明 v2 如何克服 16.38 的阻断点——尤其未说明必须**删除现有 `impl Emitter for TextEmitter`（648 LOC）和 `impl Emitter for LLVMSysEmitter`（1279 LOC）两个块，按 4 子 trait 重新切分**。
3. 未说明 v1 引入 `ValueEmitter`/`LocalStateEmitter`（vs 16.38 的 2-trait 方案）如何降低迁移成本——实际上 4-trait 方案比 2-trait 切分更细，迁移工作量更大，不是更小。

**校准建议**：
- v2 §3.2 MUV-1 增加小节"**§3.2.0 与 Stage 16.38 的关系**"，明确说明：
  - 16.38 是 2-trait 方案（ModuleEmitter + FunctionEmitter），因 impl 块迁移成本阻断；
  - v1 是 4-trait 方案，工作量 ≥ 16.38（更多切分点），但长期收益是单一职责更清晰；
  - 选 4-trait 而非 2-trait 的理由（如"ValueEmitter 22 方法需要进一步细分以利于未来加第三 backend"）。
- v2 §3.2 MUV-1 增加小节"**§3.2.1 迁移步骤**"，列出 5 步：
  1. 创建 `emitter/{mod, module, function, value, local_state}.rs` 5 文件；
  2. 把 `emitter.rs` 中 39 个方法签名按职责分到 4 个子 trait 文件；
  3. **删除** `text/mod.rs` 中 `impl Emitter for TextEmitter`（L167-815），按 4 子 trait 切成 4 个 impl 块（同一文件内）；
  4. **删除** `llvm/mod.rs` 中 `impl Emitter for LLVMSysEmitter`（L546-1825），同样切 4 块；
  5. 在 `emitter/mod.rs` 加入 blanket impl `impl<T: ModuleEmitter + FunctionEmitter + ValueEmitter + LocalStateEmitter> Emitter for T {}`。
- v2 §4.1 表格更新 MUV-1 估计 LOC：从"~600 LOC 移动 + ~150 LOC 新增"改为"~1927 LOC 重排（text 648 + llvm 1279）+ ~150 LOC 新增（4 个子 trait 定义 + blanket impl）"。

---

#### P1-3: MUV-3 mir_translation 拆分方案严重失实（733 LOC place codegen 无家可归）

**位置**：design-v1 §3.2 MUV-3（L158-176）。

**事实核查**：通读 `mir_translation.rs`（1144 LOC），实际函数清单：

| # | 函数 | 起止行 | LOC | 设计归属 | 实际归属 |
|---|------|--------|-----|---------|---------|
| 1 | `mir_type_to_emit_type_with_layouts` | L50-201 | 151 | types.rs | types.rs ✓ |
| 2 | `mir_type_to_emit_type_with_layouts_and_mono` | L202-280 | 79 | types.rs | types.rs ✓ |
| 3 | `adt_layout_to_emit_type` | L281-344 | 64 | layouts.rs | layouts.rs ✓ |
| 4 | `stdlib_type_kind_to_emit_type` | L345-367 | 23 | stdlib.rs | stdlib.rs ✓ |
| 5 | `detect_place_storage_type` | L368-469 | 102 | **未指定** | places.rs |
| 6 | `detect_place_type` | L470-572 | 103 | **未指定** | places.rs |
| 7 | `compute_place_address` | L573-775 | 203 | **未指定** | places.rs |
| 8 | `unwrap_fat_ptr_for_index` | L776-798 | 23 | **未指定** | places.rs |
| 9 | `codegen_place_load_typed` | L799-1082 | 284 | **未指定** | places.rs |
| 10 | `codegen_place_load` | L1083-1100 | 18 | **未指定** | places.rs |
| 11 | `detect_operand_type` | L1101-1144 | 44 | **未指定** | places.rs |

合计：
- 设计覆盖（types.rs + layouts.rs + stdlib.rs）= 151 + 79 + 64 + 23 = **317 LOC（27.7%）**
- 设计未覆盖（函数 5-11，全部是 place codegen）= 102 + 103 + 203 + 23 + 284 + 18 + 44 = **777 LOC（67.9%）**
- 剩余 ~50 LOC 是 imports/tests/header

设计的 LOC 估计也全部偏离实际：
| 模块 | 设计估计 | 实际 | 偏差 |
|------|---------|------|------|
| types.rs | ~400 LOC | 230 LOC (151+79) | +74% 高估 |
| layouts.rs | ~300 LOC | 64 LOC | +369% 高估 |
| stdlib.rs | ~350 LOC | 23 LOC | +1422% 高估 |
| **未指定 places** | — | 777 LOC | 完全遗漏 |

这表明设计 Agent **未实际通读 `mir_translation.rs` 的函数清单**，仅凭文件名"mir_translation"推测内容为"类型翻译"。

**校准建议**：v2 §3.2 MUV-3 重写为按 07-codegen.md 设计文档对齐的 4 模块拆分：

```
src/codegen/mir_translation/
├── mod.rs               (~80 LOC)  — re-exports + 共享 helper
├── types.rs             (~250 LOC) — mir_type_to_emit_type_with_layouts[_and_mono]
                                    （对应 07-codegen.md §2.1-§2.3）
├── layouts.rs           (~80 LOC)  — adt_layout_to_emit_type + 未来 niche 优化
                                    （对应 07-codegen.md §2.3-§2.4）
├── places.rs            (~780 LOC) — detect_place_* / compute_place_address /
                                    codegen_place_load[_typed] / unwrap_fat_ptr_for_index /
                                    detect_operand_type
                                    （对应 07-codegen.md §4.4 Place 投影映射）
└── stdlib.rs            (~30 LOC)  — stdlib_type_kind_to_emit_type
```

J1 对齐：types.rs ↔ §2.1-§2.3、layouts.rs ↔ §2.3-§2.4、places.rs ↔ §4.4、stdlib.rs 跨章节辅助。这是真正的"按设计文档章节划分"，而非 v1 的"按文件名推测"。

---

#### P1-4: CodegenError 字段形态违反 §10.1.8

**位置**：design-v1 §3.2 MUV-4（L178-204）。

**事实核查**：§10.1.8 明确规定：

> **错误类型**：所有错误类型使用 `Error` 后缀（`LexError`、`ParseError`、`LowerError`、`ResolveError`、`TypeError`、`BorrowError`）。结构共享 `{ message: String, span: Span }` 最小形态。

设计 v1 提议：

```rust
pub struct CodegenError {
    pub message: String,
    pub location: Option<CodegenLocation>,  // ❌ 自造 CodegenLocation，非 Span
}

pub struct CodegenLocation {
    pub fn_name: Option<String>,    // ❌ 堆分配 String，非 Span
    pub bb_idx: Option<usize>,
    pub stmt_idx: Option<usize>,
}
```

违反点：
1. 未使用项目标准 `Span` 类型（`crate::session::Span`），自造 `CodegenLocation`——违反 DRY（§10.1.5 单一真理源）和最小形态（§10.1.8）。
2. `location: Option<CodegenLocation>` 用 `Option` 包裹——`Span` 本身已有 `Span::DUMMY` 表示"无位置"，不需要 Option。
3. `fn_name: Option<String>` 用堆 String 表示函数名——应使用 `DefId` 或 `Span`，避免堆分配且能与 HIR/MIR 错误关联。
4. 字段命名 `bb_idx`/`stmt_idx` 不符合 §10.1.7 的命名前缀规范（无 `code_`/`emit_`/`check_` 等动词前缀，但这些是字段不是函数，可以接受；但整体结构偏离标准）。

**校准建议**：v2 MUV-4 改为符合 §10.1.8 的最小形态：

```rust
// src/codegen/error.rs
use crate::session::Span;

pub struct CodegenError {
    pub message: String,
    pub span: Span,
}

pub type CodegenResult<T> = Result<T, CodegenError>;

impl CodegenError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span }
    }
}
```

如果确实需要 codegen 特有的位置信息（bb_idx/stmt_idx），应作为 `Span` 的扩展（在 `session::Span` 上加 optional 的 codegen 层 metadata），而非在错误类型上自造结构。

---

### P2 设计缺陷（优先修复 — 边界条件 / 性能问题）

#### P2-1: ValueEmitter 22 方法违反 §13.4 J2 单一职责

**位置**：design-v1 §3.2 MUV-1 ValueEmitter 定义（L106-115）。

**事实核查**：设计 v1 的 ValueEmitter 实际包含以下方法（设计稿仅列 7 个 + "其他 value emission methods" 注释，但补全后是 22 个）：

```
emit_const, emit_binop, emit_unop,                       // 3 算术
emit_icmp, emit_fcmp,                                     // 2 比较
emit_and, emit_or, emit_zext, emit_cast, emit_select,    // 5 位/转换
emit_alloca, emit_store, emit_load,                       // 3 内存
emit_gep_field, emit_gep_index, emit_gep_index_ptr,       // 3 GEP
emit_phi,                                                 // 1 PHI
emit_insertvalue, emit_extractvalue,                      // 2 聚合
emit_call, emit_dyn_trait_method_call,                    // 2 调用
emit_checked_binop                                        // 1 checked
合计 22 方法
```

ValueEmitter 单 trait 容纳 22 方法，**比原 Emitter（39）小不了多少**，且混合了 5 种不同 LLVM IR 类别（算术/内存/GEP/聚合/调用）。这违反 §13.4 J2 单一职责——拆分的目的是让每个 trait 承担"且仅承担一个明确的职责（用一句话能描述）"。

"ValueEmitter emits values"是一句空话——任何 IR 指令都产生 value。真正的单一职责应该按 LLVM IR 类别切分。

**校准建议**：v2 MUV-1 改为 6 子 trait 拆分（仍保 `Emitter` super-trait 兼容）：

```rust
pub trait ModuleEmitter { /* 5: emit_header, emit_declare, emit_string_global,
                             emit_vtable_global, emit_dyn_trait_const */ }

pub trait FunctionEmitter { /* 8: emit_function_begin, emit_function_end, emit_block,
                               emit_ret, emit_unreachable, emit_br, emit_br_cond,
                               emit_switch */ }

pub trait ArithmeticEmitter { /* 11: emit_const, emit_binop, emit_unop, emit_icmp,
                                 emit_fcmp, emit_and, emit_or, emit_zext, emit_cast,
                                 emit_select, emit_checked_binop */ }

pub trait MemoryEmitter { /* 6: emit_alloca, emit_store, emit_load,
                             emit_gep_field, emit_gep_index, emit_gep_index_ptr */ }

pub trait AggregateEmitter { /* 5: emit_phi, emit_insertvalue, emit_extractvalue,
                                emit_call, emit_dyn_trait_method_call */ }

pub trait LocalStateEmitter { /* 4: set_local_ptr, get_local_ptr, set_local, get_local */ }

pub trait Emitter: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
                  + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}
impl<T> Emitter for T where T: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
                                  + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}
```

各 trait 单一职责清晰：ModuleEmitter="module-level globals & declares"，ArithmeticEmitter="compute a value from operands"，MemoryEmitter="stack & pointer arithmetic"，AggregateEmitter="aggregate construction & calls"。5 + 8 + 11 + 6 + 5 + 4 = 39 ✓。

如果设计 Agent 认为 6 trait 过细，可合并为 5 trait（把 AggregateEmitter 并入 ArithmeticEmitter，因为 `phi`/`insertvalue` 也是"产生 value"）——但 ValueEmitter 22 方案的中间态不可接受。

---

#### P2-2: MUV-4 是半成品，违反 §13.3 第 5 条"一步到位"

**位置**：design-v1 §3.2 MUV-4 注释（L196）："本 MUV 只引入类型，不立即改造所有 panic 路径。改造留待下一阶段（避免本阶段过大）。"

**事实核查**：§13.3 第 5 条明确要求"一步到位：不要分多步'渐进迁移'——在早期阶段，一步到位的重构比渐进迁移更高效"。MUV-4 引入 `CodegenError` 类型但不改造任何 panic 路径，结果是：

1. `CodegenError` 类型存在但无任何调用方——是死代码，违反 §1.0 原則 5"去除兼容思维"。
2. 下一阶段还需做改造工作，MUV-4 只是把工作量延后，不减少总工作量。
3. 设计 §1 第 5 条目标是"错误系统：codegen 错误传播路径"——但 MUV-4 只引入类型不传播，目标未达成。

`codegen/` 中的 panic/unwrap 实际位置（grep 结果）：
- `llvm/mod.rs`: 1 处 `panic!` (L2065)、~40 处 `unwrap()` (主要在 CString 构造)、1 处 `expect()` (L516 "emit_block called outside function")
- `emitter.rs`: 2 处 `panic!` (L439/L475，都在 test 代码内，非生产路径)

生产路径的 panic/unwrap 集中在 `llvm/mod.rs`，主要是 LLVM C-API 调用前的 CString 构造——这些 unwrap 在 NUL 字节进入字符串时会 panic（虽然 Landin 标识符不会含 NUL，但仍是潜在 panic 路径）。

**校准建议**：v2 MUV-4 二选一：

- **方案 A（推荐）**：MUV-4 完整改造——把 `llvm/mod.rs` 中 ~40 处 `unwrap()` 改为 `CodegenError::new(..., span)` 并 propagate，`run_codegen_pipeline` 返回类型从 `()` 改为 `CodegenResult<()>`，`codegen_crate` / `codegen_crate_to_module` 同步改造。估计工作量：~80 LOC 新增 + ~40 处 unwrap 改造 + ~3 个公开函数签名变更。
- **方案 B**：完全删除 MUV-4，把 CodegenError 推迟到下一阶段（v0.4+）。当前 codegen panic 路径已存在 16+ stage 未引发问题，再推迟一个 stage 无 soundness 风险。

不接受 v1 的"引入类型但不改造"中间态。

---

#### P2-3: MUV-3 设计文档对齐声明 (J1) 不成立

**位置**：design-v1 §3.2 MUV-3 J1 检查（L171）："✅ 与 07-codegen.md §2 (类型映射) 一致"。

**事实核查**：`docs/lang-design/07-codegen.md` §2 实际章节划分：

```
## 2. 类型映射
### 2.1 基本类型      (primitive int/float/bool/char)
### 2.2 复合类型      (struct/tuple/array/enum/closure)
### 2.3 类型 Layout 计算 (AdtLayout: Struct/Enum)
### 2.4 Niche optimization (未来优化)
```

设计 v1 提议的拆分是 `types.rs` / `layouts.rs` / `stdlib.rs`，与 §2 的章节划分**不对齐**：

- `types.rs` 把 §2.1 基本类型 + §2.2 复合类型 + §2.3 Layout 计算混在一起（实际函数 `mir_type_to_emit_type_with_layouts` 处理所有三类）。
- `layouts.rs` 只放 `adt_layout_to_emit_type`，对应 §2.3 一小部分。
- `stdlib.rs` 是 Landin 自有概念（stdlib type kinds），07-codegen.md §2 不涉及。
- 完全遗漏 §4.4 Place 投影映射（实际 733 LOC 的归属）。

J1 判据要求"新结构与设计文档章节划分一致"——v1 的拆分既不对齐 §2，也不对齐 §4，J1 ✅ 是误判。

**校准建议**：见 P1-3 校准建议——v2 改为 `types.rs`(§2.1-§2.3) + `layouts.rs`(§2.3-§2.4) + `places.rs`(§4.4) + `stdlib.rs`(跨章节)，并在 J1 检查中明确标注每个文件对应的 07-codegen.md 章节号。

---

#### P2-4: "不破坏现有调用"声明对实现者不成立

**位置**：design-v1 §3.2 MUV-1 第 83 行注释："不破坏现有调用，使用 super-trait 组合"。

**事实核查**：
- **对调用者（callers）**：成立。`&mut dyn Emitter` 在 14 处调用点（mod.rs 5 / statement/rvalue/terminator/operand 5 / mir_translation 4 / trait_dispatch 6）继续工作——super-trait 模式保 `dyn Emitter` 可用。
- **对实现者（implementers）**：**不成立**。`lib.rs` L425-438 把 `Emitter` 作为 **public API** re-export：

```rust
// lib.rs L425-426 注释：
// (allows third-party LLVM-IR backends to implement `Emitter` and call
// `codegen_from_mir` directly).
pub use codegen::{..., Emitter, ...};
```

任何外部 `impl Emitter for MyBackend` 在 v1 拆分后会编译失败——必须改为 4 个（或 v2 的 6 个）独立 impl 块。这是**公共 API 破坏性变更**。

虽然 §13.3 允许早期阶段破坏性变更（"可以自由重命名、删除、重构公共 API"），但设计稿应明确声明这是 breaking change，而非声称"不破坏现有调用"。

更关键：blanket impl `impl<T> Emitter for T where T: ...` 与现有的 `impl Emitter for TextEmitter`（text/mod.rs L167）/ `impl Emitter for LLVMSysEmitter`（llvm/mod.rs L546）**会编译冲突**——Rust 不允许同一 type-trait 对同时有 manual impl 和 blanket impl。必须先**删除**两个 manual impl 块，再按子 trait 重新切分。这是 MUV-1 真正的迁移成本（见 P1-2）。

**校准建议**：v2 §3.2 MUV-1 第 83 行注释改为：

> "对 `&mut dyn Emitter` 调用者不破坏（14 处调用点保 dyn 兼容）；对 `Emitter` trait 实现者是破坏性变更（lib.rs L437 public API）：现有 `impl Emitter for TextEmitter`（text/mod.rs L167-815, 648 LOC）和 `impl Emitter for LLVMSysEmitter`（llvm/mod.rs L546-1825, 1279 LOC）必须删除并按 N 个子 trait 重新切分。Per §13.3 早期阶段允许破坏性变更，但需在 RELEASE_NOTES.md 标注 'breaking: Emitter trait split into N sub-traits'。"

---

### P3 设计建议（可推迟 — 风格 / 措辞 / 微优化）

#### P3-1: `fn_sigs.rs` 命名不符现有惯例

**位置**：design-v1 §3.2 MUV-2 文件树（L147）"`fn_sigs.rs` (~70 LOC) — build_fn_sigs_map (llvm-only)"。

**事实核查**：现有 codegen 子模块命名规范（`operand.rs` / `rvalue.rs` / `statement.rs` / `terminator.rs` / `dyn_trait_emit.rs`）使用完整单词、单数名词、snake_case。`fn_sigs.rs` 使用缩写 `fn`，与惯例不一致。

此外，`build_fn_sigs_map` 在 `mod.rs` L523 标注 `#[cfg(feature = "llvm-backend")]`——是 LLVM-only 函数。命名为通用名 `fn_sigs.rs` 但实际仅 LLVM 用，会误导读者以为 text backend 也使用。

**校准建议**：v2 改为以下二选一：
- **方案 A**：`function_sigs.rs`（完整单词，仍放 `codegen/` 根目录，加 `#[cfg(feature = "llvm-backend")]` mod 声明）。
- **方案 B（推荐）**：移入 `llvm/` 子目录，命名 `llvm/fn_sigs.rs` 或 `llvm/function_sigs.rs`，与 LLVMSysEmitter 同模块——这样 LLVM-only 性质由目录结构表达，文件名可保留简短。

---

#### P3-2: LOC 估计普遍偏离实际

**位置**：design-v1 §3.2 MUV-2 文件树（L141-148）、§4.1 表格（L225-230）。

**事实核查**：实测各函数 LOC（精确 wc -l）：

| 模块 | 设计估计 | 实际 | 偏差 |
|------|---------|------|------|
| `mod.rs`（拆分后） | ~200 LOC | ~80 LOC | +150% 高估 |
| `pipeline.rs` (`run_codegen_pipeline`) | ~120 LOC | 67 LOC (L151-217) | +79% 高估 |
| `function.rs` (`codegen_function` + `codegen_from_mir` + `codegen_synthesized_closure_functions` + `get_call_dest_type`) | ~250 LOC | 316 LOC (27+62+174+53) | -21% 低估 |
| `drop_glue.rs` (`emit_drop_glue_functions`) | ~280 LOC | 235 LOC (L256-490) | +19% 高估 |
| `fn_sigs.rs` (`build_fn_sigs_map`) | ~70 LOC | 47 LOC (L524-570) | +49% 高估 |

总 LOC 估计偏差不大，但单项偏差最高达 +150%。这会影响排期与风险预评估。

**校准建议**：v2 §3.2 MUV-2 文件树和 §4.1 表格更新为实测 LOC（如上表），并在 §4.1 增加 "实测依据" 列标注起止行号。

---

#### P3-3: `mod.rs` L60-62 过期注释应在 MUV-1 一并修复

**位置**：`src/codegen/mod.rs` L60-62。

**事实核查**：现有注释：

```rust
//! - **Emitter trait bloat**: 36 methods, 1 implementation (`TextEmitter`).
//!   Decompose into sub-traits (`EmitterArith`, `EmitterMemory`, etc.)
//!   when adding a second backend. Stage 3.59 Issue #5.
```

错误：
1. "36 methods" → 实际 39。
2. "1 implementation (TextEmitter)" → 实际 2（TextEmitter + LLVMSysEmitter，自 Stage 13.5 起）。
3. "when adding a second backend" → 第二 backend 已存在（LLVMSysEmitter 自 Stage 13.5），条件已满足但拆分未做。
4. "Stage 3.59 Issue #5" → 该 issue 已被 Stage 16.38 / 16.76 接续处理，应更新引用。

**校准建议**：v2 MUV-1 任务列表增加一项："修复 `mod.rs` L60-62 过期注释，改为 '39 methods, 2 implementations (TextEmitter + LLVMSysEmitter); split into N sub-traits per Stage 16.76 MUV-1'"。

---

#### P3-4: `set_fn_sigs` 的"trait-based hook"注释误导

**位置**：`src/codegen/mod.rs` L504-506 注释。

**事实核查**：mod.rs L504-506 注释：

```rust
/// Stage 16.37: Delegates to the shared `run_codegen_pipeline` function.
/// The LLVM-specific setup (`set_fn_sigs`) is done via a trait-based hook
/// so the pipeline remains backend-agnostic.
```

但 `set_fn_sigs` 实际是 `LLVMSysEmitter` 的 inherent method（`llvm/mod.rs` L120 `pub(crate) fn set_fn_sigs(...)`），**不是 trait 方法**。`codegen_crate_to_module`（mod.rs L508-515）直接在 `LLVMSysEmitter` 具体类型上调用 `emitter.set_fn_sigs(...)`，未通过 `&mut dyn Emitter`。

"trait-based hook"措辞误导读者以为有 trait 介入，实际是 backend-specific 的 pre-pipeline setup。

**校准建议**：v2 MUV-2 任务列表增加一项："修正 `mod.rs` L504-506 注释：'trait-based hook' → 'concrete-type pre-pipeline setup (LLVMSysEmitter only); the pipeline itself remains backend-agnostic via &mut dyn Emitter'"。或者更优：把 `set_fn_sigs` 真正变成 `Emitter` trait 的可选方法（用 default method `fn set_fn_sigs(&mut self, _: ...) {}`），让 hook 真正成为 trait-based。

---

## 3. 逐项校准建议

### 3.1 方案选择（方案 B 是否最优？）

**评估**：方案 B 的方向正确（既非方案 A 的"治症不治根"，也非方案 C 的"过度激进"），符合 §12 最优 > 最小原则。但 v1 的方案 B 在三个 MUV 上未达"最优"标准：

- MUV-1 的 ValueEmitter 22 方法不符合 §13.4 J2 单一职责——需进一步细分（见 P2-1）。
- MUV-3 的拆分依据错误（见 P1-3）——需重写为对齐 07-codegen.md §2 + §4.4 的 4 模块方案。
- MUV-4 的半成品性质违反 §13.3 第 5 条（见 P2-2）——需改为完整改造或完全推迟。

方案 C 的"重新设计 EmitType 加入 layout 信息"暂不推荐——EmitType 当前已支持 `Struct`/`Array`/`Ptr`，layout 信息已通过 `AdtLayouts` side-table 传递（`mir_type_to_emit_type_with_layouts`），重新设计 EmitType 会引发 8000+ 测试回归，收益不明朗。

**建议**：选方案 B，但需按 §2 缺陷清单修订为 v2。

---

### 3.2 MUV-1 Emitter trait 拆分粒度

**评估**：4 子 trait（ModuleEmitter/FunctionEmitter/ValueEmitter/LocalStateEmitter）不够细。ValueEmitter 容纳 22 方法（占 trait 总方法 56%），是新的"fat trait"，违反 §13.4 J2。

**建议**：改为 6 子 trait（见 P2-1 校准）：
- ModuleEmitter (5)
- FunctionEmitter (8)
- ArithmeticEmitter (11)
- MemoryEmitter (6)
- AggregateEmitter (5)
- LocalStateEmitter (4)

5 + 8 + 11 + 6 + 5 + 4 = 39 ✓。每个 trait 可用一句话描述单一职责，符合 J2 通过条件。

如果设计 Agent 坚持不超过 4 trait，需在 v2 给出"为何 22 方法的 ValueEmitter 仍算单一职责"的论证——目前 v1 未论证。

---

### 3.3 MUV-2 文件命名

**评估**：`pipeline.rs` / `function.rs` / `drop_glue.rs` 符合 §10 与现有惯例（snake_case 单数名词）；`fn_sigs.rs` 不符合（缩写 `fn` + 名字暗示通用但实际 LLVM-only）。

**建议**：
- `pipeline.rs` ✓ 保留
- `function.rs` ✓ 保留
- `drop_glue.rs` ✓ 保留
- `fn_sigs.rs` → `function_sigs.rs` 或移入 `llvm/function_sigs.rs`（推荐后者，目录结构表达 LLVM-only 性质）

另需修复 `mod.rs` L504-506 误导性注释（见 P3-4）。

---

### 3.4 MUV-3 mir_translation 拆分依据

**评估**：按 `types/layouts/stdlib` 拆分**不合理**——既不对齐 07-codegen.md §2 章节划分，又遗漏 733 LOC place codegen 逻辑（占文件 64%）。

是否应按"基本类型/复合类型/泛型特化"拆？**部分是**。07-codegen.md §2 实际分为 基本类型(§2.1)/复合类型(§2.2)/Layout(§2.3)/Niche(§2.4)——按这 4 类拆 `types.rs` 是合理的，但 `mir_type_to_emit_type_with_layouts` 单函数已涵盖全部 4 类（一个 match 处理所有 TyKind），强行拆 4 文件会切断 match 表达式，反而破坏可读性。

**建议**：v2 MUV-3 改为 4 模块拆分（见 P1-3 校准）：
- `types.rs` (§2.1-§2.3，~250 LOC) — 保留 `mir_type_to_emit_type_with_layouts[_and_mono]` 完整 match
- `layouts.rs` (§2.3-§2.4，~80 LOC) — `adt_layout_to_emit_type` + 未来 niche 优化扩展点
- `places.rs` (§4.4，~780 LOC) — 7 个 place/operand codegen 函数（v1 完全遗漏的部分）
- `stdlib.rs` (~30 LOC) — `stdlib_type_kind_to_emit_type`

J1 对齐：每个文件对应 07-codegen.md 明确章节号。

---

### 3.5 MUV-4 CodegenError 范围

**评估**：只引入类型不改造路径，是半成品。违反 §13.3 第 5 条"一步到位"，制造死代码（§1.0 原則 5）。

应一并改造吗？**应该**。理由：
1. codegen 路径中 ~40 处 `unwrap()` 集中在 `llvm/mod.rs` CString 构造——改造工作量小（每处 1-2 行 `?` + CodegenError 构造）。
2. 改造后 `run_codegen_pipeline` 返回 `CodegenResult<()>`，调用者（`codegen_crate` / `codegen_crate_to_module`）能 propagate 错误而非 panic——这是 §1 第 5 条"错误系统：codegen 错误传播路径"目标的真正达成。
3. 推迟到下一阶段不会减少总工作量，只会让 `CodegenError` 类型在仓库中"死"一段时间。

**建议**：v2 MUV-4 改为完整改造方案（见 P2-2 方案 A）。如果时间不允许，完全删除 MUV-4 推迟到 v0.4+（方案 B）。不接受 v1 的中间态。

同时 CodegenError 字段需符合 §10.1.8（见 P1-4 校准）。

---

### 3.6 风险盲点

**评估**：v1 §7 列出 7 项待审查事项，但遗漏 5 个关键风险（见 §1 风险识别充分性 R-1 至 R-5）。

特别关注："LLVMSysEmitter 的 trait impl 是否会因为 super-trait 拆分而断裂？"

**答案**：**会断裂，但可修复**。具体：
- `impl Emitter for LLVMSysEmitter`（llvm/mod.rs L546-1825, 1279 LOC）必须**删除**——否则与 blanket impl `impl<T> Emitter for T where T: ...` 冲突。
- 替换为 N 个独立 impl 块（N = 子 trait 数，v1=4 或 v2=6），每个块从原 1279 LOC 中抽取对应方法。
- 抽取过程中方法签名不变，只是物理位置重排——逻辑风险低，但工作量高（~1279 LOC 重排）。
- TextEmitter 同理（648 LOC 重排）。

`dyn Emitter` 在 14 处调用点继续工作——super-trait 模式保 dyn 兼容（所有子 trait 都 object-safe：方法均为 `&mut self`/`&self`，返回 `EmitValue`/`Option<&EmitValue>`/tuple，无泛型方法）。

**外部破坏**：lib.rs L437 `pub use codegen::{..., Emitter, ...}` 是公共 API，外部 backend 实现者需重写 impl 块。Per §13.3 允许，但需在 RELEASE_NOTES 标注 breaking change。

**建议**：v2 §3.2 MUV-1 增加"迁移步骤"小节（见 P1-2 校准），明确列出 5 步迁移流程。v2 §7 风险盲点增加 R-1 至 R-5 五项。

---

### 3.7 测试覆盖

**评估**：现有测试**间接覆盖**大部分 Emitter 方法，但**直接单元测试覆盖少**。

具体分析：
- `emitter.rs` 内联测试 4 个（L421-489）：仅覆盖 `text_emitter_satisfies_emitter_trait`（compile-time check）、`fat_ptr_type_correct_shape`、`mir_type_to_emit_type_correct`、`emit_type_helpers`、`text_emitter_produces_output`——只测了 `emit_header`/`emit_declare` 两个方法。
- `llvm/mod.rs` 内联测试 5 个（L1981-2004 附近）：`emit_header_sets_target`、`emit_simple_function`、`emit_const_int` 等——覆盖 `emit_header`/`emit_function_begin`/`emit_const` 等少数方法。
- `tests/v0/stage5/plan/` 有 18 个测试文件涉及 TextEmitter/LLVMSysEmitter，共 188 处引用——但大多是 end-to-end 测试（编译 sample 程序 → 检查输出 IR），间接覆盖 `emit_binop`/`emit_call`/`emit_gep_*`/`emit_insertvalue`/`emit_extractvalue`/`emit_phi`/`emit_switch` 等方法。
- 从未被直接测试的方法：`emit_unreachable`、`emit_br`、`emit_br_cond`（控制流方法只在 end-to-end 间接覆盖）；`emit_dyn_trait_method_call` 有 9 处直接测试 ✓；`emit_vtable_global`/`emit_dyn_trait_const` 有专门测试 ✓。

**trait 拆分后的测试影响**：
- `emitter.rs` L421 `text_emitter_satisfies_emitter_trait` 测试 `let _: &dyn Emitter = &TextEmitter::new();`——拆分后仍工作（super-trait 保 dyn 兼容）✓
- `llvm/mod.rs` L1976 `let _: &dyn Emitter = &LLVMSysEmitter::new();`——同上 ✓
- 间接测试（end-to-end）全部保兼容 ✓

**是否有方法从未被测试调用**：grep 显示 `emit_unreachable` 在 `tests/` 中无直接调用——仅通过 `codegen_terminator` 在 `TerminatorKind::Unreachable` 时被调用，间接覆盖。trait 拆分不改变此状态。

**建议**：v2 MUV-1 验收清单增加一项："为每个子 trait 增加一个 compile-time 'trait satisfaction' 测试"（如 `let _: &dyn ModuleEmitter = &TextEmitter::new();`），确保拆分后所有子 trait 都被至少一处类型断言覆盖。这是最低成本的回归保护。

---

## 4. 修订建议汇总

设计 Agent (ARCH-A) 应在 v2 中：

1. **修正方法数事实**（P1-1）：全部"36 methods"改为"39 methods"，同步修复 `mod.rs` L60-62 与 emitter.rs L60 doc-comment。
2. **MUV-1 增加"与 Stage 16.38 关系"+"迁移步骤"两个小节**（P1-2）：明确 v1 是 16.38 的重启，列出 5 步迁移流程，更新 LOC 估计为 ~1927 LOC 重排。
3. **MUV-1 拆分粒度改为 6 子 trait**（P2-1）：ModuleEmitter/FunctionEmitter/ArithmeticEmitter/MemoryEmitter/AggregateEmitter/LocalStateEmitter，避免 ValueEmitter 22 方法的"新 fat trait"。
4. **MUV-3 重写拆分方案**（P1-3, P2-3）：改为 `types.rs`(§2.1-§2.3) + `layouts.rs`(§2.3-§2.4) + `places.rs`(§4.4, 733 LOC) + `stdlib.rs`，对齐 07-codegen.md 章节号；修正 LOC 估计。
5. **MUV-4 二选一**（P2-2）：完整改造（含 panic 路径迁移）或完全删除推迟——不接受半成品；同时 CodegenError 字段改为 `{ message: String, span: Span }` 符合 §10.1.8（P1-4）。
6. **修正"不破坏现有调用"声明**（P2-4）：明确区分"对调用者不破坏"vs"对实现者破坏"，在 RELEASE_NOTES 标注 breaking change。
7. **`fn_sigs.rs` 重命名**（P3-1）：改为 `function_sigs.rs` 或移入 `llvm/` 子目录。
8. **修正 LOC 估计**（P3-2）：根据实测重算各模块 LOC，附起止行号依据。
9. **修复 `mod.rs` L504-506 误导注释**（P3-4）：把 `set_fn_sigs` 真正变成 trait default method，或修正注释为"concrete-type pre-pipeline setup"。
10. **增加风险盲点 R-1 至 R-5**（§1）：public API 破坏、blanket impl 冲突、dyn Emitter 14 处调用点、Stage 16.38 历史、set_fn_sigs 非 trait hook。
11. **增加子 trait 编译时测试**（§3.7）：每个子 trait 至少 1 处 `let _: &dyn SubTrait = &TextEmitter::new();` 类型断言。

---

## 5. 循环状态

- **是否需要 v2 设计稿？** **YES**

- **v2 必须解决的问题清单**（按优先级）：

  | # | 优先级 | 问题 | 章节 |
  |---|--------|------|------|
  | 1 | P1 | 修正方法数 36 → 39（含 mod.rs L60-62 同步修复） | P1-1 |
  | 2 | P1 | MUV-1 增加"与 Stage 16.38 关系"+"迁移步骤"小节 | P1-2 |
  | 3 | P1 | MUV-3 重写为 types/layouts/places/stdlib 4 模块（含 places.rs 733 LOC） | P1-3 |
  | 4 | P1 | CodegenError 字段改为 `{ message, span }` 符合 §10.1.8 | P1-4 |
  | 5 | P2 | MUV-1 拆分改为 6 子 trait（避免 ValueEmitter 22 方法） | P2-1 |
  | 6 | P2 | MUV-4 改为完整改造或完全删除（不接受半成品） | P2-2 |
  | 7 | P2 | MUV-3 J1 对齐声明重写（标注 07-codegen.md 章节号） | P2-3 |
  | 8 | P2 | 修正"不破坏现有调用"声明（区分调用者 vs 实现者） | P2-4 |
  | 9 | P3 | `fn_sigs.rs` 重命名 | P3-1 |
  | 10 | P3 | LOC 估计修正 | P3-2 |
  | 11 | P3 | 修复 mod.rs L60-62 过期注释 | P3-3 |
  | 12 | P3 | 修复 mod.rs L504-506 误导注释（或真正 trait-ify set_fn_sigs） | P3-4 |

- **循环轮次**：本轮为 v1 → review-v1（第 1 轮）。设计 Agent 产出 v2 后，进入第 2 轮审查。Per §13.5.2 第 5 条，循环上限 5 轮。

- **未通过审查的硬性原因**：4 个 P1 缺陷中任何一个未修复，v2 仍不通过。P2 缺陷建议在 v2 一并修复，但若 v2 解决全部 P1 + 至少 P2-1（trait 粒度）+ P2-2（MUV-4 半成品），可进入实现阶段并带 P2-3/P2-4/P3 作为 limitation 表。

---

> **Review Agent 签名**：REV-A (via Plan subagent)
> **审查完成时间**：2026-08-05
> **下一步**：ARCH-A 接收本审查清单，产出 design-v2，进入第 2 轮设计-审查循环。
