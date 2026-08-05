# Stage 16.77 测试计划 — Backend File Organization + Graph Sync + Design Writeback

> **阶段**: Stage 16.77
> **对应代码**: src/codegen/llvm/ + src/codegen/text/ + docs/graph/codegen/ + docs/lang-design/07-codegen.md
> **状态**: ✅ Complete

## 1. 测试目标

验证 backend 文件组织重构（4 MUVs）未破坏任何现有功能。本阶段为纯文件重组 + 文档更新，测试目标聚焦于"回归零失败"。

## 2. 覆盖场景

| 场景 | 测试方法 | 极性 | 状态 | 说明 |
|------|---------|------|------|------|
| cargo clean + build | `cargo clean && cargo build --features llvm-backend` | positive | ✅ PASS | v0.263.0 编译成功 |
| 单元测试回归 | `cargo test --lib` | positive | ✅ PASS | 349/349 PASS |
| 集成测试回归 | `cargo test --test all_tests` | positive | ✅ PASS | 2494/2494 PASS |
| 文档格式回归 | `cargo fmt --check` | positive | ✅ PASS | exit 0 |
| Lint 回归 | `cargo clippy --all-targets` | positive | ✅ PASS | 0 warnings |
| MUV-1 llvm 文件完整性 | 9 文件齐全 | negative | ✅ PASS | mod/module/function/arithmetic/memory/aggregate/local_state/helpers/function_sigs/tests |
| MUV-1 llvm 方法数守恒 | 39 methods = 5+8+11+6+5+4 | negative | ✅ PASS | 数学验证 |
| MUV-1 helpers visibility | 7 helpers 全部 pub(crate) | negative | ✅ PASS | cstr/is_float/parse_*/collect_cstring |
| MUV-2 text 文件完整性 | 7 文件齐全 | negative | ✅ PASS | mod/module/function/arithmetic/memory/aggregate/local_state |
| MUV-2 text 方法数守恒 | 39 methods = 5+8+11+6+5+4 | negative | ✅ PASS | 数学验证 |
| MUV-3 graph 更新 | emitter-trait.md + architecture.md 已更新 | negative | ✅ PASS | v0.263.0 版本标记 |
| MUV-4 设计回写 | 07-codegen.md §16 已补写 | negative | ✅ PASS | 6 小节齐全 |
| dyn Emitter 兼容 | 20+ 调用点未改 | negative | ✅ PASS | grep `&mut dyn Emitter` 验证 |

## 3. 测试统计

- 预期测试数: 13
- 实际测试数: 13
- 正向: 5
- 负向: 8
- 比例: 5:8 ≈ 1:1.6（重构阶段负向检查更多）
- 覆盖率: 100%

## 4. 依赖

- Stage 16.76 完成（6 子 trait 拆分）
- LLVM 19 环境就绪
- rustup + cargo 已安装

## 5. 结论

回归零失败。4 个 MUV 重构未影响任何代码功能，所有 2843 测试通过，0 warnings。
