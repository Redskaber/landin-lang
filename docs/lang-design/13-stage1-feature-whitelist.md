# 13 — Stage 1 源码特性白皮书

> 本文是 R8 审查报告的关键产出。明示 stage 1（用 Landin 重写的编译器）源码允许使用的全部语言特性子集，避免 stage 0 反复"补特性 → 重冻 blob → stage 1 重试"的循环。R1 报告指出，Rust 自举过程中 OCaml rustc 滞后于语言演化是反复踩坑的根源。

---

## 1. 目的与原则

### 1.1 目的

Stage 1 是用 Landin 写的 Landin 编译器，必须能被 stage 0 编译。这要求：

- Stage 1 源码用到的每个语言特性，stage 0 都必须支持
- Stage 0 支持但 stage 1 不用的特性，不影响自举（可后续优化）
- Stage 1 不允许使用 stage 0 不支持的特性（否则无法编译）

### 1.2 三条铁律

1. **stage 0 特性集 ⊇ stage 1 用到的特性集**
2. **stage 1 标准库依赖 = core + alloc + libc FFI**（不依赖 std）
3. **stage 1 不允许使用 `unsafe` 之外的"逃生舱"**（如 asm、intrinsics 仅限最小集）

---

## 2. 允许的语言特性

### 2.1 类型系统

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| 基本类型 i8..i64, u8..u64, f32/f64, bool, char | ✅ | isize/usize 也允许 |
| Tuple | ✅ | 含 0/1/2+ 元 |
| Array `[T; N]` | ✅ | N 必须是 const expr |
| Slice `&[T]` / `&mut [T]` | ✅ | unsized 类型，已部分支持 |
| `str` / `String` | ✅ | str 是 unsized |
| Struct (named/tuple/unit) | ✅ | |
| Enum (unit/tuple/struct variants) | ✅ | |
| `&T` / `&mut T` | ✅ | 含 lifetime 标注 |
| `*const T` / `*mut T` | ✅ | 仅在 unsafe 块解引用 |
| `fn(T) -> U` 函数指针 | ✅ | |
| `Box<T>` / `Vec<T>` / `Rc<T>` / `String` | ✅ | 标准库类型 |
| `Option<T>` / `Result<T, E>` | ✅ | |
| `dyn Trait` | ✅ | 仅 `Box<dyn>` / `&dyn` 形式 |
| Generic types | ✅ | monomorphization |
| `impl Trait` in argument position | ✅ | |
| `impl Trait` in return position | ❌ | v0.2 |
| Lifetime 参数 `'a` | ✅ | 含 HRTB `for<'a>` |
| `?Sized` bound | ✅ | 仅用于 Deref Target / Box 内部 |
| Const generics | ❌ | v0.2 |
| GATs | ❌ | v0.2 |
| Union | ❌ | v0.2 |

### 2.2 Trait 系统

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| Trait 定义（含 required method） | ✅ | |
| Provided method（默认实现） | ✅ | |
| Associated type | ✅ | 不含 GATs |
| Associated const | ✅ | |
| Trait inheritance `trait B: A` | ✅ | |
| Generic trait `trait Iterator<T>` | ✅ | |
| `impl Trait for Type` | ✅ | 含泛型 impl |
| `where` clause | ✅ | |
| `Self` 关键字 | ✅ | |
| `dyn Trait` 对象 | ✅ | object safety 规则 |
| `impl Trait` 参数语法糖 | ✅ | |
| Default type param `Rhs = Self` | ✅ | 仅此形式 |
| Specialization | ❌ | 永久不做 |
| Overlapping impls | ❌ | |
| Marker trait（Copy/Sized） | ✅ | |
| Auto trait（Send/Sync） | ❌ | v0.2 |
| `#[may_dangle]` | ✅ | Drop impl 用 |
| Negative impl `impl !Trait for Type` | ❌ | v0.2 |

### 2.3 控制流

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| `if` / `else if` / `else` | ✅ | 表达式形式 |
| `match` | ✅ | 含 guard |
| `loop` | ✅ | 含 `break value` |
| `while` | ✅ | |
| `while let` | ✅ | |
| `for x in iter` | ✅ | 要求 IntoIterator |
| `if let` | ✅ | |
| `break` / `continue` | ✅ | 不带 label |
| `return` | ✅ | |
| `?` 操作符 | ✅ | Result only（不含 Option） |
| Labeled loop `'label: loop` | ❌ | v0.2 |
| `become` (effects) | ❌ | v2.0+ |

### 2.4 模式匹配

| 模式 | 允许 | 备注 |
| --- | --- | --- |
| `_` 通配符 | ✅ | |
| 字面量模式 | ✅ | |
| 变量绑定 | ✅ | |
| `x @ pat` | ✅ | |
| 范围 `1..=10` / `1..10` | ✅ | |
| Struct 解构 `Point { x, y }` | ✅ | |
| Enum 解构 `Some(x)` | ✅ | |
| Tuple 解构 `(a, b)` | ✅ | |
| Array 解构 `[a, b, ..]` | ✅ | |
| Or 模式 `1 \| 2 \| 3` | ✅ | |
| 引用模式 `&x` / `&mut x` | ✅ | |
| `..` rest | ✅ | |
| `box` pattern | ❌ | v0.2 |
| Deref pattern | ❌ | v0.2 |

### 2.5 表达式

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| 算术 / 位 / 比较运算符 | ✅ | 含重载 |
| `&&` / `\|\|` 短路 | ✅ | |
| `as` 类型转换 | ✅ | 仅数值/指针 |
| `&` / `&mut` 借用 | ✅ | |
| `*` 解引用 | ✅ | |
| `.` 字段访问 / 方法调用 | ✅ | |
| `[]` 索引 | ✅ | |
| `()` 单元 | ✅ | |
| Tuple 构造 `(a, b)` | ✅ | |
| Array 构造 `[a, b, c]` / `[a; N]` | ✅ | |
| Struct 构造 `Point { x, y }` | ✅ | |
| Range `a..b` / `a..=b` | ✅ | 仅 Iterator 上下文 |
| Closure `\|x\| x + 1` | ✅ | Fn/FnMut/FnOnce 自动推导 |
| `move` closure | ❌ | v0.2 |
| `async` / `await` | ❌ | v0.2 |
| Try block `try { ... }` | ❌ | v0.2 |

### 2.6 内建宏（仅允许的子集）

Stage 1 可用的内建宏（共 26 个，v1.2.2 修正数量与 02 文档统一，含 matches!）：

| 宏 | 用途 | 允许 |
| --- | --- | --- |
| `println!` / `print!` / `eprintln!` / `eprint!` | 输出 | ✅ |
| `format!` | 字符串格式化 | ✅ |
| `write!` / `writeln!` | 写入 Writer | ✅ |
| `vec!` | Vec 构造 | ✅ |
| `matches!` | 模式匹配判断 | ✅ |
| `assert!` / `assert_eq!` / `assert_ne!` | 测试断言 | ✅ |
| `debug_assert!` / `debug_assert_eq!` / `debug_assert_ne!` | debug 测试断言 | ✅ |
| `panic!` | panic | ✅ |
| `dbg!` | 调试输出 | ✅ |
| `unreachable!` | 不可达标记 | ✅ |
| `todo!` / `unimplemented!` | 未实现标记 | ✅ |
| `concat!` / `stringify!` / `file!` / `line!` / `column!` / `module_path!` | 编译期信息 | ✅ |

**禁止使用**：`macro_rules!` 自定义宏（v0.2 才支持）

### 2.7 语句与声明

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| `let` / `let mut` | ✅ | |
| `let` 解构 | ✅ | |
| `let-else` | ❌ | v0.2 |
| 函数声明 `fn` | ✅ | |
| `const` / `static` / `static mut` | ✅ | |
| `struct` / `enum` / `trait` / `impl` / `type` | ✅ | |
| `extern` block / `extern "C"` fn | ✅ | |
| `use` / `mod` / `pub` | ✅ | |
| `#[attr]` 属性 | ✅ | 仅 stage 0 实现的属性 |
| 嵌套 item | ❌ | 与 Austral 一致 |
| Shadowing | ❌ | 与 Austral 一致 |

### 2.8 属性（仅 stage 0 实现的）

| 属性 | 允许 | 用途 |
| --- | --- | --- |
| `#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]` | ✅ | v0.1 stage 0 实现 |
| `#[repr(C)]` | ✅ | FFI 用 |
| `#[repr(transparent)]` | ✅ | newtype 模式 |
| `#[inline]` / `#[inline(always)]` | ✅ | codegen 提示 |
| `#[cold]` | ✅ | codegen 提示 |
| `#[link(name = "...")]` | ✅ | FFI |
| `#[no_mangle]` | ✅ | FFI 导出 |
| `#[may_dangle]` | ✅ | Drop impl |
| `#[track_caller]` | ✅ | 错误信息 |
| `#[unstable]` / `#[stable]` | ✅ | feature gate |
| `#![no_std]` | ✅ | crate 级 |
| `#![feature(...)]` | ✅ | crate 级 |

### 2.9 unsafe

| 操作 | 允许 | 备注 |
| --- | --- | --- |
| `unsafe fn` 声明 | ✅ | |
| `unsafe { ... }` 块 | ✅ | |
| `unsafe impl Trait for Type` | ✅ | unsafe trait 的 impl |
| 解引用裸指针 | ✅ | |
| 访问 `static mut` | ✅ | |
| 调用 `unsafe fn` | ✅ | |
| 调用 `extern "C"` fn | ✅ | |
| 实现 `unsafe trait`（如 v0.2 的 Send/Sync） | N/A | MVP 无 unsafe trait |
| `core::ptr::read` / `write` | ✅ | |
| `core::mem::transmute` | ✅ | |
| `core::intrinsics::...` | ❌ | 仅编译器内部 |

### 2.10 FFI

| 特性 | 允许 | 备注 |
| --- | --- | --- |
| `extern "C" { fn foo(...); }` | ✅ | |
| `extern "Landin" fn` | ✅ | 默认 ABI |
| `#[link(name = "c")]` | ✅ | libc |
| `#[no_mangle]` 导出 | ✅ | |
| 可变参数 `extern "C" fn printf(fmt: *const u8, ...)` | ✅ | |
| `extern "Rust"` ABI | ❌ | v0.2 |
| `extern "System"` | ⚠️ | MVP 等同 "C" |

---

## 3. 标准库依赖清单

### 3.1 core 模块（stage 1 必须使用）

```
core::
├── prelude::*           // 默认导入
├── option::Option       // Some/None
├── result::Result       // Ok/Err
├── ops::{Add, Sub, ..., Fn, FnMut, FnOnce, Deref, DerefMut, Drop, Try}
├── cmp::{PartialEq, Eq, PartialOrd, Ord, Ordering}
├── convert::{From, Into, AsRef, AsMut}
├── iter::{Iterator, IntoIterator, DoubleEndedIterator}
├── marker::{Copy, Clone, Sized, Tuple}
├── mem::{size_of, align_of, swap, replace, take, transmute, drop, MaybeUninit}
├── ptr::{NonNull, null, null_mut, read, write, copy, copy_nonoverlapping, drop_in_place}
├── slice::{from_raw_parts, from_raw_parts_mut}
├── cell::{Cell, RefCell, UnsafeCell}
├── default::Default
├── hash::{Hash, Hasher}
├── fmt::{Display, Debug, Formatter, Write}
└── str::{FromStr, from_utf8_unchecked}
```

### 3.2 alloc 模块

```
alloc::
├── boxed::Box
├── vec::Vec
├── string::{String, ToString}
├── rc::Rc
├── borrow::Cow
├── collections::{BTreeMap, BTreeSet, HashMap, HashSet}
└── alloc::{Global, Allocator, Layout, AllocError}
```

### 3.3 libc FFI（stage 1 显式 extern）

Stage 1 不依赖 `std`，直接 extern libc：

```landin
extern "C" {
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn lseek(fd: i32, offset: i64, whence: i32) -> i64;
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    fn abort() -> !;
    fn exit(code: i32) -> !;
    fn getenv(name: *const u8) -> *const u8;
    fn argc() -> i32;          // 平台特定
    fn argv() -> *const *const u8;
}
```

平台常量（Linux x86_64 示例）：

```landin
const O_RDONLY: i32 = 0;
const O_WRONLY: i32 = 1;
const O_RDWR: i32 = 2;
const O_CREAT: i32 = 64;
const O_TRUNC: i32 = 512;
const O_APPEND: i32 = 1024;
const SEEK_SET: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;
```

### 3.4 不允许使用

Stage 1 不允许使用以下（即使存在）：

- `std::io` / `std::fs` / `std::env` / `std::process` / `std::path`
- 任何 `std::*` 模块
- `thread` / `sync` / `async` 相关

---

## 4. Stage 0 必须支持的特性清单（反向要求）

基于上述 stage 1 用到的特性，stage 0 必须实现：

### 4.1 类型系统最低要求

- 完整 HM + lifetime 推导（含 HRTB）
- Trait resolution 三阶段（含 canonical query）
- Monomorphization
- NLL borrow check（含 two-phase borrows 子集）
- Disjoint closure captures (RFC 2229)
- Associated type normalization（含终止保证）
- Drop check (`#[may_dangle]`)
- Integer fallback（仅无 trait bound 时触发）
- `?Sized` 部分支持

### 4.2 标准库最低要求

- core 完整（含 `Tuple` / `Try` / `FromStr` / `Layout` / `AllocError` / `UnsafeCell` / `MaybeUninit` / `drop_in_place` / `from_raw_parts`）
- alloc 完整（Box/Vec/String/Rc/BTreeMap/HashMap）
- libc FFI 完整

### 4.3 内建宏最低要求

26 个内建宏全部实现（v1.2.2 修正数量），清单见 §2.6。

### 4.4 属性最低要求

Stage 1 至少需 22 个核心属性，Stage 0 实际实现约 33 个 MVP 属性（v1.2.3 修正：消除假一致性声明）。完整清单见 15-attributes §2.2（27 个 MVP item-level + 9 个 crate-level，其中 `#[may_dangle]` 在 15 §2.2 标注"Drop impl 用"，必须在 stage 0 实现）。

核心 22 个属性（stage 1 自举最低要求）：`derive` / `repr(C)` / `repr(transparent)` / `inline` / `cold` / `link` / `no_mangle` / `may_dangle` / `track_caller` / `unstable` / `stable` / `must_use` / `deprecated`（13 个 item-level）+ `no_std` / `feature` / `crate_type` / `crate_name` / `recursion_limit` / `allow` / `warn` / `deny` / `doc`（9 个 crate-level）。

### 4.5 ABI 最低要求

- `"Landin"` ABI（与 C 一致）
- `"C"` ABI
- 可变参数 `...`

---

## 5. 验证流程

### 5.1 Stage 1 写作纪律

每写一个 stage 1 源文件，必须：

1. 检查所用特性是否在 §2 允许清单中
2. 检查所用标准库 API 是否在 §3 清单中
3. 检查所用属性是否在 §2.8 清单中
4. 若用 `unsafe`，检查操作是否在 §2.9 清单中

### 5.2 Stage 0 conformance 套件补充

针对 stage 1 用到的每个特性，stage 0 conformance 套件必须有对应测试：

| 特性类别 | 最少测试数 |
| --- | --- |
| §2.1 类型系统 | 200 |
| §2.2 Trait 系统 | 150 |
| §2.3 控制流 | 80 |
| §2.4 模式匹配 | 80 |
| §2.5 表达式 | 100 |
| §2.6 内建宏 | 50 |
| §2.7 语句声明 | 50 |
| §2.8 属性 | 30 |
| §2.9 unsafe | 50 |
| §2.10 FFI | 30 |
| §3 标准库 | 400 |
| **合计** | **1,220** |

加上通用 conformance 测试，stage 0 总测试数应 ≥ 3,000。

### 5.3 Stage 1 自测

Stage 1 源码完成后，必须能：

1. 被 stage 0 编译为二进制
2. 该二进制能编译自身（即 stage 1 编译 stage 1 源码 → stage 2）
3. stage 2 与 stage 1 行为一致

---

## 6. 与 v1.0 的差异

v1.0 缺失本文档，导致：

- Stage 1 写作时无明确特性边界
- Stage 0 反复需要"补特性"
- 内部矛盾（如宏系统）未被发现

v1.1 新增本文档，作为 stage 0 与 stage 1 的**契约**：

- Stage 0 实现者按本文档 §4 实现
- Stage 1 实现者按本文档 §2/§3 写代码
- Conformance 套件按本文档 §5.2 编写

---

**下一文档**: [`14-soundness-considerations.md`](./14-soundness-considerations.md) — Soundness 论证
