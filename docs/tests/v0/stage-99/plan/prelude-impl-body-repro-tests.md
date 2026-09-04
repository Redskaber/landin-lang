# Stage 99 测试计划 — prelude impl body root cause analysis

## 测试目标

验证 TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因分析结果:
1. user code 中 impl method returning String 工作正常 (Stage 98 mangling 修复后)
2. user code 中 impl method returning struct (含 if/else) 工作正常
3. 负向测试覆盖错误恢复 (undefined type, type mismatch, nonexistent method)

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| user impl method returning i32 | `stage99_user_impl_method_returning_string` | 正向 |
| user impl method returning struct | `stage99_user_impl_method_returning_struct` | 正向 |
| undefined type 报错 | `stage99_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage99_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage99_nonexistent_method_errors` | 负向 |

## 关键回归测试

- 验证 v0.637.0 (Stage 98 mangling 修复后) user code 中 impl method returning String/struct 工作
- 验证 v0.637.0 基线 0 failures (prelude Debug impl 未加)
- 为 Stage 100+ 修复提供 baseline 回归测试

## 对应代码

- 测试代码: `tests/v0/stage99/plan/prelude_impl_body_repro_tests.rs`
- 实现代码: `src/codegen/llvm/mod.rs` (LLVMSysEmitter), `src/codegen/emitter/mod.rs` (mir_type_to_emit_type)
- 根因分析: `docs/develop/v0/stage-99/dev-log.md`

## 预期/实际

- 预期测试数: 5
- 实际测试数: 5 ✓
- 覆盖率: user code impl method returning String/struct 100%

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 4687 → 4692 (+5 stage99 repro) |
| 失败数 | 0 |
| ignored | 9 |

## 注意事项

加 `impl Debug for i32 { fn fmt(&self) -> String { String::from_str("debug_i32") } }` 到 prelude 会触发 TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+) — 见 `docs/develop/v0/stage-99/dev-log.md` 根因分析。当前 stage99 5 个测试不在 prelude 加 Debug impl, 而是验证 user code 中相同结构工作正常。
