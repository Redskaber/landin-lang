# 03 — 类型系统

> 本文定义 Landin 的类型系统：类型分类、trait、impl、泛型、类型推导算法、coherence、monomorphization。算法设计基于 R3 报告的 constraint-based inference（Odersky-Wadler-Wehr 1995），避免 Algorithm W + subtyping 的不终止陷阱。

---

## 1. 类型分类

### 1.1 类型层次

```
Type
├── Primitive
│   ├── Bool
│   ├── Char
│   ├── Int (i8/i16/i32/i64/i128/isize)
│   ├── Uint (u8/u16/u32/u64/u128/usize)
│   ├── Float (f32/f64)
│   └── Never (!)                    // 不可能发生的类型
├── Reference
│   ├── SharedRef (&'a T)
│   └── MutRef (&'a mut T)
├── Pointer (unsafe)
│   ├── ConstPtr (*const T)
│   └── MutPtr (*mut T)
├── Aggregate
│   ├── Tuple ((T1, T2, ...))
│   ├── Array ([T; N])               // N: const usize
│   └── Slice ([T])
├── User-Defined
│   ├── Struct (Struct<T1, ..., Tn>)
│   ├── Enum (Enum<T1, ..., Tn>)
│   └── TypeAlias (Alias<T1, ..., Tn> = T)
├── Function
│   ├── Fn (fn(T1, ..., Tn) -> U)    // 函数指针
│   └── Closure (impl Fn/FnMut/FnOnce)
├── TraitObject (dyn Trait + 'a)
├── ImplTrait (impl Trait)            // v0.2: in return position only
├── Param (T)                         // 泛型参数
└── InferenceVar (?N)                 // 推导变量
```

### 1.2 Sized 与 Unsized

每个类型有一个 **Sizedness** 属性：

- **Sized**: 编译期已知大小，可存放在栈上、作为值传递
- **Unsized**: 编译期未知大小，只能通过 `&T` 引用

| 类型 | Sized? |
| --- | --- |
| 所有 primitive | ✅ |
| `Struct` (所有字段 Sized) | ✅ |
| `Enum` (所有 variant Sized) | ✅ |
| `[T; N]` | ✅ |
| `&T`、`&mut T`、`*const T`、`*mut T` | ✅ |
| `dyn Trait` | ❌ |
| `[T]` (slice) | ❌ |
| `str` (UTF-8 字符串切片) | ❌ |
| `Struct` 末尾的 unsized 字段 | ❌（v0.2） |

所有泛型参数默认 `T: Sized`，需要 unsized 时用 `T: ?Sized`（v1.2 修正：MVP 部分支持 `?Sized`，用于 Deref Target / Box/Vec 内部，详见 13 §2.1）。

### 1.3 类型相等性

两个类型 `T` 与 `U` **结构等价** 当且仅当：

- 同 primitive
- 同 reference 类型且 inner type 与 lifetime 等价
- 同 aggregate 且所有分量等价
- 同 user-defined 路径且类型参数等价
- 同 trait object 且 trait 与 lifetime 等价

类型别名（`type`）透明，结构等价时展开。

---

## 2. Trait 系统

### 2.1 Trait 定义

Trait 是一组方法的 **接口契约**，可包含：

```landin
pub trait Display {
    // required method（无默认实现）
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error>;
}

pub trait Iterator {
    // associated type
    type Item;
    
    // required method
    fn next(&mut self) -> Option<Self::Item>;
    
    // provided method（有默认实现）
    fn count(mut self) -> usize
    where Self: Sized
    {
        let mut n = 0;
        while self.next().is_some() { n += 1; }
        n
    }
    
    // associated const
    const NAME: &'static str;
}
```

### 2.2 Trait bound

Trait bound 是对泛型参数的约束：

```landin
// 单一 bound
fn f<T: Clone>(x: T) { ... }

// 多 bound
fn f<T: Clone + Ord + 'static>(x: T) { ... }

// where 子句（更灵活）
fn f<T, U>(x: T, y: U)
where
    T: Clone + Ord,
    U: Iterator<Item = T>,
{ ... }

// lifetime bound
fn f<'a, T: 'a>(x: &'a T) { ... }
```

`Iterator<Item = T>` 是 **associated type bound**，要求 `U::Item = T`。

### 2.3 Trait object

`dyn Trait` 是动态分发的 trait object，类型为 unsized：

```landin
let x: dyn Display = ...;       // 错误：dyn 是 unsized，不能直接放栈
let x: &dyn Display = ...;       // 正确：通过引用持有
let x: Box<dyn Display> = ...;   // 正确：堆分配
```

Trait object 的内部表示：

```
dyn Trait = (data_ptr: *mut (), vtable: *const VTable)
VTable = {
    drop: fn(*mut ()),
    size: usize,
    align: usize,
    method_1: fn(...),
    method_2: fn(...),
    ...
}
```

**Object safety** 规则（参考 Rust RFC #255）：一个 trait 可作 `dyn Trait` 当且仅当：

- 所有 method 的 receiver 是 `&self`、`&mut self`、`Box<Self>` 或 `Rc<Self>`/`Arc<Self>`（v0.2）
- 所有 method 不返回 `Self`
- 所有 method 不含泛型参数（含泛型参数的方法调用时无法静态分发）
- trait 不含 associated const（v0.2 限制）

不满足 object safety 的 trait 仍可被 impl，但不能作 `dyn Trait`。

### 2.4 Impl Trait

`impl Trait` 在两个位置使用：

- **参数位置**: `fn f(x: impl Display)` — 等价于 `fn f<T: Display>(x: T)`（语法糖）
- **返回位置**: `fn f() -> impl Display` — 表示"返回某个 impl 了 Display 的类型，但具体类型由编译器决定"（v0.2 加，MVP 不支持）

MVP 阶段支持参数位置 `impl Trait`，不支持返回位置（避免 existential type 复杂性）。

### 2.5 Marker trait

Marker trait 是无方法的 trait，仅用于标记类型属性：

```landin
pub trait Copy: Clone {}    // marker，编译器自动 impl 给所有满足的类型
pub trait Sized {}           // marker，编译器自动 impl 给所有 Sized 类型
pub trait Send {}            // v0.2: 自动 impl 给所有 Send 字段类型
pub trait Sync {}            // v0.2
pub trait Unpin {}           // v0.2
```

Marker trait 的 impl 由编译器自动派生（auto trait），用户也可手动 `impl !Send for MyType {}` 取消（v0.2）。

---

## 3. 泛型与 Monomorphization

### 3.1 泛型声明

```landin
// 泛型函数
fn map<T, U>(xs: Vec<T>, f: impl Fn(T) -> U) -> Vec<U> { ... }

// 泛型 struct
struct Pair<A, B> { a: A, b: B }

// 泛型 enum
enum Result<T, E> { Ok(T), Err(E) }

// 泛型 trait
trait FromIterator<T> {
    fn from_iter<I: IntoIterator<Item = T>>(it: I) -> Self;
}

// 默认类型参数（v0.2）
struct Foo<T = i32> { x: T }
```

### 3.2 Monomorphization 流程

Landin 的泛型采用 **monomorphization**（静态分发），流程：

1. **类型推导** 完成后，扫描所有 (函数, 实参类型) 对的集合 `MonoSet`
2. 对 `MonoSet` 中每个 `(F, T)`，若 `F<T>` 尚未生成，生成一份 F 的副本，类型参数替换为 T，加入工作队列
3. 递归处理新副本中调用的泛型函数
4. 直到 fixpoint（无新 (F, T) 对加入）
5. 对每个 `(F, T)`，emit LLVM IR 时生成一份 `F_<T_mangled>` 函数

### 3.3 Name mangling

Mangled name 编码 (函数路径, 类型参数, 生命周期)，用于 linker 区分：

```
Landin mangling: _LND <path> E <type_args> E <lifetime_args> E
示例: _LND3vec3mapE2Ti3u32E1aE
       → Vec::<T, u32>::map with lifetime 'a
```

Mangling 规则：

- 前缀 `_LND`
- Path 用 `<len><name>` 编码（Itanium 风格）
- 类型参数与生命周期参数分别编码
- 可 demangle（调试用）

### 3.4 代码膨胀控制

MVP 不做共享 specialization（R3 报告"shared vspecialization"），但有 **保守阈值**：

- 编译器统计每个泛型函数的实例数
- 超过 64 个实例时打印警告（建议重构）
- 超过 1024 个实例时报错（防止恶意代码膨胀）

---

## 4. 类型推导

### 4.1 整体策略

Landin 采用 **constraint-based type inference**（Odersky-Wadler-Wehr 1995），流程：

1. **Type collection**: 收集所有显式类型注解（函数签名、struct 字段、let 注解）
2. **Constraint generation**: 遍历 AST，为每个表达式分配 inference variable，生成 constraint
3. **Constraint solving**: 求解 constraint 系统，可能触发 trait resolution
4. **Finalization**: 把所有 inference variable 解为具体类型；若有未解变量，报"type annotations needed"

### 4.2 Inference variable

```rust
enum Ty {
    // 已知类型
    Bool, Int(IntTy), Uint(UintTy), Float(FloatTy), Char, Never,
    Ref(Region, Ty, Mutability),
    Ptr(Ty, Mutability),
    Tuple(Vec<Ty>),
    Array(Ty, ConstUsize),
    Slice(Ty),
    Adt(DefId, Vec<GenericArg>),
    FnDef(DefId, Vec<GenericArg>),
    FnPtr(Vec<Ty>, Ty),
    Closure(DefId, Vec<GenericArg>),
    Dynamic(Vec<Binder<TraitPredicate>>, Region),
    Param(ParamTy),
    // 推导变量
    Infer(InferVar),
}

struct InferVar(u32);
```

InferenceVar 是一个唯一 ID，关联一个 **pending constraint set** 与 **可能的解**。

### 4.3 Constraint 类型

```rust
enum Constraint {
    // 类型相等
    Eq(Ty, Ty),
    // 子类型关系（用于 lifetime）
    Subtype(Ty, Ty),
    // trait 满足
    Trait { ty: Ty, trait_def: DefId, args: Vec<GenericArg> },
    // lifetime 包含
    Outlives(Region, Region),
    // 类型大小
    Sized(Ty),
}
```

### 4.4 Constraint 生成规则

主要规则：

| 表达式 | 生成的 constraint |
| --- | --- |
| `let x: T = e;` | `typeof(e) = T` |
| `let x = e;` | `typeof(x) := fresh InferVar; typeof(e) = typeof(x)` |
| `f(e1, ..., en)` | `typeof(f) = fn(T1, ..., Tn) -> U; typeof(ei) <: Ti; result = U` |
| `e1 + e2` | `typeof(e1) = T; typeof(e2) = T; T: Add<Output = U>; result = U` |
| `e.method(args)` | `typeof(e): HasMethod(method); apply method signature` |
| `&'a e` | `typeof(e) = T; result = &'a T` |
| `&mut 'a e` | `typeof(e) = T; result = &'a mut T;`（要求 e 是 place） |
| `*e` | `typeof(e) = *T or &T or &mut T; result = T` |
| `e as T` | `typeof(e): AllowedToCastTo(T)` |
| `e?` | `typeof(e) = Result<T, E>; result = T; enclosing fn returns Result<_, E2>; E2: From<E>` |
| `match e { p1 => e1, ... }` | `typeof(e) = T; p1: T, p2: T, ...; typeof(ei) = U; result = U` |

### 4.5 Unification 算法（v1.2 重写为真正的 constraint-based）

v1.1 之前的 `unify` 伪代码实为 Algorithm W 的 Robinson unification（R7 报告指出）。v1.2 重写为真正的 **constraint-based unification**：constraint 不立即求解，而是加入 constraint queue，最后批量求解。

```
struct TypeChecker {
    constraints: Vec<Constraint>,
    infer_var_solutions: HashMap<InferVar, Ty>,
}

enum Constraint {
    Eq(Ty, Ty),                       // t1 = t2
    Subtype(Ty, Ty),                  // t1 <: t2 (for lifetime variance)
    Trait { ty: Ty, trait_def: DefId, args: Vec<GenericArg> },
    Outlives(Region, Region),
    Sized(Ty),
}

// 生成 constraint，不立即求解
gen_constraint(t1, t2):
    self.constraints.push(Constraint::Eq(t1, t2))

// 批量求解
solve():
    while let Some(c) = self.constraints.pop():
        match c:
            Eq(t1, t2) => unify_one(t1, t2)
            Subtype(t1, t2) => unify_subtype(t1, t2)
            Trait { .. } => fulfillment_queue.push(c)
            Outlives(r1, r2) => region_constraint.add(r1, r2)
            Sized(t) => check_sized(t)

// 单步 unify（不递归，只生成新 constraint）
unify_one(t1, t2):
    if t1 == t2: return Ok
    if t1 is Infer(v1):
        if v1 has solution s1: gen_constraint(s1, t2); return
        if t2 contains v1: return Err(occurs check)
        v1.solution := t2; return Ok
    if t2 is Infer(v2): return unify_one(t2, t1)
    if t1 is Ref(r1, ty1, m1) and t2 is Ref(r2, ty2, m2):
        if m1 != m2: return Err
        gen_constraint(ty1, ty2)              // 子约束，不递归
        gen_subtype(r1, r2)                  // lifetime 子类型约束
        return Ok
    if t1 is Adt(d1, args1) and t2 is Adt(d2, args2):
        if d1 != d2: return Err
        for (a1, a2) in zip(args1, args2): gen_constraint(a1, a2)
        return Ok
    if t1 is Tuple(elems1) and t2 is Tuple(elems2):
        if len(elems1) != len(elems2): return Err
        for (a, b) in zip(elems1, elems2): gen_constraint(a, b)
        return Ok
    return Err(types do not unify)
```

**与 Algorithm W 的差异**：

- Algorithm W 递归立即求解，深度可能指数爆炸
- Constraint-based 把 constraint 加入队列，可批量优化（如优先解 simple constraint）
- Constraint-based 更易扩展 subtyping（gen_subtype 与 gen_constraint 分离）
- 参考 Odersky-Wadler-Wehr 1995 "A Second Look at Overloading"

**Occurs check**: 防止构造无限类型（如 `T = List<T>`）。

### 4.6 整数 fallback（v1.2 修正：避免与 trait selection 交互产生 unsound）

当 inference variable 没有任何约束时，最后 fallback 到 `i32`（参考 Rust 1.0 之后规则）。

**v1.2 修正**（R5 soundness 漏洞 #6）：整数 fallback **仅在无 trait constraint 时触发**：

- 在 typeck 末尾，扫描所有 unresolved integer inference variables
- 若该变量**同时关联了 trait bound**（如 `?T: Display`），**不触发 fallback**，报错"type annotations needed"
- 否则赋为 `i32`

反例（被拒绝）：

```landin
trait Trait { fn method(self); }
impl Trait for i32 { fn method(self) {} }
impl Trait for i64 { fn method(self) {} }

().method(42);  // 42 有 trait bound，不 fallback，报错 "type annotations needed"
```

这避免了 fallback 静默选择 trait impl 导致的 unsound。

### 4.7 不做的事

- **不做 let-generalization**（R3 陷阱 #4）：`let x = ...;` 不会 generalize x 的类型，x 的类型在 let 处固定。这避免了 value restriction 问题。
- **不做全函数返回类型推导**：函数返回类型必须显式标注（`fn f() -> T`），不可省略。
- **不做 closure 参数类型推导**（MVP）：闭包参数必须显式标注或能从单次调用点确定。

---

## 5. Trait Resolution

### 5.1 整体流程

Trait resolution 分 **三阶段**（v1.2 修正；v1.2.2 修正归因：参考 rustc **老 solver** 的 `traits/resolution.html` 三阶段，**不是** next-gen solver。next-gen solver 是 rustc 实验中的 `-Znext-solver`，与老 solver 算法不同）：

1. **Evaluation**：评估候选 impl 是否适用（不真正 commit），返回 `EvaluatedToOk` / `EvaluatedToAmbig` / `EvaluatedToErr`
2. **Selection**：从候选中选最 specific 的（MVP 禁 overlapping，多候选直接报错）
3. **Fulfillment**：把选中 impl 的 where clause 作为新 obligation 加入队列，递归求解

另外引入 **Canonical query** 机制（§5.8），让 trait 求解结果可缓存。

### 5.2 Evaluation 阶段

```
evaluate(obligation: T: Trait<args>) -> EvalResult:
    candidates = []
    for impl in all_impls(Trait):
        result = evaluate_one(impl, obligation)
        if result != EvaluatedToErr:
            candidates.append((impl, result))
    
    if len(candidates) == 0: return Err(no impl)
    if len(candidates) == 1: return Ok(candidates[0])
    # 多候选拒绝（禁 overlapping）
    return Err(ambiguous)
```

`evaluate_one` 不真正绑定 inference variable，而是用 **placeholder** 代替，避免污染全局推导状态。

### 5.3 Selection 算法

```
select(obligation: T: Trait<args>) -> SelectionResult:
    eval_result = evaluate(obligation)
    match eval_result:
        Err(e) => return Err(e)
        Ok((impl, _)) =>
            # 真正绑定 inference variable
            bind(impl, obligation)
            return Ok(impl)
```

MVP 禁止 overlapping impls（R3 陷阱 #5），所以 Selection 退化为"唯一候选即选中"。

### 5.4 Fulfillment 阶段

Fulfillment 维护一个 **obligation queue**：

```
fulfillment_loop():
    while not obligation_queue.is_empty():
        obl = obligation_queue.pop()
        result = select(obl)
        match result:
            Ok(impl) =>
                # 把 impl 的 where clause 加入队列
                for clause in impl.where_clauses:
                    obligation_queue.push(clause)
            Err(ambig) =>
                # 推迟，等 inference variable 被解后再试
                pending_queue.push(obl)
            Err(no_impl) =>
                report_error(obl)
    
    # 最后检查 pending queue
    for obl in pending_queue:
        if not resolved(obl):
            report_error(obl)
```

### 5.5 Impl matching

给定 `impl<T: Clone> Trait for Vec<T>` 与查询 `Vec<i32>: Trait`：

1. 统一 `Vec<T>` 与 `Vec<i32>`，得 `T = i32`
2. 检查 impl 的 where clause：`i32: Clone`？
3. 递归 select `i32: Clone`，成功
4. 返回该 impl，绑定 `T = i32`

### 5.6 Orphan rule

`impl Trait for Type` 必须满足以下之一：

- `Trait` 在当前 crate 定义
- `Type` 在当前 crate 定义（至少一个组成部分"本地"）

违反则报"orphan rule violation"。这保证全局 coherence：任何 (Trait, Type) 对最多一个 crate 能 impl。

### 5.7 Coherence check

编译器在 crate 编译时执行：

1. 收集本 crate 的所有 impl
2. 对每对 (impl_a, impl_b)，检查是否冲突（类型签名可能 unify）
3. 若冲突且非 orphan 允许，报"conflicting impls"

跨 crate 的 coherence 通过 orphan rule 保证：只要每个 crate 遵守 orphan，全局就不会有冲突。

### 5.8 Depth limit 与 Canonical query

**Depth limit**：trait resolution 递归深度限制为 **128**（v1.2 修正：与 rustc 默认值一致，R11 报告证实 rustc 默认也是 128）。超过时报"reached recursion limit"，防止 `impl<T: A> B for T where T: B` 这类循环。

**Canonical query**（v1.2 新增）：trait 求解的输入与输出可被 canonical 化缓存，避免重复计算：

```
canonical_query(obligation: T: Trait<args>) -> CanonicalResult:
    # 1. Canonical 化：把所有 inference variable 替换为 placeholder
    canonical_input = canonicalize(obligation)
    
    # 2. 查缓存
    if cached in selection_cache[canonical_input]:
        return cached
    
    # 3. 实际求解
    result = evaluate(canonical_input)
    
    # 4. 缓存结果
    selection_cache[canonical_input] = result
    return result
```

Canonical query 是 rustc 性能关键（trait 求解可占总编译时间 30%+），Landin 复用此设计。

### 5.9 `?` 操作符与 From trait 唯一性（v1.2 修正 R5 soundness 漏洞 #7）

`?` 操作符的 `From::from(error)` 转换在多 impl 候选下可能导致歧义：

```landin
impl From<ErrorA> for Box<dyn Error> { ... }
impl From<ErrorB> for Box<dyn Error> { ... }

fn f() -> Result<(), Box<dyn Error>> {
    let x: Result<(), ErrorA> = ...;
    x?;  // 应转换 ErrorA → Box<dyn Error>，但 From 选择歧义
}
```

**v1.2 修正**：MVP 要求 `?` 上下文中 `From<E1> for E2` 必须有**唯一 impl**：

- 若多 impl 候选，报错 "ambiguous From implementation"
- 用户必须显式 `.map_err()` 转换：`x.map_err(|e| Box::new(e) as Box<dyn Error>)?`

这避免了 silently 选择错误的 From impl。

### 5.10 推迟的 trait constraint

某些 constraint 在 typeck 时无法立即求解（如 `?T: Trait`），加入 **fulfillment queue**。Fulfillment 会在以下时机重试：

- inference variable 被解为具体类型时
- 函数返回类型最终确定时
- typeck 结束前

若 fulfillment queue 末尾仍有未解 constraint，报"trait bound not satisfied"。

---

## 6. Auto impl 与 derive

### 6.1 自动 impl

编译器为以下类型自动 impl：

- 所有基本类型：`Copy`, `Clone`, `Sized`, `Send`, `Sync`, `Unpin`, `Freeze`（v0.2）
- 所有 `&T`：`Copy`, `Clone`, `Sized`, `Send` (if T: Sync), `Sync` (if T: Sync)
- 所有字段都是 Copy 的 struct/enum：`Copy`, `Clone`
- 所有字段都是 Send/Sync 的 struct/enum：`Send`/`Sync`（v0.2）

### 6.2 Derive 属性

MVP **支持** `#[derive(...)]`（v1.2 修正，与 13 文档统一）。支持的 derive 列表：

- `#[derive(Clone)]` — 生成 `impl Clone for T { fn clone(&self) -> Self { ... } }`
- `#[derive(Copy)]` — 要求字段都是 Copy，生成 `impl Copy for T {}`
- `#[derive(Debug)]` — 生成 `impl Debug for T { fn fmt(&self, f: &mut Formatter) -> Result { ... } }`
- `#[derive(PartialEq, Eq)]` — 生成 `impl PartialEq for T { fn eq(&self, other: &Self) -> bool { ... } }`
- `#[derive(PartialOrd, Ord)]` — 生成 `impl PartialOrd for T { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { ... } }`
- `#[derive(Hash)]` — 生成 `impl Hash for T { fn hash<H: Hasher>(&self, state: &mut H) { ... } }`
- `#[derive(Default)]` — 生成 `impl Default for T { fn default() -> Self { ... } }`

Derive 展开由编译器硬编码实现（不是 proc macro），在 HIR lowering 阶段插入对应的 impl item。

### 6.3 `Drop` trait

```landin
pub trait Drop {
    fn drop(&mut self);
}
```

- 用户 impl `Drop` 后，编译器在每个值离开作用域时插入 drop 调用
- Drop 顺序：**逆序析构**（fields 按声明逆序，locals 按声明逆序）
- Drop impl 不能 panic（panic-in-drop 在 MVP 直接 abort）

---

## 7. 关键算法：associated type normalization

当 trait 有 associated type 时，访问 `T::Item` 需要 **normalization**：

```landin
fn next<T: Iterator>(it: &mut T) -> Option<T::Item> { ... }
// 这里 T::Item 是投影类型，需要 normalize 为具体类型
```

### 7.1 Normalization 算法与终止性保证（v1.2 新增）

Normalization 算法：

1. 若 `T` 是具体类型且 `T: Iterator` 有已知 impl，把 `T::Item` 替换为 impl 中的具体类型
2. 若 `T` 是 inference variable，暂不 normalize，加入 fulfillment queue
3. 若 `T` 是泛型参数，保持 `T::Item` 不变（在 monomorphization 时 normalize）

Normalization 必须在 trait resolution 中循环调用，直到 fixpoint。

**终止性保证**（v1.2 修正 R5 soundness 漏洞 #2）：

- **Normalization depth limit = 32**：远小于 trait resolution 的 128，避免深层递归
- **Cycle 检测**：维护 normalization stack，若发现循环（如 `type Item = <Self as Trait>::Item`）立即报错 "recursive associated type"
- **Placeholder types**：normalization 结果只能是 closed term 或 placeholder，避免产生新 inference variable

反例（被拒绝）：

```landin
trait T { type Item; }
impl T for i32 { type Item = <i32 as T>::Item; }  // ERROR: recursive associated type
```

### 7.2 Normalization 与 trait resolution 的交互

Normalization 与 trait resolution 形成 mutually recursive 调用：

- Trait resolution 可能触发 normalization（如查询 `T: Iterator<Item = U>`）
- Normalization 可能触发 trait resolution（如 normalize `T::Item` 需先确定 `T: Iterator` 的 impl）

为避免死循环，两者共享 depth limit = 128，且 cycle 检测在 normalization 层强制执行。

---

## 8. Subtyping 规则

Landin 的 subtyping 仅来自 lifetime（不变性 / 协变 / 逆变）：

| 类型构造器 | Variance | 解释 |
| --- | --- | --- |
| `&'a T` | `'a` 协变，`T` 协变 | 长 lifetime <: 短 lifetime |
| `&'a mut T` | `'a` 协变，`T` 不变 | mutable 引用要求 T 不变 |
| `*const T` | `T` 协变 | |
| `*mut T` | `T` 不变 | |
| `Vec<T>` | `T` 不变 | 容器类型（避免 variance 灾难） |
| `Box<T>` | `T` 不变 | |
| `fn(T) -> U` | `T` 逆变，`U` 协变 | |
| `dyn Trait + 'a` | `'a` 协变 | trait 不变 |
| `struct Foo<T>` | 由 struct 字段决定 | 全部字段都不变 → struct 不变 |

MVP 默认所有 user-defined type 为 **不变**（避免 variance 推断复杂化）。v0.2 引入 `#[variant]` 显式标注。

---

## 9. 错误信息设计

类型错误信息是用户体验的关键。Landin 错误格式：

```
error[E0308]: mismatched types
   --> src/main.lin:10:5
    |
 10 |     let x: i32 = "hello";
    |            ---   -------
    |            |     |
    |            |     expected `i32`, found `&str`
    |            expected due to this
    |
    = note: expected type `i32`
            found reference `&'static str`
```

错误代码体系（v1.2.2 修正，与 16-diagnostics §2.1 完全一致，共 12 段）：

| 范围 | 类别 |
| --- | --- |
| E0001-E0499 | type system errors |
| E0500-E0699 | borrow check errors |
| E0700-E0899 | lifetime errors |
| E0900-E0999 | name resolution errors |
| E1000-E1099 | parse errors |
| E1100-E1299 | trait resolution errors |
| E1300-E1399 | codegen errors |
| E1400-E1499 | unsafe check errors |
| E1500-E1599 | coherence / orphan errors |
| E1600-E1699 | attribute errors |
| E1700-E1799 | macro errors |
| E1800-E1899 | stdlib errors |

MVP 阶段实现 30-50 个最常见错误代码，每个错误含"error code + 简短描述 + span + suggestion"。

---

**下一文档**: [`04-ownership-borrowing.md`](./04-ownership-borrowing.md) — 所有权与借用

---

## 10. 实现状态（v0.14.0，§25.8 回写）

> 本节由 Stage 6.18 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 10.1 §4 类型推导 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §4.1 constraint-based inference | ✅ 实现 | — | `typeck::checker::TypeChecker` |
| §4.2 inference variable | ✅ 实现 | — | `mir::ty::InferVar::{TyVar,IntVar,FloatVar}` |
| §4.3 constraint 类型 | ✅ 实现 | — | 通过 unification 生成 |
| §4.4 constraint 生成规则 | ✅ 实现 | — | `check_statement` + `check_terminator` + `infer_rvalue` |
| §4.5 unification 算法 | ✅ 实现 | — | `typeck::unify::UnificationTable` |
| §4.6 整数 fallback (i32 默认) | ✅ 实现 | — | `TypeChecker::check_mir_body` 末尾 default |
| §4.7 不做的事（const generics / GATs） | ✅ 遵守 | — | MVP 无 |

### 10.2 §5 Trait Resolution — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §5.1 整体流程 | ✅ 实现 | — | `traits::resolver::TraitResolver` |
| §5.2 evaluation | ✅ 实现 | B3（简化） | 实现不做 canonical query，直接查 |
| §5.3 selection | ✅ 实现 | B3（简化） | 实现不做 depth limit / specialization |
| §5.4 fulfillment | ✅ 实现 | — | driver 编排 |
| §5.5 impl matching | ✅ 实现 | — | `TraitResolver::is_copy_builtin` 等 |
| §5.6 orphan rule | ❌ 未实现 | B1 | v0.2+ |
| §5.7 coherence check | ✅ 实现 | B3（简化） | `traits::resolver` 做基本检查 |
| §5.8 depth limit / canonical query | ❌ 未实现 | B1 | v0.2+ |
| §5.9 `?` 与 From 唯一性 | ❌ 未实现 | B1 | v0.2+（需要 `?` 操作符） |
| §5.10 推迟的 trait constraint | ❌ 未实现 | B1 | v0.2+ |

### 10.3 §7-§8 Normalization + Subtyping — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §7 associated type normalization | ❌ 未实现 | B1 | v0.2+（需要 associated types） |
| §8 subtyping 规则 | ✅ 实现 | B3（简化） | `typeck::predicates::can_coerce` 实现 coercion 矩阵 |

### 10.4 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B1（orphan rule / canonical query / `?` / 推迟 constraint / normalization） | v0.2+ | MVP 不需要 |
| B3（trait resolution 简化） | v0.2+ | 当前简化版满足 MVP |
| B3（subtyping 简化） | v0.2+ | 当前 coercion 矩阵满足 MVP |
