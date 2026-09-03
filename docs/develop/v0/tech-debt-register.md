# Landin 编译器技术债完整清单 — v0.616.0 (Stage 66)

> **更新日期**: 2026-09-03
> **版本**: v0.616.0
> **状态**: v0.7+ trait 系统阶段，TD 聚焦修复（impl Trait bounds validation 完成）

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
| TD-CLONE-TRAIT-MISSING | 59 | Clone trait + impls for i32/i64/bool/usize |
| TD-DYN-TRAIT-COMPLETION | 60 | TraitObject → Ref(Error) partial fix (dyn Trait codegen works) |
| TD-DISPLAY-TRAIT-MISSING | 61 | Display trait + 5 primitive impls (i32/i64/usize/bool/str) + TextEmitter @.data dedup |
| TD-FN-TRAITS | 62 | Fn/FnMut/FnOnce traits + associated type Output (manual impl pattern; closure auto-impl deferred to v0.8+) |
| TD-IMPL-TRAIT | 63 | impl Trait in arg position desugared to generic param at HIR lowering (method calls inside body deferred to v0.8+) |
| TD-SPECIAL-16 | 64 | Drop trait added to prelude (drop glue infrastructure was already complete from Stage 15.x) |
| TD-PRELUDE-MACRO-TIMING | 65 | Prelude macro timing resolved — prelude uses direct C runtime calls (__landin_panic_msg, __landin_unreachable), not panic!/unreachable! macros. Token-level injection not needed. |
| TD-IMPL-TRAIT-NO-BOUNDS | 66 | Parser rejects `impl` with no bounds — requires at least one trait bound |
| TD-IMPL-TRAIT-UNDEFINED-BOUND | 66 | Resolver/scanner reports undefined trait bounds in `impl Trait` and generic params |
| TD-IMPL-TRAIT-MONO-RESOLUTION | 69 | TraitMethodResolutionMap + re_resolve_trait_method_calls — monomorphization re-resolves trait methods after type substitution |
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
| *(无剩余 P2 TD — 所有 P2 TD 已在 Wave 1 中修复)* | | | | |

### P3 — v0.7+ trait 系统阶段

| TD ID | 描述 | 根因 | 修复方案 | 依赖 |
|-------|------|------|---------|------|
| TD-DISPLAY-TRAIT-MISSING-PARTIAL | format! 参数 &[i64] 限制类型；Display trait 已定义但 format! 重设计未完成 | format! impl 接收 i64 数组 (Stage 36.6) | &[&dyn Display] trait dispatch | Display trait ✅ (Stage 61) + full dyn Trait (v0.8+) |
| TD-TOSTRING-DEFAULT-BODY | Display::to_string 默认方法缺失 | Bug Z7 workaround (override per impl) 触发 libLLVM 间歇性 crash | LLVM codegen crash 调查 + 修复 | libLLVM bug (P3, v0.8+) |
| TD-FN-CLOSURE-COERCION | closures 不自动实现 Fn traits | TyKind::Closure 无 Fn trait coercion | typeck closure → Fn trait coercion + vtable emission | TD-FN-TRAITS ✅ (Stage 62) |
| TD-FN-UNIT-ARGS | `Fn<()>` unit tuple arg 不支持 | typeck/codegen 不支持 () as Args | typeck/codegen 支持 unit tuple as Fn<Args> | TD-FN-TRAITS ✅ (Stage 62) |
| TD-ASSOC-TYPE-SCOPE | associated type `Output` 在 2 impls 中冲突 | resolver 未按 impl 块 scope assoc types | resolver scope assoc types per impl block | TD-FN-TRAITS ✅ (Stage 62) |
| TD-FN-IMPL-SIG-VALIDATION | typeck 不校验 impl sig 匹配 Args/Output | typeck 缺少 impl signature 检查 | typeck validate impl fn sig vs trait Args/Output | TD-FN-TRAITS ✅ (Stage 62) |
| TD-GENERIC-TRAIT-METHOD-MANGLING | 泛型 trait method 调用 mangled 名错误 | `From::<i32>::from(42)` 产生 `fn_0_i32` (未定义) | 修复 generic trait method mangling | trait resolver |
| TD-FN-ASSOC-TYPE-CALL | `<F as Fn<(Args,)>>::call(&f, args)` 显式调用语法不支持 | parser/typeck 未支持 explicit trait dispatch | parser/typeck 支持 explicit trait dispatch syntax | typeck |
| TD-DYN-TRAIT-COMPLETION | dyn Trait typeck 不完整 | typeck 无 dyn Trait 代码 | typeck trait dispatch | trait resolver |
| TD-IMPL-TRAIT-MONO-RESOLUTION | ~~impl Trait arg 方法调用在函数体内不解析~~ **FIXED Stage 69** — TraitMethodResolutionMap + re_resolve_trait_method_calls | monomorphization 不在类型替换后重新解析 trait 方法 | ~~mono pass 重新解析 trait 方法~~ **DONE: pre-computed map in driver, re-resolve in codegen** | TD-IMPL-TRAIT ✅ (Stage 63) |
| TD-IMPL-TRAIT-CALLSITE-CHECK | typeck 不校验 call site 实参是否满足 impl Trait bound | typeck 缺少 call site bound 检查 + 无 trait_resolver 访问 | typeck validate trait bounds at call site (需 trait_resolver 访问，v0.8+ 架构变更) | TD-IMPL-TRAIT ✅ (Stage 63) |
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
| TD-MEM-DROP | mem::drop() 显式 drop 函数未实现 | 无 mem::drop runtime 函数 | 实现 mem::drop 或 std::mem::drop | TD-SPECIAL-16 ✅ (Stage 64) |

---

## 三、TD 依赖关系图

```
TD-PRELUDE-MACRO-TIMING (P2) — ✅ RESOLVED Stage 65
  Root cause fixed differently: prelude uses direct C runtime calls,
  not macros. Token-level injection not needed.

TD-DISPLAY-TRAIT-MISSING (P3) — ✅ partial fix Stage 61
  ├── TD-DYN-TRAIT-COMPLETION (typeck trait dispatch) — ✅ partial fix Stage 60
  ├── TD-FN-TRAITS (闭包支持) — ✅ partial fix Stage 62
  └── TD-FORMAT-ARGS-WRITE (write! macro) — v0.8+

TD-CLONE-TRAIT-MISSING (P3) — ✅ RESOLVED Stage 59
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

### 第一波：解除 prelude 限制（P2，无 trait 依赖）— ✅ COMPLETE
1. ✅ TD-OPTION-TAKE-INCOMPLETE — Stage 40.2
2. ✅ TD-STR-INTRINSIC-MARKER-BODIES — Stages 56-58
3. ✅ TD-PRINTLN-CODEGEN-INTERCEPT — partial (println! works via codegen intercept)
4. ✅ TD-PRELUDE-MACRO-TIMING — Stage 65 (resolved by alternative approach)

### 第二波：trait 系统基础（P3，解锁后续）— ✅ COMPLETE
5. ✅ TD-DYN-TRAIT-COMPLETION — Stage 60 (partial fix)
6. ✅ TD-CLONE-TRAIT-MISSING — Stage 59
7. ✅ TD-DISPLAY-TRAIT-MISSING — Stage 61 (partial fix)

### 第三波：闭包 + 高级特性（P3，依赖 trait）— ✅ COMPLETE
8. ✅ TD-FN-TRAITS — Stage 62 (partial fix)
9. ✅ TD-IMPL-TRAIT — Stage 63 (partial fix)
10. ✅ TD-SPECIAL-16 — Stage 64 (Drop trait in prelude)

### 第四波：架构优化（P3，性能/代码质量）— v0.8+
11. TD-SPECIAL-8 — HIR reverse index (v0.8+)
12. TD-SPECIAL-10 — emitter 统一 (v0.8+)

**Wave 1-3 COMPLETE. v0.7 trait system phase feature-complete.**

---

## 五、历史已修复 TD（v0.4 及之前阶段，归档参考）

以下 TD 在 v0.4 FINAL (Stage 18.500) 及之前阶段已修复，保留作为历史记录。

### v0.4 FINAL 阶段已修复（S2-S11 + D1-D8 + LOC-* + 其他）

| TD ID | Stage | 描述 |
|-------|-------|------|
| S2 | 18.112 | Method monomorphization (Constant func operand) |
| S5 | 18.104 | type_names pre-computed |
| S6 | 18.105 | Nested Param return type resolution |
| S7 | 18.106 | MonoItem collection skips Param/Error substs |
| S8 | 18.107 | Call-site sig substitution |
| S9 | 18.111 | Dest local type writeback |
| S10 | 18.109 | DivisionByZero assert skip for const_prop |
| S11 | 18.110 | Const-prop loop safety |
| TD-13 | 18.99 | FnDef↔FnPtr soundness |
| TD-DUP2 | 18.100 | format_ty DRY |
| TD-UNWRAP1 | 18.100 | module_build unwrap → expect |
| TD-UNWRAP-DRIVER | 18.127 | driver.rs 4 unwrap → if let Some(b) pattern |
| TD-UNWRAP-BORROWCK-REGION | 18.127 | borrowck/region_inference.rs SCC unwrap → expect |
| TD-LOC-TYPECK-CHECKER | 18.128 | typeck/checker.rs 2635 LOC → split into 4 files |
| TD-LOC-MIR-LOWER-MOD | 18.130 | mir/lower/mod.rs 2016 LOC → mod.rs 960 + body_lower.rs 1110 |
| TD-LOC-MIR-LOWER-EXPR | 18.133 | mir/lower/expr_operand.rs 2171 LOC → 4 files < 1500 LOC |
| TD-LOC-DRIVER | 18.250 | driver.rs 4038 LOC → 5 files all < 1500 LOC |
| TD-LOC-MACRO-EXPAND | 18.249 | macro_expand.rs 5962 LOC → 7 files all < 1500 LOC |
| TD-CODEGEN-RESULT | 18.151 | codegen returns Result not String |
| TD-PROJECTION-RESOLVER | 18.148 | moved to src/driver/projection_resolver.rs |
| TD-BINARYOP2-PANIC | 18.151 | BinaryOp2 returns Err instead of panic |
| TD-EMITTER-PANIC | 18.254 | audit: panic!() in cfg(test) only |
| TD-SPAN-DUMMY-CLEANUP | 18.252 | audit: all Span::DUMMY legitimate |
| TD-MODULELOAD-ERROR-FIELD | 18.159 | CompileErrors.module_load + ErrorCode::ModuleLoad (E850) |
| TD-NEGATIVE-TEST-COVERAGE | 18.164 | 311 negative tests added (7.9% → 27.8%) |
| TD-UNWRAP-NONGUARDED | 18.159 | codegen/llvm/arithmetic.rs unwrap → if let Some pattern |
| TD-INT-UINT-VAR | v0.4 | types_match_loose hardcoded Int↔Uint (deferred to v0.8+) |
| TD-DEREF-NON-REF | v0.4 | Deref on non-Ref in pattern bindings (deferred to v0.8+) |
| TD-LOCALID0-FALLBACK | v0.4 | Non-Local borrowed places LocalId(0) fallback (deferred to v0.8+) |
| TD-SINGLE-FILE | 18.154 | ModuleLoader + compile_project + landinc CLI (Phase 4 remains) |
| TD-RVALUE-NO-SPAN | v0.4 | Rvalue enum doesn't carry Span (deferred to v0.8+) |
| TD-NO-INCREMENTAL | v0.4 | Full recompile every time (deferred to v0.8+) |

### v0.7 trait 系统阶段已修复（Stage 59-63）

| TD ID | Stage | 描述 |
|-------|-------|------|
| TD-CLONE-TRAIT-MISSING | 59 | Clone trait + impls for i32/i64/bool/usize |
| TD-DYN-TRAIT-COMPLETION | 60 | TraitObject → Ref(Error) partial fix |
| TD-DISPLAY-TRAIT-MISSING | 61 | Display trait + 5 primitive impls + TextEmitter @.data dedup |
| TD-FN-TRAITS | 62 | Fn/FnMut/FnOnce traits + associated type Output |
| TD-IMPL-TRAIT | 63 | impl Trait arg desugar to generic param at HIR lowering |
| TD-SPECIAL-16 | 64 | Drop trait added to prelude (drop glue infra already complete) |
| TD-PRELUDE-MACRO-TIMING | 65 | Prelude macro timing resolved (prelude uses direct C calls, not macros) |

### v0.5-v0.7 阶段已修复（P2 prelude 限制 — Wave 1）

| TD ID | Stage | 描述 |
|-------|-------|------|
| TD-OPTION-TAKE-INCOMPLETE | 40.2 | Option::take 修复（&mut self 方法 + mem::replace pattern） |
| TD-STR-INTRINSIC-MARKER-BODIES | 56-58 | str::len/is_empty/as_bytes 真实 body（3/3 complete） |
| TD-PRINTLN-CODEGEN-INTERCEPT | partial | println! via codegen intercept（partial — 统一 format_v2 路径 deferred to v0.8+） |

### v0.5 阶段已修复（Stage 39-50）

| TD ID | Stage | 描述 |
|-------|-------|------|
| TD-LEXER-UNDERSCORE | 39.3 | `_` → TokenKind::Underscore |
| TD-PAT-IDENT-VARIANT | 39.3 | resolver 转换单段 variant Ident → Path |
| TD-TEXT-IR-DEREF-ADT | 39.3 | detect_place_type Deref OpaquePtr 回退 MIR |
| TD-PANIC-MACRO-BROKEN | 40.2 | __landin_panic_msg extern 声明 |
| TD-UNREACHABLE-MACRO-BROKEN | 40.3 | unreachable! body .ptr 提取 |
| TD-SPECIAL-2 | 41 | Never (`!`) 类型完整化 |
| TD-SPECIAL-4 | 41 | i64 格式化合并 (__landin_i64_format) |
| TD-PANIC-CONSOLIDATION | 43 | 3 panic_* C wrapper → __landin_panic_fmt |
| TD-COMPILE-TIME-MACROS | 42-43 | stringify!/concat!/file!/line!/module_path! |
| TD-METHOD-LEVEL-GENERICS | 47 | 方法 substs 推断 (map_err enabled) |
| TD-SPECIAL-7 | 49 | primitive_intrinsics 数据驱动表 |
| TD-SPECIAL-9 | 50 | 3 loop {} markers → __landin_unreachable |

---

## 六、合并说明

本文档由以下版本合并而成（单一可信数据源，per §1.0 原則 10）：
- `tech-debt-register-v0.604.md` (Stage 53)
- `tech-debt-register-v0.611.md` (Stage 61)
- `tech-debt-register-v0.612.md` (Stage 62)
- `tech-debt-register-v0.613.md` (Stage 63) — 最新版本，作为合并基准
- 原 `tech-debt-register.md` (v0.510.0, Stage 18.500) — v0.4 FINAL 历史数据，已归档到第五节

合并完成后，版本化文件 (`tech-debt-register-v0.604.md` 等) 已移除。
