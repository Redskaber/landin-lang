# Stage 16.76 Design v1 — Codegen Pipeline Refactoring

> **Author**: ARCH-A (Design Agent)
> **Date**: 2026-08-05
> **Version**: design-v1
> **Status**: Draft — pending Review Agent (REV-A) critique
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环)

## 1. 设计目标 (§13.4.2 step 1: 架构现状分析)

按用户要求"胆大心细优化重构 codegen 编译管道"，本阶段聚焦：

1. **真正抽象出 codegen**：把 `Emitter` trait（36 methods）按职责拆分为多个子 trait
2. **真正组织 llvm 和 text**：明确 backend 实现的边界与共享代码
3. **数据结构选型与架构设计**：审视 `EmitType`、`EmitValue` 是否最优
4. **编译流水线组织**：审视 `run_codegen_pipeline` 的 6 步是否清晰
5. **错误系统**：codegen 错误传播路径
6. **API 接口设计**：§10 命名标准化在 codegen 的应用

## 2. 架构现状分析 (§13.4.2 step 1)

### 2.1 当前 codegen 模块结构

```
src/codegen/
├── mod.rs               (931 LOC)  — 入口 + pipeline + drop_glue + per-function
├── mir_translation.rs   (1144 LOC) — MIR→EmitType 翻译
├── emitter.rs           (490 LOC)  — Emitter trait (36 methods) + EmitType + 辅助函数
├── text/mod.rs          (841 LOC)  — TextEmitter 实现
├── llvm/mod.rs          (2133 LOC) — LLVMSysEmitter 实现 (LLVM C-API)
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
| AP-1 | **Emitter trait 臃肿**：36 methods 集中在单 trait，违反 §13.4 J2 单一职责 | P2 (architecture debt) | mod.rs L60-62 注释 |
| AP-2 | **mod.rs 职责混合**：含入口+pipeline+drop_glue+per-function+helper | P2 | 本审查 |
| AP-3 | **mir_translation.rs 过大**：1144 LOC，混合类型翻译+layout 计算+helper | P2 | 本审查 |
| AP-4 | **emit_drop_glue_functions 过长**：单函数 ~240 LOC，嵌套深 | P3 (readability) | mod.rs L256-490 |
| AP-5 | **codegen_function 过长**：单函数 ~170 LOC，含 ret_ty 计算+params+loop | P3 (readability) | mod.rs L699-872 |
| AP-6 | **EmitType 缺少 is_signed/is_unsigned 元数据**：导致 codegen 数次重新推导 | P3 (information loss) | rvalue.rs 多处 |
| AP-7 | **Emitter trait 缺少 emit_struct_decl**：struct 类型在 TextEmitter 内联展开，LLVMSysEmitter 用 LLVMIdentifiedStructType | P3 (backend divergence) | text/mod.rs vs llvm/mod.rs |
| AP-8 | **缺 codegen 错误类型**：codegen 失败时只 panic，无 Result 返回 | P2 (error system) | mod.rs L129 (panic on missing) |

### 2.3 已识别的优势（保留）

| # | 优势 | 来源 |
|---|------|------|
| ST-1 | `run_codegen_pipeline` 已经是统一入口（Stage 16.37） | mod.rs L151 |
| ST-2 | TextEmitter + LLVMSysEmitter 都实现 Emitter trait | emitter.rs L93 |
| ST-3 | §11 接口隔离已合规（codegen 是纯 MIR 消费者） | mod.rs L4-9 |
| ST-4 | trait_dispatch 已分子模块（vtable/dynptr/orchestrator） | trait_dispatch/ |
| ST-5 | codegen_{operand,rvalue,statement,terminator} 已分文件 | codegen/*.rs |

## 3. 重构方案 (§13.4.2 step 3: 拟定方案)

### 3.1 方案 A — 最小重构（保守）

只提取 `emit_drop_glue_functions` 到独立文件 `drop_glue.rs`，其他不变。

**优点**：风险低，1 文件变动
**缺点**：未解决 AP-1（Emitter trait 臃肿）、AP-2（mod.rs 职责混合）、AP-3（mir_translation 过大）

### 3.2 方案 B — 中度重构（推荐）

按 §13.4 J1-J6 全面重构，分 4 个 MUV：

#### MUV-1: Emitter trait 拆分

按职责拆分为 4 个子 trait（不破坏现有调用，使用 super-trait 组合）：

```rust
// src/codegen/emitter/mod.rs
pub trait ModuleEmitter {
    fn emit_header(&mut self);
    fn emit_declare(&mut self, signature: &str);
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;
    fn emit_vtable_global(&mut self, name: &str, methods: &[String]) -> EmitValue;
    fn emit_dyn_trait_const(&mut self, name: &str, data: &str, vtable: &str) -> EmitValue;
}

pub trait FunctionEmitter {
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType);
    fn emit_function_end(&mut self);
    fn emit_block(&mut self, label: &str);
    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>);
    fn emit_unreachable(&mut self);
    fn emit_br(&mut self, label: &str);
    fn emit_br_cond(&mut self, cond: &EmitValue, then_l: &str, else_l: &str);
    fn emit_switch(&mut self, discr: &EmitValue, ty: &EmitType, cases: &[(i128, String)], default: &str);
}

pub trait ValueEmitter {
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue;
    fn emit_binop(&mut self, op: BinOp, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue;
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue;
    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue);
    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue;
    fn emit_call(&mut self, fn_name: &str, args: &[(EmitType, &EmitValue)], ret: &EmitType) -> EmitValue;
    // ... 其他 value emission methods
}

pub trait LocalStateEmitter {
    fn set_local_ptr(&mut self, id: u32, ptr: EmitValue);
    fn get_local_ptr(&self, id: u32) -> Option<&EmitValue>;
    fn set_local(&mut self, id: u32, val: EmitValue);
    fn get_local(&self, id: u32) -> Option<&EmitValue>;
}

// 组合 trait — 现有代码继续用 `dyn Emitter`
pub trait Emitter: ModuleEmitter + FunctionEmitter + ValueEmitter + LocalStateEmitter {}
impl<T> Emitter for T where T: ModuleEmitter + FunctionEmitter + ValueEmitter + LocalStateEmitter {}
```

**J1-J6 检查**：
- J1 ✅ 与设计文档 07-codegen.md §4 (MIR→LLVM IR 映射) 一致
- J2 ✅ 每个 trait 单一职责（Module / Function / Value / LocalState）
- J3 ✅ 依赖单向（sub-traits 不互相依赖，组合 trait 依赖所有 sub-traits）
- J4 ✅ Emission 概念完整保留
- J5 ✅ 仍在 codegen 阶段，不破坏 §11
- J6 ✅ mod.rs 490→~250 LOC, emitter/mod.rs ~150 LOC, 4 sub-trait 文件各 ~80 LOC

#### MUV-2: mod.rs 职责拆分

把 mod.rs 931 LOC 拆为：

```
src/codegen/
├── mod.rs               (~200 LOC) — 入口 + re-exports
├── pipeline.rs          (~120 LOC) — run_codegen_pipeline
├── function.rs          (~250 LOC) — codegen_function + codegen_from_mir + codegen_synthesized_closure_functions
├── drop_glue.rs         (~280 LOC) — emit_drop_glue_functions
└── fn_sigs.rs           (~70 LOC)  — build_fn_sigs_map (llvm-only)
```

**J1-J6 检查**：
- J1 ✅ 与 07-codegen.md §1 总体流程对齐
- J2 ✅ pipeline/function/drop_glue/fn_sigs 各自单一职责
- J3 ✅ mod.rs → pipeline → function → drop_glue (单向)
- J4 ✅ 每个 codegen 概念在模块内完整
- J5 ✅ 仍在 codegen 阶段
- J6 ✅ 各文件 LOC 合理

#### MUV-3: mir_translation.rs 拆分

把 1144 LOC 拆为：

```
src/codegen/mir_translation/
├── mod.rs               (~100 LOC) — re-exports
├── types.rs             (~400 LOC) — mir_type_to_emit_type* 函数族
├── layouts.rs           (~300 LOC) — adt_layout_to_emit_type + helpers
└── stdlib.rs            (~350 LOC) — stdlib_type_kind_to_emit_type
```

**J1-J6 检查**：
- J1 ✅ 与 07-codegen.md §2 (类型映射) 一致
- J2 ✅ types/layouts/stdlib 各自单一职责
- J3 ✅ mod.rs 依赖所有子模块，子模块间无环
- J4 ✅ 类型翻译概念完整
- J5 ✅ 仍在 codegen 阶段
- J6 ✅ 各文件 LOC 合理

#### MUV-4: 新增 CodegenError 错误类型

```rust
// src/codegen/error.rs
pub struct CodegenError {
    pub message: String,
    pub location: Option<CodegenLocation>,
}

pub struct CodegenLocation {
    pub fn_name: Option<String>,
    pub bb_idx: Option<usize>,
    pub stmt_idx: Option<usize>,
}

pub type CodegenResult<T> = Result<T, CodegenError>;
```

注：本 MUV 只引入类型，不立即改造所有 panic 路径。改造留待下一阶段（避免本阶段过大）。

**J1-J6 检查**：
- J1 ✅ 与 16-diagnostics.md 设计对齐
- J2 ✅ 错误类型单一职责
- J3 ✅ 无新依赖
- J4 ✅ 错误概念完整
- J5 ✅ 仍在 codegen 阶段
- J6 ✅ error.rs ~80 LOC

### 3.3 方案 C — 大幅重构（激进）

方案 B + 重新设计 EmitType（加入 layout 信息）+ 引入 CodegenContext 状态对象。

**优点**：彻底解决 AP-6/AP-7
**缺点**：风险高，可能破坏 8000+ 测试

### 3.4 推荐：方案 B

理由：
1. **§12 最优 > 最小**：方案 A 治症不治根；方案 C 过度激进
2. **§13.4 J1-J6 全部满足**：方案 B 通过所有判据
3. **风险可控**：4 个 MUV 独立可验证，每个 MUV 后跑测试守护
4. **测试守护**：8000+ 测试是回归守护网

## 4. 重构执行计划

### 4.1 MUV 顺序（串行，避免冲突）

| MUV | 估计 LOC 变动 | 估计时间 | 风险 |
|-----|--------------|---------|------|
| MUV-1 Emitter trait 拆分 | ~600 LOC 移动 + ~150 LOC 新增 | 中 | 中（trait bound 影响所有调用点） |
| MUV-2 mod.rs 职责拆分 | ~900 LOC 移动 | 中 | 低（纯文件移动） |
| MUV-3 mir_translation 拆分 | ~1100 LOC 移动 | 中 | 低（纯文件移动） |
| MUV-4 CodegenError 类型 | ~80 LOC 新增 | 低 | 极低（只增不改） |

### 4.2 每个 MUV 的验收

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- worklog 记录 ✅

### 4.3 回滚策略

- 每个 MUV 单独 commit，便于 git revert
- 备份当前 mod.rs 到 /tmp/codegen-mod-backup.rs（防意外）

## 5. 与 §15 项目图管理同步

重构后更新：
- `docs/graph/stage/stage-3/codegen-data-flow.md` — 新模块结构图
- `docs/graph/design/08-codegen-flow.md` — 设计层 codegen 流程图（更新版本号）

## 6. 与 §14.5 深度审查的关系

本重构计划本身是 §14.5 D1（架构健康度）+ D5（设计合理性）的行动项。完成后：
- D1 改善：Emitter trait 不再臃肿，职责清晰
- D5 改善：mod.rs 不再混合职责

## 7. 待 Review Agent (REV-A) 审查事项

请 REV-A 重点审查：

1. **方案选择**：方案 B 是否真的最优？方案 C 的"重新设计 EmitType"是否值得做？
2. **MUV-1 trait 拆分粒度**：4 个子 trait 是否合理？是否应该更细（如 ArithmeticEmitter / MemoryEmitter / ControlFlowEmitter）？
3. **MUV-2 文件命名**：`pipeline.rs` / `function.rs` / `drop_glue.rs` / `fn_sigs.rs` 是否符合 §10 命名标准？
4. **MUV-3 拆分依据**：按 types/layouts/stdlib 拆分是否合理？是否应该按"基本类型/复合类型/泛型特化"拆？
5. **MUV-4 范围**：只引入类型不改造路径，是否半成品？应该一并改造吗？
6. **风险盲点**：是否有未识别的破坏点？特别是 LLVMSysEmitter 的 trait impl 是否会因为 super-trait 拆分而断裂？
7. **测试覆盖**：现有测试是否能覆盖所有 Emitter trait 方法？是否有方法从未被测试调用？
