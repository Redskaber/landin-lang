# Stage 18.207 — Task Review + v0.2 Phase 2 Task Re-Plan

> **Date**: 2026-08-17
> **Version**: v0.470.0 (no bump — audit only)
> **Task ID**: stage18.207
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A (Stage Committee)
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 (设计对齐) + §17.6 (缺陷纳入) + §17.7 (审查规划图缺陷) + §17.8 (Step 7 优化补充)

## 1. 触发场景

Per user directive: "如果当前设计和实现存在简写和缺陷或MVP（时机： 此条例触发时机），
则需要将简写和缺陷的原因及描述等必要信息记录在开发、设计文档中并规划修订完整计划"

Per Stage 18.204 deep review §5.2 action plan: 下一个计划任务是 "类型 3 组整体修复:
TD-TYPECK-GENERIC-INST + TD-INT-UINT-VAR + TD-TUPLE-CTOR-TYPECK"。

本 stage 对该计划做 §17.7/§17.8 任务审查，验证依赖完整性与任务定位准确性。

## 2. §13.1 设计对齐

### 2.1 设计文档定位

| 设计文档 | 章节 | 内容 |
|---------|------|------|
| `docs/lang-design/03-type-system.md` | §13 (monomorphization) | "Landin 的泛型采用 monomorphization（静态分发）" |
| `docs/lang-design/06-mir.md` | TyKind::Adt(DefId, SubstsRef) | MIR 类型槽位支持 substs |
| `docs/develop/v0/task-11-monomorphization-design.md` | Phase 1-4c | 完整的 monomorphization 设计 + 实现 |

### 2.2 设计意图摘要

Per `03-type-system.md` §13: Landin 采用 monomorphization（静态分发），在编译期为
每个具体类型实例化生成专用代码。这是 `Vec<T>`, `HashMap<K,V>` 等泛型类型的基础。

Per `task-11-monomorphization-design.md` §2.1: 泛型语法解析已完整实现（parser →
AST → HIR → MIR 类型槽位）。Task 11 Phase 1-3 + 4c 已 COMPLETE。

### 2.3 已实现项 vs 偏差项

| 设计要求 | 实现状态 | 证据 |
|---------|---------|------|
| `let x: Vec<i32>` 产生 `Adt(Vec_def_id, [i32])` | ✅ 已实现 | Task 11 Phase 1 COMPLETE |
| `substitute(ty, substs)` 函数 | ✅ 已实现 | Task 11 Phase 2 COMPLETE |
| Monomorphization 收集 pass | ✅ 已实现 | Task 11 Phase 3 COMPLETE |
| Per-mono codegen | ✅ 已实现 | Task 11 Phase 4c COMPLETE |
| `Vec<Point>::push` 正确计算 elem_size | ✅ 已实现 | 实测: elem_size=8 (Point={i32,i32}) |
| `Vec<Point>::get(0).x` 正确返回字段 | ❌ 未实现 | 实测: LLVM GEP error (out_ty=i32 hardcoded) |
| `Box<Point>` typeck 通过 | ❌ 未实现 | 实测: "expected u8, found Point" |

## 3. 依赖与基础设施完整能力审查

Per user directive: "如果在设计和开发过程中设计的内容需要依赖底层实现和功能时，应当先做
依赖与基础设施完整能力审查"

### 3.1 计划任务: TD-TYPECK-GENERIC-INST

**Tech-debt-register 原描述**:
> typeck 不解析 Vec<T>/Box<T> 的泛型实例 | typeck unify table 不支持 generic instantiation

**实测验证**:

| 测试用例 | 结果 | 分析 |
|---------|------|------|
| `let v: Vec<Point> = Vec::new(); v.push(p);` | ✅ 编译+运行成功 | substs 已传播，elem_size=8 正确 |
| `let v: Vec<Point> = Vec::new(); v.get(0).x;` | ❌ LLVM GEP error | `lower_vec_get_intrinsic` 硬编码 `out_ty = i32` |
| `let b: Box<Point> = Box::new(p); b.x;` | ❌ typeck error | typeck 不替换 tuple struct 字段类型 |

**结论**: TD-TYPECK-GENERIC-INST **标签不准确**。

实际根因分析:
1. **Vec<Point>::get 失败**: 不是 typeck generic instantiation 问题。typeck 已正确
   传播 substs（Vec<Point> 的 MIR 类型是 `Adt(Vec_def_id, [Point])`）。问题在
   `lower_vec_get_intrinsic` (expr_variants.rs:2207) 硬编码 `out_ty = i32`，没有
   从 `Vec<T>` 的 substs 提取元素类型 `T`。这是**局部 MIR lower bug**，不是 typeck
   基础设施缺失。
2. **Box<Point> 失败**: 是 REAL typeck 问题。Box 定义为 `struct Box<T>(*mut T)`
   (prelude.rs:85)，typeck 不替换 tuple struct 字段类型 `*mut T` → `*mut Point`。
   这是 **typeck tuple struct field substitution 缺失**。

### 3.2 依赖完整性结论

| 依赖项 | 状态 | 说明 |
|--------|------|------|
| Task 11 monomorphization Phase 1-3 | ✅ 完整 | substs 传播 + 替换 + 收集全部完成 |
| Task 11 Phase 4c per-mono codegen | ✅ 完整 | pipeline-integrated |
| `compute_type_size_with_fallback` | ✅ 完整 | Stage 18.203 已实现，支持 Adt HIR walk |
| `generics_of` query | ✅ 完整 | Task 11 Phase 1a |
| typeck tuple struct field substitution | ❌ 缺失 | TD-TUPLE-CTOR-TYPECK (real typeck issue) |
| MIR lower Vec::get element type extraction | ❌ 缺失 | TD-VEC-GET-TYPE-INFERENCE (localized bug) |

**结论**: 计划任务 "类型 3 组整体修复" **定位不准确**。需要重新拆分为两个独立任务:

1. **TD-VEC-GET-TYPE-INFERENCE** (局部 MIR lower fix, doable NOW)
   - 不依赖 typeck 基础设施改进
   - 不依赖 v0.2 工作
   - 只需修改 `lower_vec_get_intrinsic` 提取 `Vec<T>` 的 substs[0] 作为 out_ty

2. **TD-TUPLE-CTOR-TYPECK** (real typeck generic substitution, v0.2)
   - 依赖 typeck 重构（tuple struct field type substitution）
   - 这是真正的 v0.2 Phase 2 工作

## 4. 任务图重排 (per §17.3 + §17.8)

### 4.1 原计划任务图 (Stage 18.204 §5.2)

```
v0.2 Phase 2:
  1. TD-TYPECK-GENERIC-INST (类型 3 组整体修复) ← MISLABELED
  2. TD-DROP-MOVED-LOCALS (类型 2 组)
  3. TD-FUNCTION-REDEFINE-PARAMS ← Already fixed in Stage 18.205
  4. 类型 2/3 组剩余
  5. TD-C-WRAPPER-OVERUSE 迁移
  6. typeck 加严
```

### 4.2 重排后任务图 (Stage 18.207)

```
立即修复 (v0.1 continuation, doable NOW):
  18.208: TD-VEC-GET-TYPE-INFERENCE fix
    - 提取 Vec<T> substs[0] 作为 out_ty
    - 不依赖 typeck 改进
    - 解锁 Vec<Point>::get(0).x + Vec<Point>::get(0).field
  18.209: deep review §14.5 (close 18.208 chain)

v0.2 Phase 2 (real typeck work):
  v0.2.1: TD-TUPLE-CTOR-TYPECK fix
    - typeck tuple struct field type substitution
    - `Box<T>(*mut T)` → `Box<Point>(*mut Point)` when Box<Point> used
    - 解锁 Box<Point> + Box<MyStruct> 测试
  v0.2.2: TD-INT-UINT-VAR fix
    - typeck Int/Uint 变量统一 (separate IntOrUintVar)
  v0.2.3: 类型 2 组 (TD-DROP-MOVED-LOCALS + TD-BOX-AUTO-DROP + TD-VEC-PUSH-SHARED-BORROW)
    - drop elaboration 重构
  v0.2.4: TD-C-WRAPPER-OVERUSE 迁移
    - MIR intrinsic ops 设计 (Alloc/Copy/Branch)
    - 复合 C helper → MIR intrinsic 展开
  v0.2.5: typeck 加严 (TD-GENERIC-PARAM-CHECK, TD-TUPLE-FIELD-CHECK, TD-METHOD-RESOLVE-STRICT)

v0.3+ (self-hosting):
  v0.3.x: stage-1 Landin 重写 (C helpers → Landin stdlib)
```

### 4.3 重排理由 (per §17.8 审查规则)

| 审查项 | 原计划 | 重排后 | 理由 |
|--------|-------|--------|------|
| 任务遗漏 | TD-VEC-GET-TYPE-INFERENCE 未独立 | 独立为 18.208 | 该 TD 是局部 bug，不需要等 v0.2 |
| 依赖完整性 | "类型 3 组" 混合了 MIR lower bug + typeck issue | 拆分为 18.208 + v0.2.1 | 依赖不同，不应一起做 |
| 缺陷纳入 | TD-VEC-GET-TYPE-INFERENCE 已在 code TODO 中 | 升级为独立 task | per §17.6 缺陷纳入规则 |
| 能力边界 | 原计划超出当前能力 (typeck 重构未就绪) | 18.208 在能力范围内 | typeck generic substitution 设计未完成 |
| 递归合理性 | 类型 3 组 3 个 TD 一起做 | 拆分为 2 个独立 task | TD-TYPECK-GENERIC-INST 标签不准确 |

## 5. §17.8 Step 7 审查结论

| 检查项 | 结果 | 说明 |
|--------|------|------|
| 任务遗漏 | ⚠️ 发现遗漏 | TD-VEC-GET-TYPE-INFERENCE 应独立为 18.208 |
| 依赖完整性 | ⚠️ 依赖混淆 | TD-TYPECK-GENERIC-INST 混合了 MIR lower bug + typeck issue |
| 缺陷纳入 | ⚠️ 缺陷未纳入 | TD-VEC-GET-TYPE-INFERENCE 已在 code TODO 但未升级为独立 task |
| 测试覆盖 | ✅ 充分 | Stage 18.203 elem_size tests + Stage 18.205 format tests + Stage 18.206 ABI tests |
| 能力边界 | ⚠️ 超出能力 | "类型 3 组整体修复" 需要 typeck 重构，设计未完成 |
| 递归合理性 | ✅ 合理 | 拆分后递归深度 ≤ 2 层 |

**审查结论**: **NEEDS REVISION** — 原计划任务定位不准确，需要重排。

## 6. TD 标签修正 (per §6.2.1 技术债登记册)

### 6.1 TD-TYPECK-GENERIC-INST 标签修正

**原标签**: `typeck 不解析 Vec<T>/Box<T> 的泛型实例`

**修正后**: 拆分为两个独立 TD:

1. **TD-VEC-GET-TYPE-INFERENCE** (已存在于 code TODO):
   - 描述: `lower_vec_get_intrinsic` 硬编码 `out_ty = i32`，不提取 `Vec<T>` 的 substs[0]
   - 根因: MIR lower 局部 bug，不是 typeck 问题
   - 修复方案: 从 `recv_local` 的 `Adt(Vec_def_id, substs)` 类型提取 `substs[0]` 作为 out_ty
   - 优先级: P2 (doable NOW, 不依赖 v0.2)
   - 目标版本: Stage 18.208

2. **TD-TUPLE-CTOR-TYPECK** (已存在于 tech-debt-register):
   - 描述: typeck 不替换 tuple struct 字段类型 (`Box<T>(*mut T)` → `Box<Point>(*mut Point)`)
   - 根因: typeck tuple struct field substitution 缺失
   - 修复方案: typeck 重构 (generic field type substitution)
   - 优先级: P2 (v0.2 Phase 2)
   - 目标版本: v0.2.1

### 6.2 TD-TYPECK-GENERIC-INST 状态

**原状态**: 🟡 Active — v0.2 P2+

**修正后**: **DUPLICATE** — 该 TD 的描述混合了两个独立问题，已拆分为:
- TD-VEC-GET-TYPE-INFERENCE (MIR lower bug, Stage 18.208)
- TD-TUPLE-CTOR-TYPECK (typeck issue, v0.2.1)

**建议**: 将 TD-TYPECK-GENERIC-INST 标记为 `DUPLICATE — split into TD-VEC-GET-TYPE-INFERENCE + TD-TUPLE-CTOR-TYPECK`。

## 7. 行动计划

### 7.1 本 stage (18.207) 立即完成

1. ✅ 任务审查文档 (本文档)
2. ✅ 更新 tech-debt-register.md (TD-TYPECK-GENERIC-INST 标签修正)
3. ✅ 更新 Stage 18.204 deep-review §5.2 action plan (重排)

### 7.2 下一 stage (18.208)

**TD-VEC-GET-TYPE-INFERENCE fix**:
- 修改 `lower_vec_get_intrinsic` (src/mir/lower/expr_variants.rs:2207)
- 从 `recv_local` 的类型提取 `Vec<T>` 的 substs[0] 作为 out_ty
- 如果 substs 为空 (Infer/Param)，fallback 到 i32 (current behavior)
- 添加测试: Vec<Point>::get(0).x, Vec<Point>::get(0).y

### 7.3 v0.2 Phase 2 (重排后)

1. TD-TUPLE-CTOR-TYPECK (typeck tuple struct field substitution)
2. TD-INT-UINT-VAR (typeck Int/Uint 变量统一)
3. 类型 2 组 (drop elaboration 重构)
4. TD-C-WRAPPER-OVERUSE 迁移
5. typeck 加严

## 8. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0 (no code changes)
- ✅ cargo test --features llvm-backend --lib: 664 passed (no code changes)
- ✅ cargo test --features llvm-backend --tests: 3098 passed (no code changes)
- **Total**: 3762 tests, 0 failures, zero regression (audit only stage)

## 9. 结论

**任务审查完成** — Stage 18.204 deep review §5.2 action plan 中的 "类型 3 组整体修复"
定位不准确。通过实测验证发现:

1. TD-TYPECK-GENERIC-INST 标签混合了两个独立问题 (MIR lower bug + typeck issue)
2. TD-VEC-GET-TYPE-INFERENCE 是局部 MIR lower bug，doable NOW (Stage 18.208)
3. TD-TUPLE-CTOR-TYPECK 是 real typeck issue，v0.2 Phase 2

任务图已重排，避免在不具备能力时强行做 typeck 重构 (per §17.8 能力边界审查)。

**下一步**: Stage 18.208 — TD-VEC-GET-TYPE-INFERENCE fix (localized MIR lower fix)
