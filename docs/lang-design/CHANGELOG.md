# Forge 蓝图 v1.1 变更日志

> **版本**: v1.1 · **日期**: 2026-07-18 · **状态**: 修正 v1.0 审查发现的所有严重问题
>
> v1.1 基于 5 路深度审查（R5 理论一致性 / R6 rustc 源码对照 / R7 经典书籍审查 / R8 自举可行性 / R9 文档内部一致性）的 80+ 条问题反馈，对 v1.0 做系统性修正。

---

## 一、重大策略调整

### 1. 自举目标重新定义

**v1.0 错误**：声称 12-15 月完成自举 MVP。

**v1.1 修正**（基于 R8 报告）：

- **v0.1**（12 月）：交付 stage 0 编译器（Rust 实现），可编译第三方 Forge 程序，**不要求自举**
- **v0.2**（24 月）：标准库扩展 + 工具链完善
- **v0.3**（36 月）：完成自举（stage 1 用 Forge 重写 + stage 2 验证）

**理由**：

- R8 对照业界：Rust 用 60 月、Zig 用 84 月完成自举；8 门可比语言中只有 Zig 完成自举
- R8 工作量重估：stage 0 实际需 130-180k 行 Rust，v1.0 估算的 53k 行低估 2.5-3.5x
- R8 时间线重估：1 人全职实际需 30-54 月，v1.0 的 15 月不可行
- R4 印证：Hare 5 年仍未完成编译器自举，永久维护 C bootstrap 是反模式

### 2. Stage 0 frozen blob 策略修正

**v1.0 错误**：选 LLVM bitcode 作 stage 0 frozen blob。

**v1.1 修正**：改用 **预编译二进制 + 源码双备份** 策略（参考 Rust 模式）：

- 每个目标平台提供预编译 `forge-stage0` 二进制（约 15-30 MB）
- 同时提交 stage 0 Rust 源码（用户可用系统 Rust 工具链重建）
- SHA256 校验保证完整性

**理由**（R8 报告）：

- LLVM bitcode **backward compatible but NOT forward compatible**，major 版本升级允许破坏
- llvm-sys.rs 强制 LLVM 版本严格匹配，跨版本不可用
- 5 年内必有 3-4 次破坏性 LLVM 升级
- Rust 自身用预编译二进制而非 bitcode，正是为此
- WASM blob（Zig 路线）虽更稳定，但需自写 4000 行 WASI 解释器，MVP 阶段投入过大

### 3. 宏系统重新定义

**v1.0 错误**：声明 MVP 无 macro，但 stdlib 大量使用 `write!`/`vec!`/`println!`/`matches!`/`assert_eq!`/`panic!`。

**v1.1 修正**：MVP 包含 **内建宏集**（不开放给用户自定义）：

- `println!` / `print!` / `eprintln!` / `eprint!` — I/O 输出
- `format!` — 字符串格式化
- `write!` / `writeln!` — 写入 Writer
- `vec!` — Vec 构造
- `matches!` — 模式匹配判断
- `assert!` / `assert_eq!` / `assert_ne!` — 测试断言
- `panic!` — panic 触发
- `dbg!` — 调试输出
- `concat!` / `stringify!` / `file!` / `line!` — 编译期信息

这些宏由编译器硬编码展开，用户**不能**用 `macro_rules!` 定义新宏（推迟到 v0.2）。

### 4. `?Sized` 与 unsized 类型矛盾修正

**v1.0 错误**：声明 MVP 无 `?Sized`，但 stdlib 的 `Box<dyn Trait>`、`Vec::Deref<Target=[T]>`、`String::Deref<Target=str>`、`NonNull<[u8]>` 都需要 `?Sized`。

**v1.1 修正**：MVP **部分支持 unsized 类型**：

- ✅ 支持 `str`、`[T]`（slice）作为 unsized 类型
- ✅ 支持 `dyn Trait` 作为 unsized trait object
- ✅ 支持 `?Sized` bound（仅用于 Deref Target / Box / Rc 内部）
- ⚠️ 不支持自定义 unsized type（v0.2 加 `#[repr(transparent)]` 等）
- ⚠️ 不支持 unsized 字段（v0.2 加）

---

## 二、Soundness 漏洞修复（R5 报告）

### 5. NLL 算法补全

**v1.0 问题**：04 文档 §4.2 的 NLL 算法不完整，无法处理 universal region 与 implied bounds，可构造 use-after-free 反例。

**v1.1 修正**：在 04 文档新增 §4.6 "NLL 算法完整规范"，包含：

- Universal region 与 placeholder region 机制
- Universe 概念（避免 HRTB 求解时的变量捕获）
- Type tests（验证 `T: 'a` 约束）
- SCC 压缩（避免 O(R²×P) 退化）
- Implied bounds 推导（参考 RFC 1214）

### 6. Associated type normalization 终止保证

**v1.0 问题**：03 文档 §7 的 normalization 算法无终止保证，self-referential associated type 会无限归一化。

**v1.1 修正**：在 03 文档新增 §7.1 "Normalization 终止性"，包含：

- Normalization depth limit = 32（远小于 trait resolution 的 128）
- Cycle 检测：normalization stack 中出现循环则报错
- Placeholder types（避免归一化产生新 inference variable）

### 7. Drop check 机制补全

**v1.0 问题**：04、06 文档完全缺失 dropck 机制，generic Drop impl 可能观察已 drop 的引用数据。

**v1.1 修正**：在 04 文档新增 §5 "Drop check"，在 06 文档 §10 补全 drop elaboration：

- 引入 `may_dangle` 属性（参考 Rust RFC 1327）
- Drop impl 默认要求所有 lifetime 参数 `: 'static`
- `#[may_dangle]` 标注的 lifetime/ty 参数可放宽
- 给出 Inspector 反例与正确实现

### 8. FalseEdge 恢复

**v1.0 问题**：06 文档 §12 省略 `FalseEdge`，导致 match with guard 的 CFG lowering 错误。

**v1.1 修正**：恢复 `FalseEdge` 到 TerminatorKind，在 06 文档 §7 补全 match lowering 算法。

### 9. Lifetime elision 边界规则

**v1.0 问题**：04 文档 §3.2 的 elision 三规则在 4 类边界 case 不健全。

**v1.1 修正**：在 04 文档 §3.2 补全：

- 嵌套引用 `&'a &'b T` 的 elision 处理
- `Box<Self>` 方法的 elision
- async fn 的 elision（v0.2）
- 泛型类型隐含 lifetime 的处理

### 10. 整数 fallback 与 trait selection 交互

**v1.0 问题**：03 文档 §4.6 整数 fallback 与 trait bound 交互可产生 silently changing impl。

**v1.1 修正**：MVP 阶段整数 fallback **仅在无 trait constraint 时触发**：

- 若 inference variable 同时有 trait bound，不触发 fallback
- 报错"type annotations needed"
- v0.2 再考虑更精细的 fallback 规则

### 11. `?` 操作符与 From trait 唯一性

**v1.0 问题**：`?` 操作符的 From trait 转换在多 impl 候选下未指定选择规则。

**v1.1 修正**：MVP 要求 `From<E1> for E2` 在 `?` 上下文必须有**唯一 impl**：

- 若多 impl 候选，报错"ambiguous From implementation"
- 用户必须显式 `.map_err()` 转换

---

## 三、MIR 完备性修复（R6 报告）

### 12. MIR StatementKind 补全

**v1.0 问题**：06 文档 §3 仅 6 种 StatementKind，缺 `FakeRead`/`SetDiscriminant`/`PlaceMention`/`Intrinsic`。

**v1.1 修正**：补全为 10 种：

- `Assign(Place, Rvalue)`
- `FakeRead(ReadCause, Place)` — match scrutinee / closure capture
- `SetDiscriminant(Place, VariantIdx)` — 直接设 enum tag（优化）
- `Deinit(Place)` — 标记 place 为未初始化（v0.2）
- `StorageLive(Local)` / `StorageDead(Local)`
- `Intrinsic(NonDivergingIntrinsic)` — `copy_nonoverlapping` 等
- `AscribeUserType(Place, Ty, Variance)` — 用户类型标注
- `ConstEvalCounter` — 编译期求值限制
- `Nop`

### 13. MIR TerminatorKind 补全

**v1.0 问题**：缺 `UnwindResume`/`UnwindTerminate`/`InlineAsm`。

**v1.1 修正**：MVP 仅 `UnwindResume`（unwind 不实现，仅占位）。`UnwindTerminate` 与 `InlineAsm` 推到 v0.2。

### 14. MIR BorrowKind 补全

**v1.0 问题**：仅 Shared/Mut/Unique 三种。

**v1.1 修正**：补全为：

- `Shared` — `&T`
- `Shallow` — `let _ = expr.field`（仅借用表面）
- `Mut { kind: MutBorrowKind }` — `&mut T`，含 `Default`/`ClosureCapture`/`TwoPhaseBorrow`
- `Unique` — v0.2

### 15. MIR Operand 补全

**v1.0 问题**：缺 `RuntimeChecks`（debug/release 溢出检查）。

**v1.1 修正**：补 `Repeat`、`Repeat`，但 MVP 暂不实现 `RuntimeChecks`（溢出检查由 LLVM 生成）。

### 16. MIR CastKind 补全

**v1.0 问题**：仅 4 种 CastKind。

**v1.1 修正**：补全为 7 种（仍少于 rustc 11 种，余下推 v0.2）：

- `Numeric` — 数值类型间
- `Pointer` — 指针间
- `PointerExposeAddress` — 指针→整数
- `PointerFromExposedAddress` — 整数→指针
- `Unsize` — `[T; N]`→`[T]`
- `FnPointer` — fn item→fn pointer
- `Transmute` — `mem::transmute`

### 17. HIR HirId / Body 外置存储机制

**v1.0 问题**：05 文档说 "HIR 与 AST 共享 80% variant" 是错的。

**v1.1 修正**：在 05 文档 §12 重写 HIR 部分：

- 引入 `HirId` 作为 HIR 节点唯一标识
- 引入 `Body` / `BodyId` 外置存储（函数体与 item 分离）
- 引入 `OwnerNodes` 机制
- HIR 与 AST 共享不超过 50% 结构

### 18. HIR ItemKind 补 Union

**v1.1 修正**：MVP 不支持 union（推 v0.2），但 HIR 保留 `Union` variant 占位。

### 19. Trait resolution 补 Evaluation 阶段

**v1.0 问题**：03 文档 §5.2 仅两阶段（Selection + Fulfillment）。

**v1.1 修正**：补全为三阶段（参考 rustc next-gen solver）：

- **Evaluation**: 评估 impl 是否适用（不真正 commit）
- **Selection**: 从候选 impl 中选最 specific
- **Fulfillment**: 把 impl 的 where clause 加入 constraint 队列

### 20. Canonical query 机制

**v1.1 修正**：在 03 文档 §5.8 新增 "Canonical query"：把 trait 求解参数 canonical 化（inference variable 替换为 placeholder），结果可缓存。这是 rustc 性能关键。

### 21. Two-phase borrows 矛盾修复

**v1.0 问题**：04 文档说 MVP 不做 two-phase borrows，但 method-call auto-ref 默认就是 two-phase，禁了则 `vec.push(vec.len())` 编译失败。

**v1.1 修正**：MVP **必须支持** two-phase borrows 的 method-call 子集：

- 仅 method call 自动借用支持 two-phase
- 显式 `&mut expr` 不支持 two-phase
- 算法：reservation point（参数求值前）+ activation point（调用时）

### 22. Disjoint closure captures（RFC 2229）

**v1.0 问题**：未提及 RFC 2229，stage 1 自举时闭包代码会 borrow checker 误报。

**v1.1 修正**：在 04 文档 §6 新增 disjoint closure captures 说明，MVP 实现 RFC 2229 子集。

### 23. RegionInferenceContext 数据结构补全

**v1.0 问题**：缺 SCC 压缩、type_tests、universe_causes。

**v1.1 修正**：在 04 文档 §4.6 完整列出 RegionInferenceContext 字段。

### 24. Codegen OperandValue 4 形态

**v1.0 问题**：07 文档未列 OperandValue。

**v1.1 修正**：在 07 文档 §4 补全：

- `Ref(llvm::PointerValue)` — 通过指针访问
- `Immediate(llvm::BasicValueEnum)` — 直接值
- `Pair(llvm::BasicValueEnum, llvm::BasicValueEnum)` — fat pointer（`&str`/`&[T]`/`&dyn Trait`）
- `ZeroSized` — ZST

### 25. Codegen FunctionCx + Builder 模式

**v1.1 修正**：在 07 文档 §4.1 补全 FunctionCx 与 Builder 的实际调用模式。

### 26. Name resolution 8-pass 机制

**v1.0 问题**：05 文档说"两轮"。

**v1.1 修正**：在 01 文档 §6.2 改为多 pass 描述（仍简化，但承认复杂性）。

---

## 四、文献引用修正（R7 报告）

### 27. 03 文档 §4.5 算法描述自相矛盾

**v1.0 问题**：声称用 constraint-based (Odersky-Wadler-Wehr 1995)，实际伪代码是 Algorithm W 的 Robinson unification。

**v1.1 修正**：重写 §4.5 unify 算法为真正的 constraint-based：constraint 不立即求解，加入 constraint queue，最后批量求解。

### 28. 04 文档 §1.4 错误引用 Harper PFPL §32

**v1.0 问题**：Harper §32 是 GC，不是 drop order。

**v1.1 修正**：删除 Harper 引用，drop order 引用 rustc-dev-guide "MIR" 章节的 drop elaboration 部分。

### 29. 04 文档 NLL 算法引用错误

**v1.0 问题**：引 Jung 2017，但 Jung 是 soundness proof，RFC 2094 才是算法。

**v1.1 修正**：算法引用 RFC 2094，soundness 论证引用 Jung 2017。

### 30. 补充关键文献

**v1.1 修正**：12 文档 §8 必读书目补充：

- Braun et al. 2013 "Simple and Efficient Construction of SSA Form"
- Appel 1998 "SSA is Functional Programming"
- Pierce & Turner 2000 "Local Type Inference"
- Matsakis RFC 2094 NLL
- Maranget 2007 "Compiling pattern matching"（match exhaustive 算法）
- matklad 2020 "Simple but Powerful Pratt Parsing"

---

## 五、内部矛盾修复（R9 报告）

### 31. 错误代码分配冲突

**v1.0 问题**：E05xx 同时归 type system 与 borrow check。

**v1.1 修正**：统一为：

- E0001-E0499: type system
- E0500-E0699: borrow check
- E0700-E0899: lifetime
- E0900-E0999: name resolution
- E1000-E1099: parse
- E1100-E1299: trait resolution
- E1300-E1399: codegen

### 32. 默认类型参数矛盾

**v1.0 问题**：03 标 v0.2，但 09 大量使用。

**v1.1 修正**：MVP 支持默认类型参数（仅 trait def 的 `Rhs = Self` 形式）。

### 33. `?` on Option 矛盾

**v1.0 问题**：01 说 MVP 无，但 09 Iterator::nth 用了。

**v1.1 修正**：MVP 完全不支持 `?` on Option，09 中的用法改为 `match`。

### 34. ABI 数量不一致

**v1.0 问题**：05 enum 3 variant，01/07 说 2 个。

**v1.1 修正**：统一为 3 个（Forge / C / System），其中 System 在 MVP 中等同于 C。

### 35. 时间线/行数估算统一

**v1.0 问题**：08/12/04/10 四份文档互斥。

**v1.1 修正**：以 12-roadmap.md 为单一来源，重写 08/04/10。

### 36. stdlib API 完整性

**v1.1 修正**：在 09 文档补全以下被引用但未定义的类型与 trait：

- `core::marker::Tuple` — 闭包 Args 用
- `core::ops::Try` — `?` 操作符用
- `core::str::FromStr` — parse 用
- `core::alloc::Layout` — allocator 用
- `core::alloc::AllocError` — allocator 用
- `core::cell::UnsafeCell` — Cell 内部用
- `core::mem::MaybeUninit` — v0.2 但 stdlib 引用
- `core::ptr::drop_in_place` — Drop glue 用
- `core::slice::from_raw_parts` — Vec/slice 用
- `std::ffi::CString` — FFI 用
- `libc` 模块 — std::io 用

### 37. 02 文法补全

**v1.1 修正**：02 文档补全以下产生式：

- `if let` / `while let`
- `self_param`（`self` / `&self` / `&mut self` / `self: Type`）
- `crate::` 路径前缀
- 元组字段访问 `expr.0` / `expr.1`

### 38. AtomicI32 错误使用

**v1.0 问题**：01 §5.2 MVP 示例用 `AtomicI32`，但属 v0.2。

**v1.1 修正**：01 文档改用 `static mut STATE: i32` + unsafe 示例。

### 39. Display trait 签名不一致

**v1.0 问题**：01/03 与 09 不一致。

**v1.1 修正**：统一为 `fn fmt(&self, f: &mut Formatter) -> Result<(), Error>`。

### 40. str::is_ascii 自递归 bug

**v1.0 问题**：09 §2.5 `pub fn is_ascii(self) -> bool { self.is_ascii() }` 无限递归。

**v1.1 修正**：改为 `pub fn is_ascii(self) -> bool { (self as u32) < 128 }`。

---

## 六、新增文档

### 41. 新增 13-stage1-feature-whitelist.md

**目的**：明示 stage 1 源码允许使用的特性子集，避免 stage 0 反复补特性。

**内容**：

- Stage 1 允许的全部语言特性（约 50 项）
- 每个特性的 stage 0 实现路径
- Stage 1 禁止使用的特性（即使 stage 0 支持）
- Stage 1 标准库依赖清单

### 42. 新增 14-soundness-considerations.md

**目的**：集中论证 Forge 类型系统的 soundness。

**内容**：

- Progress + preservation 定理声明（不带证明）
- 已知 soundness 风险与缓解
- 与 Rust 已知 soundness hole 的对比
- `unsafe` 边界规范
- 未定义行为清单

### 43. 新增 CHANGELOG.md（本文档）

记录所有 v1.0 → v1.1 变更。

### 44. 更新 README.md

更新文档集导航，反映 v1.1 14 个文档结构。

---

## 七、工作量与时间线重新估算（R8 报告）

### 45. Stage 0 工作量修正

**v1.0 估算**：53,000 行 Rust

**v1.1 修正**：130,000-180,000 行 Rust

| 组件 | v1.0 | v1.1 | 依据 |
| --- | --- | --- | --- |
| Lexer | 1,500 | 3,000-4,000 | rustc lexer 3,500 行 |
| Parser | 4,000 | 12,000-18,000 | rustc parser 25,000 行 |
| AST + HIR + Lowering | 5,000 | 15,000-25,000 | rustc hir+ast 30,000 行 |
| Name resolution | 2,500 | 8,000-12,000 | rustc resolve 15,000 行 |
| Type checker | 6,000 | 20,000-35,000 | rustc typeck 50,000 行 |
| Trait resolution | 3,000 | 15,000-25,000 | rustc traits 40,000 行 |
| MIR building | 4,000 | 10,000-15,000 | rustc mir_build 12,000 行 |
| Borrow checker | 4,000 | 12,000-18,000 | rustc borrowck+region 28,000 行 |
| MIR optimization | 2,000 | 6,000-10,000 | rustc mir_opts 10,000 行 |
| LLVM codegen | 4,500 | 20,000-30,000 | rustc codegen-llvm+ssa 45,000 行 |
| Monomorphization | 2,000 | 4,000-6,000 | rustc monomorphize 5,000 行 |
| Errors + diagnostics | 2,500 | 8,000-15,000 | rustc diagnostics 20,000+ 行 |
| 标准库 core+alloc | 8,000 (Forge) | 25,000-40,000 (Forge) | Rust core+alloc 子集 30,000 行 |
| mini-cargo | 2,500 (Rust) | 6,000-9,000 (Rust) | cargo 核心 30,000 行 |
| Test runner | 1,500 (Forge) | 4,000-6,000 (Forge) | libtest 10,000 行 |
| 内建宏展开器 | - | 3,000-5,000 | v1.1 新增 |
| **合计** | **~53,000** | **~130,000-180,000**（v1.2.3 修正：与 08 §1.3/§3.3 一致） | **2.5-3.5x** |

### 46. 时间线修正

**v1.0 估算**：15 月完成自举

**v1.1 修正**：

| 阶段 | v1.0 | v1.1（乐观） | v1.1（现实） |
| --- | --- | --- | --- |
| Stage 0 开发 | 9 月 | 18-24 月 | 24-36 月 |
| Stage 0 conformance 通过 | 1 月 | 2-3 月 | 3-4 月 |
| Stage 1 重写 | 2-3 月 | 8-12 月 | 12-18 月 |
| 自举验证 + 发布 | 2-3 月 | 3-4 月 | 4-6 月 |
| **v0.1（仅 stage 0）** | - | **20-27 月** | **27-40 月** |
| **v0.3（自举完成）** | **15 月** | **31-43 月** | **43-64 月** |

### 47. Conformance 套件扩展

**v1.0 估算**：950 测试

**v1.1 修正**：3,000-5,000 测试

- Parse: 200 → 600
- Type check: 300 → 1,000
- Borrow check: 200 → 800
- Codegen: 150 → 600
- E2E: 100 → 500
- Soundness: 0 → 500（新增，专测 R5 找出的反例）

---

## 八、风险登记更新

### 48. 新增高优先级风险

| 风险 | v1.0 等级 | v1.1 等级 | 缓解 |
| --- | --- | --- | --- |
| 15 月自举不可达 | 中 | **极高** | 自举推迟到 v0.3 |
| NLL 算法 soundness 漏洞 | 中 | **高** | 补全 universal region + type tests |
| Stage 0 frozen blob 不稳定 | 低 | **高** | 改用预编译二进制 |
| 内部宏系统矛盾 | 中 | **高** | 明确内建宏清单 |
| `?Sized` 矛盾 | 低 | **高** | 部分支持 unsized |
| rustc 实现细节遗漏 | 低 | **中** | 已补 25 项 |

### 49. 应急降级方案

R8 报告建议的 6 级降级方案正式纳入 12-roadmap.md §7.4：

- 降级 1：放弃 NLL，回退 lexical lifetime（节省 3-4 月）
- 降级 2：放弃 stage 1 重写，仅发布 stage 0（参考 Hare）
- 降级 3：放弃 frozen blob，改预编译二进制（v1.1 已默认采用）
- 降级 4：放弃 trait object，仅静态分发（节省 1-2 月）
- 降级 5：放弃 mini-cargo，仅单文件编译（节省 3-4 月）
- 降级 6：放弃 Forge 重写，永久保留 Rust stage 0（参考 Roc）

---

## 九、不修正的部分

以下 v1.0 决策经审查后**确认正确**，不做修改：

1. **MIR-first 设计**（R1/R2/R3 共识）
2. **禁 shadow / 禁嵌套 item**（Austral 实践 + R1 教训）
3. **monomorphization only**（R3 强制）
4. **禁 overlapping impls/specialization**（R3 陷阱）
5. **禁 GATs/async fn in trait/const generics**（v0.2+）
6. **禁 Polonius**（复杂度未达收益）
7. **panic = abort**（MVP 简化）
8. **禁 let-generalization**（R3 陷阱）
9. **函数签名强制显式**（R3 推荐）
10. **禁 proc macro**（永久不做，v0.2 仅 macro_rules!）
11. **Box/Vec/String 不特判**（R1 教训）
12. **constraint-based inference 方向**（R3 推荐，仅修正伪代码）
13. **LLVM only 后端**（R2/R3 共识）
14. **Rust 作 stage 0 宿主语言**（生态成熟）

---

## 十、总结

v1.1 共修正 **49 项**，主要变化：

| 类别 | 修正数 |
| --- | --- |
| 重大策略调整 | 4 |
| Soundness 漏洞修复 | 7 |
| MIR 完备性修复 | 13 |
| 文献引用修正 | 4 |
| 内部矛盾修复 | 10 |
| 新增文档 | 4 |
| 工作量/时间线修正 | 3 |
| 风险登记更新 | 2 |
| 确认不修正 | 14（明确列出） |

**核心变化**：

1. 自举从 v0.1（15 月）推迟到 v0.3（31-64 月）
2. Stage 0 frozen blob 改用预编译二进制
3. MVP 加入内建宏集（不开放自定义）
4. MVP 部分支持 unsized 类型（str/[T]/dyn Trait）
5. NLL 算法补全 universal region + type tests
6. MIR variant 大量补全（25 项）
7. Trait resolution 三阶段 + canonical query
8. Two-phase borrows 必须支持（method-call 子集）
9. Stage 1 特性白皮书明示
10. Soundness 论证文档独立

v1.1 文档集为 v1.0 的严格修正版，可作为 stage 0 实现的可靠基础。
