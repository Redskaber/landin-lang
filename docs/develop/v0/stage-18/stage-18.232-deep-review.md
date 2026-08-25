# Stage 18.232 — v0.2 Phase 2 Final Deep Review §14.5 (D1-D8) + Dead Code Cleanup

> **Date**: 2026-08-23
> **Version**: v0.480.0 → v0.481.0 (planned)
> **Task ID**: stage18.232
> **Reviewer**: Super Z (main) — Stage Committee (ARCH-A + QA-A + REV-A + PM-A + ALG-C + SKL-A)
> **流程文档**: docs/stage-committee-process.md v6.4 §14.5 (阶段末尾深度审查) + §17.6
> **审查范围**: Stage 18.225-18.231 (v0.2 Phase 2 complete — 7 stages)
> **触发条件**: v0.2 Phase 2 COMPLETE (all 4 C helpers migrated) — per §14.5 触发时机 #1

## 1. 执行摘要

本次深度审查覆盖 Stage 18.225-18.231 (7 stages) 的全部工作。编译器从 v0.475.0
推进到 v0.480.0, 完成了 TD-C-WRAPPER-OVERUSE 迁移链 (4 个复合 C helpers 全部
迁移到 MIR intrinsics), 新增了 1 个原语 C helper (`__landin_i64_to_str`), 并
识别+修复了 8 个跨阶段 critical bugs (DCE, borrowck, codegen)。

**结论**: **GO** — v0.2 Phase 2 完整, 全校验流通过。但发现 dead code (4 个
已迁移的 C helpers 仍保留在 runtime.rs) 需要清理。

- **0 P0 阻塞项**
- **0 P1 阻塞项**
- **1 P2 active 项** (dead C helpers — 本 stage 修复)

## 2. 八维度审查

### D1. 架构健康度

| 子项 | 状态 |
|------|------|
| MIR intrinsic ops (Load/GEP/Store) | ✅ Stage 18.226-18.227 |
| 4 C helpers → MIR migration | ✅ Stage 18.228-18.231 |
| `__landin_i64_to_str` primitive | ✅ Stage 18.231 |
| §11 接口隔离 (codegen 不依赖 C runtime 内部) | ✅ |
| §1.0 原則 6 (通解>特解) | ✅ — one MIR sequence per operation |
| §10 DRY (reuse primitives, MemoryEmitter) | ✅ |

### D2. 技术债清单

**TD-C-WRAPPER-OVERUSE**: ✅ RESOLVED — all 4 compound C helpers migrated
| Helper | MIR Intrinsic Replacement | Stage |
|--------|---------------------------|-------|
| `__landin_vec_get` | Load + GEP + Assert(BoundsCheck) | 18.228 |
| `__landin_vec_push` | Load + GEP + Store + SwitchInt + Call(realloc) | 18.229 |
| `__landin_string_push_str` | Load + GEP + Store + SwitchInt + while loop + Call(realloc/memcpy) | 18.230 |
| `__landin_format_variadic` | Format string walker loop + Call(alloc/i64_to_str) | 18.231 |

**New primitive added**: `__landin_i64_to_str` (§16.5) — snprintf wrapper

**Critical bugs fixed during migration** (per §17.6 同类型整体修复):
| Bug | Fix | Stage |
|-----|-----|-------|
| DCE didn't collect Load/GEP/Store/Assert reads | Added all 4 variants to collect_rvalue_locals + collect_terminator_read_locals | 18.228 |
| Borrowck didn't handle Load/GEP operands | Added to rvalue_reads + check_rvalue | 18.228 |
| LLVM emit_call didn't coerce arg types | Added LLVMBuildIntCast2 coercion | 18.228 |
| GEP codegen used I32 for all element types | Derive element type from result_ty | 18.228 |
| Borrowck didn't handle StatementKind::Store | Added check_place_write + check_operand | 18.229 |
| PHI-like locals needed Mutability::Mutable | Use new_local_with_mut for PHI-like locals | 18.229 |
| Store codegen didn't handle Deref projection | Added Projection(base, Deref) special case | 18.229 |
| No API to push arbitrary StatementKind | Added push_statement method | 18.229 |

**Remaining TDs** (v0.3+ deferred):
- TD-METHOD-RESOLVE-STRICT (v0.2.3 — needs resolver tracking through typeck defaulting)
- TD-DROP-MOVED-LOCALS (v0.3+ — move tracking in drop elaboration)

### D3. 测试覆盖深度

- **总测试**: 3783 (675 lib + 3108 integration)
- 0 failures, 正负比例 27.8%
- **新增**: 11 lib unit tests for MIR intrinsic ops codegen (Stage 18.227)
- **回归**: All 4 vec_get + 6 vec_push + 6 push_str + 8+8 format tests pass

### D4. 下一阶段就绪度

**v0.2 Phase 2 核心功能完整性**:

| 功能 | 状态 |
|------|------|
| MIR intrinsic ops (Load/GEP/Store) | ✅ |
| 4 C helpers → MIR migration | ✅ |
| `__landin_i64_to_str` primitive | ✅ |
| DCE handles all MIR variants | ✅ |
| Borrowck handles all MIR variants | ✅ |
| Codegen handles all MIR variants | ✅ |
| 全校验流 (LLVM 22.1) | ✅ |

### D5. 设计合理性

✅ — 无过度设计; MVP scope 在每个 stage 都有明确记录 (§17.6)

### D6. 性能与可扩展性

✅ — ~10s test suite (release), 无 O(n²)

### D7. 文档与知识传承

✅ — 7 task-reviews + 7 dev-logs + 1 deep-review (this) + design doc §16.5-§16.6.5

### D8. 测试路径覆盖

✅ — Box/Vec/String/format!/ABI/typeck/borrowck 全覆盖

## 3. Dead Code Identification (P2 — 本 stage 修复)

**问题**: 4 个已迁移的 C helpers 仍保留在 `runtime.rs` 中, 尽管它们不再被
MIR 调用。这违反 §1.0 原則 5 (去除兼容思维) 和 §1.0 原則 6 (避免死代码)。

**受影响文件**:

| File | Dead Code |
|------|-----------|
| `src/codegen/runtime.rs` | 4 C helper definitions (vec_get/push, string_push_str, format_variadic) |
| `src/codegen/llvm/function_sigs.rs` | 4 sig entries in runtime_sigs |
| `src/driver/driver_validations.rs` | 4 DefId registrations (u32::MAX - 103/104/105/106) |
| `tests/v0/stage18/plan/stage18_206_abi_contract_tests.rs` | COMPOUND_ABI_CONTRACTS + related tests |
| `src/codegen/llvm/mod.rs` | `__landin_format_variadic` in variadic check |
| `src/codegen/llvm/aggregate.rs` | `__landin_format_variadic` in variadic check |

**清理计划** (per §1.0 原則 5 去除兼容思维):
1. Remove 4 C helper definitions from runtime.rs
2. Remove 4 sig entries from function_sigs.rs
3. Remove 4 DefId registrations from driver_validations.rs
4. Update ABI contract tests — remove COMPOUND_ABI_CONTRACTS, keep PRIMITIVE_ABI_CONTRACTS
5. Remove `__landin_format_variadic` from variadic checks (no longer needed)
6. Update comments referencing old C helpers

**风险评估**: Low — C helpers are not called from MIR (verified via grep).
The only references are in test files (ABI contract) and comments.

## 4. 委员会投票

| 角色 | 投票 |
|------|------|
| ARCH-A | **GO** (with dead code cleanup) |
| DEV-A | **GO** |
| QA-A | **GO** |
| ALG-C | **GO** |
| SKL-A | **GO** |

**一致通过**: 5/5 GO

## 5. 行动计划

**GO-WITH-CONDITIONS**: Clean up dead code (4 migrated C helpers) before
declaring v0.2 Phase 2 truly complete.

| Action | Files | Status |
|--------|-------|--------|
| Remove dead C helpers from runtime.rs | runtime.rs | Stage 18.232 |
| Remove dead sigs from function_sigs.rs | function_sigs.rs | Stage 18.232 |
| Remove dead DefId registrations | driver_validations.rs | Stage 18.232 |
| Update ABI contract tests | stage18_206_abi_contract_tests.rs | Stage 18.232 |
| Remove format_variadic from variadic checks | llvm/mod.rs + llvm/aggregate.rs | Stage 18.232 |

## 6. 结论

**GO** — v0.2 Phase 2 核心功能完整确认, 但需要清理 dead code 后才能进入 v0.3。

**v0.2 Phase 2 交付物**:
- 3783 tests, 0 failures
- LLVM 22.1 (221) 部署
- 全校验流合规
- 4/4 C helpers migrated to MIR intrinsics
- 1 new primitive (`__landin_i64_to_str`)
- 8 critical bugs fixed (DCE, borrowck, codegen)
- 7 task-reviews + 7 dev-logs + 1 deep-review

**Next**: Dead code cleanup (Stage 18.232), then v0.3 self-hosting preparation.
