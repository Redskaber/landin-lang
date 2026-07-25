# 09 — 标准库设计

> 本文定义 Landin 的 **三层标准库**：`core` / `alloc` / `std`。三层分离从 day 1 就做（R1 教训：Rust RFC #40），自举编译器仅依赖 `core` + `alloc`，不依赖 `std`。

---

## 1. 库分层

### 1.1 三层结构

```
┌──────────────────────────────────────────────┐
│  std (facade)                                │
│  - 重导出 core + alloc + os-specific         │
│  - 含 fs / io / net / thread / process       │
│  - 依赖 OS                                   │
└──────────────────────────────────────────────┘
                  ↑ 依赖
┌──────────────────────────────────────────────┐
│  alloc                                       │
│  - Box / Vec / String / Rc / Arc             │
│  - HashMap / BTreeMap                        │
│  - 依赖全局 allocator                        │
└──────────────────────────────────────────────┘
                  ↑ 依赖
┌──────────────────────────────────────────────┐
│  core (no_std)                               │
│  - 基本类型 + marker trait                   │
│  - ops / cmp / iter / convert                │
│  - cell / mem / ptr / slice                  │
│  - option / result                           │
│  - 无依赖                                    │
└──────────────────────────────────────────────┘
```

### 1.2 库选择规则

```landin
#![no_std]    // 仅 core
// 或
#![no_std] extern crate alloc;    // core + alloc
// 或
// 默认: std（含 core + alloc + OS）
```

自举编译器（stage 1）使用 `#![no_std] extern crate alloc;`，不依赖 OS。

### 1.3 库规模

| 库 | 行数 | 文件数 |
| --- | --- | --- |
| core | 4,000 | ~30 |
| alloc | 3,000 | ~15 |
| std | 1,500 | ~10 |
| **合计** | **8,500** | **~55** |

比 Rust std（~500,000 行）小 60 倍，因 MVP 不含：

- `std::async` / `Future` / `async fn`
- `std::net` (v0.2)
- `std::process::Command` 复杂特性
- `std::thread`（v0.2）
- `std::sync` 完整（v0.2）
- `std::time`（v0.2）
- `std::collections` 全套
- `std::path` 跨平台复杂逻辑（简化版）

---

## 2. core 库

### 2.1 模块结构

```
core/
├── lib.rs            // crate root，导出 prelude
├── prelude.rs        // 默认导入
├── primitives.rs     // i8/i32/bool/char 等方法
├── marker.rs         // Copy/Sized/Send/Sync 等 marker trait
├── ops.rs            // Add/Sub/Fn 等运算符 trait
├── cmp.rs            // PartialEq/Eq/PartialOrd/Ord
├── iter/             // Iterator/IntoIterator
├── convert/          // From/Into/AsRef/AsMut
├── option.rs         // Option<T>
├── result.rs         // Result<T, E>
├── mem.rs            // size_of/align_of/swap/take
├── ptr.rs            // NonNull/null/null_mut
├── slice/            // [T] slice 操作
├── cell.rs           // Cell/RefCell
├── str/              // str primitive
├── fmt.rs            // Display/Debug/Formatter
├── num/              // 数值方法
├── clone.rs          // Clone trait
├── default.rs        // Default trait
├── hash.rs           // Hash/Hasher trait
└── panicking.rs      // panic 实现入口
```

### 2.2 Prelude

```landin
// core::prelude（默认导入到所有 crate）

// 类型与构造
pub use crate::option::Option::{self, None, Some};
pub use crate::result::Result::{self, Err, Ok};

// Marker trait（auto impl）
// MVP: 仅 Copy/Clone/Sized（无并发，无 Send/Sync/Unpin）
// v0.2: 加 Send/Sync/Unpin
pub use crate::marker::{Copy, Clone, Sized};

// 运算符 trait
pub use crate::ops::{
    Add, Sub, Mul, Div, Rem, Neg,
    BitAnd, BitOr, BitXor, Not, Shl, Shr,
    AddAssign, SubAssign, MulAssign, DivAssign, RemAssign,
    BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign,
    Deref, DerefMut,
    Fn, FnMut, FnOnce,
};

// 比较 trait
pub use crate::cmp::{PartialEq, Eq, PartialOrd, Ord, Ordering};
pub use crate::convert::{From, Into, AsRef, AsMut};

// 迭代
pub use crate::iter::{Iterator, IntoIterator, DoubleEndedIterator};

// Drop
pub use crate::ops::Drop;

// Default
pub use crate::default::Default;

// 内存
pub use crate::mem::{swap, replace, take, size_of, align_of};

// 字符串
pub use crate::string::String;       // re-export from alloc
pub use crate::vec::Vec;              // re-export from alloc
```

注意：`String` 和 `Vec` 在 `alloc` 中定义，但通过 `core::prelude` 重导出（仅在 `alloc` 可用时）。`std::prelude` 类似处理。

### 2.3 关键 trait 定义

```landin
// marker.rs
pub trait Copy: Clone {}

pub trait Sized {}      // 编译器自动 impl

pub trait Send {}       // v0.2: 编译器自动 impl
pub trait Sync {}       // v0.2

pub trait Unpin {}      // v0.2

// clone.rs
pub trait Clone: Sized {
    fn clone(&self) -> Self;
    fn clone_from(&mut self, source: &Self) {
        *self = source.clone();
    }
}

// ops.rs
pub trait Add<Rhs = Self> {
    type Output;
    fn add(self, rhs: Rhs) -> Self::Output;
}

pub trait AddAssign<Rhs = Self> {
    fn add_assign(&mut self, rhs: Rhs);
}

pub trait Deref {
    type Target: ?Sized;     // v0.2: ?Sized
    fn deref(&self) -> &Self::Target;
}

pub trait DerefMut: Deref {
    fn deref_mut(&mut self) -> &mut Self::Target;
}

pub trait Drop {
    fn drop(&mut self);
}

pub trait Fn<Args: Tuple>: FnMut<Args> {
    extern "Landin" fn call(&self, args: Args) -> Self::Output;
}

pub trait FnMut<Args: Tuple>: FnOnce<Args> {
    extern "Landin" fn call_mut(&mut self, args: Args) -> Self::Output;
}

pub trait FnOnce<Args: Tuple> {
    type Output;
    extern "Landin" fn call_once(self, args: Args) -> Self::Output;
}

// cmp.rs
pub trait PartialEq<Rhs: ?Sized = Self> {
    fn eq(&self, other: &Rhs) -> bool;
    fn ne(&self, other: &Rhs) -> bool { !self.eq(other) }
}

pub trait Eq: PartialEq<Self> {}

pub trait PartialOrd<Rhs: ?Sized = Self>: PartialEq<Rhs> {
    fn partial_cmp(&self, other: &Rhs) -> Option<Ordering>;
    fn lt(&self, other: &Rhs) -> bool { ... }
    fn le(&self, other: &Rhs) -> bool { ... }
    fn gt(&self, other: &Rhs) -> bool { ... }
    fn ge(&self, other: &Rhs) -> bool { ... }
}

pub trait Ord: Eq + PartialOrd<Self> {
    fn cmp(&self, other: &Self) -> Ordering;
}

pub enum Ordering {
    Less,
    Equal,
    Greater,
}

// convert.rs
pub trait From<T>: Sized {
    fn from(value: T) -> Self;
}

pub trait Into<T>: Sized {
    fn into(self) -> T;
}

// 默认 impl：From ↔ Into
impl<T, U: From<T>> Into<U> for T {
    fn into(self) -> U { U::from(self) }
}

// iter.rs
pub trait Iterator {
    type Item;
    
    fn next(&mut self) -> Option<Self::Item>;
    
    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }
    
    // 提供方法
    fn count(mut self) -> usize where Self: Sized {
        let mut n = 0;
        while self.next().is_some() { n += 1; }
        n
    }
    
    fn last(mut self) -> Option<Self::Item> where Self: Sized {
        let mut last = None;
        while let Some(x) = self.next() { last = Some(x); }
        last
    }
    
    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        while let Some(x) = self.next() {
            if n == 0 { return Some(x); }
            n -= 1;
        }
        None
    }
    
    fn map<B, F: FnMut(Self::Item) -> B>(self, f: F) -> Map<Self, F>
    where Self: Sized
    {
        Map::new(self, f)
    }
    
    fn filter<P: FnMut(&Self::Item) -> bool>(self, predicate: P) -> Filter<Self, P>
    where Self: Sized
    {
        Filter::new(self, predicate)
    }
    
    fn for_each<F: FnMut(Self::Item)>(mut self, mut f: F)
    where Self: Sized
    {
        while let Some(x) = self.next() { f(x); }
    }
    
    fn collect<B: FromIterator<Self::Item>>(self) -> B
    where Self: Sized
    {
        FromIterator::from_iter(self)
    }
    
    // ... 其他方法
}

pub trait IntoIterator {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter;
}

pub trait FromIterator<A>: Sized {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self;
}
```

### 2.4 Option 与 Result

```landin
// option.rs
pub enum Option<T> {
    None,
    Some(T),
}

impl<T> Option<T> {
    pub fn is_some(&self) -> bool { matches!(self, Some(_)) }
    pub fn is_none(&self) -> bool { matches!(self, None) }
    
    pub fn unwrap(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("called `Option::unwrap()` on a `None` value"),
        }
    }
    
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Some(v) => v,
            None => default,
        }
    }
    
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Some(v) => v,
            None => f(),
        }
    }
    
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U> {
        match self {
            Some(v) => Some(f(v)),
            None => None,
        }
    }
    
    pub fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Option<U> {
        match self {
            Some(v) => f(v),
            None => None,
        }
    }
    
    pub fn or(self, alternative: Option<T>) -> Option<T> {
        match self {
            Some(_) => self,
            None => alternative,
        }
    }
    
    pub fn take(&mut self) -> Option<T> {
        core::mem::replace(self, None)
    }
    
    // ... 其他
}

impl<T: Default> Default for Option<T> {
    fn default() -> Self { None }
}

// result.rs
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    pub fn is_ok(&self) -> bool { matches!(self, Ok(_)) }
    pub fn is_err(&self) -> bool { matches!(self, Err(_)) }
    
    pub fn ok(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }
    
    pub fn err(self) -> Option<E> {
        match self {
            Ok(_) => None,
            Err(e) => Some(e),
        }
    }
    
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Result<U, E> {
        match self {
            Ok(v) => Ok(f(v)),
            Err(e) => Err(e),
        }
    }
    
    pub fn map_err<F, O: FnOnce(E) -> F>(self, op: O) -> Result<T, F> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(op(e)),
        }
    }
    
    pub fn unwrap(self) -> T where E: Debug {
        match self {
            Ok(v) => v,
            Err(e) => panic!("called `Result::unwrap()` on an `Err` value: {:?}", e),
        }
    }
    
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Ok(v) => v,
            Err(_) => default,
        }
    }
    
    // ... 其他
}

// 实现 `?` 操作符（Try trait）
impl<T, E> Try for Result<T, E> {
    type Ok = T;
    type Error = E;
    
    fn into_result(self) -> Result<T, E> { self }
    fn from_ok(v: T) -> Self { Ok(v) }
    fn from_error(e: E) -> Self { Err(e) }
}
```

### 2.5 基本类型方法

```landin
// i32 等
impl i32 {
    pub const MAX: i32 = 2147483647;
    pub const MIN: i32 = -2147483648;
    
    pub fn abs(self) -> i32 { if self < 0 { -self } else { self } }
    pub fn pow(self, exp: u32) -> i32 { ... }
    pub fn to_string(self) -> String { ... }
    
    pub fn checked_add(self, rhs: i32) -> Option<i32> {
        let (r, overflow) = self.overflowing_add(rhs);
        if overflow { None } else { Some(r) }
    }
    // wrapping_add 是 intrinsic，由编译器内建实现（直接映射 LLVM `add`，不检查溢出）
    // 此处签名仅用于文档展示，实际实现见 compiler intrinsics
    // pub fn wrapping_add(self, rhs: i32) -> i32 { intrinsics::add(self, rhs) }
    pub fn saturating_add(self, rhs: i32) -> i32 {
        let (r, overflow) = self.overflowing_add(rhs);
        if !overflow { r }
        else if self > 0 { i32::MAX }
        else { i32::MIN }
    }
    
    // ... 其他
}

// bool
impl bool {
    pub fn then<T>(self, f: impl FnOnce() -> T) -> Option<T> {
        if self { Some(f()) } else { None }
    }
}

// char
impl char {
    pub fn is_ascii(self) -> bool { (self as u32) < 128 }
    pub fn is_alphabetic(self) -> bool { ... }
    pub fn to_digit(self, radix: u32) -> Option<u32> { ... }
    pub fn from_digit(num: u32, radix: u32) -> Option<char> { ... }
}

// str (slice)
impl str {
    pub fn len(&self) -> usize { ... }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn is_ascii(&self) -> bool { self.bytes().all(|b| b < 128) }
    pub fn bytes(&self) -> Bytes<'_> { ... }
    pub fn chars(&self) -> Chars<'_> { ... }
    pub fn split(&self, sep: &str) -> Split<'_> { ... }
    pub fn contains(&self, needle: &str) -> bool { ... }
    pub fn starts_with(&self, prefix: &str) -> bool { ... }
    pub fn trim(&self) -> &str { ... }
    pub fn parse<F: FromStr>(&self) -> Result<F, F::Err> { ... }
}
```

### 2.6 mem / ptr / cell

```landin
// mem.rs
pub fn size_of<T>() -> usize { ... }
pub fn size_of_val<T: ?Sized>(val: &T) -> usize { ... }   // v0.2
pub fn align_of<T>() -> usize { ... }
pub fn align_of_val<T: ?Sized>(val: &T) -> usize { ... }  // v0.2

pub fn swap<T>(x: &mut T, y: &mut T) { ... }
pub fn replace<T>(dest: &mut T, src: T) -> T { ... }
pub fn take<T: Default>(dest: &mut T) -> T { replace(dest, T::default()) }

pub fn drop<T>(_x: T) {}

// ptr.rs
pub fn null<T>() -> *const T { ... }
pub fn null_mut<T>() -> *mut T { ... }

pub struct NonNull<T> { pointer: *const T }
impl<T> NonNull<T> {
    pub unsafe fn new(ptr: *mut T) -> Option<Self> { ... }
    pub unsafe fn new_unchecked(ptr: *mut T) -> Self { ... }
    pub fn as_ptr(self) -> *mut T { self.pointer as *mut T }
}

pub unsafe fn read<T>(src: *const T) -> T { ... }
pub unsafe fn write<T>(dst: *mut T, src: T) { ... }
pub unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) { ... }
pub unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize) { ... }

// cell.rs
pub struct Cell<T: ?Sized> { value: UnsafeCell<T> }      // v0.2: ?Sized

pub struct RefCell<T: ?Sized> {
    borrow_state: Cell<isize>,
    value: UnsafeCell<T>,
}
```

---

## 3. alloc 库

### 3.1 模块结构

```
alloc/
├── lib.rs
├── prelude.rs
├── boxed.rs          // Box<T>
├── vec.rs            // Vec<T>
├── string.rs         // String, ToString
├── rc.rs             // Rc<T>, Weak<T>
├── sync.rs           // Arc<T>, Weak<T> (v0.2: thread)
├── collections/
│   ├── mod.rs
│   ├── btree.rs      // BTreeMap, BTreeSet
│   └── hash_map.rs   // HashMap, HashSet (简化版)
├── raw_vec.rs        // RawVec<T> (Vec 内部)
├── borrow.rs         // Cow<T>
└── fmt.rs            // format!() (v0.2)
```

### 3.2 Box

```landin
pub struct Box<T: ?Sized, A: Allocator = Global>(*mut T, A);    // v0.2: ?Sized

// MVP 实际签名（无 ?Sized，要求 T: Sized）：
// pub struct Box<T, A: Allocator = Global>(*mut T, A);

impl<T> Box<T> {
    pub fn new(x: T) -> Self {
        let ptr = Box::alloc(Layout::new::<T>());
        unsafe {
            core::ptr::write(ptr as *mut T, x);
            Box::from_raw_in(ptr, Global)
        }
    }
    
    pub fn into_raw(b: Self) -> *mut T { ... }
    pub unsafe fn from_raw(ptr: *mut T) -> Self { ... }
}

impl<T: ?Sized, A: Allocator> Deref for Box<T, A> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.0 } }
}

impl<T: ?Sized, A: Allocator> DerefMut for Box<T, A> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.0 } }
}

impl<T: ?Sized, A: Allocator> Drop for Box<T, A> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.0);
            self.1.deallocate(NonNull::new_unchecked(self.0 as *mut u8), Layout::for_value(&*self.0));
        }
    }
}
```

### 3.3 Vec

```landin
pub struct Vec<T, A: Allocator = Global> {
    buf: RawVec<T, A>,
    len: usize,
}

impl<T> Vec<T> {
    pub fn new() -> Self { Vec { buf: RawVec::new(), len: 0 } }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Vec { buf: RawVec::with_capacity(capacity), len: 0 }
    }
    
    pub fn push(&mut self, value: T) {
        if self.len == self.buf.capacity() {
            self.buf.grow();
        }
        unsafe {
            let end = self.as_mut_ptr().add(self.len);
            core::ptr::write(end, value);
        }
        self.len += 1;
    }
    
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 { return None; }
        unsafe {
            let val = core::ptr::read(self.as_ptr().add(self.len - 1));
            self.len -= 1;
            Some(val)
        }
    }
    
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn capacity(&self) -> usize { self.buf.capacity() }
    
    pub fn as_ptr(&self) -> *const T { self.buf.ptr() }
    pub fn as_mut_ptr(&mut self) -> *mut T { self.buf.ptr() }
    
    pub fn iter(&self) -> Iter<'_, T> { ... }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> { ... }
    
    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }
    
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for v in iter { self.push(v); }
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        // RawVec 自己释放 buffer
    }
}

impl<T> Deref for Vec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.buf.ptr(), self.len) }
    }
}

impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.buf.ptr(), self.len) }
    }
}
```

### 3.4 String

```landin
pub struct String {
    vec: Vec<u8>,
}

impl String {
    pub fn new() -> Self { String { vec: Vec::new() } }
    
    pub fn from_str(s: &str) -> Self {
        let mut v = Vec::with_capacity(s.len());
        v.extend_from_slice(s.as_bytes());
        String { vec: v }
    }
    
    pub fn push_str(&mut self, s: &str) {
        self.vec.extend_from_slice(s.as_bytes());
    }
    
    pub fn push(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.push_str(s);
    }
    
    pub fn len(&self) -> usize { self.vec.len() }
    pub fn is_empty(&self) -> bool { self.vec.is_empty() }
    
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.vec) }
    }
    
    pub fn into_bytes(self) -> Vec<u8> { self.vec }
}

impl Deref for String {
    type Target = str;
    fn deref(&self) -> &str { self.as_str() }
}

impl Display for String {
    fn fmt(&self, f: &mut Formatter) -> Result { write!(f, "{}", self.as_str()) }
}
```

### 3.5 Rc / Arc（v0.2 - 单线程 Rc）

MVP 仅 Rc（单线程），Arc 推迟到 v0.2。

```landin
pub struct Rc<T: ?Sized> {
    ptr: NonNull<RcBox<T>>,
}

struct RcBox<T: ?Sized> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: T,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Self {
        let box_ = Box::new(RcBox {
            strong: Cell::new(1),
            weak: Cell::new(1),
            value,
        });
        Rc { ptr: Box::into_raw(box_) }
    }
    
    pub fn clone(&self) -> Self {
        let inner = unsafe { &*self.ptr.as_ptr() };
        inner.strong.set(inner.strong.get() + 1);
        Rc { ptr: self.ptr }
    }
}

impl<T: ?Sized> Drop for Rc<T> {
    fn drop(&mut self) {
        let inner = unsafe { &*self.ptr.as_ptr() };
        let strong = inner.strong.get() - 1;
        inner.strong.set(strong);
        if strong == 0 {
            unsafe { core::ptr::drop_in_place(&mut (*self.ptr.as_ptr()).value); }
            let weak = inner.weak.get() - 1;
            inner.weak.set(weak);
            if weak == 0 {
                unsafe { Box::from_raw(self.ptr.as_ptr()); }
            }
        }
    }
}
```

---

## 4. std 库

### 4.1 模块结构

```
std/
├── lib.rs            // crate root，re-export core+alloc+os
├── prelude.rs        // std prelude
├── io/
│   ├── mod.rs        // Read/Write/BufRead trait
│   ├── stdin.rs      // stdin/stdout/stderr
│   └── printf.rs     // println! / print! (v0.2: macro)
├── fs/
│   ├── mod.rs
│   └── file.rs       // File
├── env/
│   ├── mod.rs
│   └── args.rs       // args()
├── process/
│   └── mod.rs        // exit()
└── path/
    └── mod.rs        // Path/PathBuf (简化版)
```

### 4.2 std::io

```landin
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        // 默认实现
    }
    
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        // 默认实现
    }
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
    fn flush(&mut self) -> Result<()>;
    
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            let n = self.write(buf)?;
            buf = &buf[n..];
        }
        Ok(())
    }
    
    fn write_str(&mut self, s: &str) -> Result<()> {
        self.write_all(s.as_bytes())
    }
}

pub fn stdout() -> Stdout { ... }
pub fn stderr() -> Stderr { ... }
pub fn stdin() -> Stdin { ... }

pub struct Stdout { ... }
pub struct Stderr { ... }
pub struct Stdin { ... }

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let n = unsafe { libc::write(1, buf.as_ptr() as *const _, buf.len()) };
        if n < 0 { Err(io::Error::last_os_error()) } else { Ok(n as usize) }
    }
    fn flush(&mut self) -> Result<()> { Ok(()) }
}
```

### 4.3 println（v0.2 macro，v0.1 函数模拟）

MVP 无 macro，提供函数式 API：

```landin
pub fn println(s: &str) {
    let mut out = io::stdout();
    let _ = out.write_str(s);
    let _ = out.write_str("\n");
}

pub fn print(s: &str) {
    let mut out = io::stdout();
    let _ = out.write_str(s);
}

pub fn eprintln(s: &str) { ... }
```

v0.2 加 `println!` 宏后，旧函数废弃。

### 4.4 std::fs

```landin
pub struct File {
    fd: i32,
}

impl File {
    pub fn open(path: &str) -> Result<File> {
        let p = CString::new(path);
        let fd = unsafe { libc::open(p.as_ptr(), libc::O_RDONLY) };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        Ok(File { fd })
    }
    
    pub fn create(path: &str) -> Result<File> {
        let p = CString::new(path);
        let fd = unsafe {
            libc::open(p.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644)
        };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        Ok(File { fd })
    }
    
    pub fn read_to_string(path: &str) -> Result<String> {
        let mut file = File::open(path)?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        Ok(s)
    }
}

impl Read for File { ... }
impl Write for File { ... }

impl Drop for File {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}
```

### 4.5 std::env

```landin
pub fn args() -> Args { ... }
pub fn var(key: &str) -> Result<String> { ... }
pub fn var_os(key: &str) -> Option<String> { ... }
pub fn current_dir() -> Result<String> { ... }
```

### 4.6 std::process

```landin
pub fn exit(code: i32) -> ! {
    unsafe { libc::exit(code); }
}

pub fn abort() -> ! {
    unsafe { libc::abort(); }
}
```

---

## 5. 不实现的标准库特性

为控制 MVP 规模，以下 Rust std 特性在 Landin v0.1 **不实现**：

| Rust 模块 | 状态 | 原因 |
| --- | --- | --- |
| `std::async` | ❌ v0.2 | 单线程 MVP |
| `std::thread` | ❌ v0.2 | 单线程 MVP |
| `std::sync::Mutex/RwLock` | ❌ v0.2 | 单线程不需要 |
| `std::net::Tcp/Udp` | ❌ v0.2 | 网络推迟 |
| `std::process::Command` | ❌ v0.2 | 子进程推迟 |
| `std::time::Instant/Duration` | ❌ v0.2 | 时间推迟 |
| `std::collections::VecDeque/LinkedList` | ❌ v0.2 | 罕用 |
| `std::path` 跨平台复杂逻辑 | ⚠️ 简化版 | MVP 仅 Unix-style path |
| `std::os::unix/windows` | ❌ v0.2 | 平台特定推迟 |
| `std::ffi::CString/CStr` | ✅ MVP | FFI 必需 |
| `std::panic::catch_unwind` | ❌ 永久 | MVP panic = abort |
| `std::backtrace` | ❌ v0.2 | 调试推迟 |
| `std::io::BufReader/BufWriter` | ⚠️ v0.1 加 | 性能必需 |
| `std::collections::HashMap` | ✅ MVP | 必需 |

---

## 6. 自举编译器依赖

Stage 1（Landin 写的编译器自身）依赖：

- `core` 全部
- `alloc` 全部（Vec/String/HashMap/Box/Rc）
- **不依赖** `std`（即 `#![no_std]`）

stage 1 用到的 OS 接口通过显式 `extern "C"` 调用 libc：

```landin
#![no_std]
extern crate alloc;

extern "C" {
    fn open(path: *const u8, flags: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn close(fd: i32) -> i32;
    // ... 仅编译器需要的最小集
}
```

这让 stage 1 编译器可在任何有 libc 的平台编译自身，不依赖 `std` 实现。

---

**下一文档**: [`10-toolchain.md`](./10-toolchain.md) — 工具链

---

## 11. 实现状态（v0.14.0，§25.8 回写）

> 本节由 Stage 6.18 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 11.1 stdlib 整体 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §1 stdlib 三层架构 (core/alloc/std) | B4 | — | 实现使用单 crate `stdlib` 模块，非三层独立 crate |
| §2 core prelude | ✅ 实现 | B3（简化） | 实现使用 `stdlib::mod::register_stdlib` 注册内置类型 |
| §3 alloc 层 | ❌ 未实现 | B1 | v0.2+（需要 Box/Vec/String 完整实现） |
| §4 std 层 | ❌ 未实现 | B1 | v0.2+ |
| §5 trait 定义 (Copy/Clone/Drop/PartialEq/...) | ✅ 实现 | — | `stdlib::trait_methods::STDLIB_TRAITS` (43 traits) |
| §6 stdlib trait method 查询 API | ✅ 实现 | B4 | 设计未描述，实现已做（Stage 5.93-5.99） |
| §7 vtable 布局 | ✅ 实现 | B4 | 设计未描述，实现已做（Stage 5.40-5.80） |
| §8 互操作 (C ABI / Rust ABI) | ❌ 未实现 | B1 | v0.2+ |
| §9 最小 libc binding | ❌ 未实现 | B1 | v0.2+ |

### 11.2 stdlib trait method 查询 API — 实现扩展（B4 补写）

设计文档 §5 描述了 trait 定义，但未描述 trait method 查询 API。Stage 5.93-5.99
实现了完整的查询 API：

| 查询类型 | 函数 | Stage |
|---------|------|-------|
| 正向查询 | `find_stdlib_trait_method` | 5.93 |
| 字段访问器 | `stdlib_trait_method_return_kind` / `param_kinds` / `self_kind` / `param_count` / `is_unsafe` | 5.93-5.94 |
| 反向查询 | `stdlib_trait_methods_by_self_kind` / `by_return_kind` / `by_is_unsafe` / `by_param_count` | 5.95-5.99 |
| 语义分组 | `stdlib_marker_traits` / `arithmetic_traits` / `core_traits` / `io_traits` / `unary_traits` | 5.87-5.90 |
| 统计 | `stdlib_trait_count` / `stdlib_trait_method_count` | 5.82-5.86 |
| 成员查询 | `is_stdlib_trait` / `is_stdlib_trait_method` / `is_stdlib_marker_trait` | 5.81-5.85 |

### 11.3 vtable 布局 + emission — 实现扩展（B4 补写）

设计文档未描述 vtable 布局。Stage 5.40-5.80 实现了完整的 vtable/dynptr emission：

| 概念 | 实现位置 | Stage |
|------|---------|-------|
| `StdlibVtableSlot` + `stdlib_vtable_layout` | `stdlib::vtable_layout` | 5.40-5.50 |
| `StdlibVtablePlan` + `stdlib_vtable_plan` | `stdlib::vtable_layout` | 5.50-5.55 |
| vtable symbol 生成 | `stdlib::vtable_layout::stdlib_vtable_global_name` | 5.55-5.60 |
| vtable emission | `stdlib::vtable_layout::StdlibVtableEmission` | 5.60-5.70 |
| dynptr emission | `stdlib::vtable_layout::StdlibDynptrEmission` | 5.65-5.70 |
| codegen vtable/dynptr globals | `codegen::trait_dispatch` | 5.70-5.80 |

### 11.4 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B1（alloc/std 层 / 互操作 / libc binding） | v0.2+ | MVP 不需要 |
| B3（core prelude 简化） | v0.2+ | 当前 register_stdlib 满足 MVP |
| B4（trait method 查询 API + vtable 布局） | 已在 §11.2-11.3 补写 | — |
