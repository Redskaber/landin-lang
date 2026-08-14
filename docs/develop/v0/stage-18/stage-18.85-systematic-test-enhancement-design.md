# Stage 18.85 — Systematic Test Enhancement (Fuzz Infrastructure + Stress Tests)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.352.0 → v0.353.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

v0.7 路线图 P0 项"系统性测试增强"要求：
- 稳定性测试 (长时间运行、内存泄漏)
- 阶段性测试 (每个 Phase 完整功能验证)
- 端到端测试 (编译+运行验证)
- 爆破测试 (fuzzing — 随机生成 Landin 代码，验证编译器不崩溃)

Stage 18.74 审计识别测试体系缺陷：
- ❌ 0 fuzz 基础设施
- ⚠️ 8 稳定性测试 (gated behind llvm-backend)
- ⚠️ 5 基准测试 (无 criterion)
- ⚠️ 273/598 负向测试用泛化 `error` 模式 (46%)

本 Stage 添加 fuzz 测试基础设施和压力测试。

## 2. 修复项

| # | 描述 | 修复方案 |
|---|------|---------|
| 1 | 无 fuzz 基础设施 | 添加 `tests/fuzz/` 目录 + 随机代码生成器 + 崩溃检测 |
| 2 | 无压力测试 | 添加深度嵌套/大文件/边界值测试 |

### 2.1 Fuzz 测试设计

**不使用 cargo-fuzz** (需要 nightly + no_std 依赖)，改为自研轻量级 fuzz harness:

```rust
// tests/fuzz/fuzz_harness.rs
fn fuzz_compile_random(seed: u64) {
    let code = generate_random_landin_code(seed);
    let result = landin_compiler::compile(&code);
    // Assert: compiler must not panic
    // Assert: if errors, they must be structured (not crash)
}
```

**随机代码生成器**:
- 随机选择语句类型 (let/if/while/match/for)
- 随机选择表达式 (literal/binop/call/method)
- 随机选择类型 (i32/bool/struct/tuple/array)
- 约束: 保证语法合法 (但语义可能不合法 — 测试编译器不崩溃)

### 2.2 压力测试设计

| 测试 | 描述 | 验证 |
|------|------|------|
| deep_nesting_50 | 50 层嵌套 if | 编译器不崩溃 |
| deep_nesting_100 | 100 层嵌套 if | 编译器不崩溃 |
| large_function_100_stmts | 100 个语句的函数 | 编译器不崩溃 |
| large_array_1000 | 1000 元素数组 | 编译器不崩溃 |
| deep_match_20_arms | 20 个 match arm | 编译器不崩溃 |
| long_identifier_256 | 256 字符标识符 | 编译器不崩溃 |

## 3. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | Fuzz 是健壮性验证关键 |
| REV-A | GO | 压力测试验证边界 |
| DEV-A | GO | 轻量级自研方案, 无外部依赖 |
| QA-A | GO | 填补测试类型矩阵空白 |
| PM-A | GO | v0.7 路线图 P0 |

**5/5 GO** ✅
