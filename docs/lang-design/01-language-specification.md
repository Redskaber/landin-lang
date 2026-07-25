# 01 — 语言规范

> 本文定义 Landin 语言的**语义规范**：程序结构、值类别、表达式语义、语句语义、可见性、名称解析。语法细节见 `02-grammar.md`，类型系统见 `03-type-system.md`，所有权见 `04-ownership-borrowing.md`。

---

## 1. 程序结构

一个 Landin 程序由若干 **crate** 组成。每个 crate 是一个编译单元，包含一个 root module 与若干子 module。

### 1.1 Crate

```
crate := crate_item*
crate_item := module_decl | use_decl | item
item := fn_decl | const_decl | static_decl
      | struct_decl | enum_decl | trait_decl | impl_decl
      | type_decl | extern_decl
```

每个 crate 有一个 **crate root**（`landin.toml` 中指定，默认 `src/lib.lin` 或 `src/main.lin`）。crate 类型分为：

- **bin**: 可执行文件，crate root 必须有 `fn main()`
- **lib**: 库，对外暴露 pub items
- **rlib**: 内部库，仅供其他 crate 依赖

### 1.2 Module

Module 是名称空间与可见性边界。Module 树与文件系统树对应（参考 Rust）：

- `src/lib.lin` → crate root
- `src/network/tcp.lin` → `crate::network::tcp`
- `mod network { mod tcp { ... } }` 是 inline 等价形式

Module 内的 item 默认 **private**（仅 module 内可见），加 `pub` 修饰符后向上公开。可见性粒度：

- `pub` — 完全公开
- `pub(crate)` — crate 内可见
- `pub(super)` — 父 module 可见
- `pub(in path)` — 指定 path 内可见
- （无）— private

MVP 阶段只实现 `pub` 与 private 两种，`pub(crate)` 等推迟到 v0.2（避免过早复杂化）。

### 1.3 Use 路径

```landin
use std::io::println;
use std::collections::{HashMap, BTreeMap};
use std::prelude::*;       // glob import
use std::io as stdio;       // rename
```

Use 在 module 内有效，导入的名称受可见性约束。Glob import（`*`）遵循 Rust 1.0 之后的"shadow rule"：显式声明的本地 item 优先于 glob import。

---

## 2. 值类别

Landin 有三类值（与 Rust 一致）：

| 类别 | 英文 | 说明 | 示例 |
| --- | --- | --- | --- |
| **Place** | place expression | 表示一个内存位置，可被取地址、可被赋值 | `x`, `*p`, `a[0]`, `s.field` |
| **Rvalue** | value expression | 计算出一个值，无固定内存位置 | `1 + 2`, `f(x)`, `&x` |
| **Constant** | constant expression | 编译期可求值的值 | `1`, `"hello"`, `2 + 3` |

### 2.1 Place 表达式

Place 是"可被取地址或赋值"的表达式。Landin 的 place 包括：

- 局部变量：`x`
- 解引用：`*p`（p 类型为 `*T` / `&T` / `&mut T` / `Box<T>`）
- 字段访问：`s.field`、`(*s).field`
- 索引：`a[i]`（要求 `a: Index` trait）
- 元组字段：`t.0`、`t.1`

把 rvalue 当 place 用（如 `&(1 + 2)`）会触发临时变量插入，编译器隐式创建一个 unnamed local 存储 rvalue，然后取其地址。临时变量的生命周期遵循 **"until end of statement"** 规则（非 end of block，避免 NLL 之前的反直觉行为）。

### 2.2 Rvalue 表达式

Rvalue 是"产生值"的表达式，包括：

- 字面量：`1`, `true`, `'a'`, `"hello"`, `b"bytes"`
- 算术/逻辑：`a + b`, `a && b`, `!a`
- 比较：`a == b`, `a < b`
- 函数调用：`f(x, y)`
- 构造：`Struct { field: v }`, `Enum::Variant(x)`, `(a, b, c)`, `[a, b, c]`
- 借用：`&x`, `&mut x`
- 转换：`x as T`
- 闭包：`|x| x + 1`

### 2.3 Move 与 Copy

每个值有一个 **move semantics** 属性：

- **Copy type**: 赋值/传参时按位复制，原变量仍可用。Copy 由 trait 标记，编译器为以下类型自动 impl Copy：
  - 所有基本类型（`i32`, `u64`, `bool`, `char`, `f64`, ...）
  - 所有 `&T` 与 `&mut T`
  - 所有 `*const T` 与 `*mut T`
  - 所有字段都是 Copy 的 struct/enum（除非显式 `impl !Copy`，MVP 不支持）
  - 所有大小为 0 的类型（unit `()`, 空数组 `[T; 0]`）
- **Non-Copy type**: 赋值/传参时 move，原变量不可再用。包括 `String`, `Vec<T>`, `Box<T>`, `&mut T` 自身（虽然 `&T` 是 Copy，但 `&mut T` 不是）。

显式复制用 `clone()` 方法（要求 `T: Clone`）。

---

## 3. 表达式语义

### 3.1 字面量

| 字面量 | 类型 | 示例 |
| --- | --- | --- |
| 整数 | 由后缀决定，默认 `i32` | `42`, `0xff`, `0b1010`, `1_000_000`, `42i64`, `0u8` |
| 浮点 | 由后缀决定，默认 `f64` | `3.14`, `1.0f32`, `1e10` |
| 布尔 | `bool` | `true`, `false` |
| 字符 | `char` (Unicode scalar value) | `'a'`, `'\n'`, `'\u{1F600}'` |
| 字符串 | `&'static str` | `"hello"`, `r"raw"`, `r#"with "quotes"#` |
| 字节串 | `&'static [u8]` | `b"bytes"`, `br#"raw bytes"#` |
| 字节字符 | `u8` | `b'A'`, `b'\n'` |
| Unit | `()` | `()` |

**整数 fallback 规则**: 无后缀的整数字面量先尝试根据上下文推断类型；若上下文无法决定（如 `let x = 42;`），默认 `i32`（参考 Rust 1.0 之后的规则；Rust 早期曾尝试取消 fallback 但失败了 RFC #115/#212，Landin 直接保留 i32 fallback）。

### 3.2 运算符

| 类别 | 运算符 | trait | 备注 |
| --- | --- | --- | --- |
| 算术 | `+ - * / %` | `Add Sub Mul Div Rem` | 整数溢出在 debug 模式 panic，release 模式 wrapping |
| 位 | `& \| ^ << >>` | `BitAnd BitOr BitShl Shr` | |
| 一元 | `- ! *` | `Neg Not Deref` | |
| 比较 | `== != < > <= >=` | `PartialEq PartialOrd` | 完全序需 `Ord` trait |
| 逻辑短路 | `&& \|\|` | （内建，不可重载） | |
| 赋值 | `= += -= ...` | 对应运算 trait | `a += b` 等价 `a = a + b`，但 `a` 只求值一次 |
| 借用 | `& &mut` | （内建） | |
| 解引用 | `*` | `Deref MutDeref` | |
| 错误传播 | `?` | `Try` trait | MVP 仅对 `Result` impl |
| 类型转换 | `as` | （内建） | 仅数值类型与指针类型 |

### 3.3 控制流表达式

Landin 把所有控制流都做成 **表达式**（参考 Rust/ML），包括：

#### if 表达式

```landin
let x = if cond { 1 } else { 2 };
```

两个分支的类型必须一致（unify），否则报错。无 else 分支时，if 分支类型必须为 `()`。

#### match 表达式

```landin
match expr {
    Pattern1 => result1,
    Pattern2 if guard => result2,
    _ => default,
}
```

Match 必须穷尽（exhaustive）。MVP 支持的模式：

- 字面量模式：`1`, `true`, `'a'`
- 变量绑定：`x`（绑定整个值），`x @ Pattern`（绑定 + 子模式）
- 通配符：`_`
- 范围模式：`1..=10`, `1..10`
- 结构解构：`Point { x, y }`, `Point { x: px, .. }`
- enum 解构：`Option::Some(x)`, `Option::None`
- 元组解构：`(a, b, _)`
- 数组解构：`[a, b, *rest]`（rest 模式 v0.2）
- 或模式：`1 | 2 | 3`
- 引用模式：`&x`, `&mut x`

#### loop / while / for

```landin
let result: i32 = loop {
    if cond { break 42; }
};

while cond { body }

for item in iterator { body }
```

- `loop` 的 break 可带值，类型即 loop 表达式类型
- `while` 与 `for` 的类型为 `()`（break 不带值）
- `for` 要求 rhs 实现 `IntoIterator`，循环变量类型为 `Iterator::Item`

### 3.4 闭包

```landin
let add = |a, b| a + b;
let add_one = |x| x + 1;
let capture = |x| { let y = outer; x + y };
```

闭包类型由捕获方式决定，对应三个 trait：

- `Fn`: 不可变捕获（`&self`）
- `FnMut`: 可变捕获（`&mut self`）
- `FnOnce`: move 捕获（`self`）

编译器从闭包体推断最弱可能的 trait（Fn 优先，否则 FnMut，否则 FnOnce）。MVP 不支持 `move` 关键字（v0.2 加）。

### 3.5 函数调用

```landin
f(arg1, arg2);
method.receiver_arg();
Trait::method(receiver, arg);
<Type as Trait>::method(receiver, arg);
```

调用形式：

- 函数路径：`crate::module::function`
- 方法调用：`expr.method(args)` — 要求 expr 类型 impl 了带 `method` 的 trait
- UFCS（uniform function call syntax）: `Trait::method(receiver, args)` 或 `<Type as Trait>::method(receiver, args)`

参数求值顺序：**从左到右**（与 Rust 一致，与 C++ 的"未指定"不同）。

### 3.6 错误传播 `?`

```landin
fn read_config() -> Result<Config, Error> {
    let file = File::open("config.toml")?;  // Error: From<io::Error>
    let content = file.read_to_string()?;    // Error: From<io::Error>
    Ok(parse_config(&content))
}
```

`?` 操作符语义：

1. 若 `expr` 是 `Ok(v)`，整体求值为 `v`
2. 若 `expr` 是 `Err(e)`，返回 `Err(From::from(e))` 给调用者

`?` 仅在返回类型为 `Result<T, E>` 或 `Option<T>` 的函数中可用。MVP 仅支持 `Result`，`Option` 推迟（避免 `Try` trait 复杂化）。

---

## 4. 语句语义

### 4.1 let 语句

```landin
let x: i32 = 42;
let mut y = 0;
let (a, b) = (1, 2);       // 解构
let Point { x, y } = p;    // 解构
```

- 默认 **immutable**（不可重新赋值）
- `let mut` 声明可变绑定
- 类型注解可选，省略时由类型推导决定
- 解构要求 RHS 类型与模式匹配（unify）
- **不允许 shadow**（与 Austral 一致，与 Rust 不同；R1 教训：shadow 在大型代码中容易引发 bug，Austral 的禁 shadow 实践证明可行）

### 4.2 表达式语句

任何表达式后加分号即成为语句：

```landin
f(x, y);
x + 1;          // 警告：unused computation
```

注意：**块的最后一条表达式不带分号，作为块的值**。这与 Rust 一致。

### 4.3 控制流语句

```landin
if cond { ... } else { ... };
while cond { ... };
loop { ... };
for x in iter { ... };
match expr { ... };
return expr;
break [expr];
continue;
```

控制流语句类型为 `()`（除了 `return/break/continue` 类型为 `!`，never type）。

### 4.4 Item 语句

MVP **不支持** item statement（嵌套 fn/struct 声明）。所有 item 必须在 module 顶层。这与 Rust 不同（Rust 允许嵌套 item），但简化了 parser 与名称解析（Austral 也做此限制，R4 报告验证可行）。

---

## 5. 项声明

### 5.1 函数

```landin
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub extern "C" fn callback(ptr: *const u8, len: usize) -> i32 {
    // ...
}

pub fn generic<T: Clone + Ord>(x: T) -> T {
    x.clone()
}
```

- `pub` 可见性
- `extern "ABI"` 指定调用约定（MVP 仅 `"C"` 与默认 `"Landin"`）
- 函数签名中的所有类型参数与生命周期必须显式标注（不推导）
- `where` 子句可选：`fn f<T>() where T: Clone`
- 默认参数不支持
- 可变参数仅 `extern "C"` 函数支持

### 5.2 常量与静态

```landin
const MAX_SIZE: usize = 1024;
static mut COUNTER: i32 = 0;       // unsafe to access（v1.2.2 修正：MVP 无 AtomicI32，v0.2 加）
static mut STATE: i32 = 0;       // unsafe to access
```

- `const`: 编译期内联，无固定地址，每次使用重新求值
- `static`: 程序生命周期，固定地址，不可变
- `static mut`: 可变静态，访问需要 `unsafe`（数据竞争风险）
- const 表达式必须是 **常量表达式**（comptime evaluable）

### 5.3 Struct

```landin
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub struct Color(u8, u8, u8);          // tuple struct

pub struct Empty;                       // unit struct

#[repr(C)]
pub struct Layout {
    a: u32,
    b: u32,
}
```

- 默认 `#[repr(Rust)]`，布局未指定（LLVM 自由优化）
- `#[repr(C)]` 按 C ABI 布局
- 字段默认 private，加 `pub` 公开
- 不支持 struct 继承（参考 Rust RFC #341 教训，R1 报告）
- MVP 不支持 `#[repr(packed)]`（v0.2 加）

### 5.4 Enum

```landin
pub enum Shape {
    Circle(f64),                        // tuple variant
    Rectangle { w: f64, h: f64 },       // struct variant
    Point,                              // unit variant
}

pub enum Option<T> {
    Some(T),
    None,
}

pub enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

- Enum 是 sum type（tagged union）
- 每个 variant 可以是 unit / tuple / struct 形式
- MVP 不支持 `#[repr(C)]` enum（v0.2 加，用于 FFI）
- Discriminant 大小由编译器选（最小够用，通常 1 字节）

### 5.5 Trait

```landin
pub trait Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error>;
}

pub trait Iterator {
    type Item;                          // associated type
    fn next(&mut self) -> Option<Self::Item>;
    
    fn map<F, B>(self, f: F) -> Map<Self, F>
    where F: FnMut(Self::Item) -> B, Self: Sized
    {
        Map { iter: self, f }
    }
}

pub trait From<T> {
    fn from(value: T) -> Self;
}
```

- Trait 可包含：required method、provided method（默认实现）、associated type、associated const
- Trait 可继承其他 trait：`trait B: A { ... }` 要求 impl B 必须先 impl A
- MVP 不支持 GATs（`type Item<'a>`），推迟到 v0.2
- MVP 不支持 `async fn` in trait

### 5.6 Impl

```landin
impl Display for Point {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl<T: Clone> Clone for Vec<T> {
    fn clone(&self) -> Self {
        // ...
    }
}

impl Point {
    pub fn new(x: f64, y: f64) -> Point { Point { x, y } }
    pub fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
}
```

- Trait impl: `impl Trait for Type { ... }`
- Inherent impl: `impl Type { ... }`（与 trait 无关的方法）
- Trait impl 必须满足 **orphan rule**: `impl Trait for Type` 必须在定义 Trait 的 crate 或定义 Type 的 crate 中（不能在第三方 crate impl 第三方 trait 给第三方 type）
- Trait impl 必须满足 **coherence**: 同一 (Trait, Type) 对在全局最多一个 impl（无 specialization）
- Inherent impl 必须与 Type 定义在同一 crate

### 5.7 Type alias

```landin
pub type Score = i32;
pub type Map<K, V> = HashMap<K, V>;
```

类型别名是 **透明** 的（与原类型完全等价）。

### 5.8 Extern

```landin
extern "C" {
    fn printf(fmt: *const u8, ...) -> i32;
    static GLOBAL_VAR: i32;
}

#[link(name = "m")]
extern "C" {
    fn sqrt(x: f64) -> f64;
}
```

外部函数与变量声明，用于 FFI。`extern` 块内的函数无函数体，由链接器解析。

---

## 6. 名称解析

### 6.1 路径

路径分两类：

- **绝对路径**: `crate::a::b::c`、`::core::mem::size_of`
- **相对路径**: `a::b::c`（在当前 module 起步）、`self::a::b`、`super::a::b`

### 6.2 解析顺序

名称解析在 HIR lowering 阶段做（不在 parser 做），实际是 **多 pass** 过程（v1.2 修正，R6 报告指出 v1.0/v1.1 的"两轮"描述不准确，rustc 实际有 8+ pass）：

1. **Build reduced graph**：收集所有 module 内的 item 名称、use 导入，建立初始符号表
2. **Finalize imports**：解析所有 use 导入的目标
3. **Compute effective visibilities**：计算每个 item 的有效可见性
4. **Late resolve crate**：解析所有路径表达式、类型路径、模式路径
5. **Resolve main**：确定 crate root（main 函数或 lib root）
6. **Check unused imports**：警告未使用的 use
7. **Report errors**：报告所有 unresolved name
8. **Postprocess**：清理临时数据

MVP 阶段可简化为 4 pass（合并 1-3 / 4-5 / 6-7 / 8），但仍需多 pass 而非两轮。

未限定路径按以下优先级查找：

1. 局部变量（在函数体内）
2. 当前 module 的 item
3. use 导入的名称（显式优先于 glob）
4. glob import
5. 外部 prelude（`core` / `std` / `alloc` 的 prelude）
6. 失败：unresolved name

### 6.3 Preludes

- **`core::prelude`**: 默认导入到所有 crate（除非 `#![no_core]`），含 `Option`, `Result`, `Drop`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Iterator`, `IntoIterator`, `Add`, `Sub`, `Mul`, `Div`, `Neg`, `Not`, `BitAnd`, `BitOr`, `Fn`, `FnMut`, `FnOnce`, `Box`, `Vec`, `String`, `drop`, `println!`(v0.2), etc.
- **`std::prelude`**: 在 `std` 可用时额外导入 `Vec` 的扩展、`HashMap`、`File`、`println` 等
- **`alloc::prelude`**: 在 `alloc` 可用时额外导入 `Box`, `Rc`, `Arc`

MVP 阶段 prelude 固定，不支持 `#[prelude_import]` 自定义。

---

## 7. 可见性规则

| 来源 | 可访问范围 |
| --- | --- |
| `pub` | 任何能看到本 module 的地方 |
| (无) | 仅本 module |
| `pub(crate)` (v0.2) | 本 crate 内 |
| `pub(super)` (v0.2) | 父 module |
| `pub(in path)` (v0.2) | 指定路径内 |

可见性是 **module 级** 而非 type 级（与 Rust 一致；与 Java/C++ 的 class-level 不同）。

**继承规则**: 嵌套 type 的字段默认 private（仅 type 所在 module 可见）。`pub` 字段则公开。但访问嵌套 type 的 `pub` 字段需要先能看到 type 本身。

---

## 8. 不变量与未定义行为

### 8.1 健全性保证

Landin 在 safe 子集内保证以下不变量：

1. **无数据竞争**（MVP 单线程，自动满足；v0.2 通过 Send/Sync）
2. **无空指针解引用**（safe 代码中 `&T` 不为 null；`Box<T>` 不为 null）
3. **无 use-after-free**（所有权 + 借用规则保证）
4. **无未初始化内存读取**（liveness + maybe-init 分析在 MIR 上）
5. **无越界访问**（slice 索引在运行时检查；数组索引 `[T; N]` 在编译期检查）
6. **无整数溢出 UB**（debug panic，release wrapping；`checked_*` / `wrapping_*` / `saturating_*` 显式方法）

### 8.2 unsafe

```landin
unsafe fn raw_read(ptr: *const u8) -> u8 {
    *ptr
}

fn safe_wrapper() {
    let x = 42u8;
    let ptr = &x as *const u8;
    unsafe { raw_read(ptr) }   // unsafe block required
}
```

`unsafe` 用于：

- 调用 `unsafe fn`
- 解引用裸指针 `*ptr`
- 访问 `static mut`
- 调用 `extern` 函数
- 实现 `unsafe trait`（如 `Send`/`Sync` 在 v0.2）

`unsafe` **不关闭 borrow checker**，只关闭上述 5 类检查。

### 8.3 未定义行为

即使在 `unsafe` 中，以下行为仍是 UB（编译器可假设它们不发生）：

- 数据竞争（v0.2 才可能发生）
- 解引用未对齐指针
- 解引用悬垂指针
- 读取未初始化内存（包括 `MaybeUninit` 之外）
- 违反 `unsafe trait` 的契约（如错误 impl `Send`）
- 整数溢出（debug 模式 panic，release 模式为 wrapping，非 UB；与 C 不同）
- 错误的 lifetime 标注（理论上 borrow checker 会拒绝，绕过则 UB）

---

## 9. 与 Rust 的关键差异

| 维度 | Rust | Landin | 理由 |
| --- | --- | --- | --- |
| 变量 shadow | 允许 | **禁止** | Austral 实践 + R1 教训 |
| 嵌套 item | 允许 | **禁止** | 简化 parser + 名称解析 |
| macro_rules! | 有 | **MVP 无**（v0.2） | R1 教训：宏系统 reform 极痛苦 |
| async/await | 有 | **MVP 无** | 单线程 MVP |
| `?` on Option | 有 | **MVP 无** | 简化 Try trait |
| 整数 fallback | i32 | **i32** | 一致 |
| 默认 panic 策略 | unwind | **abort** | MVP 简化 |
| Const generics | 有 | **MVP 无** | 复杂度 |
| GATs | 有 | **MVP 无** | 复杂度 |
| Specialization | nightly | **永久不做** | R3 陷阱 #5 |
| `dyn` trait | 有 | **MVP 有**（必须，用于闭包/错误类型） | 必需 |
| `cargo` workspace | 有 | **MVP 无** | 简化 |
| `build.rs` | 有 | **MVP 无** | 简化 |

---

**下一文档**: [`02-grammar.md`](./02-grammar.md) — 完整 BNF/EBNF 文法

---

## 13. 实现状态（v0.14.0，§25.8 回写）

> 本节由 Stage 6.18 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。
> 仅记录"设计 + 理由"，实现细节归 `docs/develop/v0/stage-N/dev-log.md`。

### 13.1 §6 名称解析 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §6.2 pass 1 (build reduced graph) | ✅ 实现 | — | `resolve::module_build::build_module_tree` |
| §6.2 pass 2 (finalize imports) | ✅ 实现 | — | `resolve::module_build::resolve_uses` + `resolve_use_tree` + `resolve_use_leaf` + `resolve_use_glob` |
| §6.2 pass 3 (compute visibilities) | ✅ 实现 | B3（实现更宽松） | `check_visibility` 当前 permissive（同 module 私有访问允许），严格 enforcement 推迟 |
| §6.2 pass 4 (late resolve) | ✅ 实现 | — | `resolve::path_resolve::resolve_all_paths` + 11 个 path/expr 解析函数 |
| §6.2 pass 5 (resolve main) | ✅ 实现 | — | driver 层处理 |
| §6.2 pass 6 (check unused imports) | ❌ 未实现 | B1 | v0.2+ |
| §6.2 pass 7 (report errors) | ✅ 实现 | — | `Resolver::into_errors` |
| §6.2 pass 8 (postprocess) | ✅ 实现 | — | 无临时数据需清理 |
| §6.3 prelude | B4 | — | 实现使用 stdlib 内置 traits，非显式 prelude 导入 |

### 13.2 §7 模块系统 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| `mod foo { ... }` (inline) | ✅ 实现 | — | `resolve::module_build::build_child_module` |
| `mod foo;` (external) | ❌ 未实现 | B1 | v0.2+（需要文件系统加载） |
| `use` 导入 | ✅ 实现 | — | `resolve_use_leaf` + `resolve_use_glob` |
| glob import `use foo::*` | ✅ 实现 | — | `resolve_use_glob` |
| 嵌套 module 路径 `a::b::c` | ✅ 实现 | — | `resolve_path` |
| visibility `pub` / `pub(crate)` / `pub(super)` | ✅ 实现 | B3 | `check_visibility` 当前 permissive |

### 13.3 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B1（pass 6 unused imports warning） | v0.2+ | MVP 不需要 |
| B1（external mod） | v0.2+ | 需要文件系统 |
| B3（visibility 严格 enforcement） | v0.2+ | 当前 permissive 避免误报 |
| B4（prelude 显式导入） | v0.2+ | 当前用 stdlib 内置 traits 替代 |
