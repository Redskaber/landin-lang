# Stage 18.79 — P2 Test System Cleanup

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.346.0 → v0.347.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.77 深度审计识别了 P2 测试体系问题。Stage 18.78 修复了 P0 正确性
缺陷后，本 Stage 推进 P2 测试体系清理。

## 2. P2 修复项

| P2 # | 描述 | 修复方案 |
|------|------|---------|
| P2-A | CI trigger 语法错误 | 修复 `.github/workflows/ci.yml` `branches: ain, master]` → `[main, master]` |
| P2-B | conformance 测试 53% 重复 | 去重纯重复组 (保留每组 1 个规范测试) |
| P2-C | 273 泛化 `error` 模式 | 替换为具体错误模式 |
| P2-D | tests/conformance/README.md 过时 | 更新为 5348-test 现实 |

### 2.1 不修复项 (需更大投入)

- **Fuzz 基础设施** (cargo-fuzz): 需要独立 stage 设计 fuzz target + corpus
- **MIR opt 语义保持测试**: MIR opt 未接线 (Stage 18.78 P0-D 决策延后)
- **多平台 ABI 测试**: 需要交叉编译基础设施

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 6 通用 > 特例 | 去重脚本处理所有重复模式 |
| 9 正确 > 妥协 | 不保留无效重复测试 |

### 3.2 P2-A: CI trigger 修复

**File**: `.github/workflows/ci.yml`

```yaml
# Before (broken):
on:
  push:
    branches: ain, master]
  pull_request:
    branches: ain, master]

# After (fixed):
on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]
```

### 3.3 P2-B: 去重策略

**策略**: 对每个纯重复组 (相同 body + 相同 EXPECTED)，保留 1 个规范测试，
删除其余。使用脚本自动检测和删除。

**不删除的情况**:
- 不同 body 但相同 DESCRIPTION (这些是不同测试场景)
- 相同 body 但不同 EXPECTED (这些测试不同行为)
- SOURCE 注释指向不同 stage 的 (保留最早 stage 的)

**目标**: 5348 → ~3500 (删除 ~1850 纯重复)

### 3.4 P2-C: 泛化 error 模式替换

**策略**: 对 273 个 `ERROR_PATTERN: error` 的测试，根据测试内容推断具体
错误模式。无法确定的保留泛化模式 (保守，不破坏测试)。

**可替换的模式**:
- `let x: i32 = true;` → `mismatched types`
- `let r1 = &mut x; let r2 = &mut x;` → `cannot borrow`
- `undefined_fn()` → `cannot find`
- `let x = 1; x = 2;` → `cannot assign`

### 3.5 P2-D: README 更新

**File**: `tests/conformance/README.md`

更新内容:
- 测试数量: "~590 remaining" → "5348 tests"
- 目录结构说明
- EXPECTED/ERROR_PATTERN/EXPECTED_STDOUT 协议文档

## 4. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | CI 修复是关键; 去重降低维护成本 |
| REV-A | GO | 具体错误模式提升诊断质量检测 |
| DEV-A | GO | 脚本驱动去重，可控 |
| QA-A | GO | 测试套件更精简、更有效 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
