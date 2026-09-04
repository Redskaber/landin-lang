# Stage 101 开发计划 — mir_type_to_emit_type Param fallback 返回 Error

> **阶段**: v0.10 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 修复 - Layer 2)
> **TD**: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+) — Stage 99 RCA Layer 2 修复
> **复杂度**: L3 (跨模块: codegen/emitter + mir_translation + rvalue + operand + drop_glue)
> **版本基线**: v0.639.0 (Stage 100 Layer 1, 5592 tests)
> **目标版本**: v0.640.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 1) `mir_type_to_emit_type` 对 Param/Never/Infer 等 fallback 从 i32 改为 CodegenError (报错而非静默); 2) `codegen_operand` FnDef 处理: 当 FnDef substs 非空时用 mono_item_name mangle 实例化名 |
| **WHY** | Layer 2 根因: Param fallback 到 i32 产生不正确 LLVM IR (e.g., GEP 用 i32 替代 String struct)。codegen_operand FnDef substs 不 mangle 导致 `store ptr @landin_Box_new` 引用 generic def 而非 instance, generic def body 必须仍 emit (产生 Param warnings) |
| **WHO** | ARCH-A 设计; DEV-A 实施; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 101 完成 → 进入 Stage 102 (LLVMSysEmitter ownership 重构) |
| **WHERE** | `src/codegen/emitter/mod.rs:275-403`, `src/codegen/operand.rs:82-99`, `src/codegen/mir_translation/types.rs:255`, `src/codegen/rvalue.rs`, `src/codegen/terminator.rs`, `src/codegen/drop_glue.rs` |
| **HOW** | 1) Param fallback 调用 mir_type_to_emit_type_checked 已存在, 改为用它并传播 Err; 2) codegen_operand 接收 mono_names + type_name_by_def_id, FnDef substs 非空时 lookup mono_item_name |
| **HOW MUCH** | ~5 文件, ~80 LOC, 1:3+ 正负测试 |

## 二、对齐设计文档 (§13.1 / §8.4.5)

### docs/lang-design/08-codegen.md 对齐
Rust 设计: rustc codegen 对 Param/Never 类型直接 panic 或返回 Error (per §1.0 原则 4 报错>静默)。当前 Landin 静默 fallback 到 i32 违反此原则。

### docs/graph/codegen/data-flow.md 对齐
data flow: MIR → codegen translation → Emitter trait。当前 translation layer 对 unresolved type 静默 fallback, 违反 data flow — 应返回 CodegenError 给 caller。

## 三、决策点 (§12 最优>最小, §1.0 原则 4 报错>静默)

### 决策 1: Param fallback 返回 CodegenError 而非 i32

**选择**: 用现有的 `mir_type_to_emit_type_checked` (返回 CodegenResult<EmitType>), 在 codegen_function 中传播 Err。

**替代方案 (拒绝)**:
- ❌ 保持 fallback 到 i32 + 警告 — 治症不治根, 不正确 LLVM IR 仍产生
- ❌ panic on Param — 太激进, 破坏现有错误恢复路径

**理由** (§1.0 原则 4 报错>静默, §1.0 原则 6 通解>特解):
- 一个 CodegenError 类型覆盖所有 unresolved type
- 现有 mir_type_to_emit_type_checked 已实现, 只需传播 Err

### 决策 2: codegen_operand FnDef substs mangle

**选择**: codegen_operand 接收 mono_names + type_name_by_def_id, FnDef substs 非空时 lookup mono_item_name 生成实例化名。

**理由** (§1.0 原则 6 通解>特解, §1.0 原则 10 唯一可信数据源):
- mono_names 已在 pipeline.rs 构建 (Stage 100 提前 collect_mono_items)
- codegen_operand 复用同一份 mono_names 数据, 不重建
- 与 rustc 设计一致 — generic function 引用用 mangled name

### 决策 3: codegen_operand 不接收新参数, 而通过 mono_names lookup

**选择**: 给 codegen_operand 添加 mono_names + type_name_by_def_id 参数 (而非重建)。

**理由** (§1.0 原则 10 唯一可信数据源):
- mono_names 在 pipeline.rs 构建, codegen_operand 接收引用
- 20+ codegen_operand 调用点都需更新 — 但这是正确的根因修复

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 101.1 | 修改 mir_type_to_emit_type_checked 在 codegen_function 中传播 Err | CodegenError 正确返回 |
| 101.2 | 给 codegen_operand 添加 mono_names + type_name_by_def_id 参数 | 编译通过 |
| 101.3 | FnDef substs 非空时用 mono_item_name mangle | `@landin_Box_new` → `@Box_new_i32` |
| 101.4 | 更新 20+ codegen_operand 调用点传递新参数 | 编译通过 |
| 101.5 | codegen_from_mir 跳过被实例化的 generic def body (Stage 100 留下的 TODO) | Param warnings 进一步减少 |
| 101.6 | 添加 4 个 stage101 测试 | cargo test 全绿 |
| 101.7 | §3.2 验收 + 验证 Param warnings 减少 | fmt/clippy/test 全绿 |
| 101.8 | 更新 worklog + tech-debt + matrix + README + RELEASE_NOTES + calibration-data | 文档同步 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- [ ] `cargo test --release --features llvm-backend --lib` ✓ (898+ tests, 0 failures)
- [ ] `cargo test --release --features llvm-backend --test all_tests` ✓ (5596+ tests, 0 failures, 9 ignored)
- [ ] Param warnings 进一步减少 (基线 24 → 目标 0)
