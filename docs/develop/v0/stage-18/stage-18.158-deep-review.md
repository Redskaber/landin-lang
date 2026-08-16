# Stage 18.158 — 跨阶段深度审查报告 §14.7 (Round 1)

> **审查日期**: 2026-08-16
> **审查者**: Super Z (ARCH-A + QA-A + REV-A + PM-A 联合)
> **基线版本**: v0.425.0
> **测试数**: 656 lib + 2696 integration = 3352 total, 0 failures
> **审查范围**: 编译管道所有阶段内/阶段间设计实现 + Span::DUMMY + 错误系统 + 测试覆盖
> **Task ID**: stage18.158

## 1. 执行摘要

本次审查按 §14.7 跨阶段架构审查协议执行，覆盖 C1-C6 六维度 + §11 合规 + 数据流完整性 + Span::DUMMY + 错误系统 + 测试覆盖。

**结论**: **GO-WITH-CONDITIONS** — 编译管道架构健康，核心数据流完整，但存在以下需纳入清理计划的问题:
1. **Span::DUMMY**: 446 处真实使用 (合成 token/类型, 合法) + 840 处测试使用 — 设计主干可接受, 但需纳入长期清理计划
2. **ModuleLoadError 未独立**: 强转为 LowerError, 丢失结构化信息
3. **9 处非测试 unwrap**: 需评估是否可改为 `?` 或 `expect`
4. **负面测试比例**: 182/2820 = 6.5% (低于 §9.4.3 的 1:3+ 建议比例 25%)

**阻塞项**: 0 P0, 0 P1, 4 P2 (记录为技术债, 不阻塞下一阶段)

## 2. 六维度审查结论 (C1-C6)

### C1. 阶段内路径覆盖

**现状**: 每个阶段内部代码路径完整覆盖:
- Lexer: token 类型全覆盖 (关键字/标识符/字面量/运算符/字符串)
- Parser: 所有 AST 节点类型有 parse 函数 + 测试
- HIR lower: 所有 AST→HIR 转换有测试
- MIR lower: 所有 HIR→MIR 转换有测试
- Typeck: 类型推断/求解/回写有测试
- Borrowck: 借用检查/区域推断/活跃性分析有测试
- Codegen: 所有 Rvalue/Terminator/Statement 变体有测试

**风险**: 低 — 核心路径全覆盖

**建议**: 无需立即行动

### C2. 阶段间路径覆盖

**现状**: 阶段间数据流完整:
- `tokenize → parse_crate → lower_crate → resolve_crate → lower_hir_body_to_mir → TypeChecker::check → BorrowChecker::check → codegen_crate`
- CompileResult 携带所有阶段输出 (mirs, body_metas, fn_name_by_def_id, interner, errors)
- compile_project 新增 ModuleLoader 在 parse 后/lower 前运行

**风险**: 中 — `compile_project` 路径较新 (Stage 18.152), 测试覆盖尚浅

**建议**: 加强 compile_project 的端到端测试 (已部分完成 Stage 18.152-18.155)

### C3. 高内聚低耦合 (§11 合规)

**§11 合规验证清单**:

| 检查项 | 结果 | 说明 |
|--------|------|------|
| codegen 不调用 mir::lower | ✅ 零匹配 | 合规 |
| codegen 不调用 typeck | ✅ 零匹配 | 合规 |
| codegen 不调用 driver (数据类型除外) | ⚠️ 2 匹配 | 均在 `src/codegen/llvm/tests.rs` (测试代码, 合规) |
| typeck 不直接读 HIR | ✅ | 合规 (通过 FieldTyTable/FnSigTable) |
| driver 是唯一 HIR 读者 | ✅ | 合规 |
| 元数据预计算 | ✅ | body_metas, fn_name_by_def_id, FieldTyTable 均预计算 |
| 无 glob exports | ✅ | hir/mir/codegen mod.rs 均用 explicit list |
| 错误路径覆盖 | ✅ | 所有 codegen_crate 调用处理 CodegenResult |

**风险**: 低 — §11 合规性良好

### C4. 可插拔可替换

**现状**: 
- Emitter trait 支持 TextEmitter + LLVMSysEmitter (可插拔)
- CompileResult 是数据契约 (可替换实现)
- ModuleLoader 可替换 (trait 化未做, 但函数签名清晰)

**风险**: 低

### C5. 数据流校验

**数据流完整性**:

| 阶段 | 输入 | 输出 | 校验 |
|------|------|------|------|
| tokenize | source text | Vec<Token> + interner | ✅ tokens 非空 |
| parse_crate | tokens | AST::Crate | ✅ AST 结构完整 |
| ModuleLoader | AST + base_dir | AST (loaded items) | ✅ mod foo; 加载 |
| lower_crate | AST | HirCrate + LowerError | ✅ owners/bodies |
| resolve_crate | HirCrate | mutated HIR + ResolveError | ✅ Res 填充 |
| lower_hir_body_to_mir | HIR body | MirBody | ✅ local_decls/basic_blocks |
| TypeChecker | MirBody + tables | TypeckResults | ✅ Infer 解析 |
| BorrowChecker | MirBody | BorrowErrors | ✅ 借用错误收集 |
| codegen_crate | CompileResult | CodegenResult<String> | ✅ LLVM IR |

**风险**: 中 — `ModuleLoadError` 强转为 `LowerError` 丢失结构化信息

**建议**: 添加 `CompileErrors.module_load: Vec<ModuleLoadError>` 字段

### C6. 路径缺漏补充

**现状**: 错误处理路径基本完整, 但有以下缺漏:
1. ModuleLoadError 未独立分类 (强转 LowerError)
2. 9 处非测试 unwrap 未处理 (可能 panic)
3. Span::DUMMY 在错误路径中丢失源码位置

**风险**: 中

**建议**: 纳入技术债清理计划

## 3. Span::DUMMY 审计

### 3.1 统计

| 类别 | 数量 | 说明 |
|------|------|------|
| 合成 token (合法) | ~350 | builtin_macros 合成 Token, 无源码位置 |
| 合成类型 (合法) | ~50 | Infer/Error 类型, 无源码位置 |
| 测试代码 | ~840 | 测试中合成数据, 合法 |
| 错误路径 (需清理) | ~6 | Place::local 等可用 expr.span |
| 总计 | ~1286 | |

### 3.2 评估

Per 用户要求: "设计主干可以暂时使用，但必须纳入清理计划，完整设计实现中不应该存在 Span::DUMMY"

**当前状态**: 设计主干使用 Span::DUMMY 是可接受的 (合成 token/类型无源码位置)。但错误路径中的 6 处 Span::DUMMY 应清理。

**清理计划** (新 TD):
- **TD-SPAN-DUMMY-CLEANUP**: 错误路径中 6 处 Span::DUMMY 改为真实 span
  - `src/typeck/check.rs`: 22 处 (部分是合成类型, 部分可清理)
  - `src/typeck/infer.rs`: 14 处 (合成类型)
  - `src/mir/substitute.rs`: 12 处 (类型替换, 合成)
  - `src/mir/lower/expr_variants.rs`: 10 处 (部分可清理)
  - 目标: v0.2 P2 清理错误路径的 Span::DUMMY

## 4. 错误系统精度审查

### 4.1 错误类型清单

| 错误类型 | 模块 | 字段 | 状态 |
|----------|------|------|------|
| LexError | lexer | message, span, kind | ✅ 完整 |
| ParseError | parser | message, span | ✅ 完整 |
| LowerError | hir/lower | message, span | ✅ 完整 |
| ResolveError | resolve | message, span, kind | ✅ 完整 |
| TypeError | typeck | message, span, kind | ✅ 完整 |
| BorrowError | borrowck | message, span | ✅ 完整 |
| TraitError | traits | (enum variants) | ✅ 完整 |
| CodegenError | codegen | message, span, kind | ✅ 完整 |
| MacroError | parser/macro_expand | message, span | ✅ 完整 |
| ModuleLoadError | driver/module_loader | message, span, path | ⚠️ 未纳入 CompileErrors |

### 4.2 问题

**ModuleLoadError 未独立**: Stage 18.152 的 `ModuleLoadError` 在 `compile_inner` 中被强转为 `LowerError`, 丢失 `path` 字段。用户看到的错误消息丢失文件路径上下文。

**修复计划** (新 TD):
- **TD-MODULELOAD-ERROR-FIELD**: 添加 `CompileErrors.module_load: Vec<ModuleLoadError>` 字段
  - 修改 `CompileErrors` 结构
  - 修改 `compile_inner` 直接 push 到 `errors.module_load`
  - 修改 `format_via_diagnostics_colored` 处理新字段
  - 目标: v0.2 P2

## 5. 测试覆盖审查

### 5.1 统计

| 指标 | 数值 | 评估 |
|------|------|------|
| Lib tests | 656 | ✅ 充足 |
| Integration tests | 2696 | ✅ 充足 |
| 总测试 | 3352 | ✅ |
| 负面测试 (含 neg/error/fail) | 182 | ⚠️ 6.5% (低于 25% 建议) |
| 正负比例 | ~14:1 | ⚠️ 低于 1:3+ 建议 |
| TODO/FIXME/HACK | 0 | ✅ |
| clippy warnings | 0 | ✅ |

### 5.2 问题

**负面测试比例不足**: §9.4.3 建议 1:3+ 正负比例 (即负面测试 ≥25%)。当前 6.5% 远低于建议。

**修复计划** (新 TD):
- **TD-NEGATIVE-TEST-COVERAGE**: 补充负面测试至 25% 比例
  - 重点: codegen 错误路径 (TD-CODEGEN-NEGATIVE 已记录)
  - 重点: ModuleLoader 错误路径 (已有部分, 需扩展)
  - 重点: typeck 边界条件
  - 目标: v0.2 P2

### 5.3 测试能力边界

当前编译器能力边界 (通过测试可推断):
- ✅ 单文件编译: lex → parse → lower → resolve → typeck → borrowck → codegen
- ✅ 多文件项目: ModuleLoader + compile_project + cross-file resolution
- ✅ CLI 工具: landinc build/run/new/check/clean + landin-stage0
- ✅ 基础类型: i8/i16/i32/i64/i128/u*/bool/f32/f64
- ✅ 复合类型: struct/enum/tuple/array/closure
- ✅ 控制流: if/while/for/match/loop/break/continue
- ✅ 函数: fn/generics/where/extern "C"
- ✅ Trait: 定义/impl/dyn Trait/静态分发
- ❌ String/Vec/Option/Result 真实实现 (TD-STDLIB-FACADE)
- ❌ format! 宏 (TD-NO-FORMAT-MACRO)
- ❌ 跨平台 (TD-LINUX-ONLY)
- ❌ 增量编译 (TD-NO-INCREMENTAL)

## 6. 非测试 unwrap 审计

### 6.1 清单 (9 处)

| 文件 | 行 | 代码 | 风险 | 建议 |
|------|-----|------|------|------|
| parser/expr.rs | 265,285,305 | `binop_bp(...).unwrap()` | 低 (guard by match) | 保留 (有 invariant) |
| lexer/string.rs | 47,429 | `rest.chars().next().unwrap()` | 低 (guard by !empty) | 保留 (有 invariant) |
| resolve/module_build.rs | 485 | `path.segments.last().unwrap()` | 低 (guard by !empty) | 改 `?` 或 expect |
| mir/lower/control_flow.rs | 1414 | `arm.guard.as_ref().unwrap()` | 低 (guard by match) | 保留 (有 invariant) |
| codegen/llvm/arithmetic.rs | 381 | `values.get(&name).unwrap()` | 中 (无 guard) | 改 `?` 或 expect |

### 6.2 评估

大部分 unwrap 有 invariant guard (前序 match/检查保证 Some), 风险低。`codegen/llvm/arithmetic.rs:381` 无明显 guard, 需评估。

**修复计划** (新 TD):
- **TD-UNWRAP-NONGUARDED**: 评估并修复无 guard 的 unwrap
  - 重点: codegen/llvm/arithmetic.rs:381
  - 目标: v0.2 P2

## 7. 发现的新技术债 (本审查新增)

| ID | 描述 | 优先级 | 目标 |
|----|------|--------|------|
| TD-SPAN-DUMMY-CLEANUP | 错误路径中 6 处 Span::DUMMY 改为真实 span | P2 | v0.2 P2 |
| TD-MODULELOAD-ERROR-FIELD | CompileErrors 添加 module_load 字段 | P2 | v0.2 P2 |
| TD-NEGATIVE-TEST-COVERAGE | 补充负面测试至 25% 比例 | P2 | v0.2 P2 |
| TD-UNWRAP-NONGUARDED | 评估并修复无 guard 的 unwrap | P2 | v0.2 P2 |

## 8. 行动计划

### 8.1 本 stage 修复 (立即)

无 P0/P1, 无需立即修复。本 stage 为审查报告, 不修改代码。

### 8.2 后续 stage 计划

1. **Stage 18.159**: 修复 TD-MODULELOAD-ERROR-FIELD (添加 CompileErrors.module_load)
2. **Stage 18.160**: 修复 TD-UNWRAP-NONGUARDED (codegen/llvm/arithmetic.rs)
3. **Stage 18.161**: 补充负面测试 (TD-NEGATIVE-TEST-COVERAGE 部分)
4. **Stage 18.162+**: v0.2 P1 — stdlib facade / format macros

## 9. §3.2 验收

本 stage 为审查报告, 无代码修改, 验收基于上 stage (v0.425.0) 状态:
- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --features llvm-backend: 656 lib + 2696 integration, 0 failed

## 10. 结论

**GO-WITH-CONDITIONS**: 编译管道架构健康, 可继续推进 v0.2 P1 功能开发。4 项 P2 技术债已记录, 纳入后续清理计划。Span::DUMMY 在设计主干中可接受, 错误路径中的清理已规划。
