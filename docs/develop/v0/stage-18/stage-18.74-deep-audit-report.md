# Stage 18.74 — Deep Audit Report (Full Pipeline Technical Debt Assessment)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.341.0 (audit only, no code changes)
> **Process**: stage-committee-process.md v5.0 §14 (深度审查) + §13.1 (设计对齐)
> **Status**: ✅ Complete — Audit report with prioritized remediation plan

## 1. 审计范围与方法

本审计对 Landin 编译器 v0.341.0 进行全面深度审查，覆盖：
- **Span::DUMMY 使用审计** (47 文件, 1320 处)
- **编译管道设计审计** (9 阶段 + driver)
- **测试体系完整性审计** (8623 tests)
- **错误系统精确性审计** (8 错误类型)
- **API 命名标准审计** (命名违规检查)

## 2. 审计发现汇总

### 2.1 编译管道健康度: 🟡 中等技术债

| 维度 | 评估 | 详情 |
|------|------|------|
| 阶段隔离 (§11) | ✅ 良好 | codegen 是纯 MIR 消费者，typeck 接收数据表 |
| 错误处理原则 | ⚠️ 中等 | 5 处静默 Error (Stage 0 limitation)，3 处生产 panic! |
| 特解 vs 通解 | ⚠️ 中等 | 4 处硬编码 `__landin_*` 列表，trait 默认体用第一个 impl |
| 死代码 | ⚠️ 轻微 | `validate_main_exists` 死代码，`Println` variant 保留 |
| 健壮性 | ⚠️ 中等 | 30+ 处 `CString::new().unwrap()`，1 处 Range 静默返回 "0" |

### 2.2 Span::DUMMY 使用: 🟡 受控但需持续清理

| 类别 | 数量 | 优先级 |
|------|------|--------|
| 测试代码 | ~783 (59%) | LOW (可接受) |
| 宏展开合成 token | ~351 (27%) | LOW (设计如此，展开时重写) |
| 类型构造 (MIR/HIR) | ~110 (8%) | MEDIUM (有 span 可用但未传递) |
| **错误报告** | **~14 (1%)** | **HIGH (必须修复)** |
| 防御性 `!= Span::DUMMY` 检查 | ~15 | N/A (合法) |
| 注释中提及 | ~38 | N/A |

### 2.3 测试体系: 🟡 体量大但结构有缺陷

| 测试类型 | 状态 | 详情 |
|---------|------|------|
| 功能正确性测试 | ✅ 强 | 4682 正向 conformance + 3251 正向 Rust test |
| 语言标准合规性 | ⚠️ 部分 | 804 "Stage 0 limitation" 测试，无 rustc 差分测试 |
| 优化正确性测试 | ❌ 弱 | 16 结构测试，0 端到端语义保持测试 |
| 鲁棒性/压力测试 | ⚠️ 最小 | 8 稳定性测试 (gated)，0 大文件测试 |
| 性能/基准测试 | ⚠️ 最小 | 5 基准测试 (无 criterion)，无二进制大小追踪 |
| 诊断信息质量 | ⚠️ 部分 | 273/598 负向测试用泛化 `error` 模式 (46%) |
| 目标平台/ABI | ❌ 单平台 | 仅 x86_64-linux，无 aarch64/Windows/macOS |
| 破坏性/fuzz 测试 | ❌ 缺失 | 0 fuzz 基础设施，8 手动 fuzz 测试 (gated) |

**关键发现**: 5348 conformance 测试中 **2818 (53%) 是纯重复** — 405 个重复组，最大组 49 份相同测试。

### 2.4 错误系统精确性: 🟡 中等

| 错误类型 | Kind enum? | Span? | ErrorCode? | Spanned? |
|---------|-----------|-------|-----------|----------|
| LexError | ❌ String | ✅ | E001 | ✅ |
| ParseError | ❌ String | ✅ | E100 | ✅ |
| LowerError | ❌ String | ✅ | E200 | ✅ |
| ResolveError | ✅ 8 kinds | ✅ | E300 | ✅ |
| TypeError | ✅ 6 kinds | ✅ | E400 | ✅ |
| BorrowError | ✅ 9 kinds | ✅ | E500 | ✅ |
| TraitError | ✅ enum | ✅ | E600 | ❌ |
| CodegenError | ❌ String | ✅ | ❌ 缺失 | ❌ 缺失 |
| MacroError | ❌ String | ✅ | ❌ 缺失 | ❌ 缺失 |

**关键发现**:
- `LowerError` 有完整实现但 **CompileErrors 无 lower 字段** → HIR lowering 错误被静默丢弃
- `CodegenError` 和 `MacroError` 无 ErrorCode → 错误码目录不完整
- `MacroError` 被收集但 `to_diagnostics_with_resolver` **从不迭代 macro_errors** → 宏错误对用户不可见
- 5 处 `{:?}` Debug 格式泄露到用户消息中

### 2.5 API 命名标准: ✅ 强

- 11 处 `get_` 前缀 (Rust 惯例禁止)
- 6 处名词作为访问器 (`owner()`, `body()`, `local()`)
- `IncompleteImpl` 缺少 `Error` 后缀
- `format_for_user` 是遗留名 (已标记 deprecated)
- ~30 处 `pub fn` 应降级为 `pub(crate)`

## 3. Top 20 关键技术债 (按优先级)

### P0 — 正确性缺陷 (静默错误丢失)

| # | 位置 | 问题 | 影响 |
|---|------|------|------|
| 1 | `driver.rs:161` CompileErrors | 无 `lower`/`codegen` 字段 | HIR lowering + codegen 错误被静默丢弃 |
| 2 | `driver.rs:254-354` to_diagnostics | 不迭代 `macro_errors` | 宏错误对用户不可见 |
| 3 | `diagnostics/mod.rs:42` ErrorCode | 缺 Codegen (E700) + Macro (E800) | 错误码目录不完整 |
| 4 | `codegen/llvm/*.rs` (30+ 处) | `CString::new().unwrap()` | NUL 字节导致 panic |
| 5 | `codegen/rvalue.rs:521` | `BinaryOp2` 静默返回 "0" | Range 表达式静默错误编译 |
| 6 | `typeck/unify.rs:368-370` | `Param` 与任何类型 unify | 泛型不安全 (`Vec<i32> = Vec<bool>::new()` 通过) |

### P1 — 健壮性 + 错误精确性

| # | 位置 | 问题 | 影响 |
|---|------|------|------|
| 7 | `typeck/checker.rs:1070,1121,1129` | 3 处静默 `Ty::Error` (Deref/Index on wrong type) | 无 TypeError 报告 |
| 8 | `mir/lower/mod.rs:738,757` | 生产 `panic!` for And/Or/Deref | 编译器崩溃而非报错 |
| 9 | `borrowck/region_inference.rs:718,750` | `_ => LocalId(0)` 静默降级 | 区域约束错误 |
| 10 | 5 处 `{:?}` Debug 泄露 | 用户消息含 Debug 格式 | 诊断质量差 |
| 11 | `driver.rs:66` TraitError 位置 | 定义在 driver 而非 traits/ | 违反单一数据源 |
| 12 | 5 个错误类型无 Kind enum | LexError/ParseError/LowerError/CodegenError/MacroError | 不可机器分类 |
| 13 | `hir/lower/item.rs:66-72` | MacroRules 静默丢弃 | 占位符 hack |

### P2 — 测试体系 + 命名

| # | 位置 | 问题 | 影响 |
|---|------|------|------|
| 14 | tests/conformance/ | 2818/5348 (53%) 纯重复测试 | 维护成本高 |
| 15 | tests/conformance/ | 273 负向测试用泛化 `error` 模式 | 无法检测诊断回归 |
| 16 | 无 fuzz 基础设施 | 0 cargo-fuzz/AFL | 健壮性未验证 |
| 17 | 无 MIR opt 语义保持测试 | 仅结构测试 | 优化正确性未验证 |
| 18 | 11 处 `get_` 前缀 | 违反 Rust 命名惯例 | API 不一致 |
| 19 | `.github/workflows/ci.yml:9` | `branches: ain, master]` 语法错误 | CI 可能未触发 |
| 20 | `format_for_user` 遗留名 | 已 deprecated 但仍存在 | API 混乱 |

## 4. 修复计划 (Stage 18.75+)

### Stage 18.75: P0 错误系统修复 (正确性)
1. 添加 `CompileErrors.lower` + `CompileErrors.codegen` 字段
2. `to_diagnostics_with_resolver` 迭代 `macro_errors`
3. 添加 `ErrorCode::Codegen` (E700) + `ErrorCode::Macro` (E800)
4. 修复 30+ `CString::new().unwrap()` → 使用 `cstr_result`
5. 修复 `BinaryOp2` 静默 "0" → 报错
6. 修复 `Param` unify 不安全 → 拒绝非匹配类型

### Stage 18.76: P1 健壮性 + 错误精确性
7. 推送 3 处静默 `Ty::Error` TypeError
8. 替换 2 处生产 `panic!` 为 `Result` 返回
9. 修复 `LocalId(0)` 静默降级
10. 修复 5 处 `{:?}` Debug 泄露
11. 移动 `TraitError` 到 `traits/error.rs`
12. 添加 5 个错误类型的 Kind enum

### Stage 18.77: P2 测试体系清理
13. 去重 conformance 测试 (5348 → ~2530)
14. 替换 273 泛化 `error` 模式为具体模式
15. 添加 cargo-fuzz 基础设施
16. 添加 MIR opt 语义保持测试
17. 修复 CI trigger 语法错误

### Stage 18.78: P2 API 命名 + Span::DUMMY 清理
18. 重命名 11 处 `get_` 前缀函数
19. 重命名 6 处名词作为访问器方法
20. 降级 ~30 处 `pub fn` 为 `pub(crate)`
21. 清理 14 处 HIGH 优先级 Span::DUMMY (错误报告)

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 审计全面，修复计划清晰 |
| REV-A | GO | P0 正确性缺陷必须优先修复 |
| DEV-A | GO | 分阶段实施可控 |
| QA-A | GO | 测试去重 + fuzz 基础设施是关键改进 |
| PM-A | GO | 路线图明确 |

**5/5 GO** ✅ — 审计报告通过，进入 Stage 18.75 修复。

## 6. 当前编译器能力边界 (基于审计)

### 已支持 (正向覆盖)
- 基本类型 (int/uint/float/bool/char/str)
- 结构体/枚举/元组/数组
- 函数/闭包/generic fn
- trait 定义/impl/dyn Trait
- 模式匹配 (let/match/嵌套)
- 所有权/借用/NLL
- macro_rules! (9 fragment specifiers)
- LLVM codegen (x86_64-linux)
- 类型检查 (let/return/if-branch/match-arm mismatch)
- trait impl signature 校验
- struct field count 校验
- tuple index bounds 校验
- pattern arity 校验
- array index type 校验
- assignment target 校验
- cast type 校验
- missing main 检测
- associated const 完整性校验

### Stage 0 限制 (804 测试记录)
- 595 处: 编译器接受 Rust 拒绝的代码 (lax check)
- 209 处: 编译器拒绝 Rust 接受的代码 (over-strict)
- Param unify 过度宽松
- 3 处静默 Ty::Error (Deref/Index 投影)
- trait 默认体用第一个 impl 特化
- 迭代 typeck 丢弃中间错误

### 不支持
- 交叉编译 (仅 x86_64-linux)
- 自举 (远期目标)
- 过程宏
- async/await (语法支持但无 runtime)
- 完整标准库
