# 18 — 术语表

> 本文提供 Landin 蓝图使用的所有术语的统一定义。v1.2 新增（R9/R12 完备性审查建议）。所有文档应使用本表中的术语，避免歧义。

---

## A. 编译器架构术语

### AdtDef

**中文**：代数数据类型定义
**定义**：struct 或 enum 的定义。包含字段信息、variant 信息、generic params。
**位置**：03-type-system §1, 05-ast §10

### AST（Abstract Syntax Tree）

**中文**：抽象语法树
**定义**：Parser 输出的树结构，仅承载语法信息，不做名称解析或类型检查。
**与 HIR 关系**：AST → HIR lowering → HIR
**位置**：05-ast 全文

### Body / BodyId

**中文**：函数体 / 函数体 ID
**定义**：HIR 中函数体与 item 分离存储的机制。BodyId 是函数体的唯一标识，Item 只存函数签名。
**位置**：05-ast §12

### BasicBlock

**中文**：基本块
**定义**：MIR 中的控制流单元，包含一系列 Statement 和一个 Terminator。
**位置**：06-mir §2

### Canonical Query

**中文**：规范化查询
**定义**：trait 求解时把 inference variable 替换为 placeholder，结果可缓存的机制。
**位置**：03-type-system §5.8

### Constraint-based inference

**中文**：基于约束的推导
**定义**：类型推导时不立即求解 unification，而是把 constraint 加入队列批量求解。
**对比**：Algorithm W（Robinson unification）立即递归求解
**位置**：03-type-system §4.5

### Conformance Suite

**中文**：符合性测试套件
**定义**：Landin 的完整测试集合，stage 0/1 必须通过才能进入下一阶段。
**位置**：17-conformance-suite 全文

### Crate

**中文**：crate（不翻译）
**定义**：Landin 的编译单元。一个 crate 是一个 .lin 项目，编译为一个二进制或库。
**类型**：bin / lib / rlib
**位置**：01-language-specification §1.1

### DefId

**中文**：定义 ID
**定义**：每个 item 的全局唯一标识，由 (crate_id, local_id) 组成。用于跨 crate 引用。
**位置**：03-type-system, 05-ast

### HirId

**中文**：HIR 节点 ID
**定义**：HIR 中每个节点的全局唯一标识，由 (OwnerId, ItemLocalId) 组成。用于挂载 typeck 结果、borrow check 结果。
**位置**：05-ast §12

### HIR（High-level IR）

**中文**：高级中间表示
**定义**：AST lowering 后的中间表示，做了 name resolution、lifetime elision、desugaring 等变换。
**与 AST 共享比例**：约 50%
**位置**：05-ast §12

### ItemLocalId

**中文**：owner 内本地 ID
**定义**：在 OwnerNodes 内的本地节点 ID，与 OwnerId 组合成 HirId。
**位置**：05-ast §12

### Local（MIR）

**中文**：MIR 局部变量
**定义**：MIR 中的局部变量，用 Local（u32）索引。包括函数参数与函数体内声明的变量。
**位置**：06-mir §2

### MIR（Mid-level IR）

**中文**：中级中间表示
**定义**：Landin 编译器的灵魂 IR。CFG-based，三地址码，承载 borrow check、liveness、init 分析。
**位置**：06-mir 全文

### Operand

**中文**：操作数
**定义**：MIR Rvalue 中的操作数，三种：Copy(Place) / Move(Place) / Constant。
**位置**：06-mir §6

### OwnerId / OwnerNodes

**中文**：owner ID / owner 节点集合
**定义**：HIR 中每个 item 是一个 owner，OwnerNodes 包含 owner 本身与 owner 下所有节点。
**位置**：05-ast §12

### Place

**中文**：内存位置（不翻译 place，部分文档用"左值"但不推荐）
**定义**：MIR 中"可被取地址、可被赋值"的表达式。包括 Local、字段访问、解引用、索引。
**对比 Rvalue**：Rvalue 产生值，无固定内存位置
**位置**：06-mir §4, 01-language-specification §2.1

### PointIndex

**中文**：MIR 点索引
**定义**：MIR 中 (BasicBlock, Statement index) 的组合，标识一个程序点。用于 region inference。
**位置**：04-ownership-borrowing §4

### Pratt parser

**中文**：Pratt 解析器
**定义**：top-down operator precedence 解析器，专治表达式优先级与结合性。
**位置**：02-grammar §2

### Rvalue

**中文**：右值（不翻译 rvalue）
**定义**：MIR 中"产生值"的表达式，无固定内存位置。包括 Use、Ref、BinOp、Aggregate 等。
**位置**：06-mir §5, 01-language-specification §2.2

### SCC（Strongly Connected Component）

**中文**：强连通分量
**定义**：region constraint graph 中互相可达的 region 集合，可压缩为一个节点加速求解。
**位置**：04-ownership-borrowing §4.6.5

### Span

**中文**：源码位置范围
**定义**：源码中的字节范围 (lo, hi, file_id)。用于错误信息定位。
**位置**：05-ast §2, 16-diagnostics

### Statement（MIR）

**中文**：MIR 语句
**定义**：BasicBlock 内的指令，如 Assign、StorageLive、StorageDead。不改变控制流。
**位置**：06-mir §3

### Terminator（MIR）

**中文**：MIR 终结符
**定义**：BasicBlock 的最后一条指令，决定控制流转移。如 Goto、SwitchInt、Call、Return。
**位置**：06-mir §7

### Type Test

**中文**：类型测试
**定义**：NLL 中验证 `T: 'a` 约束在借用点的检查。
**位置**：04-ownership-borrowing §4.6.4

### Universe

**中文**：universe（不翻译）
**定义**：HRTB `for<'a>` 创建的新 placeholder region 集，避免变量捕获。
**位置**：04-ownership-borrowing §4.6.3

---

## B. 类型系统术语

### Associated Type

**中文**：关联类型
**定义**：trait 中 `type Item;` 声明的类型，impl 时指定具体类型。
**位置**：03-type-system §2.1

### Auto Trait

**中文**：自动 trait
**定义**：编译器自动 impl 的 marker trait，如 Send/Sync（v0.2）。
**位置**：03-type-system §2.5

### Bound（类型 bound）

**中文**：类型约束
**定义**：对泛型参数的约束，如 `T: Clone`。
**位置**：03-type-system §2.2

### Canonical Form

**中文**：规范形式
**定义**：把 inference variable 替换为 placeholder 后的形式，用于缓存 trait 求解结果。
**位置**：03-type-system §5.8

### Coherence

**中文**：一致性
**定义**：保证同一 (Trait, Type) 对在全局最多一个 impl。
**位置**：03-type-system §5.7

### Copy Type

**中文**：复制类型
**定义**：赋值时按位复制的类型。由 Copy trait 标记。
**位置**：01-language-specification §2.3

### Derive

**中文**：派生（不翻译 derive）
**定义**：`#[derive(Trait)]` 属性，编译器自动生成 trait impl。
**位置**：15-attributes §4

### Discriminant

**中文**：判别值
**定义**：enum variant 的 tag 值，用于运行时区分 variant。
**位置**：07-codegen §2.3

### Dyn Trait

**中文**：动态分发 trait 对象
**定义**：`dyn Trait` 类型的值，运行时通过 vtable 分发方法。
**位置**：03-type-system §2.3

### Evaluation（trait）

**中文**：trait 求值
**定义**：trait resolution 第一阶段，评估候选 impl 是否适用。
**位置**：03-type-system §5.2

### Fulfillment

**中文**：约束履行
**定义**：trait resolution 第三阶段，把 impl 的 where clause 加入 obligation queue 递归求解。
**位置**：03-type-system §5.4

### Generic Args

**中文**：泛型参数
**定义**：泛型类型/函数的参数，包括 type arg、lifetime arg、const arg（v0.2）。
**位置**：03-type-system §3

### HRTB（Higher-Rank Trait Bound）

**中文**：高阶 trait 约束
**定义**：`for<'a> fn(&'a T)` 形式的约束，对所有 lifetime 成立。
**位置**：04-ownership-borrowing §7.3

### Implied Bounds

**中文**：隐含约束
**定义**：`&'a T` 隐含 `T: 'a` 的约束，由编译器自动推导。
**位置**：04-ownership-borrowing §4.6.2

### Inference Variable

**中文**：推导变量
**定义**：类型推导中的未知类型，用 `?N` 表示，最终解为具体类型。
**位置**：03-type-system §4.2

### Monomorphization

**中文**：单态化
**定义**：为每个泛型实例生成专门代码的机制。
**位置**：03-type-system §3.2

### Niche Optimization

**中文**：niche 优化
**定义**：利用类型的无效值编码 enum variant，减小内存占用。如 `Option<NonNull<T>>` 用 null 编码 None。
**位置**：07-codegen §2.4

### Normalization

**中文**：归一化
**定义**：把 associated type projection（如 `T::Item`）替换为具体类型的过程。
**位置**：03-type-system §7

### Object Safety

**中文**：对象安全
**定义**：trait 可作 `dyn Trait` 的条件。
**位置**：03-type-system §2.3

### Obligation

**中文**：义务（trait obligation）
**定义**：trait resolution 中待求解的约束，如 `T: Display`。
**位置**：03-type-system §5.4

### Orphan Rule

**中文**：孤儿规则
**定义**：`impl Trait for Type` 必须在定义 Trait 或 Type 的 crate 中。
**位置**：03-type-system §5.6

### Placeholder Region

**中文**：占位 region
**定义**：canonical query 中代替 inference variable 的 region，避免变量捕获。
**位置**：04-ownership-borrowing §4.6.1

### Projection Type

**中文**：投影类型
**定义**：`<T as Trait>::Item` 形式的类型，需 normalization 才能确定具体类型。
**位置**：03-type-system §7

### Selection（trait）

**中文**：trait 选择
**定义**：trait resolution 第二阶段，从候选 impl 中选最 specific 的。
**位置**：03-type-system §5.3

### Sized

**中文**：大小已知
**定义**：编译期已知大小的类型。所有泛型参数默认 `T: Sized`。
**位置**：03-type-system §1.2

### Unsized

**中文**：大小未知
**定义**：编译期未知大小的类型，如 `str`、`[T]`、`dyn Trait`。只能通过 `&T` 引用。
**位置**：03-type-system §1.2

### Universal Region

**中文**：全称 region
**定义**：函数签名中的 `'a`、`'b`、`'static`，对所有调用方成立的 region。
**位置**：04-ownership-borrowing §4.6.1

### Variance

**中文**：变型
**定义**：类型构造器对子类型关系的保持方式：协变 / 逆变 / 不变。
**位置**：03-type-system §8

---

## C. 所有权与借用术语

### Borrow

**中文**：借用
**定义**：通过 `&` 或 `&mut` 获取值的引用，不转移所有权。
**位置**：04-ownership-borrowing §2

### Borrow Check

**中文**：借用检查
**定义**：编译期验证借用规则的 pass，在 MIR 上做 dataflow 分析。
**位置**：04-ownership-borrowing 全文

### Drop

**中文**：析构（不翻译 drop）
**定义**：值离开作用域时调用 Drop trait 的 drop 方法。
**位置**：03-type-system §6.3

### Drop Check

**中文**：drop 检查
**定义**：验证 Drop impl 不会访问已 drop 的引用数据的机制。
**位置**：04-ownership-borrowing §5

### Drop Glue

**中文**：drop glue
**定义**：编译器生成的析构函数，调用 user Drop impl 后递归 drop 字段。
**位置**：07-codegen §6

### Drop Elaboration

**中文**：drop 展开
**定义**：在 MIR 中插入 Drop terminator 的 pass。
**位置**：06-mir §8.2

### Lifetime

**中文**：生命周期
**定义**：引用的有效范围。用 `'a`、`'b`、`'static` 表示。
**位置**：04-ownership-borrowing §3

### Lifetime Elision

**中文**：lifetime 省略
**定义**：函数签名中省略 lifetime 标注的规则，编译器自动补全。
**位置**：04-ownership-borrowing §3.2

### Liveness Analysis

**中文**：活跃性分析
**定义**：dataflow 分析，确定变量在哪些点被使用。用于 NLL。
**位置**：04-ownership-borrowing §4.3

### Maybe-initialized

**中文**：可能已初始化
**定义**：dataflow 分析，确定 place 在每个点是否可能已初始化。
**位置**：04-ownership-borrowing §4.4

### Move

**中文**：移动
**定义**：赋值时转移所有权，原变量不可再用。Non-Copy 类型默认 move。
**位置**：01-language-specification §2.3

### NLL（Non-Lexical Lifetimes）

**中文**：非词法生命周期
**定义**：借用结束点不是词法作用域结束，而是借用最后一次使用的点。
**位置**：04-ownership-borrowing §2.3

### Ownership

**中文**：所有权
**定义**：每个值有唯一 owner，owner 离开作用域时 drop。
**位置**：04-ownership-borrowing §1

### Partial Move

**中文**：部分移动
**定义**：struct/enum 字段单独 move，原变量整体不可用但未 move 字段仍可访问。
**位置**：04-ownership-borrowing §1.3

### Region

**中文**：region（不翻译，等价于 lifetime）
**定义**：NLL 算法中的 lifetime 变量，是 CFG 上点的集合。
**位置**：04-ownership-borrowing §4

### Region Inference

**中文**：region 推导
**定义**：求解 region constraint 系统，把每个 region 解为 CFG 点集。
**位置**：04-ownership-borrowing §4.6

### Two-phase Borrow

**中文**：两阶段借用
**定义**：method call 的 `&mut` 借用分 reservation 与 activation 两阶段，允许 `vec.push(vec.len())`。
**位置**：04-ownership-borrowing §2.4

### Disjoint Closure Captures

**中文**：闭包不相交捕获
**定义**：RFC 2229，闭包只捕获访问的字段而非整个 struct。
**位置**：04-ownership-borrowing §8

---

## D. 编译流程术语

### Bootstrapping

**中文**：自举
**定义**：用语言自身实现自己的编译器。
**位置**：08-bootstrap-strategy 全文

### Conformance

**中文**：符合性
**定义**：编译器实现与语言规范的一致性。
**位置**：17-conformance-suite 全文

### Crate Type

**中文**：crate 类型
**定义**：bin / lib / rlib 三种。
**位置**：01-language-specification §1.1

### Edition

**中文**：edition（不翻译）
**定义**：Landin 的版本集合，不同 edition 允许破坏性变更。v0.2 引入。
**位置**：10-toolchain §3.2

### Feature Gate

**中文**：特性门控
**定义**：unstable 特性需 `#![feature(name)]` 才能使用。
**位置**：08-bootstrap-strategy §6.3

### Frozen Blob

**中文**：冻结 blob
**定义**：stage 0 编译器的不可变分发形式（预编译二进制）。
**位置**：08-bootstrap-strategy §2.2

### ICE（Internal Compiler Error）

**中文**：编译器内部错误
**定义**：编译器自身 bug 导致的 panic，退出码 101。
**位置**：16-diagnostics §5

### Lints

**中文**：lints（不翻译）
**定义**：代码风格与潜在 bug 的静态检查警告。
**位置**：16-diagnostics §2.2

### Nightly / Beta / Stable

**中文**：nightly / beta / stable（不翻译）
**定义**：Landin 的三个发布通道。MVP 仅 nightly。
**位置**：08-bootstrap-strategy §6.2

### Pass

**中文**：编译 pass
**定义**：编译器的一个处理阶段，如 lexer、parser、typeck、borrowck。
**位置**：12-roadmap

### Prelude

**中文**：prelude（不翻译）
**定义**：默认导入到所有 crate 的标准库符号集合。
**位置**：09-stdlib §2.2

### RFC（Request for Comments）

**中文**：RFC（不翻译）
**定义**：Landin 设计变更的提案流程。
**位置**：00-overview §7

### Stage 0/1/2

**中文**：stage 0/1/2（不翻译）
**定义**：自举三阶段。Stage 0 = Rust 实现的 Landin 编译器，Stage 1 = Landin 自身重写，Stage 2 = Stage 1 自编译产物。
**位置**：08-bootstrap-strategy §2

### Stage 0 Frozen

**中文**：stage 0 冻结
**定义**：stage 0 写完即不再演进，仅修 critical bug。
**位置**：08-bootstrap-strategy §2.4

### Sysroot

**中文**：sysroot（不翻译）
**定义**：Landin 工具链的根目录，含 stdlib 与 runtime。
**位置**：10-toolchain §2.3

### Token Tree

**中文**：token 树
**定义**：`(...)` / `{...}` / `[...]` 包围的 token 序列，用于宏参数。
**位置**：02-grammar §5.3

---

## E. ABI 与 Codegen 术语

### ABI（Application Binary Interface）

**中文**：应用二进制接口
**定义**：函数调用约定的规范。Landin MVP 支持 "Landin"/"C"/"System" 三种。
**位置**：07-codegen §3.1

### Calling Convention

**中文**：调用约定
**定义**：函数参数传递、返回值传递、寄存器使用的规范。
**位置**：07-codegen §3.2

### Drop Glue Function

**中文**：drop glue 函数
**定义**：编译器为每个需要 drop 的类型生成的析构函数。
**位置**：07-codegen §6.1

### Fat Pointer

**中文**：胖指针
**定义**：包含 data pointer 与 metadata 的指针。如 `&str`、`&[T]`、`&dyn Trait`。
**位置**：07-codegen §4.6

### Layout

**中文**：类型布局
**定义**：类型在内存中的大小、对齐、字段偏移。
**位置**：07-codegen §2.3

### Mangling

**中文**：name mangling
**定义**：泛型实例化后函数名的编码方案，用于 linker 区分。
**位置**：03-type-system §3.3

### OperandValue

**中文**：操作数值
**定义**：Codegen 时 operand 在 LLVM 层的 4 种形态：Ref / Immediate / Pair / ZeroSized。
**位置**：07-codegen §4.6

### Panic

**中文**：panic（不翻译）
**定义**：程序运行时不可恢复的错误。MVP 用 abort 实现。
**位置**：07-codegen §4.5

### Sret（Struct Return）

**中文**：结构体返回
**定义**：大 struct 返回值通过隐式第一个指针参数传递的 ABI 优化。
**位置**：07-codegen §3.1

### VTable

**中文**：虚表
**定义**：`dyn Trait` 的方法分发表，含 drop 函数、size、align、各方法指针。
**位置**：07-codegen §7

---

## F. 其他术语

### Allocator

**中文**：分配器
**定义**：管理内存分配/释放的 trait。MVP 用 libc malloc/free。
**位置**：07-codegen §5

### Arena Allocation

**中文**：arena 分配
**定义**：一批对象共享同一 allocator，统一释放。用于编译器内部 AST/HIR/MIR 存储。
**位置**：05-ast §1

### Builtin Macro

**中文**：内建宏
**定义**：编译器硬编码展开的宏，如 `println!`、`vec!`。MVP 26 个。
**位置**：13-stage1-feature-whitelist §2.6

### Closure

**中文**：闭包
**定义**：捕获环境的匿名函数。分 Fn / FnMut / FnOnce 三种。
**位置**：01-language-specification §3.4

### Constant Evaluation

**中文**：常量求值
**定义**：编译期求值 const 表达式。MVP 简化版。
**位置**：01-language-specification §5.2

### Macro

**中文**：宏
**定义**：代码生成机制。MVP 仅内建宏（26 个，含 matches!），v0.2 加 `macro_rules!`。proc macro 永久不做（参考 12 §5.3）。
**位置**：02-grammar §4.4

### Pattern Matching

**中文**：模式匹配
**定义**：通过 match 表达式对值进行结构化解构与分支。
**位置**：01-language-specification §3.3, 02-grammar §3.5

### Soundness

**中文**：健全性
**定义**：类型系统保证 well-typed 程序不会产生类型错误（progress + preservation）。
**位置**：14-soundness 全文

### Trait

**中文**：trait（不翻译）
**定义**：Landin 的接口抽象机制，类似 Haskell type class。
**位置**：03-type-system §2

### Unsafe

**中文**：unsafe（不翻译）
**定义**：Landin 中关闭部分安全检查的代码块/函数。作者需手动维护不变量。
**位置**：14-soundness §5

---

**Landin 蓝图术语表 — 完**
