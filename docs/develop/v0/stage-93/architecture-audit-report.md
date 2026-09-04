# Landin 编译器架构审查报告 — v0.632.0 (Stage 93)

> **审查日期**: 2026-09-03
> **版本**: v0.632.0
> **审查范围**: 类型系统、特解转通解、runtime.rs、宏系统
> **审查方法**: 5W2H + Rust 设计哲学对比

---

## 一、类型系统审查

### 1.1 MIR TyKind 完整性

| Rust TyKind | Landin TyKind | 状态 |
|-------------|---------------|------|
| Bool | Bool | ✅ |
| Char | Char | ✅ |
| Int(IntTy) | Int(IntTy) | ✅ |
| Uint(UintTy) | Uint(UintTy) | ✅ |
| Float(FloatTy) | Float(FloatTy) | ✅ |
| Str | Str | ✅ |
| Never (!) | Never | ✅ |
| Ref | Ref | ✅ |
| RawPtr | RawPtr | ✅ |
| Array | Array | ✅ |
| Slice | Slice | ✅ |
| Tuple | Tuple | ✅ |
| FnDef | FnDef | ✅ |
| FnPtr | FnPtr | ✅ |
| Closure | Closure | ✅ |
| Adt | Adt | ✅ |
| Alias (assoc type) | Projection | ✅ |
| Foreign | Foreign | ✅ |
| Dyn | Dyn | ✅ (Stage 87) |
| Param | Param | ✅ |
| Bound | — | ❌ v0.9+ |
| Placeholder | — | ❌ v0.9+ |
| Infer | Infer | ✅ |
| Error | Error | ✅ |

**结论**: 类型系统基本完整，仅缺 Bound/Placeholder (HRTB, v0.9+)。

### 1.2 Prelude Trait 覆盖率

| Rust Trait | Landin | 状态 |
|-------------|--------|------|
| Clone | ✅ | Stage 59 |
| Copy | ✅ | Stage 59 |
| Display | ✅ | Stage 61 |
| Fn/FnMut/FnOnce | ✅ | Stage 62 |
| Drop | ✅ | Stage 64 |
| Debug | ❌ | TD-PRELUDE-TRAIT-COVERAGE |
| Eq/PartialEq | ❌ | TD-PRELUDE-TRAIT-COVERAGE |
| Hash | ❌ | TD-PRELUDE-TRAIT-COVERAGE |
| Ord/PartialOrd | ❌ | TD-PRELUDE-TRAIT-COVERAGE |
| Default | ❌ | TD-PRELUDE-TRAIT-COVERAGE |
| From/Into | ❌ | TD-PRELUDE-TRAIT-COVERAGE |

### 1.3 Prelude 类型方法覆盖率

| 类型 | 缺失方法 |
|------|---------|
| i32/i64/bool | abs, pow, min, max, to_string, checked_*, wrapping_* |
| str | chars, bytes, starts_with, ends_with, contains, find, split, trim, parse |
| String | push, pop, clear, len, is_empty, as_str, into_bytes |
| Vec<T> | clear, extend, insert, remove, swap_remove, truncate, iter |
| Box<T> | into_inner, leak, from_raw |
| Option<T> | map_or, map_or_else, cloned, copied, get_or_insert, zip |

**新 TD**: TD-PRELUDE-METHOD-COVERAGE (P3, v0.9+)

---

## 二、特解转通解规划

| # | 特解 | 通解路径 | 优先级 | 新 TD |
|---|------|---------|--------|-------|
| 1 | println!/print! codegen intercept | macro → printf | P3 v0.9 | TD-PRINT-CODEGEN-INTERCEPT-TO-MACRO |
| 2 | OpaquePtr unchecked variant | use AdtLayouts | P3 v0.9 | TD-OPAQUE-PTR-UNCHECKED-MIGRATION |
| 3 | Empty Closure → OpaquePtr | FnPtr direct | P3 v0.9 | TD-EMPTY-CLOSURE-OPAQUE-PTR-SPECIAL-CASE |
| 4 | dyn Trait vtable hardcoded | fat pointer value | P3 v0.9 | TD-DYN-TRAIT-VTABLE-HARDCODED-GLOBAL |
| 5 | String/Vec MIR intrinsic | method dispatch | P3 v0.9 | TD-VEC-STRING-INTRINSIC-TO-METHOD-DISPATCH |
| 6 | format_variadic MIR intrinsic | Display trait dispatch | P3 v0.9 | TD-FORMAT-VARIADIC-INTRINSIC-TO-DISPLAY |
| 7 | TextEmitter/LLVMSysEmitter 双路径 | 统一 emitter | P3 v0.10+ | TD-SPECIAL-10 (已有) |

---

## 三、runtime.rs C Wrapper 审查

### 基石 (不可消除 — C/OS 接口)
1. `__landin_alloc` (malloc)
2. `__landin_dealloc` (free)
3. `__landin_realloc` (realloc)
4. `__landin_memcpy` (memcpy)
5. `__landin_eprintf` (vfprintf — variadic)
6. `__landin_abort` (新 — abort, panic 终止)

### 可转通解 (Landin fn 替代)
- 所有 panic_* 函数 → Landin prelude fn (格式化用 Landin, 仅 abort 用 C)
- `__landin_assert` → Landin fn (条件检查, 仅 abort 用 C)

**新 TD**: TD-RUNTIME-PANIC-TO-LANDIN (P3, v0.9+)

---

## 四、宏系统审查

### 基石宏
| 宏 | 方式 | 状态 |
|----|------|------|
| println!/print!/eprintln!/eprint! | codegen intercept (特解) | → macro expansion (v0.9) |
| format! | macro → __landin_format_v2 (通解) | ✅ |
| panic!/unreachable! | macro → C wrapper (通解) | ✅ |
| assert! | macro → conditional panic (通解) | ✅ |

### 拓展宏
| 宏 | 方式 | 状态 |
|----|------|------|
| format_args! | macro → format! backend (通解) | ✅ Stage 91 |
| write! | macro → dst.write_str(format_args!) (通解) | ✅ Stage 91 |
| vec! | macro → Vec::new + push (通解) | ✅ |
| dbg! | macro → panic_msg (特解) | → Debug trait (v0.9+) |
| todo!/unimplemented! | macro → panic_msg (通解) | ✅ |

### 编译期宏 (全部通解 ✅)
stringify!, concat!, file!, line!, module_path!, env!, option_env!, include_str!

### 未实现宏
cfg!, cfg_attr!, asm!, compile_error!, matches!, trace_macros!

### 依赖关系
```
基石层: println→printf, panic→__landin_panic_msg, unreachable→__landin_unreachable
拓展层: format→__landin_format_v2, format_args→format!, write→format_args!+write_str
编译期层: stringify/concat/file/line/env/include_str → compile-time
```

---

## 五、新 TD 汇总

| TD | 描述 | 优先级 |
|----|------|--------|
| TD-PRELUDE-METHOD-COVERAGE | prelude 类型方法覆盖率不完整 | P3, v0.9+ |
| TD-PRELUDE-TRAIT-COVERAGE | prelude trait 覆盖率不完整 | P3, v0.9+ |
| TD-PRINT-CODEGEN-INTERCEPT-TO-MACRO | println!/print! codegen intercept → macro | P3, v0.9 |
| TD-OPAQUE-PTR-UNCHECKED-MIGRATION | OpaquePtr unchecked → with_layouts | P3, v0.9 |
| TD-EMPTY-CLOSURE-OPAQUE-PTR-SPECIAL-CASE | Empty Closure special-case → FnPtr | P3, v0.9 |
| TD-DYN-TRAIT-VTABLE-HARDCODED-GLOBAL | vtable hardcoded global → fat ptr value | P3, v0.9 |
| TD-VEC-STRING-INTRINSIC-TO-METHOD-DISPATCH | MIR intrinsic → method dispatch | P3, v0.9 |
| TD-FORMAT-VARIADIC-INTRINSIC-TO-DISPLAY | format_variadic → Display trait | P3, v0.9 |
| TD-RUNTIME-PANIC-TO-LANDIN | Panic C wrappers → Landin prelude fns | P3, v0.9+ |
| TD-COMPILE-ERROR-MACRO | compile_error! 未完整实现 | P3, v0.9+ |
| TD-MATCHES-MACRO | matches! 未完整实现 | P3, v0.9+ |
| TD-TRACE-MACROS-MACRO | trace_macros! 未完整实现 | P3, v0.9+ |
| TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION | turbofish path MIR lower 解析错误 | P3, v0.9+ |

---

## 六、通解转换路线图

**v0.9 (高影响)**:
1. println!/print! codegen intercept → macro expansion
2. String/Vec intrinsic → method dispatch
3. format_variadic → Display trait dispatch
4. Panic C wrappers → Landin prelude fns
5. prelude trait coverage (Debug, Eq, Hash, Ord, Default, From/Into)

**v0.9-0.10 (中影响)**:
6. OpaquePtr unchecked → with_layouts
7. Empty Closure special-case → FnPtr direct
8. dyn Trait vtable hardcoded → fat pointer value
9. turbofish path resolution fix
10. prelude method coverage expansion

**v0.10+ (低影响)**:
11. TextEmitter/LLVMSysEmitter 统一
12. cfg!/asm!/compile_error!/matches! macro implementation
13. HIR reverse index
14. FatPtrLit/sizeof/OpaquePtr 标准化
