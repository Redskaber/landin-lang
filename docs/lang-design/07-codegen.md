# 07 — LLVM IR 生成

> 本文定义 MIR → LLVM IR 的转换规则、ABI、链接策略。Landin 用 **LLVM only** 后端（R2、R3 共识），不自研寄存器分配与 codegen。

---

## 1. 总体流程

```
MIR Body
  ↓
Monomorphization 收集
  ↓
类型 layout 计算
  ↓
MIR → LLVM IR per-function
  ↓
LLVM 优化（O2 / O3）
  ↓
LLVM codegen → 目标文件（.o / .obj）
  ↓
系统链接器（ld / lld / link.exe）
  ↓
可执行文件 / 静态库 / 动态库
```

---

## 2. 类型映射

### 2.1 基本类型

| Landin 类型 | LLVM IR 类型 | LLVM 类型字符串 |
| --- | --- | --- |
| `i8` | i8 | `i8` |
| `i16` | i16 | `i16` |
| `i32` | i32 | `i32` |
| `i64` | i64 | `i64` |
| `i128` | i128 | `i128` |
| `isize` | platform-dependent | `i64` (64-bit) |
| `u8`-`u128` | iN | `i8` ... `i128` |
| `usize` | platform-dependent | `i64` (64-bit) |
| `f32` | float | `float` |
| `f64` | double | `double` |
| `bool` | i8 (with range metadata) | `i8` |
| `char` | i32 | `i32` |
| `()` | `{}` (empty struct) | `{}` |
| `!` | 不映射 | （never type，不直接 codegen） |

### 2.2 复合类型

> **Stage 3.49 (L13 closure)**: `&str` 和 `&[T]` 的 fat pointer 表示
> (`{ ptr, len }`) 已在 Stage 3.49 实现。之前 Stage 3.27/3.28 用 thin
> pointer 简化（L13 debt），现已闭合。`&dyn Trait` 的 vtable fat pointer
> 仍待 Stage 5 trait dispatch 实现时加入。

| Landin 类型 | LLVM IR 类型 |
| --- | --- |
| `&T` | `{ T*, i64 }` 或 `T*`（若 T: Sized） |
| `&mut T` | 同 `&T`，但 metadata 标记 noalias |
| `*const T` / `*mut T` | `T*` |
| `[T; N]` | `[T x N]` |
| `[T]` (slice) | `{ T*, i64 }` (data ptr, length) |
| `str` | `{ i8*, i64 }` (data, byte length) |
| `(T1, T2, ...)` | `{ T1, T2, ... }` |
| `Struct { ... }` | `{ field1_ty, field2_ty, ... }`（按 `#[repr]` 决定布局） |
| `Enum` | `{ iN, [padding], union { variants } }` |
| `dyn Trait` | `{ *mut (), *mut VTable }` (fat pointer) |
| `fn(T) -> U` | `U (T)*` |
| `Box<T>` | `T*`（同裸指针，但带有 `noalias` 标记） |

### 2.3 类型 Layout 计算

```rust
struct LayoutCache {
    cache: HashMap<(TyId, Option<TyParamContext>), Layout>,
}

struct Layout {
    size: Size,           // 字节
    align: AbiAlign,      // ABI 对齐
    fields: FieldsShape,  // 字段布局
    variants: Variants,   // enum variants
    abi: Abi,             // 类型 ABI（Scalar / Aggregate / ...）
}

enum FieldsShape {
    Primitive,
    Union(u32),
    Array { stride: Size, count: u64 },
    Arbitrary { offsets: Vec<Size>, memory_index: Vec<u32> },
}

enum Variants {
    Empty,                // 不可达类型
    Single { index: u32 },
    Multiple {
        tag: Scalar,
        tag_encoding: TagEncoding,
        tag_field: u32,
        variants: Vec<Layout>,
    },
}

enum TagEncoding {
    Direct,
    Niche { untagged_variant: u32, niche_variants: Range<u32>, niche_start: u128 },
}
```

Layout 算法：

1. 计算 struct 字段 layout，按 `#[repr]` 决定
2. 计算 enum：选最小 discriminant type，应用 niche optimization（如 `Option<NonNull<T>>` 用 0 作 None）
3. union：所有字段对齐取最大，大小取最大
4. array：`stride * count`

### 2.4 Niche optimization

利用无效值编码 enum variant：

```landin
enum Option<NonNull<T>> {
    Some(NonNull<T>),    // NonNull 永不为 0
    None,
}
```

→ size = `sizeof(*T)` = 8 字节，无需额外 tag。

常见 niche：

- `NonNull<T>`, `Box<T>`, `&T`, `&mut T`: 0 是无效值
- `bool`: 仅 0/1 合法，2-255 是 niche
- `char`: 仅 0-0x10FFFF 合法
- reference to unsized: length = 0 invalid（某些情况）

---

## 3. 函数签名映射

### 3.1 ABI

Landin 默认 ABI（`extern "Landin"`）在 MVP 阶段 **与 C ABI 完全一致**（简化实现，避免 Rust 那样的多套 calling convention）。这意味着：

- `extern "Landin" fn` 与 `extern "C" fn` 在 MVP 中类型可互换
- v0.2 计划：`extern "Landin"` 可能引入优化（如 `sret` 优化、`nonnull` 标注），届时与 C ABI 分道扬镳

**MVP 限制**：支持 `"Landin"`、`"C"`、`"System"` 三个 ABI（v1.2 修正与 05 文档统一），且 MVP 阶段三者实现等价。`"System"` 在 Windows 上等同于 `"C"`（即 `stdcall`/`vectorcall`，MVP 不做区分，统一用 `"C"`）。

| Landin 函数 | LLVM IR |
| --- | --- |
| `fn f(a: i32, b: i32) -> i32` | `define i32 @f(i32 %a, i32 %b)` |
| `fn f(s: Big)` (Big > 16 bytes) | `define void @f(%Big* sret %s)` (struct return) |
| `fn f(s: Small)` (Small ≤ 16 bytes) | `define i64 @f(i64 %s)` (in registers if possible) |
| `fn f(s: &Big)` | `define void @f(%Big* %s)` |
| `fn f(s: &mut [u8])` | `define void @f(i8* %data, i64 %len)` |

返回值 > 16 字节通过 `sret` 参数返回（隐式第一个参数）。

### 3.2 调用约定

LLVM 调用约定：

- `ccc` (C calling convention): 默认，所有外部 ABI
- `fastcc`: 内部函数优化用（v0.2）

### 3.3 函数属性

```llvm
define void @f(i32* noalias %x) {
    ; noalias: x 不与其他指针别名
    ; dereferenceable(8): x 可解引用 8 字节
    ; readonly: x 通过此指针只读
    ...
}
```

Landin 自动添加的属性：

- `&T` → `readonly noalias dereferenceable(N)`
- `&mut T` → `noalias dereferenceable(N) writeonly`
- `Box<T>` → `noalias dereferenceable(N)`

---

## 4. MIR → LLVM IR 映射

### 4.1 Local 映射

每个 MIR Local 映射为一个 LLVM `alloca`：

```llvm
; MIR: let mut _1: i32;
%i32 %_1 = alloca i32, align 4

; MIR: let mut _2: Big;
%Big %_2 = alloca %Big, align 8
```

LLVM 的 `mem2reg` pass 会把 SSA-able 的 alloca 提升为寄存器，性能无损。

### 4.2 Statement 映射

| MIR Statement | LLVM IR |
| --- | --- |
| `Assign(_1, Use(_2))` | `%tmp = load i32, i32* %_2; store i32 %tmp, i32* %_1` |
| `Assign(_1, BinOp(Add, _2, _3))` | `%a = load i32, i32* %_2; %b = load i32, i32* %_3; %c = add i32 %a, %b; store i32 %c, i32* %_1` |
| `Assign(_1, Ref(_, Shared, _2))` | `store i32* %_2, i32** %_1` |
| `Assign(_1, Ref(_, Mut, _2))` | `store i32* %_2, i32** %_1` |
| `Assign(_1, Aggregate(Tuple, [_2, _3]))` | `%ge0 = getelementptr %tuple, %tuple* %_1, 0, 0; store ... %_2, ... %ge0` |
| `StorageLive(_1)` | （不生成 IR，仅用于分析） |
| `StorageDead(_1)` | （不生成 IR，drop 已在 Drop terminator 中处理） |

### 4.3 Terminator 映射

| MIR Terminator | LLVM IR |
| --- | --- |
| `Goto(bb1)` | `br label %bb1` |
| `SwitchInt(_1, [0: bb1, 1: bb2, else: bb3])` | `%v = load i32, i32* %_1; switch i32 %v, label %bb3 [i32 0, label %bb1; i32 1, label %bb2]` |
| `Call(f, [_1, _2], _3, bb1)` | `%a = load i32, i32* %_1; %b = load i32, i32* %_2; %r = call i32 @f(i32 %a, i32 %b); store i32 %r, i32* %_3; br label %bb1` |
| `Return` | `%ret = load i32, i32* %_0; ret i32 %ret` |
| `Unreachable` | `unreachable` |
| `Drop(_1, bb1)` | `call void @drop_i32(i32* %_1); br label %bb1` |
| `Assert(cond, expected, msg, bb1)` | （debug 模式）`%c = load i1, i1* %cond; %e = icmp eq i1 %c, %expected; br i1 %e, label %bb1, label %panic_drop_<n>` |

### 4.4 Place 投影映射

```llvm
; _1.0 (tuple field)
%gep = getelementptr inbounds { i32, i64 }, { i32, i64 }* %_1, i32 0, i32 0

; _1.field (struct field)
%gep = getelementptr inbounds %Struct, %Struct* %_1, i32 0, i32 <field_index>

; *_1 (deref)
; %_1 已是指针，直接使用 %_1

; _1[i] (array index)
%gep = getelementptr inbounds [10 x i32], [10 x i32]* %_1, i32 0, i64 %i

; _1[index] (slice index — 带 bounds check)
%len = load i64, i64* %_1.len
%cmp = icmp ult i64 %index, %len
br i1 %cmp, label %ok, label %panic_bounds_check
ok:
  %data = load i32*, i32** %_1.data
  %gep = getelementptr inbounds i32, i32* %data, i64 %index
```

### 4.5 panic 调用

```llvm
; Assert 失败 → 调用 panic
call void @__landin_panic_bounds_check(i64 %index, i64 %len) #1
unreachable

; panic 函数属性: noreturn
attributes #1 = { noreturn }
```

### 4.6 OperandValue 4 形态（v1.2 新增，R6 报告要求）

Codegen 时每个 Operand 在 LLVM 层有 4 种表示形态：

```rust
enum OperandValue<'tcx> {
    /// 直接通过指针访问（如大 struct 通过 & 引用传递）
    Ref(llvm::PointerValue<'tcx>),
    
    /// 直接值（如 i32、f64、bool 等标量）
    Immediate(llvm::BasicValueEnum<'tcx>),
    
    /// Pair（fat pointer：&str / &[T] / &dyn Trait / Box<dyn Trait>）
    /// 第一个值是 data pointer，第二个是 metadata（length 或 vtable pointer）
    Pair(llvm::BasicValueEnum<'tcx>, llvm::BasicValueEnum<'tcx>),
    
    /// ZeroSized（如 unit ()、空 struct）
    /// 不分配 LLVM value，调用时跳过参数
    ZeroSized,
}
```

**何时用哪种**：

- `Ref`：类型大小 > 16 字节，或类型 unsized
- `Immediate`：标量类型（i8-i128、f32、f64、bool、char、`*const T`、`*mut T`、`&T` 当 T: Sized 且 ≤ 16 字节）
- `Pair`：fat pointer（`&str`、`&[T]`、`&dyn Trait`、`Box<dyn Trait>`、`Rc<dyn Trait>` 等）
- `ZeroSized`：unit `()`、`PhantomData<T>`、空 struct/enum

### 4.7 FunctionCx 与 Builder 模式（v1.2 新增）

Codegen 模块的核心数据结构：

```rust
struct FunctionCx<'a, 'tcx> {
    /// 当前编译的函数 MIR
    mir: &'tcx Body<'tcx>,
    
    /// 函数的 LLVM value
    llfn: llvm::FunctionValue<'tcx>,
    
    /// 每个 MIR Local 对应的 LLVM value
    local_map: IndexVec<Local, LocalRef<'tcx>>,
    
    /// 每个 BasicBlock 对应的 LLVM basic block
    blocks: IndexVec<BasicBlock, Option<llvm::BasicBlock<'tcx>>>,
    
    /// 当前所在的 MIR block
    current_block: Option<BasicBlock>,
}

enum LocalRef<'tcx> {
    /// 普通地址（alloca 后的指针）
    Place(llvm::PointerValue<'tcx>),
    
    /// Unsized place（data pointer + metadata）
    UnsizedPlace { ptr: llvm::PointerValue<'tcx>, metadata: OperandValue<'tcx> },
    
    /// 已经是 OperandValue（无需 alloca，直接存值）
    Operand(OperandValue<'tcx>),
    
    /// 等待求值的 operand（如函数参数尚未 store）
    PendingOperand(Option<OperandValue<'tcx>>),
}

/// Builder：封装 LLVM builder 调用，提供 MIR-aware 的方法
struct Builder<'a, 'b, 'tcx> {
    cx: &'a mut FunctionCx<'b, 'tcx>,
    llbuilder: llvm::BuilderValue<'tcx>,
}

impl<'a, 'b, 'tcx> Builder<'a, 'b, 'tcx> {
    fn codegen_operand(&mut self, operand: &Operand) -> OperandValue<'tcx> { ... }
    fn codegen_rvalue(&mut self, rvalue: &Rvalue) -> LocalRef<'tcx> { ... }
    fn codegen_terminator(&mut self, term: &Terminator) { ... }
    fn codegen_call(&mut self, func: &Operand, args: &[Operand], dest: &Place) { ... }
    // ...
}
```

每个 MIR basic block 对应一个 LLVM basic block，codegen 按 RPO 顺序处理。

panic 实现：

- `__landin_panic_bounds_check(index, len)` — 打印 "index out of bounds: the len is N but index is M" 后 abort
- `__landin_panic_overflow(op, lhs, rhs)` — 打印溢出信息后 abort
- `__landin_panic_div_by_zero()` — 打印 "division by zero" 后 abort

MVP 全部 panic 直接 `abort()`，不做 unwind。

---

## 5. 内存分配

### 5.1 Allocator API

```landin
pub unsafe trait Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
    // v0.2: allocate_zeroed, grow, shrink, ...
}

pub struct Global;
unsafe impl Allocator for Global { ... }
```

### 5.2 默认 allocator

MVP 链接 libc 的 `malloc` / `free`：

```llvm
; Vec::push 调用 allocator
%ptr = call i8* @malloc(i64 %size)
%cmp = icmp eq i8* %ptr, null
br i1 %cmp, label %oom, label %ok
ok:
  ...
oom:
  call void @__landin_oom_abort()
  unreachable
```

### 5.3 自定义 allocator

通过 `#[global_allocator]` 属性指定：

```landin
#[global_allocator]
static ALLOC: MyAllocator = MyAllocator;
```

编译器在 `__landin_alloc` / `__landin_dealloc` 调用中替换为用户的 allocator 方法。

---

## 6. Drop glue

### 6.1 Drop 实现

每个用户类型若 impl Drop 或含需要 drop 的字段，编译器生成一个 **drop glue function**：

```llvm
; 定义
define void @"drop_<Adt>"(%Adt* %self) {
    call void @"<Adt>::drop"(%Adt* %self)     ; user impl Drop
    ; 然后逐字段 drop
    call void @"drop_<FieldType1>"(%FieldType1* %field1_ptr)
    call void @"drop_<FieldType2>"(%FieldType2* %field2_ptr)
    ret void
}

; 类型无 Drop impl 时生成 thin drop glue
define void @"drop_<SimpleType>"(%SimpleType* %self) {
    ret void    ; nothing to do
}
```

### 6.2 Drop 调用

MIR 的 `Drop(_1, bb1)` terminator 在 codegen 时：

1. 检查类型是否需要 drop（编译期已知）
2. 若需要，调用 drop glue
3. 跳转 bb1

需要 drop 的判断：类型 impl Drop，或任何字段需要 drop。递归判断到 primitive（不需要）。

### 6.3 Drop in-place

drop glue 接收 `*mut Self`，原地析构，不释放底层内存（释放由 Box/Vec 等 container 负责）。

---

## 7. Trait object vtable

### 7.1 vtable layout

```llvm
%VTable = type {
    i64,                    ; drop function pointer
    i64,                    ; size
    i64,                    ; align
    i64,                    ; reserved
    i64,                    ; method_1 pointer
    i64,                    ; method_2 pointer
    ...
}
```

### 7.2 vtable 生成

```landin
impl Display for Foo { fn fmt(&self, ...) -> ... { ... } }
```

编译器生成：

```llvm
@vtable_Foo_Display = constant %VTable {
    i64 ptrtoint (void (%Foo*)* @"drop_Foo" to i64),
    i64 24,    ; sizeof(Foo)
    i64 8,     ; alignof(Foo)
    i64 0,     ; reserved
    i64 ptrtoint (i32 (%Foo*, %Formatter*)* @"<Foo as Display>::fmt" to i64)
}
```

### 7.3 dyn 调用

```landin
let x: &dyn Display = &foo;
x.fmt(f);
```

→

```llvm
%data = ...   ; i8*
%vtable = ... ; %VTable*
%fmt_ptr = getelementptr %VTable, %VTable* %vtable, i32 0, i32 4
%fmt = load i64, i64* %fmt_ptr
%fmt_fn = inttoptr i64 %fmt to i32 (i8*, %Formatter*)*
%result = call i32 %fmt_fn(i8* %data, %Formatter* %f)
```

---

## 8. 闭包 codegen

### 8.1 闭包类型

每个闭包字面量生成一个唯一的匿名 struct：

```landin
let f = |a: i32| a + outer;
```

→

```landin
struct Closure<'a> {
    outer: &'a i32,
}

impl<'a> Fn<(i32,)> for Closure<'a> {
    extern "Landin" fn call(&self, a: i32) -> i32 {
        a + *self.outer
    }
}
```

闭包类型在 typeck 阶段确定，每个调用点唯一。

### 8.2 闭包调用 codegen

```llvm
; let f = |a| a + outer;
%closure = alloca %Closure_type
%outer_gep = getelementptr %Closure_type, %Closure_type* %closure, 0, 0
store i32* %outer, i32** %outer_gep

; f(42)
%result = call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)
```

---

## 9. Monomorphization 与 codegen

### 9.1 Mono item 收集

```rust
fn collect_mono_items(cxt: &mut CollectionCtxt) {
    let mut worklist = VecDeque::new();
    
    // 从 entry point 开始
    worklist.push_back(MonoItem::Fn(entry_fn));
    
    while let Some(item) = worklist.pop_front() {
        if !cxt.seen.insert(item.clone()) {
            continue;
        }
        match item {
            MonoItem::Fn(def_id, args) => {
                let body = cxt.monomorphize(def_id, &args);
                for callee in body.callees() {
                    worklist.push_back(callee);
                }
                cxt.emit_function(body);
            }
            MonoItem::Static(def_id) => { ... }
            MonoItem::GlobalAsm(node_id) => { ... }
        }
    }
}
```

### 9.2 Mangling

```
fn ::foo<T: Display>(x: T) with T = i32
→ _LND3foo3fooE3i32E

fn ::bar<U>(x: U, y: &str) with U = String
→ _LND3bar3barE6StringE
```

Mangling 规则：

- 前缀 `_LND`
- Path: `<len><name>` 递归
- 类型参数：`E` 分隔
- 生命周期参数：在类型参数后

### 9.3 重复实例去重

若多个 crate 都实例化了 `Vec<i32>::push`，去重为一份。Linker 通过 weak symbol 处理。

---

## 10. 链接

### 10.1 链接器调用

```bash
# Linux 默认
ld -dynamic-linker /lib64/ld-linux-x86-64.so.2 -pie \
   -o my_program \
   /usr/lib/crt1.o /usr/lib/crti.o \
   my_program.o \
   -llandin_runtime -lc \
   /usr/lib/crtn.o

# macOS
ld -platform_version macos 13.0 13.0 \
   -o my_program \
   my_program.o \
   -llandin_runtime -lSystem

# Windows (lld-link)
lld-link /subsystem:console \
   /out:my_program.exe \
   my_program.obj \
   landin_runtime.lib libcmt.lib
```

### 10.2 Landin runtime

`liblandin_runtime` 提供：

- panic 函数（abort）
- allocator fallback
- type name 函数（用于 panic 信息）
- stack overflow 检测（v0.2）
- backtrace（v0.2）

MVP runtime 极小（< 5 KB）。

### 10.3 LTO

MVP 不做 LTO，每个 crate 编译为独立 `.o`/`.obj`，链接器常规链接。

v0.2 加 thin LTO（跨 crate 内联）。

---

## 11. 目标平台

### 11.1 MVP 支持的目标

| Target triple | 平台 |
| --- | --- |
| `x86_64-unknown-linux-gnu` | Linux x86-64 (glibc) |
| `aarch64-unknown-linux-gnu` | Linux ARM64 |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows MSVC |

### 11.2 v0.2 目标

- `x86_64-unknown-linux-musl`（静态链接）
- `wasm32-unknown-unknown`（WebAssembly，无 std）
- `wasm32-wasi`（WebAssembly + WASI）

### 11.3 交叉编译

Landin 通过 `--target <triple>` 指定目标。LLVM 后端原生支持交叉编译，只需目标平台的 sysroot。

---

## 12. 优化级别

### 12.1 优化等级

| Landin 等级 | LLVM 等级 | 说明 |
| --- | --- | --- |
| `-O0` (default dev) | -O0 | 无优化，最快编译 |
| `-O1` (default release) | -O2 | 常规优化 |
| `-O2` | -O2 + inline_threshold=275 | 更激进 inline |
| `-O3` | -O3 | 最高优化（可能增加代码体积） |
| `-Os` | -Os | 优化大小 |
| `-Oz` | -Oz | 最小体积 |

### 12.2 Debug 信息

`-g` 启用 DWARF debug info（Linux/macOS）或 PDB（Windows）。Landin 自动生成：

- 类型定义
- 局部变量位置
- 行号信息
- 宏展开信息（v0.2）

支持 `gdb` / `lldb` 调试。

---

## 13. ABI 兼容性

### 13.1 与 C 的 ABI 兼容

`#[repr(C)]` struct 与 C struct 完全一致：

```landin
#[repr(C)]
struct Point { x: f64, y: f64 }
// sizeof = 16, alignof = 8
// 与 C: struct Point { double x, y; } 完全一致
```

### 13.2 不与 C++ 兼容

MVP 不支持 C++ ABI（如 name mangling、vtable 布局、异常）。需通过 C wrapper 中转。

### 13.3 与 Rust 互操作

MVP 不支持。v0.2 通过 `extern "Rust"` ABI 支持有限互操作（要求 Rust 端也用 `repr(C)` 暴露）。

---

## 14. 实现扩展（v0.13.0，§25.8 回写）

> 本节由 Stage 6.11 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。
> 设计文档 §1-§13 描述了 MIR → LLVM IR 的核心转换规则，但 Stage 5.40-5.80
> 实现了**动态派发（dyn Trait）的完整 codegen** 子系统，设计文档未覆盖。
> 本节补写设计意图（实现细节归 `docs/develop/v0/stage-5/dev-log.md`）。

### 14.1 Trait dispatch codegen 子系统（Stage 5.40-5.80 实现，B4 补写）

#### 14.1.1 设计目标

支持 `dyn Trait` 类型的动态派发，符合 Landin "拒绝语言层特判" 哲学
（00-overview.md §1.3）：`dyn Trait` 不是编译器特判类型，而是通过
**vtable + fat pointer** 机制实现的普通类型。

#### 14.1.2 数据结构

| 数据结构 | 设计意图 | 实现位置 |
|---------|---------|---------|
| `vtable global` | 每个 `impl Trait for Type` 生成一个 LLVM global，前 3 个 slot 是 size/align/drop，后续是 method 指针 | `codegen/trait_dispatch.rs::emit_vtable_global` |
| `dynptr global` | 每个 `dyn Trait` 类型对应一个 fat pointer 全局变量，指向其 vtable + instance | `codegen/trait_dispatch.rs::emit_dynptr_global` |
| `DynTraitFatPtr` (MIR) | fat pointer 的 MIR 表示（vtable_ptr + data_ptr 两字段 struct） | `mir/dyn_trait.rs` |
| `DynTraitMethodCall` (MIR) | 一次 dyn Trait 方法调用的元信息（trait/type/method/slot/param_count/return_kind/param_kinds） | `mir/dyn_trait.rs` |
| `DynTraitMIRPlan` | 一个 MIR body 内所有 dyn Trait 调用的预计算 plan | `mir/dyn_trait.rs` |
| `MirBody.dyn_trait_calls` | side-table，索引由 `Terminator::Call` 的 marker const 携带 | `mir/body.rs` |

#### 14.1.3 转换规则

**`dyn Trait` 类型 → LLVM IR**：

```
dyn Trait  →  { ptr, ptr }   // { vtable_ptr, data_ptr }
```

**`receiver.method(args)` dyn Trait 调用 → LLVM IR**：

```llvm
; 1. Load vtable pointer from fat ptr
%vtable = load ptr, ptr %fat_ptr_gep0
; 2. Load data pointer from fat ptr
%data   = load ptr, ptr %fat_ptr_gep8
; 3. Load method function pointer from vtable slot
%method = load ptr, ptr %vtable_slot_N
; 4. Indirect call
%result = call i32 %method(ptr %data, ...)
```

**`DynTraitFatPtr` 常量 → LLVM IR**：codegen 发出一个 `{ vtable_global, data_global }`
constant global，把 vtable 引用 + 数据实例引用打包。

#### 14.1.4 §16 合规性

所有 dyn Trait codegen 信息都通过 MIR 数据结构传递：
- driver 阶段从 `TraitResolver` 构造 `DynTraitMIRPlan`
- MIR lower 把 plan 信息 sunk 到 `MirBody.dyn_trait_calls` side-table
- codegen 只读 MIR（含 side-table），不查 HIR / TraitResolver

数据流单向：`TraitResolver → DynTraitMIRPlan → MirBody → codegen → LLVM IR`。

#### 14.1.5 设计参考

| 来源 | 借鉴点 | Landin 调整 |
|------|--------|------------|
| rustc `Ty::Dynamic` + `TraitObject` | fat pointer 双字段布局 | 一致 |
| rustc `VirtualCall` terminator | 在 MIR 层显式标记 dyn 调用 | Landin 用 `Const{ty:Error, val:Int(index)}` marker 替代（无新增 terminator variant） |
| rustc vtable layout | size/align/drop 前 3 slot + method 顺序按 trait def 顺序 | 一致 |
| C++ Itanium ABI | vtable 是 global，不在对象内 | 一致 |

**设计理由**：用 marker const + side-table 而非新增 `TerminatorKind::VirtualCall`
variant，是因为 Landin MIR 的 `Terminator::Call` 已有足够字段承载调用语义，
新增 variant 会增加所有 pattern match 点的维护成本。marker const 是零侵入方案。

### 14.2 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B4（Trait dispatch codegen 子系统补写） | 已在 §14.1 补写 | — |
| B1（`#[repr(C)]` 完整实现、`extern "Rust"` ABI） | v0.2 | MVP 不需要 C 互操作 |

### 14.3 未实现项（B1，纳入 v0.2+）

| 设计章节 | 未实现项 | 计划阶段 |
|---------|---------|---------|
| §6 控制流 | `unwind` LLVM 属性 | v0.2 |
| §7 函数调用 ABI | `extern "C"` 严格 ABI | v0.2 |
| §9 全局变量 | thread-local storage | v0.2 |
| §10 链接策略 | 静态库 / 动态库链接 | v0.2 |
| §11 调试信息 | DWARF 完整字段 | v0.2 |
| §13 ABI 兼容性 | C++ / Rust 互操作 | v0.2+ |

---

**下一文档**: [`08-bootstrap-strategy.md`](./08-bootstrap-strategy.md) — 自举策略

---

## 15. Stage 8 实现状态更新（v0.15.4，§25.8 回写）

> 本节由 Stage 8.6 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 15.1 v0.2 特性实现状态

| 设计 § | 特性 | Stage 7 状态 | Stage 8 状态 | 实现 |
|--------|------|-------------|-------------|------|
| §7 函数调用 ABI | extern "C" ABI | ❌ B1 | ✅ (8.3) | `BodyMeta.abi` + `codegen_function` abi 参数 |
| §6 控制流 | unwind | ❌ B1 | ❌ 未实现 | v0.3+ (需要 unwind terminator) |
| §9 全局变量 | thread-local | ❌ B1 | ❌ 未实现 | v0.3+ |
| §10 链接策略 | 静态/动态库 | ❌ B1 | ❌ 未实现 | v0.3+ |
| §11 调试信息 | DWARF 完整 | ❌ B1 | ❌ 未实现 | v0.3+ |
| §13 ABI 兼容性 | C++/Rust 互操作 | ❌ B1 | ❌ 未实现 | v0.3+ |

### 15.2 偏差处理计划更新

| 偏差 | Stage 7 计划 | Stage 8 更新 |
|------|-------------|-------------|
| B1（extern "C" ABI） | v0.2 | ✅ **已实现** (8.3, MVP: Landin/C 同 CC) |
| B1（unwind） | v0.2 | v0.3+ (推迟) |
| B1（thread-local / 链接 / DWARF / 互操作） | v0.2 | v0.3+ (推迟) |

**关键变化**: extern "C" ABI 已实现。ABI 信息从 HIR → driver → codegen 全管线跟踪。
MVP 中 Landin ABI 和 C ABI 使用相同 LLVM 调用约定，未来可添加自定义 CC。

---

## 16. Emitter trait 架构（v0.263.0 §25.8 回写）

> **回写触发**: Stage 16.76 + 16.77 完成 codegen pipeline 重构后，按 §14.8 阶段末尾设计回写协议补写本节。本节描述 Stage 16.76-16.77 引入的 6 子 trait 拆分 + backend 文件组织架构。

### 16.1 6 子 trait 拆分（Stage 16.76 MUV-1）

Stage 16.76 MUV-1 把 39-method `Emitter` trait 拆分为 6 个单一职责子 trait（per §13.4 J2）：

| Sub-trait | 方法数 | 职责 |
|-----------|--------|------|
| `ModuleEmitter` | 5 | module-level globals & declarations |
| `FunctionEmitter` | 8 | function scope & control flow |
| `ArithmeticEmitter` | 11 | value computation from operands |
| `MemoryEmitter` | 6 | stack allocation & pointer arithmetic |
| `AggregateEmitter` | 5 | aggregate construction & calls |
| `LocalStateEmitter` | 4 | local value/pointer mapping |
| **Total** | **39** | (matches original `Emitter` trait) |

`Emitter` 是 super-trait，通过 blanket impl 自动为实现了全部 6 子 trait 的类型提供实现：

```rust
pub trait Emitter: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
                 + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}
impl<T> Emitter for T where
    T: ModuleEmitter + FunctionEmitter + ArithmeticEmitter
     + MemoryEmitter + AggregateEmitter + LocalStateEmitter {}
```

**设计理由**：
- §13.4 J2 单一职责：原 39-method trait 是"fat trait"，拆分后每个子 trait 可用一句话描述职责
- 未来扩展：添加第三 backend 时，各子 trait 独立演进
- 测试隔离：每个子 trait 可独立做 compile-time trait satisfaction 测试

**Breaking change**: 外部 `Emitter` 实现者需改为实现 6 子 trait。Blanket impl 保证 `dyn Emitter` 对调用者兼容（20+ 调用点未改）。

### 16.2 Backend 文件组织（Stage 16.77 MUV-1/2）

Stage 16.77 把每个 backend 的 6 个 impl 块拆分到独立文件：

```
src/codegen/llvm/
├── mod.rs          — LLVMSysEmitter struct + new() + Drop + public API (to_module, to_object_file, set_fn_sigs)
├── module.rs       — impl ModuleEmitter for LLVMSysEmitter (5 methods)
├── function.rs     — impl FunctionEmitter for LLVMSysEmitter (8 methods)
├── arithmetic.rs   — impl ArithmeticEmitter for LLVMSysEmitter (11 methods)
├── memory.rs       — impl MemoryEmitter for LLVMSysEmitter (6 methods)
├── aggregate.rs    — impl AggregateEmitter for LLVMSysEmitter (5 methods)
├── local_state.rs  — impl LocalStateEmitter for LLVMSysEmitter (4 methods)
├── helpers.rs      — 私有 helpers (cstr, is_float, parse_*, collect_cstring)
├── function_sigs.rs — build_fn_sigs_map (LLVM-only, forward-reference resolution)
└── tests.rs        — 单元测试
```

text backend 同结构（无 helpers.rs 和 function_sigs.rs）：

```
src/codegen/text/
├── mod.rs          — TextEmitter struct + new() + output_with_globals + helpers (emit_type_to_llvm_str, binop_to_llvm_str)
├── module.rs       — impl ModuleEmitter for TextEmitter
├── function.rs     — impl FunctionEmitter for TextEmitter
├── arithmetic.rs   — impl ArithmeticEmitter for TextEmitter
├── memory.rs       — impl MemoryEmitter for TextEmitter
├── aggregate.rs    — impl AggregateEmitter for TextEmitter
└── local_state.rs  — impl LocalStateEmitter for TextEmitter
```

**设计理由**：
- §13.4 J2 单一职责：每个文件承担一个 impl 块或 struct 定义
- §13.4 J6 科学合理粒度：最大文件 arithmetic.rs ~420 LOC（远低于 1500 阈值）
- 可维护性：修改一个子 trait 实现 only 影响对应文件
- 可测试性：每个 impl 块可独立做 trait satisfaction 测试

### 16.3 dyn Emitter 兼容性

20+ 调用点使用 `&mut dyn Emitter`，super-trait + blanket impl 模式保证 dyn 兼容性。所有子 trait 都是 object-safe（方法均为 `&mut self`/`&self`，返回 `EmitValue`/`Option<&EmitValue>`/tuple，无泛型方法）。

### 16.4 共享翻译层（mir_translation/）

Stage 16.76 MUV-3 把 1144-LOC `mir_translation.rs` 拆分为 4 子模块，按本节 §2-§4 章节对齐：

| 子模块 | 对应章节 | 内容 |
|--------|---------|------|
| `types.rs` | §2.1-§2.3 | `mir_type_to_emit_type_with_layouts[_and_mono]` |
| `layouts.rs` | §2.3-§2.4 | `adt_layout_to_emit_type` |
| `places.rs` | §4.4 | 7 个 place codegen 函数 |
| `stdlib.rs` | 跨章节 | `stdlib_type_kind_to_emit_type` |

### 16.5 历史背景

- **Stage 3.21**: `Emitter` trait 引入（36 methods, 单 trait, 单 impl TextEmitter）
- **Stage 3.59**: Issue #5 标记 trait bloat（36 methods，deferred decomposition）
- **Stage 13.5**: 第二 backend `LLVMSysEmitter` 加入 → 39 methods, 2 impls
- **Stage 16.38**: 尝试 2-trait 拆分（ModuleEmitter + FunctionEmitter）— 因 ~1000 LOC 跨文件迁移风险被 defer，留下 documentation groups 妥协方案
- **Stage 16.76 MUV-1**: 6-trait 拆分执行完成（39 methods → 5+8+11+6+5+4），blanket impl 保 dyn 兼容
- **Stage 16.76 MUV-2**: `mod.rs` 拆分为 pipeline/function/drop_glue/llvm/function_sigs
- **Stage 16.76 MUV-3**: `mir_translation.rs` 拆分为 types/layouts/places/stdlib
- **Stage 16.77 MUV-1**: `llvm/mod.rs` 拆分为 8 文件（6 impl 块 + helpers + tests）
- **Stage 16.77 MUV-2**: `text/mod.rs` 拆分为 7 文件（6 impl 块）

### 16.6 偏差处理

无偏差。本节描述的实现与 §2-§4 设计文档完全一致，6 子 trait 拆分 + backend 文件组织是 §13.4 J2 单一职责原则在 codegen 模块的具体应用。
