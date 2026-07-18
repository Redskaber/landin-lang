# 15 — 属性系统

> 本文定义 Landin 的属性（attribute）系统：完整清单、处理 pipeline、derive 展开规则。v1.2 新增（R12 完备性审查建议）。

---

## 1. 属性分类

Landin 属性分两类：

### 1.1 Outer attribute（`#[...]`）

作用于"其后跟随的 item"：

```landin
#[derive(Clone, Debug)]
pub struct Point { x: i32, y: i32 }

#[inline]
pub fn fast_add(a: i32, b: i32) -> i32 { a + b }
```

### 1.2 Inner attribute（`#![...]`）

作用于"包含它的 module/crate"：

```landin
#![no_std]
#![feature(generic_associated_types)]

mod foo {
    #![allow(dead_code)]
    // ...
}
```

`#!` 必须出现在 module/crate 顶部（首条语句之前）。

---

## 2. MVP 支持的属性完整清单

### 2.1 Crate-level 属性

| 属性 | 用途 | MVP |
| --- | --- | --- |
| `#![no_std]` | 不链接 std，仅 core+alloc | ✅ |
| `#![feature(name)]` | 启用 nightly 特性 | ✅ |
| `#![allow(lint)]` | 关闭 lint（v0.2） | ⚠️ MVP 仅 `unused` |
| `#![warn(lint)]` | 启用 lint 警告 | v0.2 |
| `#![deny(lint)]` | 把 lint 升级为 error | v0.2 |
| `#![crate_type = "bin"\|"lib"\|"rlib"]` | 显式 crate 类型 | ✅ |
| `#![crate_name = "name"]` | 显式 crate 名 | ✅ |
| `#![recursion_limit = "128"]` | 递归深度限制 | ✅ |
| `#![type_length_limit = "..."]` | 类型长度限制 | v0.2 |

### 2.2 Item-level 属性

| 属性 | 用途 | MVP |
| --- | --- | --- |
| `#[derive(Trait1, Trait2, ...)]` | 自动派生 trait | ✅ |
| `#[repr(C)]` | C ABI 布局 | ✅ |
| `#[repr(transparent)]` | 透明包装（newtype） | ✅ |
| `#[repr(packed)]` | packed 布局 | v0.2 |
| `#[repr(align(n))]` | 显式对齐 | v0.2 |
| `#[inline]` | 建议内联 | ✅ |
| `#[inline(always)]` | 强制内联 | ✅ |
| `#[inline(never)]` | 禁止内联 | ✅ |
| `#[cold]` | 标记冷代码 | ✅ |
| `#[no_mangle]` | 关闭 name mangling | ✅ |
| `#[link(name = "name")]` | 链接外部库 | ✅ |
| `#[link_name = "name"]` | 外部符号重命名 | ✅ |
| `#[link_section = "section"]` | 链接到指定 section | v0.2 |
| `#[export_name = "name"]` | 导出符号重命名 | ✅ |
| `#[track_caller]` | panic 信息含调用位置 | ✅ |
| `#[must_use]` | 返回值必须使用 | ✅ |
| `#[deprecated]` | 标记弃用 | ✅ |
| `#[deprecated(since = "0.2", note = "use bar")]` | 弃用详情 | ✅ |
| `#[unstable(feature = "name", reason = "...")]` | unstable 标记 | ✅ |
| `#[stable(feature = "name", since = "0.1")]` | stable 标记 | ✅ |
| `#[path = "path/to/file.lin"]` | mod 文件路径 | ✅ |
| `#[doc = "..."]` | 文档注释（等价 `///`） | ✅ |
| `#[cfg(condition)]` | 条件编译 | v0.2 |
| `#[test]` | 测试函数标记 | ✅ |
| `#[ignore]` | 跳过测试 | ✅ |
| `#[should_panic(expected = "...")]` | 期望 panic | ✅ |
| `#[global_allocator]` | 全局 allocator | ✅ |
| `#[alloc_error_handler]` | allocator 错误处理 | ✅ |
| `#[panic_handler]` | panic 处理函数 | ✅ |
| `#[start]` | 自定义程序入口 | v0.2 |
| `#[lang = "name"]` | 编译器内部 lang item | ✅ |
| `#[fundamental]` | 标记 fundamental trait | v0.2 |
| `#[automatically_derived]` | derive 生成的 impl 标记 | ✅ |

### 2.3 字段级属性（v0.2）

- `#[serde(rename = "name")]` — 通过 proc macro 实现，v0.2 加
- MVP 不支持字段级属性

### 2.4 表达式级属性（v0.2）

- `#[allow(...)]` on 单个表达式
- MVP 不支持

---

## 3. 属性处理 pipeline

```
源码
  ↓
Lexer/Parser 收集 attr 到 AST
  ↓
HIR lowering
  ↓ (1) 处理 #![feature(...)] 启用 nightly
  ↓ (2) 处理 #![no_std] 切换 std → core
  ↓ (3) 处理 #[derive(...)] 展开（生成 impl item）
  ↓ (4) 处理 #[repr(...)] 影响 type layout
  ↓ (5) 处理 #[cfg(...)] 条件编译（v0.2）
  ↓ (6) 处理 #[test]/#[ignore]/#[should_panic] 测试收集
  ↓ (7) 处理 #[global_allocator]/#[panic_handler]
  ↓ (8) 处理 #[link(name)] 收集链接信息
  ↓ (9) 处理 #[inline]/#[cold]/#[no_mangle] 传给 codegen
  ↓ (10) 检查 #[must_use]/#[deprecated] 在使用点
  ↓ (11) 报告未识别的属性
Codegen
  ↓ 应用 #[inline]/#[no_mangle]/#[export_name]/#[link_section] 到 LLVM IR
```

---

## 4. derive 展开规则

### 4.1 支持的 derive trait

| Trait | MVP | 展开规则 |
| --- | --- | --- |
| `Clone` | ✅ | 生成 `fn clone(&self) -> Self { Self { f1: self.f1.clone(), f2: self.f2.clone() } }` |
| `Copy` | ✅ | 要求所有字段 Copy，生成 `impl Copy for T {}`（空 impl） |
| `Debug` | ✅ | 生成 `fn fmt(&self, f) -> Result { f.debug_struct("T").field("f1", &self.f1)...finish() }` |
| `PartialEq` | ✅ | 生成 `fn eq(&self, other: &Self) -> bool { self.f1 == other.f1 && ... }` |
| `Eq` | ✅ | 要求所有字段 Eq，生成 `impl Eq for T {}`（空 impl） |
| `PartialOrd` | ✅ | 生成 `fn partial_cmp(&self, other) -> Option<Ordering> { self.f1.partial_cmp(&other.f1)... }` |
| `Ord` | ✅ | 生成 `fn cmp(&self, other) -> Ordering { self.f1.cmp(&other.f1)... }` |
| `Hash` | ✅ | 生成 `fn hash<H: Hasher>(&self, state: &mut H) { self.f1.hash(state); ... }` |
| `Default` | ✅ | 生成 `fn default() -> Self { Self { f1: Default::default(), ... } }` |

### 4.2 derive 展开时机

derive 在 **HIR lowering** 阶段展开，生成对应的 impl item 注入到 HIR：

```rust
// HIR lowering 前
#[derive(Clone, Debug)]
struct Point { x: i32, y: i32 }

// HIR lowering 后
struct Point { x: i32, y: i32 }

impl Clone for Point {
    #[automatically_derived]
    fn clone(&self) -> Self {
        Point { x: self.x.clone(), y: self.y.clone() }
    }
}

impl Debug for Point {
    #[automatically_derived]
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_struct("Point")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}
```

### 4.3 derive 限制

MVP 阶段 derive 限制：

- ❌ 不支持 derive for enum variant 单独（仅整个 enum）
- ❌ 不支持自定义 derive（proc macro 推 v0.2）
- ❌ 不支持 `#[derive]` helper attribute（v0.2）

---

## 5. repr 属性与 type layout

### 5.1 `#[repr(Rust)]`（默认）

- 字段顺序未指定，编译器可重排优化
- 大小可能小于字段总和（padding 优化）

### 5.2 `#[repr(C)]`

- 字段按声明顺序
- 满足 C ABI 兼容
- padding 与 C 编译器一致

### 5.3 `#[repr(transparent)]`

- 要求 struct 仅 1 个非 ZST 字段
- struct 与该字段布局完全一致
- 用于 newtype 模式：`struct Wrapper(i32)` 与 `i32` 同布局

### 5.4 `#[repr(packed)]`（v0.2）

- 移除所有 padding
- 字段对齐为 1
- 访问字段需 unsafe（可能未对齐）

### 5.5 `#[repr(align(n))]`（v0.2）

- 显式指定对齐为 n（必须为 2 的幂）

### 5.6 enum 的 repr

```landin
#[repr(C)]
enum E { A, B, C }      // discriminant 大小 = sizeof(c_int) = 4

#[repr(u8)]
enum E { A, B, C }      // discriminant 强制为 u8

#[repr(C, u8)]
enum E { A, B(u8) }     // C 布局 + u8 discriminant
```

MVP 仅支持 `#[repr(C)]` 与默认，`#[repr(u8/u16/...)]` 推 v0.2。

---

## 6. cfg 条件编译（v0.2）

```landin
#[cfg(target_os = "linux")]
fn platform_specific() { /* linux impl */ }

#[cfg(target_os = "macos")]
fn platform_specific() { /* macos impl */ }
```

MVP **不支持** cfg（v0.2 加），用户需用 `#[cfg]` 替代为 build script 或手动管理。

---

## 7. 属性错误处理

### 7.1 未识别属性

MVP 报 warning（不是 error），允许用户用第三方属性（v0.2 proc macro 处理）：

```
warning: unknown attribute `my_attr`
 --> src/foo.lin:5:1
  |
5 | #[my_attr]
  | ^^^^^^^^^^ unknown attribute, ignored
```

### 7.2 属性位置错误

```landin
// 错误：#[inline] 不能用在 struct 上
#[inline]
struct Foo;
```

报 error：`attribute #[inline] can only be applied to functions`。

### 7.3 属性参数错误

```landin
#[derive(Unknown)]   // 错误：Unknown 不在 MVP derive 清单
struct Foo;
```

报 error：`unknown derive trait: Unknown`。

---

## 8. 与 Rust 属性的差异

| 属性 | Rust | Landin | 理由 |
| --- | --- | --- | --- |
| `#[proc_macro_derive]` | ✅ | ❌ 永久不做 | 仅内建 derive |
| `#[cfg]` | ✅ | v0.2 | MVP 简化 |
| `#[serde(...)]` | ✅（第三方） | v0.2 | proc macro |
| `#[tokio(...)]` | ✅（第三方） | v0.2 | proc macro |
| `#[non_exhaustive]` | ✅ | v0.2 | API 稳定性 |
| `#[repr(C, u8)]` | ✅ | v0.2 | MVP 仅 repr(C) |
| `#[track_caller]` on closures | ✅ | v0.2 | MVP 简化 |

---

**下一文档**: [`16-diagnostics.md`](./16-diagnostics.md) — 诊断系统
