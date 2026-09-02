# Landin 编译器技术债完整清单 — v0.604.0 (Stage 53)

> **更新日期**: 2026-09-02
> **版本**: v0.604.0
> **状态**: v0.7+ trait 系统阶段，TD 聚焦修复

---

## 一、已修复 TD（不在此清单）

| TD ID | Stage | 描述 |
|-------|-------|------|
| TD-SPECIAL-2 | 41 | Never (`!`) 类型完整化 |
| TD-SPECIAL-4 | 41 | i64 格式化合并 (__landin_i64_format) |
| TD-PANIC-CONSOLIDATION | 43 | 3 panic_* C wrapper → __landin_panic_fmt |
| TD-COMPILE-TIME-MACROS | 42-43 | stringify!/concat!/file!/line!/module_path! (5/8) |
| TD-METHOD-LEVEL-GENERICS | 47 | 方法 substs 推断 (map_err enabled) |
| TD-SPECIAL-7 | 49 | primitive_intrinsics 数据驱动表 |
| TD-SPECIAL-9 | 50 | 3 loop {} markers → __landin_unreachable |
| TD-SPECIAL-11 | 18.334 | variadic 检测从签名解析 (已通解) |
| TD-LEXER-UNDERSCORE | 39.3 | `_` → TokenKind::Underscore |
| TD-PAT-IDENT-VARIANT | 39.3 | resolver 转换单段 variant Ident → Path |
| TD-TEXT-IR-DEREF-ADT | 39.3 | detect_place_type Deref OpaquePtr 回退 MIR |
| TD-PANIC-MACRO-BROKEN | 40.2 | __landin_panic_msg extern 声明 |
| TD-PANIC-MACRO-STR-PTR | 40.2 | panic! body .ptr 提取 |
| TD-PANIC-MACRO-HYGIENE-FIELD | 40.2 | hygiene 跳过 ptr/len/cap |
| TD-UNREACHABLE-MACRO-BROKEN | 40.3 | unreachable! body .ptr 提取 |

---

## 二、当前未修复 TD（按优先级排序）

### P2 — 架构阻断，需 v0.7+ 修复

| TD ID | 描述 | 根因 | 修复方案 | 依赖 |
|-------|------|------|---------|------|
| TD-PRELUDE-MACRO-TIMING | prelude 在 macro_expand 后注入 | compile_inner.rs:57 (inject_prelude 在 parse 后) | 移到 token 级注入 | DefId 解耦 |
| TD-OPTION-TAKE-INCOMPLETE | take() 消耗 self 而非 &mut self | Landin 无 mem::replace | 实现 mem::replace 或 &mut self 方法 | 无 |
| TD-STR-INTRINSIC-MARKER-BODIES | str len/is_empty/as_bytes 用 __landin_unreachable | typeck 不支持 fat pointer field access | typeck 支持 &str.field | 无 |
| TD-PRINTLN-CODEGEN-INTERCEPT | println! 绕过 __landin_format_v2 | codegen 直接拦截 __landin_println → printf | 统一走 format_v2 路径 | Display trait |

### P3 — v0.7+ trait 系统阶段

| TD ID | 描述 | 根因 | 修复方案 | 依赖 |
|-------|------|------|---------|------|
| TD-DISPLAY-TRAIT-MISSING | format! 参数 &[i64] 限制类型 | format! impl 接收 i64 数组 | &[&dyn Display] trait dispatch | Display trait + dyn Trait |
| TD-FN-TRAITS | 无 Fn/FnMut/FnOnce traits | 闭包只能用 fn 指针 | 实现 Fn traits | trait dispatch |
| TD-DYN-TRAIT-COMPLETION | dyn Trait typeck 不完整 | typeck 无 dyn Trait 代码 | typeck trait dispatch | trait resolver |
| TD-IMPL-TRAIT | impl Trait 未实现 | parser/typeck 不支持 | 参数/返回位置推断 | typeck |
| TD-CLONE-TRAIT-MISSING | 无 Clone trait | prelude 仅有 Copy trait | Clone trait + auto-derive | trait dispatch |
| TD-CFG-MACROS | cfg!/cfg_attr! 未实现 | 需配置系统 | 编译期 cfg 评估 | build system |
| TD-ASM-MACRO | asm! 未实现 | 需 LLVM inline asm | LLVM asm 支持 | LLVM backend |
| TD-FORMAT-ARGS-WRITE | format_args!/write! 未实现 | 需 Display trait | Display trait 依赖 | Display trait |
| TD-ENV-MACROS | env!/option_env!/include_str! 未实现 | 需编译期 I/O | 编译期文件读取 | build system |

### P3 — 架构重构（非功能缺失）

| TD ID | 描述 | 根因 | 修复方案 | 影响 |
|-------|------|------|---------|------|
| TD-SPECIAL-8 | resolve_inherent_method O(N) scan (5处) | 无 reverse index | HIR reverse index | 性能 |
| TD-SPECIAL-10 | TextEmitter + LLVMSysEmitter 双路径 | 2 个 emitter 实现 | 统一为单一 emitter | ~2000 LOC 减少 |
| TD-SPECIAL-13 | OpaquePtr for &Adt | 递归 struct 打破 | Ptr(Adt) + 循环检测 | codegen |
| TD-SPECIAL-14 | FatPtrLit 特殊语法 | 非 struct literal + coercion | 标准 struct literal + auto-coercion | MIR lower |
| TD-SPECIAL-15 | sizeof 特殊 MIR rvalue | 非 C sizeof 调用 | 标准 layout 查询 | codegen |
| TD-SPECIAL-16 | Drop::drop marker bodies | 无 Drop trait | Drop trait + drop glue | trait dispatch |

---

## 三、TD 依赖关系图

```
TD-PRELUDE-MACRO-TIMING (P2)
  └── DefId 解耦（v0.7+ 架构重构）

TD-DISPLAY-TRAIT-MISSING (P3)
  ├── TD-DYN-TRAIT-COMPLETION (typeck trait dispatch)
  ├── TD-FN-TRAITS (闭包支持)
  └── TD-FORMAT-ARGS-WRITE (write! macro)

TD-CLONE-TRAIT-MISSING (P3)
  └── TD-DYN-TRAIT-COMPLETION (trait dispatch)

TD-STR-INTRINSIC-MARKER-BODIES (P2)
  └── typeck fat pointer field access（&str.len 作为 field access）

TD-OPTION-TAKE-INCOMPLETE (P2)
  └── mem::replace 或 &mut self 方法

TD-PRINTLN-CODEGEN-INTERCEPT (P2)
  ├── TD-DISPLAY-TRAIT-MISSING
  └── 统一走 __landin_format_v2 路径
```

---

## 四、v0.7+ 修复优先级建议

### 第一波：解除 prelude 限制（P2，无 trait 依赖）
1. TD-OPTION-TAKE-INCOMPLETE — 实现 mem::replace 或改用 &mut self
2. TD-STR-INTRINSIC-MARKER-BODIES — typeck 支持 fat pointer field access
3. TD-PRINTLN-CODEGEN-INTERCEPT — 统一 println! 走 format_v2

### 第二波：trait 系统基础（P3，解锁后续）
4. TD-DYN-TRAIT-COMPLETION — typeck trait dispatch
5. TD-CLONE-TRAIT-MISSING — Clone trait
6. TD-DISPLAY-TRAIT-MISSING — Display trait + format! 重设计

### 第三波：闭包 + 高级特性（P3，依赖 trait）
7. TD-FN-TRAITS — Fn/FnMut/FnOnce
8. TD-IMPL-TRAIT — impl Trait 语法
9. TD-SPECIAL-16 — Drop trait + drop glue

### 第四波：架构优化（P3，性能/代码质量）
10. TD-SPECIAL-8 — HIR reverse index
11. TD-SPECIAL-10 — emitter 统一
12. TD-PRELUDE-MACRO-TIMING — DefId 解耦 + token 级注入
