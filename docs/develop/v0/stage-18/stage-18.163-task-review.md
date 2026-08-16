# Stage 18.163 — 任务审查 + 任务排版图重排

> **Author**: redskaber (PM-A + ARCH-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.431.0 (Stage 18.163 任务审查报告)
> **Process**: docs/stage-committee-process.md v6.4 §17 (任务规划排版图) + §5.1 (复杂度预评估)
> **Task ID**: stage18.163

## 1. 任务审查背景

用户要求: "在开始选择处理的任务时，应当先做任务审查（即，当前项目的能力是否具备处理当前任务，是否为最佳处理时机等，如果不能则应该重构重排任务排版图）"

本 stage 对所有候选任务进行能力审查, 评估项目是否具备处理条件, 重排任务优先级。

## 2. 候选任务审查

### 2.1 TD-STDLIB-FACADE (String/Vec/Option/Result 真实实现)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| 堆分配支持 | ❌ 不具备 | codegen 无 malloc/calloc/free 调用; LLVM IR 无 heap allocation |
| Iterator 支持 | ❌ 不具备 | 仅有 trait 名称注册, 无 Iterator::next codegen |
| Box::new 支持 | ❌ 不具备 | 无 Box::new codegen (仅 struct Box { val: T } 字面构造) |
| Drop/Free 支持 | ✅ 部分具备 | drop_glue.rs 有 drop 函数生成, 但仅栈对象 |
| String 类型 | ❌ 不具备 | Str 是 fat pointer {ptr, len}, 但无堆分配的 String |
| Vec 类型 | ❌ 不具备 | Array 是栈分配 [N x T], 无堆分配的 Vec |
| Option/Result | 🟡 部分具备 | 可用 enum 实现, 但依赖泛型 monomorphization (已具备) |

**结论**: **不具备处理能力** — String/Vec 需要 heap allocation, 当前 codegen 无此能力。Option/Result 可部分实现 (enum + 泛型), 但 String/Vec 需要先实现 heap allocation。

**最佳时机**: 实现 heap allocation (malloc/free codegen) 后, 再实现 String/Vec。Option/Result 可先实现 (不依赖 heap)。

### 2.2 TD-NO-FORMAT-MACRO (format! 宏)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| println! 已有 | ✅ 具备 | println!/print!/eprintln!/eprint! 已实现 (Stage 18.27) |
| 格式化参数 | ✅ 具备 | println!("{} {}", 1, 2) 已支持 |
| String 类型 | ❌ 不具备 | format! 返回 String, 但 String 未实现 (依赖 TD-STDLIB-FACADE) |
| Display trait | ❌ 不具备 | 无 Display trait 实现 |

**结论**: **不具备处理能力** — format! 返回 String, 依赖 TD-STDLIB-FACADE 的 String 实现。

**最佳时机**: TD-STDLIB-FACADE 实现后。

### 2.3 TD-LINUX-ONLY (跨平台支持)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| TargetTriple | ✅ 具备 | Stage 18.89 已实现 TargetTriple |
| 交叉编译 | ✅ 具备 | codegen_crate_to_module_with_target 已实现 |
| Windows/macOS 测试 | ❌ 不具备 | 无 CI 环境, 无法验证 |

**结论**: **具备处理能力** — 但无法验证 (无 Windows/macOS 环境)。

**最佳时机**: 低优先级, 等有 CI 环境时处理。

### 2.4 TD-NO-INCREMENTAL (增量编译)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| 项目系统 | ✅ 具备 | compile_project + ModuleLoader 已实现 |
| 文件哈希 | ❌ 不具备 | 无文件修改时间/哈希跟踪 |
| 缓存系统 | ❌ 不具备 | 无 MIR/对象文件缓存 |
| 依赖图 | ❌ 不具备 | 无模块依赖图构建 |

**结论**: **不具备处理能力** — 需要先实现文件哈希 + 缓存系统 + 依赖图。

**最佳时机**: 实现缓存基础设施后。

### 2.5 TD-INT-UINT-VAR (IntOrUintVar 分离)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| Unification table | ✅ 具备 | typeck/unify.rs 有 unify table |
| IntVar/UintVar | ✅ 具备 | 当前 IntVar 同时表示 Int 和 Uint |
| types_match_loose | ✅ 具备 | 有 hardcoded Int↔Uint 匹配 |
| 影响面 | 🟡 中等 | 需修改 unify table + typeck infer + writeback |

**结论**: **具备处理能力** — 但影响面中等, 需谨慎。

**最佳时机**: 可处理, 但优先级低于测试覆盖达标。

### 2.6 TD-DEREF-NON-REF (pattern bindings 引用类型跟踪)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| Pattern binding | ✅ 具备 | parser/hir/mir 有 pattern 支持 |
| Reference type | ✅ 具备 | TyKind::Ref 已实现 |
| 影响面 | 🟡 中等 | 需修改 pattern lowering + typeck |

**结论**: **具备处理能力** — 可处理。

**最佳时机**: 可处理, 中等优先级。

### 2.7 负面测试达标 (剩余 ~40 个)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| 测试框架 | ✅ 具备 | tests/ 结构完善 |
| 当前比例 | 🟡 22.9% | 接近 25% 目标 |
| 可补充领域 | ✅ 具备 | vtable/drop_glue/monomorphization/closure 等 |

**结论**: **具备处理能力** — 可立即处理。

**最佳时机**: 现在处理, 快速达标。

### 2.8 TD-SPAN-DUMMY-CLEANUP (剩余评估)

**审查维度**:

| 维度 | 评估 | 详情 |
|------|------|------|
| Span 类型 | ✅ 具备 | Span = (u32, u32) |
| 剩余 Span::DUMMY | 🟡 评估完成 | 大部分为合法合成用法 (builtin_macros 349 处) |
| 可清理项 | 🟡 少量 | mir/lower/expr_operand.rs 等 ~17 处可评估 |

**结论**: **具备处理能力** — 但大部分已评估为合法, 剩余可清理项少。

**最佳时机**: 可处理, 低收益。

## 3. 任务排版图重排

基于审查结果, 重排任务优先级:

### 3.1 原排版图 (Stage 18.162 末尾规划)

```
v0.2 P1:
  1. TD-STDLIB-FACADE (String/Vec/Option/Result)
  2. TD-NO-FORMAT-MACRO (format!)
  3. 补充负面测试达标

v0.2 P2:
  4. TD-LINUX-ONLY
  5. TD-NO-INCREMENTAL
  6. TD-INT-UINT-VAR
  7. TD-DEREF-NON-REF
```

### 3.2 新排版图 (审查后重排)

```
Stage 18.163 (本 stage):
  → 任务审查 + 排版图重排 (无代码修改, 文档输出)

Stage 18.164 (下一步, 立即可做):
  → 补充负面测试达标 25% (可立即处理, 低风险)
  → 补充 vtable/drop_glue/monomorphization 负面测试 ~40 个

Stage 18.165 (Option/Result 实现, 不依赖 heap):
  → 实现 Option<T> enum (Some/None)
  → 实现 Result<T, E> enum (Ok/Err)
  → 基本方法: unwrap, is_some, is_ok, map, and_then
  → 不需要 heap allocation (enum 是栈分配)

Stage 18.166-18.168 (heap allocation 基础设施):
  → Stage 18.166: codegen 添加 malloc/free 调用 (LLVM IR)
  → Stage 18.167: Box::new 实现 (调用 malloc)
  → Stage 18.168: Box deref + drop (调用 free)

Stage 18.169-18.171 (String/Vec 实现, 依赖 heap):
  → Stage 18.169: Vec<T> 实现 (基于 heap allocation)
  → Stage 18.170: String 实现 (基于 Vec<u8>)
  → Stage 18.171: format! 宏 (基于 String)

v0.2 P2 (后续):
  → TD-LINUX-ONLY (需 CI 环境)
  → TD-NO-INCREMENTAL (需缓存基础设施)
  → TD-INT-UINT-VAR (影响面中等)
  → TD-DEREF-NON-REF (影响面中等)
```

### 3.3 重排原因

| 原任务 | 重排原因 | 新位置 |
|--------|---------|--------|
| TD-STDLIB-FACADE | 不具备 heap allocation 能力, 拆分为 Option/Result (不依赖 heap) + String/Vec (依赖 heap) | 18.165 (Option/Result) + 18.169-18.171 (String/Vec) |
| TD-NO-FORMAT-MACRO | 依赖 String 实现 | 18.171 (在 String 之后) |
| 负面测试达标 | 可立即处理, 低风险 | 18.164 (提前) |
| TD-LINUX-ONLY | 需 CI 环境验证 | v0.2 P2 (推迟) |
| TD-NO-INCREMENTAL | 需缓存基础设施 | v0.2 P2 (推迟) |

## 4. 简写和缺陷记录

### 4.1 任务审查简写

**简写1**: TD-STDLIB-FACADE 原计划整体实现, 但审查发现 String/Vec 依赖 heap allocation, 拆分为 Option/Result (不依赖 heap) + String/Vec (依赖 heap)。
- **原因**: codegen 无 malloc/free 支持, 无法实现堆分配类型。
- **修订计划**: 18.165 实现 Option/Result, 18.166-18.168 实现 heap 基础设施, 18.169-18.171 实现 String/Vec。

**简写2**: 负面测试达标原计划在 stdlib 实现后, 但审查发现可立即处理 (不依赖 stdlib)。
- **原因**: 负面测试覆盖编译器错误路径, 不依赖 stdlib 实现。
- **修订计划**: 18.164 立即补充 ~40 个负面测试达标 25%。

### 4.2 能力缺口记录

| 缺口 | 影响任务 | 修复计划 |
|------|---------|---------|
| heap allocation (malloc/free) | String/Vec/Box 实现 | Stage 18.166 |
| Iterator trait codegen | for loop on Vec | Stage 18.169+ |
| Display trait | format! 宏 | Stage 18.171 |
| 文件哈希/缓存 | 增量编译 | v0.2 P2+ |
| CI 环境 | 跨平台验证 | 外部依赖 |

## 5. §3.2 验收

本 stage 为任务审查报告, 无代码修改, 验收基于上 stage (v0.430.0) 状态:
- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 638 passed, 0 failed

## 6. Stage Summary

- **Stage 18.163 PASSED** — 任务审查 + 任务排版图重排
- **审查范围**: 8 个候选任务 (stdlib/format/cross-platform/incremental/int-uint-var/deref/测试/span-dummy)
- **结论**: TD-STDLIB-FACADE 不具备整体处理能力 (缺 heap allocation), 拆分为 Option/Result + String/Vec
- **重排**: 负面测试达标提前 (18.164), Option/Result 先行 (18.165), heap 基础设施 (18.166-18.168), String/Vec (18.169-18.171)
- **能力缺口**: heap allocation, Iterator codegen, Display trait, 文件哈希/缓存, CI 环境
- **v0.431.0**: patch bump (任务审查, 无代码修改)
- **下一步**: Stage 18.164 补充负面测试达标 25%
