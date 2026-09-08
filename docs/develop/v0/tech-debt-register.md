# Landin 编译器技术债完整清单 — v0.652.0 (Stage 122, v0.12 COMPLETE)

> **更新日期**: 2026-09-06
> **版本**: v0.652.0
> **状态**: v0.12 TD 修复阶段 COMPLETE. 所有 Landin 侧技术债已修复 (Stages 99-121). Debug impl bodies 永久延迟 — 受 LLVM C++ DenseMap 非确定性阻断 (Stage 121 最终 RCA). 进入 v0.13 规划阶段.

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
| TD-FN-CLOSURE-COERCION | 79+83 | typeck Closure↔FnPtr unification (Stage 79) + runtime alloca/load fix (Stage 83: removed Stage 16.21 redundant closure-arg alloca-pointer special-case in codegen/terminator.rs) |
| TD-CLOSURE-PARAM-ANNOT-IGNORE | 84 | MIR lower respects explicit closure param type annotations (`\|n: i64\|` now honored, was: always fresh_infer_ty) — three dispatch sites fixed: expr_operand.rs, body_lower.rs, compile_inner.rs |
| TD-FN-UNIT-ARGS | 85 | `Fn<()>` unit tuple arg now correctly elided from LLVM forward declarations (was: leaked as `void` param type → LLVM module verification failed). Fix in `build_fn_sigs_map` — filter `EmitType::Void` from sig map, mirroring ZST elision in codegen_function + terminator.rs |
| TD-FN-IMPL-SIG-VALIDATION | 78+86 | param type check (Stage 78: substitute trait generic args before comparing) + return type check (Stage 86: resolve `Self::Output` assoc type projection via HIR-aware ty lowering + `resolve_projection_in_ty_pub` before comparing). Also fixed `find_assoc_type_def_id` to match by name AND owner trait (was: name only → `Self::Output` in `FnMut::call_mut` found `Fn`'s Output, not FnMut's). |
| TD-DYN-TRAIT-COMPLETION | 60+87 | Stage 60 partial fix (TraitObject → Ref(Error) placeholder) replaced by Stage 87 proper `TyKind::Dyn(DefId)`. typeck now carries trait DefId + verifies trait impl bounds via `implements_by_def_ids`. Method resolution looks up methods directly in trait declaration for `Dyn` receivers. Codegen emits fat pointer `{ptr,ptr}`. 12 files updated for new TyKind variant. |
| TD-DYN-TRAIT-RUNTIME-DISPATCH | 88 | vtable dispatch wiring — `dyn Trait` method calls now go through vtable indirect call (GEP + load vtable + load method ptr + indirect call), not static dispatch. Fix in `method_call_lower.rs`: `receiver_is_dyn` check forces vtable dispatch for Dyn/Ref(Dyn) receivers; `use_dyn_trait_dispatch` bypasses type_name check for Dyn receivers. |
| TD-DYN-TRAIT-FAT-PTR-COERCION | 89 | Call site fat pointer construction — `&Concrete → &dyn Trait` coercion at call sites now passes `@.dynptr.Trait.Concrete` (fat pointer global) instead of thin data pointer. Fix in `codegen/terminator.rs`: detect Ref(Dyn) callee param + Ref(Adt) arg, construct dynptr symbol. Also fixed `build_type_name_by_def_id` to include Trait DefIds. |
| TD-DYN-TRAIT-DATA-PTR-EXTRACT | 90 | vtable indirect call extracts data pointer from fat pointer field 0 and passes it to the impl method (was: passed fat pointer → method read garbage → returned 0). Fix in `codegen/llvm/aggregate.rs` + `codegen/text/aggregate.rs`: GEP field 0 + load data ptr before indirect call. **First successful end-to-end dyn Trait runtime test** — `use_greeter(&e)` returns 42. |
| TD-FORMAT-ARGS-WRITE | 91 | `format_args!` and `write!` macros now compile and run (was: linker error — `__landin_format_args` and `__landin_write` had no codegen support). Fix: `format_args!` routes to `__landin_format_v2` (same as `format!`); `write!` expands to `dst.write_str(format_args!(...))`; `write_str` added to hygiene skip list. |
| TD-GENERIC-TRAIT-METHOD-MANGLING | 92 (partial) | `re_resolve_trait_method_calls` now runs for ALL functions (not just generic). Added `lookup_by_trait_method` + `lookup_by_method_name` fallbacks. Full turbofish path resolution still needs MIR lower fix (TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION, v0.9+). |
| TD-PRELUDE-TRAIT-COVERAGE (Default) | 94 | Default trait added to prelude + 4 primitive impls (i32/i64/bool/usize) |
| TD-PRELUDE-TRAIT-COVERAGE (PartialEq+Eq) | 95 | PartialEq<Rhs> + Eq traits added; Eq declared WITHOUT supertrait (avoids object safety interference) |
| TD-PRELUDE-TRAIT-COVERAGE (Ord) | 96 | Ord marker trait added + 4 primitive impls |
| TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH | 97+98 | Stage 97 root cause analysis; Stage 98 FIXED — trait impl method symbol collision (`landin_i32_fmt` for both Display and Debug). Fix: mangling includes trait name → `landin_Display_i32_fmt` vs `landin_Debug_i32_fmt`. 4 source files + 32+ test files updated. |
| TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (Layer 1) | 100 | Stage 99 RCA + Stage 100 Layer 1 fix — monomorphization 跳过未实例化的 prelude generic function bodies. Param warnings 1360→24 (-98%). CompileResult 添加 user_item_count; codegen_from_mir 接收 user_item_count + collected_mono_items; 跳过条件: DefId >= user_item_count AND MIR 含 Param AND no MonoItem::Fn 实例化. 4 src + 1 test file. |
| TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (Layer 2 partial) | 101 | Stage 101 Layer 2 部分修复 — codegen_operand FnDef substs mangling 基础设施 + turbofish path 修复. mono_names 参数传递链建立 (5 src 文件, 20+ 调用点). turbofish path (From::<i32>::from(42)) 正确 mangle; 非 turbofish path (Box::new) fallback 到 generic def name (TD-MONO-INFER 跟踪). 5 src + 1 test file. |
| TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (Layer 4) | 102 | Stage 102 Layer 4 修复 — LLVMSysEmitter::Drop 释放 module + context. 之前 Drop 只释放 builder, 不释放 module + context → LLVM 资源累积 → cargo test 多次 compile() 后 SIGSEGV/SIGABRT. 修复后 3 次稳定性验证全绿. 1 src + 1 test file. |
| TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (Layer 3 partial) | 103 | Stage 103 Layer 3 部分修复 — resolve_lit_ty_from_expected for RawPtr expected types. 之前 `String { ptr: 0, ... }` 中 `0` 字面量无 suffix → Infer(IntVar) → codegen i32 (4 bytes) 而非 usize (8 bytes), String struct layout 错误 → SIGSEGV. 修复后 ptr field 类型正确解析为 usize. 保守策略: 只处理 RawPtr, 不处理 Int/Uint (避免破坏 typeck validation). 1 src + 1 test file. |
| TD-CODEGEN-CALL-ARG-TYPE-SOURCE | 107 | Stage 107 修复 — codegen call arg type 优先用 callee sig.inputs[arg_idx] (non-Param 时); 含 Param (generic) 时 fallback 到 detect_operand_type. 新增 mir_type_contains_param helper (pub(crate), 递归). 1 src 文件 (~60 LOC). Stage 106 RCA: Constant type writeback (Phase 3.6) 产生 7 回归, 根因是 codegen call arg type source 不一致. |
| TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL | 109 | Stage 109 修复 — codegen operand.rs Stage 14.64 cast 逻辑: 当 c.ty 为 concrete Int/Uint/Bool/Char 时用 emit_const_typed 直接 emit (跳过 sext/trunc cast); 否则 fallback 到 ConstVal 路径 (preserves Stage 107 behavior). **同时修复 Stage 18.287 遗留 bug**: TextEmitter emit_const_typed 返回 raw value (无 type prefix), 与 LLVM emitter contract 对齐 (LLVM emitter 返回 SSA name, 无 type prefix). 之前 TextEmitter 返回 `"i64 0"` (typed literal), 导致 `emit_store` 双前缀 `store i64 i64 0` + `emit_icmp` 双前缀 `icmp eq i64 2, i64 0` (invalid LLVM IR). Stage 109 路由所有 concrete-typed constants 通过 emit_const_typed, 触发 21 text IR 测试失败, 修复 contract 后全绿. 2 src 文件 (~70 LOC) + 1 test 文件 (20 tests: 8 正 + 5 text IR + 4 负 + 3 边界). |
| TD-TYPECK-WRITEBACK-INCOMPLETE (Phase 3.6) | 110 | Stage 110 重新引入 Phase 3.6 (Constant type writeback) — typeck Phase 3 后添加 Phase 3.6: 遍历所有 basic_blocks 的 statement (Assign(_, Rvalue)) 和 terminator (SwitchInt/Call/Assert), 对每个 Operand::Constant(c) 写回 unify.resolve(&c.ty) (Infer→concrete). 添加两个 helper: `writeback_constant_ty_in_operand` (单 Operand) + `writeback_constant_tys_in_rvalue` (递归 Rvalue 所有 variant). 覆盖所有 Rvalue variant (Use/BinaryOp/UnaryOp/Cast/Aggregate/Load/GetElementPtr/BinaryOp2) 和所有含 Operand 的 TerminatorKind (SwitchInt discr / Call func+args / Assert cond). Infer warnings 41→19 (-54%) on Vec<String, i32> program. 0 回归 (Stage 107 call arg type source + Stage 109 codegen src_ty + TextEmitter contract 修复了所有前置依赖). 1 src 文件 (~100 LOC + 60 LOC helper) + 1 test 文件 (20 tests: 8 正 + 5 text IR + 4 负 + 3 边界). |
| TD-LLVM-OBJ-EMIT-CRASH + TD-MONO-INFER | 113 | Stage 113 修复 — TD-LLVM-OBJ-EMIT-CRASH 根因: `build_fn_sigs_map` 缺少 specialized function sigs (e.g., `process_i32`) → LLVMSysEmitter `interpret_adhoc` 找不到 sig → 用 variadic `i32 ()` 创建 forward declaration → `codegen_mono_functions` 后续 emit 实际函数 `i32 (i32)` → 类型不匹配 → old decl 被 delete + re-add → 引用 old decl 的 store 指令变成 dangling pointer → LLVMTargetMachineEmitToFile SIGSEGV. TD-MONO-INFER 根因: `writeback_fndef_substs` 仅更新 terminator Call 的 func Constant, 不更新 Assign 的 Rvalue::Use(Operand::Constant) → codegen 读 c.ty (empty substs) → 用 generic def name → linker error. 三部分修复: (1) `build_fn_sigs_map` 添加 specialized function sigs (对每个 MonoItem::Fn with non-empty substs, compute specialized name + substituted signature); (2) `writeback_fndef_substs` secondary pass (propagate inferred substs from local_decls into Assign's Operand::Constant); (3) `codegen_from_mir` skip ALL prelude generic def bodies (not just those without MonoItem::Fn instantiation). 3 src files (~100 LOC) + 1 test 文件 (13 tests) + 1 docs/tools/debug-tools.md + 1 debug script (scripts/debug_obj_emit_crash.sh). v0.646.0. §3.2 全套验收通过 (898 lib + 4788 integration, 0 failures). |
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

### P2 — v0.12 TD 修复阶段 (Stages 99-121) — ALL RESOLVED or PERMANENTLY DEFERRED

| TD ID | 状态 | 描述 |
|-------|------|------|
| TD-TYPECK-WRITEBACK-INCOMPLETE | ✅ Stage 110 修复 (Phase 3.6) | typeck Phase 3.6 Constant type writeback (Infer→concrete). Infer warnings 41→19 (-54%). |
| TD-CODEGEN-CALL-ARG-TYPE-SOURCE | ✅ Stage 107 修复 | codegen call arg type 优先用 callee sig (非 Param 时), generic 时 fallback 到 operand type. |
| TD-CODEGEN-CONST-SRC-TY-FROM-CONSTVAL | ✅ Stage 109 修复 | codegen operand.rs 当 c.ty 为 concrete Int/Uint/Bool/Char 时用 emit_const_typed 直接 emit. 同时修复 Stage 18.287 遗留 bug (TextEmitter contract). |
| TD-MONO-INFER | ✅ Stage 113 修复 | writeback_fndef_substs secondary pass (propagate inferred substs from local_decls into Assign's Operand::Constant). |
| TD-LLVM-OBJ-EMIT-CRASH | ✅ Stage 113 修复 | build_fn_sigs_map 添加 specialized function sigs → 正确 forward declaration → 无 type mismatch. |
| TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH | ✅ Stage 100-103 修复 | Layer 1 (skip prelude generic def bodies) + Layer 2 (FnDef substs mangling) + Layer 4 (Drop releases context) + Layer 3 (resolve_lit_ty_from_expected). |
| TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION | ✅ Stage 115 修复 (partial) + Stage 119-120 (process isolation) | 4 sort fixes (HashMap → deterministic emission order) + compile_src/compile_silent subprocess isolation. |
| TD-PROCESS-PER-TEST-ISOLATION | ✅ Stage 119-120 实现 | compile_src + compile_silent 使用 subprocess (`landin-stage0 --check-errors`). `--check-errors` flag added to CLI. |
| TD-LLVM-INTERNAL-NONDETERMINISM | ⚠️ PERMANENTLY DEFERRED (Stage 121 最终 RCA) | LLVM C++ DenseMap hash function depends on heap layout → varies between runs. Cannot be fixed from Rust side. Debug impl bodies permanently deferred. |
| TD-TRAIT-METHOD-AMBIGUITY | ✅ Stage 127 修复 (UFCS fully-qualified form) | `<T as Trait>::method(receiver, args)` 完整实现. Parser + HIR + Resolver + MIR lower + codegen. 26 tests (8 正 + 11 负 + 4 边界 + 3 回归). 短形式 `Trait::method(receiver)` 推迟到 TD-UFCS-SHORT-FORM. |
| Debug impl bodies (i32/i64/bool/usize) | ⚠️ PERMANENTLY DEFERRED | Blocked by TD-LLVM-INTERNAL-NONDETERMINISM. Trait declaration preserved (users can impl Debug for own types). |


### P3 — v0.7+ trait 系统阶段

| TD ID | 描述 | 根因 | 修复方案 | 依赖 |
|-------|------|------|---------|------|
| TD-DISPLAY-TRAIT-MISSING-PARTIAL | format! 参数 &[i64] 限制类型；Display trait 已定义但 format! 重设计未完成 | format! impl 接收 i64 数组 (Stage 36.6) | &[&dyn Display] trait dispatch | Display trait ✅ (Stage 61) + full dyn Trait (v0.8+) |
| TD-TOSTRING-DEFAULT-BODY | Display::to_string 默认方法缺失 | Bug Z7 workaround (override per impl) 触发 libLLVM 间歇性 crash | LLVM codegen crash 调查 + 修复 | libLLVM bug (P3, v0.8+) |
| TD-FN-CLOSURE-COERCION | ~~closures 不自动实现 Fn traits~~ **FIXED Stage 79 + Stage 83** — typeck Closure↔FnPtr unification (Stage 79) + runtime alloca/load fix (Stage 83: removed Stage 16.21 redundant closure-arg alloca-pointer special-case) | ~~TyKind::Closure 无 Fn trait coercion~~ **DONE: typeck unify closure sig with fn ptr sig + codegen_operand loads fn ptr value** | ~~typeck closure → Fn trait coercion + vtable emission~~ **DONE** | TD-FN-TRAITS ✅ (Stage 62) |
| TD-FN-UNIT-ARGS | ~~`Fn<()>` unit tuple arg 不支持~~ **FIXED Stage 85** — `build_fn_sigs_map` filters `EmitType::Void` from sig map (mirrors ZST elision in codegen_function + terminator.rs) | ~~typeck/codegen 不支持 () as Args~~ **DONE** | ~~typeck/codegen 支持 unit tuple as Fn<Args>~~ **DONE** | TD-FN-TRAITS ✅ (Stage 62) |
| TD-ASSOC-TYPE-SCOPE | ~~associated type `Output` 在 2 impls 中冲突~~ **FIXED Stage 73** — resolver skips impl assoc types in global namespace | resolver 未按 impl 块 scope assoc types | ~~resolver scope assoc types per impl block~~ **DONE: pre-collect impl assoc type DefIds, skip in registration** | TD-FN-TRAITS ✅ (Stage 62) |
| TD-FN-IMPL-SIG-VALIDATION | ~~typeck 不校验 impl sig 匹配 Args/Output~~ **FIXED Stage 78+86** — param check (Stage 78: substitute trait generic args) + return check (Stage 86: resolve Self::Output projection via resolve_projection_in_ty_pub + fix find_assoc_type_def_id to match by name AND owner trait) | ~~typeck 缺少 impl signature 检查~~ **DONE** | ~~typeck validate impl fn sig vs trait Args/Output~~ **DONE** | TD-FN-TRAITS ✅ (Stage 62) |
| TD-GENERIC-TRAIT-METHOD-MANGLING | ~~泛型 trait method 调用 mangled 名错误~~ **PARTIAL FIX Stage 92** — `re_resolve_trait_method_calls` 现在为所有函数运行 (was: 仅泛型函数)。添加 `lookup_by_trait_method` + `lookup_by_method_name` 回退。完整 turbofish path resolution 仍需 MIR lower 修复。 | ~~`From::<i32>::from(42)` 产生 `fn_0_i32`~~ **PARTIAL** (re_resolve 基础设施修复; turbofish path 解析仍 broken) | ~~修复 generic trait method mangling~~ **PARTIAL** | trait resolver ✅ |
| TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION | `From::<i32>::from(42)` turbofish path 在 MIR lower 中解析为错误的 DefId (DefId(85) = landin_String_push_str 而非 trait 方法 DefId)。导致 re_resolve 找不到正确的 impl method。 | MIR lower 的 path resolution 对 turbofish trait method path 解析错误 — 返回了无关的 fn DefId 而非 trait 声明方法的 DefId | 1) 修复 MIR lower 的 turbofish path resolution; 2) 正确解析 `From::<i32>::from` 为 trait 方法 DefId + substs [i32] | TD-GENERIC-TRAIT-METHOD-MANGLING ✅ (Stage 92) — 发现于 Stage 92 调查 |
| TD-FN-ASSOC-TYPE-CALL | `<F as Fn<(Args,)>>::call(&f, args)` 显式调用语法不支持 | parser/typeck 未支持 explicit trait dispatch | parser/typeck 支持 explicit trait dispatch syntax | typeck |
| TD-DYN-TRAIT-COMPLETION | ~~dyn Trait typeck 不完整~~ **FIXED Stage 60+87** — Stage 60 partial (TraitObject → Ref(Error) placeholder) replaced by Stage 87 proper `TyKind::Dyn(DefId)`. typeck carries trait DefId + verifies trait impl bounds via `implements_by_def_ids`. Method resolution looks up methods in trait declaration for `Dyn` receivers. | ~~typeck 无 dyn Trait 代码~~ **DONE** | ~~typeck trait dispatch~~ **DONE** (typeck foundation; runtime vtable dispatch deferred to TD-DYN-TRAIT-RUNTIME-DISPATCH) | trait resolver ✅ |
| TD-IMPL-TRAIT-MONO-RESOLUTION | ~~impl Trait arg 方法调用在函数体内不解析~~ **FIXED Stage 69** — TraitMethodResolutionMap + re_resolve_trait_method_calls | monomorphization 不在类型替换后重新解析 trait 方法 | ~~mono pass 重新解析 trait 方法~~ **DONE: pre-computed map in driver, re-resolve in codegen** | TD-IMPL-TRAIT ✅ (Stage 63) |
| TD-IMPL-TRAIT-CALLSITE-CHECK | typeck 不校验 call site 实参是否满足 impl Trait bound | typeck 缺少 call site bound 检查 + 无 trait_resolver 访问 | typeck validate trait bounds at call site (需 trait_resolver 访问，v0.8+ 架构变更) | TD-IMPL-TRAIT ✅ (Stage 63) |
| TD-CFG-MACROS | cfg!/cfg_attr! 未实现 | 需配置系统 | 编译期 cfg 评估 | build system |
| TD-ASM-MACRO | asm! 未实现 | 需 LLVM inline asm | LLVM asm 支持 | LLVM backend |
| TD-FORMAT-ARGS-WRITE | ~~format_args!/write! 未实现~~ **FIXED Stage 91** — `format_args!` routes to `__landin_format_v2` (same as `format!`); `write!` expands to `dst.write_str(format_args!(...))`; `write_str` added to hygiene skip list. | ~~需 Display trait~~ **DONE** (reuses existing format! backend) | ~~Display trait 依赖~~ **DONE** | Display trait ✅ (Stage 61) |
| TD-ENV-MACROS | ✅ Stage 131 修复 | env!/option_env!/include_str! 编译期宏. std::env::var + std::fs::read_to_string. 5 tests. | ✅ |
| TD-DYN-TRAIT-RUNTIME-DISPATCH | ~~dyn Trait 运行时 vtable dispatch 不完整~~ **FIXED Stage 88** — `method_call_lower.rs` forces vtable dispatch for Dyn/Ref(Dyn) receivers (was: static dispatch used because Stage 87's resolve_trait_method found the method → `call i32 @null` broken). | ~~codegen fat pointer arg 传递 + vtable indirect call 未正确连接~~ **DONE** (vtable dispatch wired; fat pointer coercion at call sites deferred to TD-DYN-TRAIT-FAT-PTR-COERCION) | ~~1) Dyn alloca {ptr,ptr}; 2) call site 传 fat pointer; 3) vtable indirect call~~ **DONE** (vtable indirect call works; call site fat pointer construction pending) | TD-DYN-TRAIT-COMPLETION ✅ (Stage 87) |
| TD-DYN-TRAIT-FAT-PTR-COERCION | ~~dyn Trait unsized coercion codegen 不完整~~ **FIXED Stage 89** — call site now passes `@.dynptr.Trait.Concrete` (fat pointer global) instead of thin data pointer. Fix in `codegen/terminator.rs` + `build_type_name_by_def_id` (added Trait support). | ~~codegen coercion site 未构造 fat pointer~~ **DONE** | ~~detect Adt→Ref(Dyn) coercion site; construct fat pointer~~ **DONE** | TD-DYN-TRAIT-RUNTIME-DISPATCH ✅ (Stage 88) |
| TD-DYN-TRAIT-DATA-PTR-EXTRACT | ~~dyn Trait method call 传 fat pointer 给 impl method~~ **FIXED Stage 90** — vtable indirect call 从 fat pointer field 0 提取 data pointer 传给 impl method。`use_greeter(&e)` 返回 42 (was: 返回 0)。 | ~~codegen emit_dyn_trait_method_call 传 fat pointer~~ **DONE** | ~~GEP fat ptr field 0 → data ptr；传 data ptr 给 method fn~~ **DONE** | TD-DYN-TRAIT-FAT-PTR-COERCION ✅ (Stage 89) |

### P3 — v0.13+ UFCS 后续 (Stage 127 发现)

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-UFCS-SHORT-FORM | ✅ Stage 128 修复 | 短形式 `Trait::method(receiver, args)` 完整实现. MIR lower 双 patch (local_decl + Assign Constant). 17 tests. | — | ✅ |
| TD-UFCS-DEFAULT-BODY-EMPTY-IMPL | ✅ Stage 130 部分修复 | `resolve_impl_method_by_name` 回退到 trait 声明的默认方法 DefId。但 codegen 参数类型未正确特化 (TD-UFCS-DEFAULT-BODY-CODEGEN). 5 tests (1 ignored). |
| TD-UFCS-AMBIGUITY-E1109 | ✅ Stage 129 基本修复 | 普通方法调用 `obj.method()` 多 trait 同名方法时报错（返回 None 触发 "no method found"）。`resolve_trait_method` 收集所有候选，>1 不同 trait 时返回 None。精确 E1109 错误信息 + 候选 trait 名列出留给 v0.14+ (TD-UFCS-AMBIGUITY-E1109-CLEANUP). 10 tests. |


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

---

## 七、Stage 93 架构审查新发现 TD (2026-09-03)

### 新发现 TD — 架构审查 (Stage 93)

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PRELUDE-METHOD-COVERAGE | prelude 类型方法覆盖率不完整 (i32/str/String/Vec/Box/Option 缺多个方法) | prelude 只实现了 MVP 方法 | 扩展 prelude 方法覆盖 | P3, v0.9+ |
| TD-PRELUDE-TRAIT-COVERAGE | prelude trait 覆盖率不完整 (缺 Debug, Eq, Hash, Ord, Default, From/Into) | 只实现了 Clone/Copy/Display/Fn/Drop | 添加缺失 trait + impls | P3, v0.9+ |
| TD-PRINT-CODEGEN-INTERCEPT-TO-MACRO | println!/print! 用 codegen intercept 而非 macro expansion | Stage 18.18 特解 | 转为 macro expansion to printf call | P3, v0.9 |
| TD-OPAQUE-PTR-UNCHECKED-MIGRATION | OpaquePtr unchecked variant 用特解 (all Adt→OpaquePtr) | Stage 14.63 特解 | 迁移到 with_layouts variant | P3, v0.9 |
| TD-EMPTY-CLOSURE-OPAQUE-PTR-SPECIAL-CASE | 空 Closure → OpaquePtr 特解 (Stage 82 fix) | Closure coercion 特解 | FnPtr 类型直接 emit | P3, v0.9 |
| TD-DYN-TRAIT-VTABLE-HARDCODED-GLOBAL | dyn Trait vtable dispatch 用 hardcoded global name | Stage 5.79 特解 | 使用 fat pointer 值 (not global) | P3, v0.9 |
| TD-VEC-STRING-INTRINSIC-TO-METHOD-DISPATCH | String::push_str / Vec::push 用 MIR intrinsic 而非 method dispatch | Stage 18.229-230 特解 | 转 regular impl method dispatch | P3, v0.9 |
| TD-FORMAT-VARIADIC-INTRINSIC-TO-DISPLAY | __landin_format_variadic 用 MIR intrinsic 而非 Display trait | Stage 18.231 特解 | 转 Display trait dispatch | P3, v0.9 |
| TD-RUNTIME-PANIC-TO-LANDIN | Panic C wrappers 可转 Landin prelude fns (仅保留 abort 基石) | C wrapper 特解 | 格式化→Landin fn, abort→C 基石 | P3, v0.9+ |
| TD-COMPILE-ERROR-MACRO | ✅ Stage 132 修复 | compile_error! 编译期宏. 编译期 eprintln 报错 + 空 token 流. 4 tests. | ✅ |
| TD-MATCHES-MACRO | ✅ Stage 133 修复 | matches! 编译期宏. 展开为 match expr { pat => true, _ => false }. 使用 KwMatch/KwTrue/KwFalse. 5 tests. | ✅ |
| TD-TRACE-MACROS-MACRO | ✅ Stage 134 修复 | trace_macros! no-op 宏. 展开为 () unit 表达式. 3 tests. | ✅ |
| TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION | turbofish path `From::<i32>::from` 在 MIR lower 中解析为错误 DefId | MIR lower path resolution bug | 修复 turbofish path resolution | P3, v0.9+ |

### P3 — v0.15+ 系统性架构审查发现 (Stage 135)

#### 1. LEXER 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-LEX-RAW-STRING | ✅ Stage 144 修复 | TD 描述与实际不符: lexer 已实现 (Stage 6.13), 仅 parser 缺 RawStrLit arm | parser/expr.rs 添加 RawStrLit arm → LitKind::Str (§1.0 原則 6 通解). lexer/string.rs lex_byte 添加未闭合 ' 错误 push (§1.0 原則 4). 35 tests. | ✅ |
| TD-LEX-BYTE-LITERAL | ✅ Stage 144 修复 | 全链路已实现 (lex + parse + typeck + codegen), lex_byte 缺 closing ' 错误 push | 同 TD-LEX-RAW-STRING 修复. | ✅ |
| TD-CODEGEN-CAST-UNSIGNED | ✅ Stage 145 修复 | b'\xFF' as i64 返回 -1 而非 255 | codegen emit_cast 添加 src_signed: bool 参数 (§1.0 原則 5/6/10). LLVMSysEmitter 用 LLVMBuildIntCast2(is_signed). TextEmitter 用 sext vs zext. 添加 is_mir_type_signed + operand_is_signed helpers. 更新 7 个调用点. 副作用: bool as i64 现在返回 1 (正确 Rust 语义). 26 tests. | ✅ |
| TD-LEX-DOC-COMMENT | 缺少 doc comment tokens (///, //!, /** */) | Lexer 不区分 doc comment 和普通 comment | 添加 doc comment 检测 + 存储到 AST attributes | P4, v0.16+ |

#### 2. PARSER 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PARSE-ASSOC-TYPE-BOUND | 缺少 associated type bounds (impl Iterator<Item = i32>) | Parser 不支持 assoc type in bound | 添加 assoc type bound 解析 | P3, v0.15+ |
| TD-PARSE-GAT | 缺少 Generic Associated Types (trait Foo<T> { type Bar<U>; }) | Parser + HIR + Typeck 不支持 GAT | GAT 完整实现 (parser → HIR → typeck → codegen) | P4, v0.16+ |
| TD-PARSE-CONST-GENERIC | 缺少 const generics (fn foo<const N: usize>()) | Parser + HIR + Typeck 不支持 const generics | const generics 完整实现 | P4, v0.16+ |
| TD-PARSE-ASYNC-AWAIT | 缺少 async/await syntax | Parser + MIR 不支持 async | async/await state machine lowering | P4, v0.17+ |
| TD-PARSE-RANGE-PATTERN | ✅ 已实现 (审查时发现) | Parser 已支持 1..=5 range pattern in match. 测试通过. | ✅ |
| TD-PARSE-SLICE-PATTERN | 缺少 slice pattern ([a, .., b]) | Parser pattern 不支持 slice pattern | 添加 slice pattern 到 HirPatKind | P3, v0.15+ |
| TD-PARSE-EXTERN-BLOCK | ✅ 已实现 (审查时发现) | Parser + HIR 已支持 extern "C" { fn foo(); } 和 extern "C" fn foo() 语法. 2 tests 验证通过. | ✅ |
| TD-PARSE-IMPL-TRAIT-RETURN | 缺少 impl Trait in return position (fn foo() -> impl Trait) | Parser + Typeck 不支持 return position impl Trait | RPIT 完整实现 | P3, v0.15+ |
| TD-PARSE-LABEL | 缺少 label syntax ('label: loop { ... }) | Parser 不支持 label | 添加 label 到 loop/break/continue | P4, v0.16+ |

#### 3. TYPECK 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-TYPECK-ASSOC-TYPE-PROJECTION | ✅ Stage 146 修复 | 缺少 associated type projection resolution (<T as Iterator>::Item) | unify.rs 添加 Projection unify 分支 (§1.0 原則 6/9/10). codegen post-mono 调用 resolve_projections_in_mir (§11). driver/mod.rs projection_resolver pub. pipeline.rs 传递 HIR. 24 tests. 发现 TD-ASSOC-TYPE-MULTI-RUNTIME. | ✅ |
| TD-TYPECK-HRTB | 缺少 higher-ranked trait bounds (for<'a> fn(&'a str)) | Typeck 不支持 HRTB unification | HRTB 完整实现 | P4, v0.16+ |
| TD-TYPECK-AUTO-TRAIT | 缺少 auto trait coherence checking (Send/Sync auto impls) | Typeck + Trait solver 不支持 auto trait | auto trait 自动派生 | P4, v0.16+ |
| TD-TYPECK-NEGATIVE-IMPL | 缺少 negative impls (impl !Send for Foo) | Typeck 不支持 negative impl | 添加 negative impl 语法 + coherence | P4, v0.17+ |
| TD-TYPECK-LIFETIME-ELISION | 缺少完整 lifetime elision rules (3 rules from RFC 3081) | Typeck lifetime elision 不完整 | 实现 3 条 elision rules | P3, v0.15+ |
| TD-TYPECK-NEVER-FALLBACK | 缺少 never type fallback (! → ()) | Typeck 不支持 ! → () 自动转换 | 实现 never type fallback | P4, v0.16+ |

#### 4. BORROWCK 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-BORROWCK-NLL | 缺少 NLL (Non-Lexical Lifetimes) | Borrowck 使用 region_inference 但可能不完整 | 验证 NLL 完整性 + 修复 edge cases | P3, v0.15+ |
| TD-BORROWCK-TWO-PHASE | 缺少 two-phase borrows (let r = &mut x; *r = ...) | Borrowck 不支持 two-phase borrows | 实现 two-phase borrow analysis | P4, v0.16+ |
| TD-BORROWCK-CLOSURE-CAPTURE | 缺少 closure capture analysis (RFC 2229 disjoint capture) | Borrowck 不支持 disjoint closure capture | 实现 RFC 2229 capture analysis | P4, v0.16+ |
| TD-BORROWCK-VARIANCE | 缺少 variance checking (invariance/covariance/contravariance) | Typeck 不检查 type variance | 实现 variance checker | P4, v0.16+ |

#### 5. MIR 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-MIR-OPTIMIZATION | 缺少 MIR optimizations (const propagation, dead code elimination) | MIR 没有 optimization pass | 添加 MIR optimization passes | P4, v0.16+ |
| TD-MIR-BORROWCK | 缺少 MIR borrowck (current borrowck runs before MIR) | Borrowck 不在 MIR 上运行 | 迁移 borrowck 到 MIR-based (like rustc) | P4, v0.17+ |
| TD-MIR-ASYNC | 缺少 Generator/async state machine lowering | MIR 不支持 async state machine | 实现 async → state machine MIR lowering | P4, v0.17+ |

#### 6. CODEGEN 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-CODEGEN-DEBUG-INFO | 缺少 debug info generation (-g) | Codegen 不生成 DWARF debug info | 添加 LLVM debug info emission | P4, v0.16+ |
| TD-CODEGEN-OPT-LEVELS | ✅ Stage 136 修复 | -O/--opt-level CLI flag (0/1/2/3). LLVMCodeGenOptLevel 选择. 4 tests. | ✅ |
| TD-CODEGEN-LTO | 缺少 LTO (Link-Time Optimization) | Codegen 不支持 LTO | 添加 LLVM LTO 支持 | P4, v0.17+ |
| TD-CODEGEN-EMITTER-UNIFY | TextEmitter + LLVMSysEmitter 双路径 (~2000 LOC 重复) | 历史双 emitter 架构 | 统一为单一 emitter | P3, v0.15+ (已有 TD-SPECIAL-10) |
| TD-CODEGEN-OPAQUE-PTR | OpaquePtr for &Adt (递归 struct workaround) | Stage 14.63 特解 | 迁移到 Ptr(Adt) + 循环检测 | P3, v0.15+ (已有 TD-SPECIAL-13) |

#### 7. TRAIT SYSTEM 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-TRAIT-SPECIALIZATION | 缺少 specialization (RFC 1210) | Trait solver 不支持 specialization | 实现 specialization (min_specialization first) | P4, v0.17+ |
| TD-TRAIT-BLANKET-IMPL | 缺少 blanket impls (impl<T> Foo for T where T: Bar) | Trait solver 不支持 blanket impl | 添加 blanket impl 到 trait solver | P3, v0.15+ |
| TD-TRAIT-DYN-LIFETIME | 缺少 dyn Trait lifetime bounds (dyn Trait + 'a) | Typeck 不支持 dyn lifetime bounds | 添加 lifetime bounds to Dyn type | P3, v0.15+ |

#### 8. STDLIB / PRELUDE 缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-STDLIB-ITERATOR | ✅ Stage 159 修复 | 缺少 Iterator trait + adapters (map, filter, collect) | Prelude 不包含 Iterator | 添加 Iterator trait 到 prelude (`src/stdlib/prelude.rs`). 用户无需每次自定义 `trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }`. 10 tests. 同时移除了 5 个测试文件中的用户定义 `trait Iterator` 块. Adapters (map, filter, collect) 延迟到 v0.2+ (需要闭包支持 — TD-FN-CLOSURE-COERCION). | ✅ |
| TD-STDLIB-OPTION-METHODS | 缺少 Option/Result full method coverage (ok_or, map_err, and_then) | Prelude 只实现 MVP 方法 | 扩展 Option/Result 方法覆盖 | P3, v0.15+ |
| TD-STDLIB-HASH | 缺少 Hash trait + HashMap/HashSet | Prelude 不包含 Hash | 添加 Hash trait + HashMap/HashSet | P4, v0.16+ |
| TD-STDLIB-STRING-VEC | 缺少 String/Vec/Box full method coverage | Prelude 只实现 MVP 方法 | 扩展 String/Vec/Box 方法覆盖 | P3, v0.15+ |
| TD-STDLIB-PARTIALORD | 缺少 PartialOrd impls (i32/i64/bool/usize) | 受 TD-LLVM-INTERNAL-NONDETERMINISM 阻断 (与 Debug impl bodies 同类问题) | 等待 LLVM 非确定性解决 或使用 Stage 125 vtable filtering | P3, v0.15+ |

#### 9. 宏系统缺陷

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-MACRO-PROC | 缺少 procedural macros (derive, attribute, function-like) | 不支持 proc macro 系统 | 实现 proc macro 基础设施 | P4, v0.17+ |
| TD-MACRO-CRATE-PATH | 缺少 $crate path in macros | macro_rules! 不支持 $crate | 添加 $crate 特殊 metavariable | P3, v0.15+ |
| TD-MACRO-HYGIENE-COMPLETE | 宏 hygiene 不完整 | macro_rules! hygiene 不完整 | 完善 hygiene 系统 | P3, v0.15+ |

#### 10. 特解转通解

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-SPECIAL-PRINT-INTERCEPT | println!/print! 用 codegen intercept 而非 macro expansion (Stage 18.18 特解) | Stage 18.18 选择特解因 macro_rules! 未就绪 | 转为 macro_rules! expansion to printf call | P3, v0.15+ (已有 TD-PRINT-CODEGEN-INTERCEPT-TO-MACRO) |
| TD-SPECIAL-VEC-STRING-INTRINSIC | String::push_str / Vec::push 用 MIR intrinsic (Stage 18.229 特解) | Stage 18.229 选择特解 | 转 regular impl method dispatch | P3, v0.15+ (已有 TD-VEC-STRING-INTRINSIC-TO-METHOD-DISPATCH) |
| TD-SPECIAL-FORMAT-VARIADIC | __landin_format_variadic 用 MIR intrinsic (Stage 18.231 特解) | Stage 18.231 选择特解 | 转 Display trait dispatch | P3, v0.15+ (已有 TD-FORMAT-VARIADIC-INTRINSIC-TO-DISPLAY) |

#### 11. 自举能力差距

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-BOOTSTRAP-MINIMUM | 最小自举能力差距 — 需要全 Rust 语法子集 + 闭包 + trait + 泛型 + 错误处理 + 文件 I/O | 缺少 associated types, GATs, const generics, async, NLL, proc macros | 需要 50+ 个 stage 达到最小自举能力 | P4, v0.20+ |

#### 12. 可拓展性差距

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PLUGIN-SYSTEM | 缺少 plugin system (compiler plugins) | 架构无 plugin 接口 | 设计 plugin trait + 注册机制 | P4, v0.17+ |
| TD-MULTI-BACKEND | 缺少 trait for codegen backends (cranelift support) | 只有 LLVM backend | 设计 CodegenBackend trait + 添加 cranelift | P4, v0.17+ |
| TD-INCREMENTAL | 缺少 incremental compilation cache | 每次全量编译 | 设计 incremental compilation cache (like rustc) | P4, v0.18+ |
| TD-PARALLEL-COMP | 缺少 parallel compilation | 单线程编译 | 设计 parallel MIR lowering + typeck | P4, v0.18+ |
| TD-LSP | 缺少 LSP (Language Server Protocol) integration | 无 LSP server | 实现 LSP server for IDE support | P4, v0.18+ |

### P3 — v0.15+ Stage 137 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PTR-INDEX-CONST | ✅ Stage 138 修复 | *const T 索引支持. typeck/infer.rs + mir/lower/field_resolution.rs 添加 RawPtr(_, inner) 分支. 3 tests. | ✅ |
| TD-STDLIB-STRING-VEC-PARTIAL | ✅ Stage 143 修复 | TD-STDLIB-STRING-VEC 完整修复 — String + str 的 starts_with/ends_with/contains 添加到 prelude. byte-by-byte loop 用 raw pointer indexing (依赖 TD-PTR-INDEX-CODEGEN-2 修复). 35 tests. | ✅ |

### P3 — v0.15+ Stage 139 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-STR-FAT-PTR-LAYOUT-MISMATCH | ✅ Stage 140+143 修复 | String::starts_with/ends_with/contains 方法体无法正确访问 &str 参数的 ptr/len 字段 — &str 是 fat pointer {ptr, i64} 但 prelude str struct 用 {ptr: *mut u8, len: usize} 布局，两者不互操作 | Stage 140 完成 MIR lower (field_resolution.rs). Stage 143 完成 codegen (unwrap_fat_ptr_for_index Ptr(_) 不 LOAD + array_ty 不 strip + detect_place_type Ptr 分支 + emit_load 用 detect_place_type). String + str 方法已添加. 35 tests. | ✅ |

### P3 — v0.15+ Stage 140 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PTR-INDEX-CODEGEN | ✅ Stage 143 修复 (合并到 TD-PTR-INDEX-CODEGEN-2) | RawPtr 索引的 codegen GEP 生成错误 | Stage 143 完整修复 — 见 TD-PTR-INDEX-CODEGEN-2 | ✅ |

### P3 — v0.15+ Stage 141 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PTR-INDEX-CODEGEN-2 | ✅ Stage 143 修复 | RawPtr 索引 codegen 修复 — unwrap_fat_ptr_for_index Ptr(_) 不 LOAD (caller 责任, §1.0 原則 11) + array_ty 仅对 Ptr(Array) strip + detect_place_type Index 添加 Ptr/OpaquePtr 分支 + emit_load 用 detect_place_type + codegen_place_load_typed Index arm 添加 base_ty.is_ptr() 检查 | Stage 143 完整修复. 35 tests. | ✅ |

### P3 — v0.15+ Stage 142 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-PTR-INDEX-GEP-TYPE | ✅ Stage 143 修复 | emit_gep_index_ptr 需要动态使用 index local 的实际类型（i32 或 i64），而非固定类型 | Stage 143 修改 trait 方法签名添加 idx_ty 参数 — caller 查询 MIR local_decls 获取实际类型, TextEmitter 用 idx_ty 替代硬编码 i64. LLVMSysEmitter 忽略 idx_ty. | ✅ |

### P3 — v0.15+ Stage 146 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-ASSOC-TYPE-MULTI-RUNTIME | ✅ Stage 147 修复 | 多关联类型 trait + 泛型投影的运行时 segfault (exit code 0 but no output) | bodyless trait 方法获得唯一 DefId (enter_owner) + module_build 跳过 trait 方法注册 + mir_ty_kinds_compatible 添加 Projection 分支. 修复 TraitMethodResolutionMap key 冲突. 18 tests. | ✅ |

### P3 — v0.15+ Stage 148 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-GENERIC-ENUM-MATCH-ARMS | ✅ Stage 150 修复 | match arm pattern binding for generic enums 不解析 payload type T | resolve_enum_variant 使用 with_hir_and_generics + pattern_bindings 从 scrutinee substs 替换 Param(0). 9 tests. | ✅ |
| TD-TRAIT-METHOD-REMONO-LINK | Stage 147 bodyless trait 方法获得新 DefId 后, vtable 引用 `landin_Iterator_Counter_next` 但函数未 emit (linker error) | vtable method name resolution 使用新 trait method DefId pattern, 但 impl method emit 使用不同 name | 修复 vtable method name resolution 以匹配 impl method 的 emitted name (landin_Counter_next 而非 landin_Iterator_Counter_next) | P3, v0.16+ |

### P3 — v0.15+ Stage 149 调查发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-TRAIT-METHOD-GENERIC-RET-SKIP | ✅ Stage 149 修复 | trait 方法返回泛型枚举时 impl 方法不被 emit | statement_contains_param 的 Aggregate 分支不再检查 substs/field_tys (仅检查 operands). 15 tests. | ✅ |

### P3 — v0.15+ Stage 150 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-TRAIT-METHOD-RET-MATCH-GEP | ✅ Stage 151 修复 | Iterator sum 的 match arm 提取值不正确 | build_adt_layout 使用 with_hir_and_generics + adt_layout_to_emit_type 对 Param 用 I64 fallback. | ✅ |
| TD-OPTION-UNWRAP-OR-MATCH | unwrap_or 在 Some+None 组合时 Some 返回 0 | unwrap_or 内部的 match 对 Some 分支的 payload 提取不正确 | 调查 prelude Option::unwrap_or 的 match arm GEP | P3, v0.16+ |

### P3 — v0.15+ Stage 151 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-OPTION-AND-THEN-I32-MISMATCH | ✅ Stage 152 修复 | prelude Option/Result 的 map/and_then 方法在 i32 类型上返回 {i32,i64} 但调用者期望 {i32,i32} | Revert Stage 151 I64 fallback + 新增 substitute_adt_layout 替换 AdtLayout variant_payloads 的 Param → 具体类型. stage40 测试恢复 i32. | ✅ |

### P3 — v0.15+ Stage 152 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-CALL-DEST-TYPE-SUBSTS | ✅ Stage 153 修复 | `call_dest_type` 使用 `fn_sigs.get(&did).output` (带 Param) 而非特化后的签名 → loc 类型错误 (Param→I32 fallback, 但实际是 i64) | 复用 `terminator.rs:655-686` (Stage 18.107) 的成熟 substitute 模式: 从 `c.ty.kind = FnDef(did, substs)` 提取 substs (而非只用 `c.val`), 当 substs 非空时 `substitute(sig.output, substs)` 后传入 `mir_type_to_emit_type_with_layouts_and_mono`. 8 tests (3 正 + 3 回归 + 2 边界). | ✅ |

### P3 — v0.16+ Stage 153 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-TYPECK-GENERIC-ARG-VALIDATION | ✅ Stage 160 修复 | `identity::<i64>(42i32)` 等泛型调用的实参类型不匹配 turbofish 期望的类型时, typeck 静默接受而非报错 | typeck 的 generic call arg type 验证缺失 — 推断的 T 来自 turbofish 但不与实参类型比对 | 在 typeck 的 call arg check 中, 当 callee 是 generic 且 turbofish 指定了 substs 时, 验证每个实参类型与特化后的 inputs 一致, 不一致则报 type error (E0308: mismatched types). 同时修复 `bind_ty_var` 的 bounds-safe access. 8 tests. | ✅ |

### P3 — v0.16+ Stage 154 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-VTABLE-MISSING-DEFAULT-BODY | vtable 只包含 impl 提供的方法, 不包含 trait default body 方法 | resolver.rs vtable 构建只遍历 impl items, 不遍历 trait items 的 default body | 在 vtable 构建中, 遍历 trait items, 对有 default body 的方法添加 vtable entry (fn_name = `landin_{trait}_default_{method}`) | P3, v0.16+ |
| TD-DYN-TRAIT-METHOD-ARG-PLACEHOLDER | `codegen_dyn_trait_call_direct` 对 args[0] (receiver) 使用 `%arg0` 占位符而非实际 codegen | 历史: receiver 由 `emit_dyn_trait_method_call` 从 fat pointer 提取, 所以占位符无影响. 但对非 receiver args, 占位符是 bug | codegen_dyn_trait_call_direct 对 args[1:] 使用 `codegen_operand` 正确生成值 (Stage 154 已修复非 receiver args) | P3, v0.16+ (部分修复) |
| TD-OPTION-UNWRAP-OR-MATCH | ✅ Stage 155 验证已修复 | unwrap_or 在 Some+None 组合时 Some 返回 0 | Stage 152 substitute_adt_layout 间接修复了此问题 — AdtLayout 的 Param 被正确替换为具体类型. 验证: some.unwrap_or(0) = 42, none.unwrap_or(99) = 99. | ✅ |
| TD-TRAIT-METHOD-REMONO-LINK | ✅ Stage 155 验证 linker 已修复 | Stage 147 bodyless trait 方法获得新 DefId 后, vtable 引用 `landin_Iterator_Counter_next` 但函数未 emit (linker error) | Stage 147-149 的 DefId 修复 + module_build 跳过 trait 方法注册已解决 linker 问题. vtable 符号与函数 emit 名称匹配. 但运行时值仍有问题 (见 TD-OPTION-NONE-GENERIC-SUBSTS-MISSING). | ✅ (linker 修复) |

### P3 — v0.16+ Stage 155 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-OPTION-NONE-GENERIC-SUBSTS-MISSING | ✅ Stage 156 修复 | trait 方法体内部 `Option::None` 构造使用空 substs → Param fallback → `{i32,i32}` 而非 `{i32,i64}`. 导致 trait 方法返回 `Option<i64>` 的 None 分支 payload 类型错误. | `lower_path_expr` 添加 `infer_substs_from_return_type` helper: 当 `lower_path_generic_args` 返回空 substs 时, 从当前函数的返回类型 (`fn_sigs[owner_def_id].output`) 推断 concrete substs. 如果返回类型是 `Adt(enum_def_id, concrete_substs)` 且 def_id 匹配, 使用 concrete substs. 否则回退到空 substs (保留旧行为 — writeback 的 let-binding 路径处理). 10 tests. | ✅ |

### P3 — v0.16+ Stage 157 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-DEFAULT-BODY-SELF-TYPE | ✅ Stage 157 修复 | trait default body 方法的 `&self` 参数类型解析为 `Error` (因为方法在 trait 声明中, 不在 `method_to_impl_index` 中). 导致 codegen 将 `&self` 当作 `i32` → Call parameter type mismatch (`i32` vs `ptr`). | `compile_inner.rs` 添加 `resolve_default_body_self_type` fallback: 当 `resolve_self_param_type_for_sig` 返回 `None` 时, 查找声明该方法的 trait, 找到第一个 impl, 用 impl 的 self_ty 作为 `&self` 类型. 8 tests. | ✅ |
| TD-VTABLE-DEFAULT-BODY-MISSING-ENTRY | ✅ Stage 158 修复 | dyn dispatch 调用 default body 方法时, vtable 缺少 default body 方法的 entry | vtable 构建只遍历 impl items, 不遍历 trait items 的 default body. 当 dyn dispatch 尝试调用 default body 方法时, vtable slot index 越界或指向错误方法. | `traits/resolver.rs` vtable 构建后, 遍历 trait items, 对有 default body 但 impl 没有覆盖的方法添加 vtable entry (fn_name = `landin_{trait}_default_{method}`). 10 tests. | ✅ |
| TD-DEFAULT-BODY-FIELD-ACCESS | ✅ Stage 158 验证已修复 | default body 方法内部访问 `self.x` 等字段时返回错误值 (1 而非实际字段值) | default body 方法的 `self` 参数类型解析正确后, 字段访问的 GEP 仍然使用错误的 layout (可能是因为 self_ty 使用了 first impl 的类型, 但字段 layout 不匹配) | 调查 default body 方法的 self 字段访问 GEP — 可能需要从 impl 的 self_ty 构建正确的 AdtLayout. | ✅ (Stage 157 间接修复) |

### P3 — v0.17+ Stage 159 发现

| TD ID | 描述 | 根因 | 修复方案 | 优先级 |
|-------|------|------|---------|--------|
| TD-DYN-ITERATOR-ASSOC-TYPE | dyn dispatch `&mut dyn Iterator<Item = i64>` codegen 失败 — 关联类型投影 + dyn dispatch 组合问题 | 函数 `count_iterations(it: &mut dyn Iterator<Item = i64>)` 的 codegen 产生大量 Param warnings + linker error (undefined reference to `landin_count_iterations`) | 调查 dyn dispatch 与关联类型投影的组合 codegen — 可能需要在 codegen 中正确解析 `dyn Iterator<Item = i64>` 的关联类型绑定 | P3, v0.17+ |
| TD-INFERRED-TYPE-METHOD-MANGLING | 当 receiver 类型是 Infer/Error 时 (如 `let r = none.or(some); r.unwrap()` 无类型注解), method dispatch 找到 trait declaration 的 DefId 而非 impl 的 DefId → mono name 使用 Error 类型生成 `Option_unwrap_error` → linker error (undefined reference). Workaround: 添加 type annotation (`let r: Option<i32> = ...`) | typeck 未从方法返回类型推断 receiver 类型 — 当 `r` 是 Infer 时, `resolve_trait_method` 找到 trait declaration 的 DefId, 而非 impl 的 DefId. `mangle_ty_with_interner` 将 Error 类型 mangle 为 "error" | 在 typeck post_check 中从方法返回类型推断 receiver 类型, 或在 codegen re_resolve 中使用 fn_name_by_def_id 的方法名匹配而非 type_name 匹配 | P3, v0.17+ |
