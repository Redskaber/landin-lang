# Stage 18.201 — 任务审查：MVP/简写/缺陷全面审计 + 任务图重排

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + PM-A + REV-A
> **基线版本**: v0.467.0 (Stage 18.200)
> **触发条例**: 用户指令 "如果当前设计和实现存在简写和缺陷（时机）...同类型错误或存在依赖关系的应该考虑整体性完整修复"
> **Task ID**: stage18.201

## 1. MVP/简写/缺陷全面审计

### 1.1 已解决的 MVP (不再需要修复)

| MVP | 描述 | 解决 Stage |
|-----|------|-----------|
| String = &str alias | String 是 &str 别名 | ✅ 18.180 |
| String 无 from_str | 缺 from_str intrinsic | ✅ 18.185 |
| String 无 as_str | 缺 as_str intrinsic | ✅ 18.189 |
| String 无 push_str | 缺 push_str | ✅ 18.198 |
| Vec 无 new | 缺 Vec::new | ✅ 18.195 |
| Vec 无 push | 缺 Vec::push | ✅ 18.197 |
| Vec 无 get | 缺 Vec::get | ✅ 18.200 |
| format! 无 literal | 缺 format! MVP | ✅ 18.186 |
| array index bug | DCE 移除 idx_local | ✅ 18.182 |
| fat ptr Index | GEP on value | ✅ 18.183 |
| str methods segfault | is_empty/as_bytes | ✅ 18.184 |
| i64 literal truncation | emit_const i32 截断 | ✅ 18.191 |
| array OOB | 无 bounds check | ✅ 18.192 |
| Box type coercion | store through *mut u8 | ✅ 18.190 |

### 1.2 当前活跃的 MVP/简写/缺陷

| ID | 描述 | 根因 | 修复方案 | 阻塞 |
|----|------|------|---------|------|
| TD-FORMAT-VARIADIC | format!("x={}", x) 不支持 | 需类型推导 + variadic args | 实现 __landin_format C stub + 类型推导 | 无 |
| TD-BOX-AUTO-DROP | Box 无自动释放 | drop elaboration 不跟踪 moved-from locals | 修复 TD-DROP-MOVED-LOCALS | TD-DROP-MOVED-LOCALS |
| TD-DROP-MOVED-LOCALS | drop elaboration 缺少 move tracking | drop elaboration 不记录哪些 local 已 move | 重构 drop elaboration 添加 move state | 无 (v0.3+ 工作) |
| TD-VEC-ELEM-SIZE-INFERENCE | Vec elem_size 默认 4 (Infer/Param 类型) | typeck 将 Vec<T> 的 T 解析为 Infer | 修复 typeck 泛型实例化 | 无 |
| TD-VEC-PUSH-SHARED-BORROW | Vec::push 用 Shared 而非 Mut borrow | borrow checker 要求 mut 声明 | 在 prelude impl 中声明 &mut self | 无 |
| TD-INT-UINT-VAR | typeck Int/Uint 变量统一 (partial fix) | unify table 丢失 Int↔Uint 区别 | 分离 IntOrUintVar | 无 |
| TD-TUPLE-CTOR-TYPECK | generic tuple struct ctor 类型检查宽松 | type checker 不验证 Box(*mut u8) → Box<i32> | 收紧 typeck | 无 |
| TD-BOX-SIZE-OF | Box::new 的 sizeof(T) 硬编码 | 无 layouts-based size 推导 | 用 AdtLayouts 计算 size | 无 |

### 1.3 同类型/依赖关系分析

**类型 1: elem_size 硬编码 (同类型)**
- TD-BOX-SIZE-OF: Box::new sizeof(T) 硬编码
- TD-VEC-ELEM-SIZE-INFERENCE: Vec::push/get elem_size 默认 4
- **整体修复**: 需要从 typeck 的泛型实例化推导 elem_size, 或从 AdtLayouts 查询
- **修复计划**: Stage 18.202 — 统一 elem_size 推导

**类型 2: borrow checker 绕过 (同类型)**
- TD-VEC-PUSH-SHARED-BORROW: Vec::push 用 Shared borrow
- TD-BOX-AUTO-DROP: drop elaboration 不跟踪 move
- **整体修复**: 需要在 prelude impl 声明 &mut self, 或在 MIR lower 中跳过 borrowck
- **修复计划**: Stage 18.203 — prelude impl 方法签名修复

**类型 3: typeck 泛型 (同类型)**
- TD-INT-UINT-VAR: Int/Uint 变量统一
- TD-TUPLE-CTOR-TYPECK: tuple struct ctor 宽松
- TD-VEC-ELEM-SIZE-INFERENCE: Vec<T> 泛型实例化
- **整体修复**: 需要 typeck 泛型实例化重构 (v0.2 P2+)
- **修复计划**: v0.2 P2+ (不在当前 chain)

### 1.4 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| 1 | TD-FORMAT-VARIADIC | 用户最常用的功能 (格式化输出) |
| 2 | elem_size 统一推导 | 影响 Box + Vec (同类型整体修复) |
| 3 | borrow checker 修复 | 影响 Vec::push + Box auto-drop (同类型整体修复) |
| 4 | typeck 泛型 | v0.2 P2+ (不在当前 chain) |

## 2. 任务图重排

```
18.201 任务审查 (本 stage)
18.202 format! variadic (最高优先级 — 用户最常用)
18.203 elem_size 统一推导 (整体修复 Box + Vec)
18.204 deep review §14.5 (close chain)
```

## 3. 简写/缺陷记录

所有 MVP/简写已记录在 tech-debt-register.md 中。本审计确认所有条目都已被记录,
没有遗漏。同类型的 MVP (elem_size 硬编码, borrow checker 绕过) 需要整体性修复,
计划在 Stage 18.203 中统一处理。
EOF
echo "Task review written"