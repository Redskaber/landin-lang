# Stage 5.83 开发计划：dyn Trait 端到端集成测试

> **阶段**: Stage 5.83
> **版本**: v0.11.78 → v0.11.79
> **状态**: ✅ Complete

## 1. 目标

编写更深的端到端集成测试，验证完整的 dyn Trait 编译管线：
源码 → driver compile → MIR 含 dyn_trait_calls side-table → codegen
生成 vtable indirect call IR + vtable/dynptr 全局定义。

这些测试不测试单个 API，而是测试**整个 pipeline 的协同工作**，
确保 5.78-5.82 的所有组件正确集成。

## 2. 设计

### 2.1 测试策略

每个测试：
1. 用 `compile(src)` 编译一段 Landin 源码
2. 检查 `result.mirs` 中的 `dyn_trait_calls` side-table
3. 用 `codegen_crate(&result)` 生成 LLVM IR
4. 检查 IR 中包含预期的 vtable indirect call 指令

### 2.2 测试场景

| # | 源码特征 | 期望 |
|---|---------|------|
| 1 | 无 trait/impl 的空函数 | mirs 中无 dyn_trait_calls，IR 无 vtable |
| 2 | trait + impl 但无 dyn 调用 | mirs 无 dyn_trait_calls，IR 有 vtable 全局但无 indirect call |
| 3 | trait + impl + dyn 调用（method 匹配 stdlib） | mirs 有 dyn_trait_calls，IR 有 indirect call |
| 4 | Drop trait + impl + drop() 调用 | indirect call 返回 void |
| 5 | Clone trait + impl + clone() 调用 | indirect call 返回 ptr (OpaquePtr) |
| 6 | 多个 dyn 调用 | side-table 多条目，IR 多个 indirect call |
| 7 | trait method 不在 stdlib registry | mirs 无 dyn_trait_calls（不匹配） |
| 8 | 端到端：vtable 全局存在 | IR 含 @.vtable.<trait>.<type> |
| 9 | 端到端：dynptr 全局存在 | IR 含 @.dynptr.<trait>.<type> |
| 10 | 端到端：vtable 含正确方法符号 | IR 含 @landin_<type>_<method> |
| 11 | return_kind 端到端：Drop::drop → call void | 精确返回类型 |
| 12 | return_kind 端到端：Clone::clone → call i32* | 精确返回类型 |

### 2.3 §16 合规

测试只用公共 API（`compile` + `codegen_crate` + `result.mirs` 公开字段）。
不访问内部数据结构。

### 2.4 命名标准化

无新 API——本 stage 纯测试。测试函数命名遵循 `<verb>_<noun>_<context>` 模式。

## 3. 不在本 stage 范围

- ❌ 新增公共 API
- ❌ 修改现有代码逻辑
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
