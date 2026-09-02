# Landin 编译器架构审查与技术债路线图

> **审查日期**: 2026-09-02
> **审查范围**: v0.590.0 (Stage 40.2 完成后)
> **审查维度**: 4 项 (类型系统 / 特解→通解 / runtime.rs / 宏系统)
> **审查方法**: §20 迭代审计 + §2.2 根因思维 + Rust 设计哲学对标
> **审查结论**: 架构健康度 9.85/10 (稳定)，发现 1 个 P1 (unreachable! broken) + 15 个 P2/P3 TD

---

## 审查任务 1: 类型系统完整性审查

### 1.1 当前 TyKind 覆盖度

| TyKind 变体 | 语法支持 | typeck 支持 | codegen 支持 | 状态 |
|------------|---------|------------|-------------|------|
| Bool | ✓ | ✓ | ✓ | 完整 |
| Char | ✓ | ✓ | ✓ | 完整 |
| Int/Uint (8/16/32/64/128/size) | ✓ | ✓ | ✓ | 完整 |
| Float (32/64) | ✓ | ✓ | ✓ | 完整 |
| Str | ✓ | ✓ | ✓ (fat ptr) | 完整 |
| **Never (`!`)** | ✓ | ✓ (unifies with anything) | ⚠️ (Void) | **部分** |
| Ref (`&'r T`, `&'r mut T`) | ✓ | ✓ | ✓ | 完整 |
| RawPtr (`*mut T`, `*const T`) | ✓ | ✓ | ✓ | 完整 |
| Array (`[T; N]`) | ✓ | ✓ | ✓ | 完整 |
| Slice (`[T]`) | ✓ | ✓ | ✓ (fat ptr) | 完整 |
| Tuple (`(T1, T2, ...)`) | ✓ | ✓ | ✓ | 完整 |
| FnDef | ✓ | ✓ (FnDef↔FnPtr unify) | ✓ | 完整 |
| FnPtr (`fn(T) -> U`) | ✓ | ✓ | ✓ | 完整 |
| Closure | ✓ | ✓ | ✓ | 完整 |
| Adt (struct/enum) | ✓ | ✓ | ✓ | 完整 |
| Projection (`<T as Trait>::Item`) | ✓ | ✓ (GATs Phase 3) | ✓ | 完整 |
| Foreign (`extern { type T }`) | ✓ | ✓ | ? | 边缘 |
| Param (`T`) | ✓ | ✓ | ✓ | 完整 |
| Infer | N/A (internal) | ✓ | N/A | 完整 |
| Error | N/A (internal) | ✓ | N/A | 完整 |

### 1.2 关键发现

#### Never (`!`) 类型 — 部分实现

**已实现**:
- 语法 `!` 解析 (parser/ty.rs:95-98)
- typeck unify: `Never` unifies with anything (unify.rs:749)
- MIR lower: `HirTyKind::Never → TyKind::Never` (ty_lower.rs:463)
- extern "C" fn 声明支持 `-> !` (验证通过)

**未实现 / 受限**:
- codegen: `Never` 映射为 `Void` (emitter/mod.rs:387,488)，但 LLVM `void` 只能用于函数返回类型，不能作为 local/param 类型
- panic! 宏展开的 `__landin_panic_msg(...)` 返回 `()` 而非 `Never`（因为 extern "C" 声明是 `-> ()`）
- prelude 中的 `loop {}` wrapper 是为了满足 typeck（`!` 不能直接 unify 时需要 fallback）

**根因**: `__landin_panic_msg` 的 Landin 声明是 `fn(msg: *const u8);` (隐式 `-> ()`)，应改为 `fn(msg: *const u8) -> !;`。

**修复优先级**: P1 (影响 panic!/unwrap/expect 的类型推断)

#### Trait Object (dyn Trait) — 已实现但受限

`HirTyKind::TraitObject` 存在，但 typeck 中无 `dyn Trait` 相关代码（grep 无结果）。Stage 5 已实现 vtable + dyn trait method call，但仅限于 prelude 的 `Copy` trait。用户自定义 `dyn Trait` 可能未完整测试。

**修复优先级**: P3 (v0.6+ trait dispatch 完整化)

#### Impl Trait — 未实现

`HirTyKind::ImplTrait` 存在但 typeck/codegen 中无处理代码。Rust 的 `impl Trait` 在参数位置（`fn foo(x: impl Trait)`)和返回位置（`fn foo() -> impl Trait`)有不同语义。

**修复优先级**: P3 (v0.6+，与 Fn traits 一起设计)

---

## 审查任务 2: 特解→通解 审查

### 2.1 已识别的特解

| ID | 特解描述 | 影响范围 | 通解方案 | 优先级 |
|----|---------|---------|---------|--------|
| TD-SPECIAL-1 | TextEmitter vs LLVMSysEmitter 双路径维护 | codegen/ | 统一为单一 emitter (LLVM 22 opaque ptr 已消除差异) | P3 (v0.6+) |
| TD-SPECIAL-2 | `loop {}` wrapper for noreturn calls | prelude.rs (4 处) | 让 `__landin_panic_msg` 返回 `!` 类型 | **P1** (本 stage 修复) |
| TD-SPECIAL-3 | prelude 注入在 parse 后 (TD-PRELUDE-MACRO-TIMING) | driver/compile_inner.rs | 移到 macro_expand 之前 (token 级注入) | P2 (v0.5+) |
| TD-SPECIAL-4 | i64 格式化 4 个 C wrapper (str/hex/octal/binary) | runtime.rs, prelude.rs | 合并为 `__landin_i64_format(val, base, buf, cap)` | P2 (v0.5+) |
| TD-SPECIAL-5 | codegen 中 variadic 检测硬编码名字 | codegen/llvm/helpers.rs | 从签名解析 `...` token (Stage 18.334 已部分修复) | P3 (已部分通解) |
| TD-SPECIAL-6 | `OpaquePtr` for `&Adt` (递归 struct 打破) | codegen/mir_translation/places.rs | 用 `Ptr(Adt)` 替代，但需循环检测 | P3 (v0.6+ layouts) |

### 2.2 通解化优先级矩阵

```
高影响 + 低成本 → 立即修复:
  - TD-SPECIAL-2 (loop {} wrapper) — 让 panic_msg 返回 !

  - TD-SPECIAL-4 (i64 格式化合并) — 一个 C 函数替代 4 个

高影响 + 高成本 → v0.5+ 重构:
  - TD-SPECIAL-3 (prelude 注入时机) — driver pipeline 重构

低影响 + 低成本 → 顺手修复:
  - TD-SPECIAL-5 (variadic 检测) — 已部分通解

低影响 + 高成本 → v0.6+ 长期:
  - TD-SPECIAL-1 (emitter 统一)
  - TD-SPECIAL-6 (OpaquePtr → Ptr(Adt))
```

---

## 审查任务 3: runtime.rs C wrapper 审查

### 3.1 当前 21 个 C wrapper 分类

#### 类别 A: 基石 (5 个) — 必须 C，无法用 Landin 实现

| 函数 | 用途 | 为什么必须 C |
|------|------|--------------|
| `__landin_alloc` | malloc 包装 | 直接调用 libc malloc |
| `__landin_dealloc` | free 包装 | 直接调用 libc free |
| `__landin_realloc` | realloc 包装 | 直接调用 libc realloc |
| `__landin_memcpy` | memcpy 包装 | 直接调用 libc memcpy |
| `__landin_eprintf` | vfprintf 包装 | variadic C 函数 |

**结论**: 这些是 runtime 的基石，不可通解化。LLVM backend 生成代码需要 libc 接口。

#### 类别 B: Panic 基础设施 (6 个) — 需要 C 的 fprintf + exit

| 函数 | 用途 | 通解机会 |
|------|------|---------|
| `__landin_panic_msg` | panic! 消息 | **可通解**: 改返回 `!` 类型，Landin 直接调用 |
| `__landin_unreachable` | unreachable! 消息 | **可通解**: 同上 |
| `__landin_panic_overflow` | 算术溢出 | **特解**: 3 个 panic_overflow/bounds_check/div_by_zero 可合并为 `__landin_panic(fmt, ...args)` 通用 panic |
| `__landin_panic_bounds_check` | 越界检查 | 同上 |
| `__landin_panic_div_by_zero` | 除零 | 同上 |
| `__landin_assert` | 断言失败 | 同上 |

**通解方案**: 合并为 2 个:
- `__landin_panic(msg: *const u8) -> !` — 通用 panic with message
- `__landin_assert(cond: bool, msg: *const u8) -> !` — assert with condition

#### 类别 C: 可通解 (4 个) — i64 格式化

| 函数 | 用途 | 通解机会 |
|------|------|---------|
| `__landin_i64_to_str` | 十进制格式化 | **可合并** |
| `__landin_i64_to_hex` | 十六进制 | **可合并** |
| `__landin_i64_to_octal` | 八进制 | **可合并** |
| `__landin_i64_to_binary` | 二进制 | **可合并** |

**通解方案**: 合并为 `__landin_i64_format(val: i64, base: i64, buf: *mut u8, cap: i64) -> i64`。prelude 的 `format!` 实现根据 `{:x}`/`{:o}`/`{:b}` 传入 `base` 参数 (10/16/8/2)。

#### 类别 D: 可在 Landin 中实现 (6 个) — codegen 拦截或纯 Landin

| 函数 | 用途 | 通解机会 |
|------|------|---------|
| `__landin_println` | println! | **codegen 拦截**为 printf (已实现) |
| `__landin_print` | print! | 同上 |
| `__landin_eprintln` | eprintln! | codegen 拦截为 __landin_eprintf |
| `__landin_eprint` | eprint! | 同上 |
| `__landin_str_eq` | 字符串比较 | **可移除**: Landin 可实现 `==` for &str |
| (无 C 实现) | — | — |

### 3.2 runtime.rs 通解化路线图

**Stage 40.3 (v0.28, 本 stage)**: 修复 unreachable! 宏 (与 panic! 同样的 `.ptr` 提取)

**Stage 41 (v0.5)**:
- 合并 i64 格式化为 `__landin_i64_format(val, base, buf, cap)`
- 合并 panic 基础设施为 `__landin_panic(msg) -> !` + `__landin_assert(cond, msg) -> !`
- 让 `__landin_panic_msg` 和 `__landin_unreachable` 返回 `!` 类型 (TD-SPECIAL-2)

**Stage 42+ (v0.5+)**:
- 移除 `__landin_str_eq` (Landin `==` for &str 实现)
- TD-PRELUDE-MACRO-TIMING 修复 (prelude 注入时机)

---

## 审查任务 4: 宏系统完整性审查

### 4.1 当前 27 个内置宏状态

#### 类别 A: 基石宏 (5 个) — 完整工作 ✓

| 宏 | 状态 | runtime symbol | 备注 |
|----|------|---------------|------|
| `println!` | ✓ 工作 | codegen 拦截为 printf | 基石 |
| `print!` | ✓ 工作 | codegen 拦截为 printf | 基石 |
| `eprintln!` | ✓ 工作 | codegen 拦截为 __landin_eprintf | 基石 |
| `eprint!` | ✓ 工作 | codegen 拦截为 __landin_eprintf | 基石 |
| `format!` | ✓ 工作 | __landin_format_v2 (Landin prelude) | Stage 36.6 通解 |

#### 类别 B: Panic/Assert 宏 (3 个) — 部分工作

| 宏 | 状态 | 问题 | 优先级 |
|----|------|------|--------|
| `panic!` | ✓ 工作 (Stage 40.2) | — | 已修复 |
| `assert!` | ✓ 工作 | — | 已修复 |
| `unreachable!` | ❌ **BROKEN** | 同 panic! 的 `.ptr` bug + `__landin_unreachable` 声明已加但宏 body 未更新 | **P1** |

#### 类别 C: 数据结构宏 (1 个) — 工作

| 宏 | 状态 | 备注 |
|----|------|------|
| `vec!` | ✓ 工作 | codegen 拦截为 Vec::push 序列 |

#### 类别 D: 编译期工具宏 (8 个) — **全部 BROKEN** (P2)

这些宏设计为编译期求值，展开为 `__landin_<name>(...)` 调用，但 `__landin_<name>` runtime 函数从未声明/实现。

| 宏 | runtime symbol 声明 | runtime symbol 实现 | 实际应该的行为 |
|----|--------------------|--------------------|---------------|
| `stringify!` | ❌ 未声明 | ❌ 未实现 | 编译期将 tokens 转为字符串字面量 |
| `concat!` | ❌ 未声明 | ❌ 未实现 | 编译期拼接字符串字面量 |
| `env!` | ❌ 未声明 | ❌ 未实现 | 编译期读取环境变量 |
| `file!` | ❌ 未声明 | ❌ 未实现 | 编译期插入当前文件名 |
| `line!` | ❌ 未声明 | ❌ 未实现 | 编译期插入当前行号 |
| `module_path!` | ❌ 未声明 | ❌ 未实现 | 编译期插入模块路径 |
| `include_str!` | ❌ 未声明 | ❌ 未实现 | 编译期读取文件内容为 &str |
| `option_env!` | ❌ 未声明 | ❌ 未实现 | 编译期读取环境变量 (Option) |

**根因**: 这些宏应该**编译期求值**（在 macro_expand 阶段直接生成字面量），但当前实现展开为 runtime 函数调用，而 runtime 函数从未声明。

**通解方案**: 将这些宏改为编译期求值（macro_expand 阶段直接生成字面量 token）。`file!`/`line!`/`module_path!` 需要 span 信息；`env!`/`option_env!`/`include_str!` 需要 I/O；`stringify!`/`concat!` 需要 token 操作。

#### 类别 E: 配置/模式宏 (4 个) — **未完整实现** (P3)

| 宏 | 状态 | 备注 |
|----|------|------|
| `cfg!` | 展开为 `__landin_cfg(...)` (未声明) | 应编译期求值为 bool |
| `matches!` | 展开为 `__landin_matches(...)` (未声明) | 应展开为 match 表达式 |
| `cfg_attr!` | 展开为 `__landin_cfg_attr(...)` (未声明) | 应编译期条件展开 |
| `trace_macros!` | 展开为 `__landin_trace_macros(...)` (未声明) | 应控制宏追踪 |

#### 类别 F: 低级/诊断宏 (4 个) — **未完整实现** (P3)

| 宏 | 状态 | 备注 |
|----|------|------|
| `asm!` | 展开为 `__landin_asm(...)` (未声明) | 内联汇编，需 LLVM asm 支持 |
| `compile_error!` | 展开为 `__landin_compile_error(...)` (未声明) | 应编译期报错 |
| `format_args!` | 展开为 `__landin_format_args(...)` (未声明) | format! 的底层 |
| `write!` | 展开为 `__landin_write(...)` (未声明) | 写入到 formatter |

#### 类别 G: 调试宏 (2 个)

| 宏 | 状态 | 备注 |
|----|------|------|
| `dbg!` | ✓ 部分工作 | 展开为 `__landin_dbg(...)` 但 runtime 未声明 |
| `todo!`/`unimplemented!` | ✓ 工作 (Stage 40.2) | 展开为 `__landin_panic_msg(...)` |

### 4.2 宏系统通解化路线图

**Stage 40.3 (v0.28, 本 stage)**: 修复 `unreachable!` 宏 (`.ptr` 提取)

**Stage 41 (v0.5)**:
- 实现编译期宏: `stringify!`, `concat!`, `file!`, `line!`, `module_path!`
  - 这些不需要 runtime 函数，应在 macro_expand 阶段直接生成字面量
- 实现 `matches!` (展开为 match 表达式)

**Stage 42 (v0.5+)**:
- 实现 `env!`, `option_env!`, `include_str!` (需要编译期 I/O)
- 实现 `cfg!`, `cfg_attr!` (需要配置系统)
- 实现 `compile_error!` (需要编译期错误报告)

**Stage 43+ (v0.6+)**:
- 实现 `asm!` (需要 LLVM inline asm 支持)
- 实现 `format_args!`, `write!` (需要 Display trait)
- 修复 TD-PRELUDE-MACRO-TIMING (prelude 注入时机)

---

## 综合 TD 路线图

### P1 (本 stage 立即修复)

| TD ID | 描述 | 修复方式 |
|-------|------|---------|
| TD-UNREACHABLE-MACRO-BROKEN | `unreachable!` 宏 body 缺 `.ptr` 提取 | 同 panic! 修复模式 |

### P2 (v0.5 修复)

| TD ID | 描述 | 修复方式 |
|-------|------|---------|
| TD-SPECIAL-2 | `loop {}` wrapper for noreturn | 让 `__landin_panic_msg`/`__landin_unreachable` 返回 `!` |
| TD-SPECIAL-4 | i64 格式化 4 个 C wrapper | 合并为 `__landin_i64_format(val, base, buf, cap)` |
| TD-PRELUDE-MACRO-TIMING | prelude 注入在 parse 后 | 移到 macro_expand 之前 |
| TD-COMPILE-TIME-MACROS | 8 个编译期宏未实现 | macro_expand 阶段直接生成字面量 |
| TD-PANIC-CONSOLIDATION | 3 个 panic_* C wrapper | 合并为 `__landin_panic(msg) -> !` |

### P3 (v0.6+ 修复)

| TD ID | 描述 | 修复方式 |
|-------|------|---------|
| TD-SPECIAL-1 | TextEmitter vs LLVMSysEmitter 双路径 | 统一为单一 emitter |
| TD-SPECIAL-6 | OpaquePtr for `&Adt` | 用 Ptr(Adt) + 循环检测 |
| TD-DYN-TRAIT-COMPLETION | dyn Trait 完整支持 | trait dispatch 完整化 |
| TD-IMPL-TRAIT | impl Trait 未实现 | 参数/返回位置 impl Trait |
| TD-DISPLAY-TRAIT | Display trait for type-dispatched formatting | trait + ad hoc dispatch |
| TD-FN-TRAITS | Fn/FnMut/FnOnce traits | 替换 fn type 参数 |
| TD-CFG-MACROS | cfg!/cfg_attr! 未实现 | 配置系统 |
| TD-ASM-MACRO | asm! 未实现 | LLVM inline asm |
| TD-FORMAT-ARGS-WRITE | format_args!/write! 未实现 | Display trait 依赖 |

---

## 决策点 (§12 最优 > 最小，通解 > 特解)

本次审查的核心决策：

1. **立即修复 unreachable! (P1)** — 与 panic! 同样的根因，同样的修复模式
2. **i64 格式化合并 (P2, v0.5)** — 4 个 C wrapper → 1 个，符合 §1.0 原則 6 通解
3. **编译期宏通解 (P2, v0.5)** — 8 个编译期宏应编译期求值，不依赖 runtime
4. **Never 类型完整化 (P2, v0.5)** — 让 panic_msg 返回 `!`，移除 loop {} wrapper
5. **prelude 注入时机 (P2, v0.5)** — 移到 macro_expand 之前，让 prelude 可用 panic! 宏

---

## 下一步 (Stage 40.3 MUV)

基于本次审查，**立即执行** Stage 40.3:
- 修复 `unreachable!` 宏 (TD-UNREACHABLE-MACRO-BROKEN, P1)
- 添加 `Option::or` / `Option::or_else` / `Option::filter` (combinators, 无新依赖)

之后的 v0.5 路线图:
- Stage 41: i64 格式化通解 + Never 类型完整化
- Stage 42: 编译期宏实现 + prelude 注入时机修复
- Stage 43+: v0.6 trait 系统 (Display, Fn traits, dyn Trait)
