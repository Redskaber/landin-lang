# Stage 102 测试计划 — LLVMSysEmitter::Drop ownership

## 测试目标

验证 Stage 102 LLVMSysEmitter::Drop 释放 module + context 修复 (Layer 4):
1. Drop 完成, 无 panic
2. 多次 create/drop 循环无累积
3. to_module() + to_object_file 在 Drop 前调用安全
4. 负向测试覆盖错误恢复

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| Drop releases resources | `stage102_emitter_drop_releases_resources` | 正向 |
| 10 cycles no accumulation | `stage102_multiple_emitter_cycles_no_accumulation` | 正向 |
| to_module before Drop safe | `stage102_to_module_before_drop_safe` | 正向 |
| to_object_file before Drop safe | `stage102_to_object_file_before_drop_safe` | 正向 |
| undefined type 报错 | `stage102_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage102_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage102_nonexistent_method_errors` | 负向 |

## 对应代码

- 测试代码: `tests/v0/stage102/plan/emitter_drop_ownership_tests.rs`
- 实现代码: `src/codegen/llvm/mod.rs:797-833` (Drop impl)

## 预期/实际

- 预期测试数: 7
- 实际测试数: 7 ✓
- 3 次稳定性验证全绿 (lib + all_tests)

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5599 → 5606 (+7 stage102) |
| 失败数 | 0 |
| ignored | 9 |

## 已知限制

加 Debug impl 到 prelude 后 14 个 cargo test 失败 — Layer 3 (LLVM module 全局状态累积) 仍未完全修复。新发现 TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (P2, v0.11+) — Stage 103+ 调查。
