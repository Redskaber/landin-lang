# Stage 5.36 开发计划：stdlib trait method signatures

> **阶段**: Stage 5.36
> **版本**: v0.11.31 → v0.11.32
> **状态**: ✅ Complete

## 1. 目标

为标准库 trait 注册方法签名信息（`StdlibTraitMethod` + `StdlibSelfKind`），
为 dyn Trait MIR lowering 准备"trait 声明了哪些方法、各方法接收者类型、
参数数量、返回类型 kind"等元数据。这是 TD-014 解阻的关键预备步骤：
后续 `dyn Trait` 调用 lowering 需要查询 trait 的方法签名才能生成 vtable
偏移 + 参数类型 + 返回值类型。

## 2. 设计

### 2.1 新增类型

```rust
/// 接收者类型 — 决定方法在 vtable 中的 self 参数 ABI。
pub enum StdlibSelfKind {
    SelfByValue,  // fn(self) -> ...
    SelfByRef,    // fn(&self) -> ...
    SelfByMutRef, // fn(&mut self) -> ...
    NoSelf,       // 关联函数（无 self）
}

/// 单个 trait 方法的签名描述。
pub struct StdlibTraitMethod {
    pub name: &'static str,
    pub self_kind: StdlibSelfKind,
    pub param_count: u32,       // 不含 self 的参数数量
    pub return_kind: StdlibTypeKind,
    pub is_unsafe: bool,
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_trait_methods` | `(trait_name: &str) -> Option<&'static [StdlibTraitMethod]>` | 获取 trait 全部方法 |
| `stdlib_trait_method_count` | `(trait_name: &str) -> Option<usize>` | trait 方法数量 |
| `find_stdlib_trait_method` | `(trait_name: &str, method_name: &str) -> Option<&'static StdlibTraitMethod>` | 按 trait + 方法名查询 |
| `is_stdlib_trait_method` | `(trait_name: &str, method_name: &str) -> bool` | trait + 方法是否存在 |
| `stdlib_traits_with_method` | `(method_name: &str) -> Vec<&'static str>` | 反向查询：含某方法的所有 trait |

### 2.3 注册范围（覆盖核心 trait）

- **Markers**（无方法）: Copy, Send, Sync, Sized, Unpin — `Some(&[])` 非空但 0 方法
- **Clone**: `clone(&self) -> Self` / `clone_from(&mut self, source: &Self)`
- **Drop**: `drop(&mut self)`
- **Default**: `default() -> Self` (NoSelf)
- **Display**: `fmt(&self, f: &mut Formatter) -> Result<(), Error>`
- **Debug**: `fmt(&self, f: &mut Formatter) -> Result<(), Error>` (与 Display 共享方法名)
- **PartialEq**: `eq(&self, other: &Self) -> bool` / `ne(&self, other: &Self) -> bool`
- **Eq**: 标记 trait（继承 PartialEq, 无新方法）
- **PartialOrd**: `partial_cmp(&self, other: &Self) -> Option<Ordering>` 等
- **Ord**: `cmp(&self, other: &Self) -> Ordering`
- **Hash**: `hash(&self, state: &mut Hasher)`
- **Add/Sub/Mul/Div/Rem**: `add(self, rhs: Rhs) -> Self::Output` 等
- **AddAssign 等赋值运算**: `add_assign(&mut self, rhs: Rhs)`
- **Deref**: `deref(&self) -> &Self::Target`
- **DerefMut**: `deref_mut(&mut self) -> &mut Self::Target`
- **IntoIterator**: `into_iter(self) -> Self::IntoIter`
- **Iterator**: `next(&mut self) -> Option<Self::Item>`
- **Read**: `read(&mut self, buf: &mut [u8]) -> Result<usize>`
- **Write**: `write(&mut self, buf: &[u8]) -> Result<usize>`

未注册的 trait（Fn/FnMut/FnOnce/From/Into/AsRef 等暂略）返回 `None`。

### 2.4 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `StdlibTraitMethod` | `<Noun><Noun><Noun>` | ✅ |
| `StdlibSelfKind` | `<Noun><Noun><Noun>` | ✅ |
| `stdlib_trait_methods` | `<noun>_<noun>_<noun>`（free fn 查询） | ✅ |
| `stdlib_trait_method_count` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `find_stdlib_trait_method` | `find_<noun>_<noun>_<noun>` | ✅ |
| `is_stdlib_trait_method` | `is_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_traits_with_method` | `<noun>_<noun>_with_<noun>` | ✅ |

字段命名：
- `self_kind` — `<noun>_<noun>` ✅
- `param_count` — `<noun>_<noun>` ✅
- `return_kind` — `<noun>_<noun>` ✅
- `is_unsafe` — `is_<adj>` ✅

### 2.5 §16 接口隔离

`StdlibTraitMethod` 使用 `StdlibTypeKind`（已有，stdlib 内部定义）+ 标量字段，
不引用 `mir::ty`，无循环依赖。`stdlib_trait_methods()` 是纯函数，输入 `&str`，
输出 `Option<&'static [T]>`，可在 driver/typeck/codegen 任一阶段调用。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1106 + 新增 ~12 = ~1118）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_trait_methods_clone` | Clone 2 个方法 |
| `test_stdlib_trait_methods_drop` | Drop 1 个方法 |
| `test_stdlib_trait_methods_default` | Default 1 个 NoSelf 方法 |
| `test_stdlib_trait_methods_display` | Display fmt 方法 |
| `test_stdlib_trait_methods_partial_eq` | PartialEq 2 方法 |
| `test_stdlib_trait_methods_ord` | Ord cmp 方法 |
| `test_stdlib_trait_methods_marker_empty` | Copy/Send/Sync 空数组 |
| `test_stdlib_trait_methods_add` | Add 运算符 |
| `test_stdlib_trait_methods_iterator` | Iterator next |
| `test_stdlib_trait_methods_none` | 未知 trait 返回 None |
| `test_find_stdlib_trait_method_hit` | find Clone::clone |
| `test_find_stdlib_trait_method_miss` | find 不存在方法 |
| `test_is_stdlib_trait_method_true` | Iterator::next |
| `test_is_stdlib_trait_method_false` | Iterator::bogus |
| `test_stdlib_traits_with_method` | 找含 `clone` 的 trait |

## 5. 后续依赖

- **Stage 5.37+（dyn Trait MIR lowering）**: 直接使用 `stdlib_trait_methods()` 生成
  vtable 函数指针类型签名
- **Stage 5.38+（typeck trait bound solving）**: 使用 `find_stdlib_trait_method()`
  校验方法调用是否匹配 trait 接口

---

**创建日期**: 2026-07-23
