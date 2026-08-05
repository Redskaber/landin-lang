# Stage 16.76 测试计划 — Codegen Pipeline Refactoring (3 MUVs)

> **阶段**: Stage 16.76
> **对应代码**: src/codegen/ (3 MUV refactoring)
> **状态**: ✅ Complete

## 1. 测试目标

验证 codegen pipeline 3 个 MUV 重构未破坏任何现有功能。本阶段为结构性重构，不引入新功能，测试目标聚焦于"回归零失败"。

## 2. 覆盖场景

| 场景 | 测试方法 | 极性 | 状态 | 说明 |
|------|---------|------|------|------|
| cargo clean + build | `cargo clean && cargo build --features llvm-backend` | positive | ✅ PASS | v0.262.0 编译成功 |
| 单元测试回归 | `cargo test --lib` | positive | ✅ PASS | 350/350 PASS |
| 集成测试回归 | `cargo test --test all_tests` | positive | ✅ PASS | 2494/2494 PASS |
| 文档格式回归 | `cargo fmt --check` | positive | ✅ PASS | exit 0 |
| Lint 回归 | `cargo clippy --all-targets` | positive | ✅ PASS | 0 warnings |
| MUV-3 文件完整性 | 检查 mir_translation/ 5 文件齐全 | negative | ✅ PASS | mod/types/layouts/places/stdlib.rs 全部存在 |
| MUV-3 函数完整性 | 检查 11 个函数全部迁移 | negative | ✅ PASS | grep 验证全部函数在新位置 |
| MUV-2 文件完整性 | 检查 4 个新文件齐全 | negative | ✅ PASS | pipeline/function/drop_glue/llvm/function_sigs.rs |
| MUV-2 入口保留 | codegen_crate + codegen_crate_to_module 在 mod.rs | negative | ✅ PASS | grep 验证 |
| MUV-1 sub-trait 完整性 | 6 sub-traits 全部定义 | negative | ✅ PASS | Module/Function/Arithmetic/Memory/Aggregate/LocalState Emitter |
| MUV-1 方法数守恒 | 39 methods = 5+8+11+6+5+4 | negative | ✅ PASS | 数学验证 |
| MUV-1 dyn Emitter 兼容 | 20+ 调用点未改 | negative | ✅ PASS | grep `&mut dyn Emitter` 验证 |
| MUV-1 blanket impl | impl<T: ...> Emitter for T 存在 | negative | ✅ PASS | grep 验证 |
| MUV-1 测试 imports | 6 测试文件已加 sub-trait imports | negative | ✅ PASS | grep 验证 |

## 3. 测试统计

- 预期测试数: 14
- 实际测试数: 14
- 正向: 5
- 负向: 9
- 比例: 5:9 ≈ 1:1.8（重构阶段负向检查更多，因无新功能正向测试）
- 覆盖率: 100%

## 4. 依赖

- Stage 16.75 v5.0 process doc（已完成）
- Stage 16.76 design-v2 定稿（已完成）
- LLVM 19 环境就绪

## 5. 结论

回归零失败。3 个 MUV 重构未影响任何代码功能，所有 2844 测试通过，0 warnings。
