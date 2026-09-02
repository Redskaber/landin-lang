# Landin 编译器架构审查报告 — Stage 49

> **审查日期**: 2026-09-02
> **审查范围**: v0.599.0 (Stage 48 完成后)
> **审查维度**: (1) 基础类型系统完整性 + primitive_intrinsics.rs 特解→通解; (2) 项目中其他特解→通解
> **审查方法**: §20 迭代审计 + §2.2 根因思维
> **审查结论**: 发现 10 个特解，3 个可立即通解化，7 个需 v0.7+ 架构重构

---

## 任务 1: primitive_intrinsics.rs 审查

### 当前状态

`src/mir/lower/primitive_intrinsics.rs` 包含 4 个 primitive intrinsics:

| Intrinsic | 类型 | 方法 | 实现方式 | 是否可通解 |
|-----------|------|------|---------|-----------|
| StrLen | str | len() | Field(1) projection | ❌ 需要特殊 MIR（fat pointer field access） |
| StrIsEmpty | str | is_empty() | Field(1) == 0 comparison | ❌ 同上 |
| StrAsBytes | str | as_bytes() | Cast &str → &[u8] | ❌ 需要类型 cast |
| SliceLen | [T] | len() | 同 StrLen (fat pointer) | ❌ 同上 |

### 根因分析

**这些 intrinsics 是特解** — 它们在 MIR lower 阶段拦截 str/slice 方法调用，生成特殊 MIR，而非走标准方法解析路径。

**为什么需要特解**: `str` 和 `[T]` 是 unsized types（编译时大小未知），它们的 `&str` / `&[T]` 是 fat pointer（`{ptr, len}`）。prelude 中 `impl str { fn len(&self) -> usize { loop {} } }` 的 body 是 marker（`loop {}`），因为 `&self` 的 `self.len` field access 需要 fat pointer 的 Field(1) projection — 这不是标准 struct field access。

### 通解化方案

**方案 A（v0.7+）**: 让 prelude 中 `impl str` 的方法有真实 body — 通过 FatPtrLit 语法直接访问 fat pointer 字段。Stage 31.5 已为 `String::as_str` 实现了类似机制。

**方案 B（当前可做）**: 将 `identify_intrinsic` 的 match 表扩展为数据驱动的注册表。

**推荐**: 方案 B（短期改进）+ 方案 A（v0.7+ 目标）

---

## 任务 2: 项目中其他特解审查

### 已识别的 10 个特解

| ID | 特解描述 | 影响范围 | 通解方案 | 优先级 |
|----|---------|---------|---------|--------|
| TD-SPECIAL-7 | primitive_intrinsics.rs 硬编码 match | mir/lower/ | 数据驱动注册表 | P3 (v0.7+) |
| TD-SPECIAL-8 | resolve_inherent_method O(N) scan (5 处) | method_resolution.rs | HIR 索引化 | P3 (v0.7+) |
| TD-SPECIAL-9 | prelude 中 14 处 `loop {}` marker bodies | prelude.rs | str/slice intrinsics 有真实 body | P3 (v0.7+) |
| TD-SPECIAL-10 | TextEmitter vs LLVMSysEmitter 双路径 | codegen/ | 统一为单一 emitter | P3 (v0.7+) |
| TD-SPECIAL-11 | is_landin_print_macro 硬编码名字列表 | terminator.rs | 从签名解析 variadic | P3 (已部分通解) |
| TD-SPECIAL-12 | format! 参数 `&[i64]` 限制类型 | prelude.rs | `&[&dyn Display]` trait dispatch | P3 (v0.7+) |
| TD-SPECIAL-13 | OpaquePtr for `&Adt` 递归打破 | codegen/ | Ptr(Adt) + 循环检测 | P3 (v0.7+) |
| TD-SPECIAL-14 | FatPtrLit 特殊语法 | expr_operand.rs | 标准 struct literal + auto-coercion | P3 (v0.7+) |
| TD-SPECIAL-15 | sizeof 特殊 MIR rvalue | mir/lower/ + codegen/ | 标准 C sizeof 或 layout 查询 | P3 (v0.7+) |
| TD-SPECIAL-16 | Drop::drop marker bodies | prelude.rs | 实现 Drop trait + drop glue | P3 (v0.7+) |

### 基础类型系统完整性评估

| 类型类别 | 完整度 | 缺失/受限 |
|---------|--------|----------|
| 基本类型 | ✅ 完整 | — |
| Str / Slice | ✅ 完整 (fat pointer) | — |
| Array / Tuple / Ref / RawPtr | ✅ 完整 | — |
| Never (`!`) | ✅ 完整 (Stage 41) | — |
| FnDef / FnPtr / Closure | ✅ 完整 | — |
| Adt (struct/enum) | ✅ 完整 | — |
| Projection (associated type) | ✅ 完整 (GATs Phase 3) | — |
| TraitObject (dyn Trait) | ⚠️ 部分 | typeck 无 dyn Trait 代码 |
| ImplTrait | ❌ 未实现 | 参数/返回位置 impl Trait |
| Foreign | ⚠️ 边缘 | extern type 声明支持，codegen 未测试 |

### 结论

基础类型系统在 v0.6 阶段基本完整。主要缺失：
1. `dyn Trait` 完整支持（需 typeck trait dispatch）
2. `impl Trait` 语法（需参数/返回位置推断）
3. Fat pointer field access（需 typeck 支持 `&str.len` 作为 field access）

这些缺失不阻塞当前 v0.6 prelude 扩展工作，但影响 v0.7+ 的 Display trait 和 trait dispatch 实施。
