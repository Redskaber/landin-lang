# 04 — 所有权与借用

> 本文定义 Landin 的所有权模型、借用规则、生命周期系统、NLL（Non-Lexical Lifetimes）算法实现。所有静态分析在 **MIR** 上进行，不走 Rust 1.0 之前的 AST-based 老路（R1 报告强烈警告）。

---

## 1. 所有权模型

### 1.1 三大原则

Landin 的所有权模型遵循 Rust 三原则：

1. **每个值有唯一所有者**（owner）：值在 owner 离开作用域时自动 drop
2. **任意时刻可有一个 `&mut` 或多个 `&`，但不能并存**
3. **引用必须不outlive被引用者**（lifetime 约束）

### 1.2 Move 语义

赋值/传参时：

- 若类型 impl `Copy`：按位复制，原变量仍可用
- 否则：move 所有权，原变量不可再用

```landin
let s1 = String::from("hello");
let s2 = s1;            // s1 moved to s2
println!("{}", s1);     // 错误：s1 已 moved
```

### 1.3 Partial move

Struct/enum 字段可以单独 move：

```landin
let p = Point { x: String::from("a"), y: 42 };
let x = p.x;            // p.x moved out
println!("{}", p.y);    // OK，p.y 仍可用
println!("{:?}", p);    // 错误：p.x 已 moved，p 不能整体使用
```

partial move 后，原变量整体不可用，但未 moved 的字段仍可访问。

### 1.4 Drop 顺序

Drop 顺序规则（参考 rustc 行为）：

1. **局部变量**：按声明顺序逆序析构
2. **函数参数**：按声明顺序逆序析构（与局部变量顺序一致）
3. **临时变量**：在所在完整表达式结束时析构（不是语句结束时，避免反直觉）
4. **Struct 字段**：按声明顺序逆序析构
5. **Tuple 字段**：按 index 逆序析构
6. **Match arm 绑定**：在 arm 块结束时析构

`Drop` trait 的 `drop` 方法在析构时调用。若用户没 impl `Drop`，编译器生成"字段逐个 drop"的隐式实现。

---

## 2. 借用规则

### 2.1 共享借用与独占借用

| 借用类型 | 语法 | 数量限制 | 操作限制 |
|---|---|---|---|
| 共享借用 | `&x` | 多个并存 | 只读 |
| 独占借用 | `&mut x` | 至多一个 | 可读可写 |

### 2.2 借用检查规则

在 MIR 上的 dataflow 分析检查：

1. **独占借用活跃区间内**：原 place 不可被读/写
2. **共享借用活跃区间内**：原 place 不可被写，但可读
3. **不可有活跃的独占借用 + 任何其他借用（共享或独占）**
4. **不可 move 一个被借用中的 place**

### 2.3 NLL（Non-Lexical Lifetimes）

借用结束点 **不是** 借用变量的词法作用域结束点，而是借用最后一次被使用的点。例：

```landin
let mut v = vec![1, 2, 3];
let r = &v[0];
println!("{}", r);          // r 最后一次使用
v.push(4);                   // OK，r 已不再活跃
```

NLL 让这种模式合法，是 R3 报告强调的"MVP 必须有"特性。

### 2.4 Two-phase borrows（v1.2 修正：MVP 支持子集）

`vec.push(vec.len())` 这种调用，`vec.len()` 先求值再 `vec.push`，逻辑上无冲突，但简单借用检查会报错。Rust 用 **two-phase borrows** 解决：把 `&mut` 的"借用开始点"推迟到所有参数求值完成后。

**v1.2 修正**（R6 报告指出 v1.0 矛盾）：MVP **必须支持** two-phase borrows 的 **method-call 子集**，否则 `vec.push(vec.len())` 这种常见模式编译失败。

支持范围：

- ✅ Method call 自动借用：`vec.push(vec.len())` — reservation 在参数求值前，activation 在调用时
- ✅ 显式 `&mut expr` 作为函数参数：`f(&mut v, v.len())` — 同上
- ❌ 显式 `&mut expr` 作为 let rhs：`let r = &mut v; v.len();` — 不支持 two-phase，需用户手动调整

算法：

```
BorrowKind::Mut { kind: MutBorrowKind::TwoPhaseBorrow }:
    reservation_point = current_point    // 参数求值前
    activation_point = call_point        // 调用时
    // 在 [reservation_point, activation_point) 期间，借用不 "激活"，
    // 允许其他共享借用
    // 在 activation_point 之后，借用激活，排斥其他借用
```

未支持显式 `&mut expr` 的 two-phase 是为了简化实现，v0.2 可放宽。

---

## 3. 生命周期系统

### 3.1 生命周期标注

```landin
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

fn first_word<'a>(s: &'a str) -> &'a str { ... }
// lifetime elision 简化为：
fn first_word(s: &str) -> &str { ... }
```

### 3.2 Lifetime elision 规则（v1.2 补全边界 case）

参考 Rust RFC #141（lifetime elision，2014-06-24），R1 报告支持。规则：

1. 每个引用参数的 lifetime 自动分配一个 fresh lifetime `'a`、`'b`、`'c`...
2. 若只有一个输入 lifetime，所有输出引用 lifetime 取 `'a`
3. 若有多个输入 lifetime 但其中一个是 `&self`/`&mut self`，所有输出引用 lifetime 取 self 的 lifetime
4. 否则，输出引用 lifetime 必须显式标注

这三条规则覆盖了约 87% 的 Rust 函数签名（RFC #141 数据）。Landin 直接采用。

**v1.2 补全的边界 case**（R5 soundness 漏洞 #5）：

1. **嵌套引用**：`fn f(x: &Box<&u8>) -> &u8` — elision 不应用，要求显式标注。原因：嵌套引用的 lifetime 关系不明确
2. **`Box<Self>` 方法**：`fn f(self: Box<Self>) -> &Something` — elision 取 self 的 lifetime（与 `&self` 同等处理）
3. **泛型类型隐含 lifetime**：`fn f(x: &Vec<&T>) -> &T` — 隐含 `T: 'a`，需在 typeck 时记录
4. **async fn**（v0.2）：`async fn f(x: &T) -> &T` — elision 不应用，需显式（async 的 lifetime 复杂）
5. **多 lifetime 输入且无 self**：`fn f(x: &i32, y: &i32) -> &i32` — elision 规则 4，要求显式

未应用的 case 报错 "missing lifetime specifier"，不静默选择错误 lifetime。

### 3.3 `'static` lifetime

`'static` 表示"整个程序生命周期"。所有 const/static 值、字符串字面量、函数指针类型都是 `'static`。

### 3.4 Lifetime bound

```landin
fn f<'a, T: 'a>(x: &'a T) { ... }
// 表示 T 在 'a 期间有效（T 不含比 'a 短的引用）

fn f<T: 'static>(x: T) { ... }
// 表示 T 不含任何非 'static 引用，可任意长期持有
```

---

## 4. NLL 算法实现

### 4.1 数据结构

```rust
// MIR 上的 region（lifetime 变量）
type Region = u32;

// Region 集合 = CFG 上点的集合
type RegionSet = BitSet<PointIndex>;

// Constraint 图
struct ConstraintGraph {
    // 'a: 'b 表示 'a 包含 'b（'a 至少和 'b 一样长）
    edges: Vec<(Region, Region)>,
}

// Borrow 记录
struct BorrowRecord {
    borrow_region: Region,         // 这个借用所属的 lifetime
    borrowed_place: Place,          // 被借用的 place
    borrow_kind: BorrowKind,        // Shared / Mut / Unique (v0.2)
    reservation_point: PointIndex,  // 借用开始点
    activation_point: PointIndex,   // 借用激活点（two-phase borrows）
    start_point: PointIndex,        // 借用产生点
}
```

### 4.2 算法三阶段

NLL 算法分三阶段：

#### 阶段 1：Constraint collection

遍历 MIR，收集：

- **Outlives constraints** `'a: 'b`：来自函数签名、struct 字段、借用关系
- **Lifetime regions**：每个引用类型 / 借用产生一个 region
- **Universal regions**：函数签名中的 `'a`、`'b`、`'static`（不可推断的）
- **Borrow records**：每个 `&x` / `&mut x` 操作

#### 阶段 2：Region inference

求解 constraint 系统，把每个 region 解为 **CFG 点的集合**：

```
algorithm region_inference:
    # 初始化：每个 region = 空 set
    for r in all_regions: r.points = {}
    
    # 不动点迭代
    changed = true
    while changed:
        changed = false
        for (r1, r2) in constraints:  # r1: r2 表示 r1 ⊇ r2
            new = r1.points ∪ r2.points
            if new != r1.points:
                r1.points = new
                changed = true
        # 加上每个 region 自身的使用点
        for r in regions:
            for use_point in r.use_points:
                if use_point not in r.points:
                    r.points = r.points ∪ {use_point}
                    changed = true
    
    # 检查 universal region
    for ur in universal_regions:
        for r in non_universal_regions:
            if r.points ⊄ ur.points:
                # r 包含了 ur 之外的点，r 不能 escape ur
                report_error(ur, r)
```

复杂度：O(R² × P)，其中 R = region 数，P = CFG 点数。实际中 R/P 都小，几乎线性。

#### 阶段 3：Borrow check

基于 region inference 结果，检查借用规则：

```
algorithm borrow_check:
    for borrow in all_borrows:
        # 找出 borrow 活跃区间
        live_range = compute_live_range(borrow)
        
        # 检查 borrow 活跃期间，原 place 没有冲突访问
        for point in live_range:
            for access in accesses_at[point]:
                if access.place conflicts with borrow.borrowed_place:
                    if borrow.kind == Mut:
                        # 任何访问都冲突
                        report_error(borrow, access)
                    elif borrow.kind == Shared and access.kind == Write:
                        # 共享借用期间的写访问冲突
                        report_error(borrow, access)
                    elif borrow.kind == Shared and access.kind == Move:
                        # 共享借用期间的 move 冲突
                        report_error(borrow, access)
```

`compute_live_range` 用 liveness analysis 算出借用变量在哪些点被使用，借用区间 = [start_point, last_use_point]。

### 4.3 Liveness analysis

Liveness 在 MIR 上做：

```
# 反向数据流分析
# live_in[n] = use[n] ∪ (live_out[n] - def[n])
# live_out[n] = ⋃ live_in[s] for s in succ[n]

algorithm liveness:
    # 初始化
    for n in all_blocks: live_in[n] = {}; live_out[n] = {}
    
    # 反向 RPO 迭代
    changed = true
    while changed:
        changed = false
        for n in reverse_postorder(cfg):
            new_out = ⋃ live_in[s] for s in succ[n]
            new_in = use[n] ∪ (new_out - def[n])
            if new_in != live_in[n] or new_out != live_out[n]:
                live_in[n] = new_in; live_out[n] = new_out
                changed = true
```

复杂度：O(E × V)，E = 边数，V = 变量数。用 bitset 表示 live set，单点分析毫秒级。

### 4.4 Maybe-initialized places

除了 liveness，还需 **初始化分析**：

```rust
enum InitState {
    Uninit,        // 未初始化
    MaybeInit,     // 可能初始化（在 CFG 某分支初始化）
    Init,          // 确定初始化
}
```

数据流分析收集每个 place 在每个点的 init state。读取 uninit place 是 UB（编译期报错）。

### 4.5 Move tracking

类似 liveness，但追踪 move 后的状态：

```
move_state[place] := { Alive, Moved, MaybeMoved }
```

若 place 在某点 MaybeMoved 或 Moved，访问报错"use of moved value"。

---

### 4.6 NLL 算法完整规范（v1.2 新增，修复 R5 soundness 漏洞 #1）

v1.0/v1.1 的 NLL 算法不完整，无法处理 universal region 与 implied bounds，可构造 use-after-free 反例。

### 4.6.1 Universal region 与 placeholder

**Universal region**：函数签名中的 `'a`、`'b`、`'static` 是 universal（对所有调用方成立的 region）。所有引用参数的 lifetime ⊆ 某 universal region。

**Placeholder region**：在 trait resolution 的 canonical query 中，用 placeholder 代替 inference variable，避免变量捕获。

### 4.6.2 Implied bounds

`&'a T` 隐含 `T: 'a`（T 中所有引用 lifetime ⊇ 'a）。所有 `'b` 出现在 `T` 中则 `'b: 'a`。

参考 Rust RFC #1214 "WF & implied bounds"。

### 4.6.3 Universe 机制

HRTB `for<'a> fn(&'a T)` 创建新 **universe**，每个 universe 有独立的 placeholder region 集。避免变量捕获导致的 unsound。

### 4.6.4 Type tests

验证 `T: 'a` 约束在借用点：

```
TypeTest { universal_region: Region, ty: Ty, span: Span }
```

Type tests 在 region inference 后检查，若失败报错 `T does not live long enough`。

### 4.6.5 SCC 压缩

Region constraint graph 用 **SCC（强连通分量）** 压缩，避免 O(R²×P) 退化为指数复杂度。

### 4.6.6 RegionInferenceContext 完整数据结构

```rust
struct RegionInferenceContext<'tcx> {
    universal_regions: Vec<Region>,          // 函数签名的 'a, 'b, 'static
    region_defs: Vec<RegionInfo>,            // 所有 region 的定义
    constraints: Vec<OutlivesConstraint>,    // 'a: 'b 约束
    type_tests: Vec<TypeTest>,               // T: 'a 验证
    universe_causes: Vec<UniverseCause>,     // universe 创建原因
    sccs: Sccs<Region>,                      // SCC 压缩
    scc_values: IndexVec<Scc, RegionSet>,    // 每个 SCC 的点集
}
```

算法不变点：每个 non-universal region 的点集 ⊆ 某 universal region 的点集。

---

## 5. Drop check（v1.2 新增，修复 R5 soundness 漏洞 #3）

### 5.1 问题

默认 Drop 实现可能观察已 drop 的引用数据：

```landin
struct Inspector<'a>(&'a u8);
impl<'a> Drop for Inspector<'a> {
    fn drop(&mut self) {
        println!("{}", self.0);  // 可能访问已 drop 的数据
    }
}

struct World<'a> {
    inspector: Inspector<'a>,
    data: Box<u8>,  // 可能先于 inspector 被 drop（逆序析构）
}
```

### 5.2 默认 Drop check

默认情况下，Drop impl 要求所有 lifetime/类型参数满足 `: 'static`。即上例中的 `Inspector<'a>` 若 `'a` 非 `'static`，编译器报错。

### 5.3 `#[may_dangle]` 属性

用户可显式标注 Drop impl 不访问某个参数，放宽限制：

```landin
unsafe impl<#[may_dangle] 'a> Drop for Inspector<'a> {
    fn drop(&mut self) {
        // 不访问 self.0
    }
}
```

`#[may_dangle]` 是 unsafe 标注，作者承诺 Drop impl 不访问该参数。若违反，UB。

### 5.4 Drop 顺序

Drop 顺序规则：

1. 局部变量：按声明顺序逆序析构
2. Struct 字段：按声明顺序逆序析构
3. Match arm 绑定：在 arm 块结束时析构

参考 Rust RFC #1327 "dropck-param-eyepatch"。

---

## 6. 借用错误诊断

### 6.1 错误类型

| 错误代码 | 含义 |
| --- | --- |
| E0500 | closure requires unique access to `place` but it is already borrowed |
| E0502 | cannot borrow `place` as mutable because it is also borrowed as immutable |
| E0503 | cannot use `place` because it was mutably borrowed |
| E0505 | cannot move out of `place` because it is borrowed |
| E0507 | cannot move out of `place`, a captured variable in an closure |
| E0515 | cannot return reference to local variable |
| E0597 | `place` does not live long enough |
| E0599 | no method named `xxx` found |

### 6.2 诊断信息设计

借用错误必须给出：

1. 错误位置（产生冲突的访问点）
2. 借用来源（哪个借用导致冲突）
3. 借用活跃区间可视化（高亮 CFG 上的活跃点）
4. 修复建议

```
error[E0502]: cannot borrow `v` as mutable because it is also borrowed as immutable
   --> src/main.lin:5:5
    |
 3 |     let r = &v;
    |              - immutable borrow occurs here
 4 |     println!("{}", r);
 5 |     v.push(4);
    |     ^^^^^^^^^ mutable borrow occurs here
 6 |     println!("{}", r);
    |                   - immutable borrow later used here
    |
help: the borrow of `v` as immutable needs to be released before borrowing it as mutable
   |
 4 |     drop(r);
 5 |     v.push(4);
   |
```

---

## 7. 显式 lifetime 与推导

### 7.1 函数签名中的 lifetime

函数签名中的所有 lifetime 必须显式声明或经 elision 规则补全：

```landin
// 显式
fn f<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { x }

// Elision 规则 1：单输入 lifetime，输出取之
fn f(x: &i32) -> &i32 { x }

// Elision 规则 2：&self 方法
impl Foo {
    fn get(&self, k: &str) -> &str { ... }     // 返回 &self 的 lifetime
}

// 需要显式：多输入 lifetime，无 self
fn f(x: &i32, y: &i32) -> &i32 { ... }     // 错误：missing lifetime specifier
```

### 7.2 Lifetime 推导

函数体内的 lifetime 不需要标注，由 NLL 推导。但函数签名中的 lifetime **不推导**（必须显式），保证 API 稳定。

### 7.3 Higher-rank lifetime（HRTB）

`for<'a> fn(&'a T) -> &'a U` 表示"对所有 lifetime 'a 都成立"。MVP 限制：仅在 trait bound 与函数指针类型中支持 HRTB，其他位置不支持。

```landin
fn apply<T, U, F: for<'a> Fn(&'a T) -> &'a U>(x: &T, f: F) -> &U {
    f(x)
}
```

---

## 8. Disjoint closure captures（RFC 2229，v1.2 新增）

### 8.1 问题

Rust 2018 之前，闭包捕获整个 struct，即使只访问一个字段：

```landin
struct Big { a: i32, b: HugeStruct }

let big = Big { a: 1, b: HugeStruct::new() };
let f = |x| big.a + x;   // Rust 2015: 捕获整个 big，导致 big.b 也被 move
big.b;                    // ERROR: big 已 moved
```

### 8.2 RFC 2229 disjoint closure captures

Rust 2018+（RFC 2229）让闭包只捕获访问的字段：

```landin
let f = |x| big.a + x;   // 只捕获 big.a，big.b 仍可用
big.b;                    // OK
```

### 8.3 Landin MVP 实现

MVP **必须实现** RFC 2229（R6 报告指出 stage 1 自举时闭包代码会 borrow checker 误报）。

实现：在 HIR lowering 阶段分析闭包体，把对 `big.a` 的访问转换为对 `big.a` 的直接捕获（而非整个 `big`）。

### 8.4 与 borrow check 的交互

Disjoint closure captures 与 borrow check 配合：

- 闭包捕获 `big.a` 的 `&` 借用
- 外部代码仍可 `&mut big.b`（不冲突）
- 外部代码不可 `&mut big.a`（与闭包借用冲突）

---

## 9. 与 Rust 的差异（v1.2 修正）

| 维度 | Rust | Landin | 理由 |
| --- | --- | --- | --- |
| NLL | 默认 | **默认** | R3 推荐 |
| Two-phase borrows | 默认 | **MVP 支持子集**（method-call auto-ref，见 §2.4） | R6 修正 |
| Disjoint closure captures | 默认 | **MVP 支持**（RFC 2229，见 §8） | R6 修正 |
| Polonius | 实验 | **永久不做** | 复杂度未达收益 |
| Variance | 推导 | **MVP 全部不变** | 简化 |
| `?Sized` bound | 支持 | **MVP 部分支持**（str/[T]/dyn Trait，见 13 §2.1） | R9 修正 |
| Higher-kinded types | 不支持 | **不支持** | 一致 |
| GATs | nightly | **MVP 不支持** | 复杂度 |
| `&move` references | nightly | **不支持** | 不必要 |

---

## 10. 实现路线

按 R8 报告"v0.1/v0.3 分期"路线（v1.2 修正）：

| 阶段 | 时间 | borrow checker 状态 |
| --- | --- | --- |
| 阶段 1 | 月 1-3 | MIR 构建 + 基础 liveness |
| 阶段 2 | 月 4-6 | NLL region inference + borrow check on MIR |
| 阶段 3 | 月 7-9 | move tracking + maybe-init 分析 |
| 阶段 4 | 月 10-12 | 错误诊断优化 + 边界 case 修复 |
| 阶段 5 | 月 13-15+ | v0.1 发布（仅 stage 0）；自举在 v0.3（43-64 月） |

---

**下一文档**: [`05-ast.md`](./05-ast.md) — AST 结构定义

---

## 11. 实现状态（v0.14.0，§25.8 回写）

> 本节由 Stage 6.18 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 11.1 §2 借用规则 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §2.1 共享借用 vs 独占借用 | ✅ 实现 | — | `borrowck::BorrowSet` + `BorrowKind::{Shared,Mut}` |
| §2.2 借用检查规则 | ✅ 实现 | — | `borrowck::BorrowChecker::check_place_write/read` |
| §2.3 NLL | ✅ 实现 | — | `borrowck::liveness::compute_last_use_map` |
| §2.4 two-phase borrows | ❌ 未实现 | B1 | v0.2+ |

### 11.2 §3 生命周期系统 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §3.1 lifetime 标注 | ✅ 实现（语法层） | B3 | parser 解析 lifetime，但 typeck 不做 region inference |
| §3.2 lifetime elision | ✅ 实现（简化） | B3 | 实现用 `Region::Erased` 替代 |
| §3.3 `'static` | ✅ 实现 | — | `mir::ty::Region::Static` |
| §3.4 lifetime bound | ❌ 未实现 | B1 | TD-015（Region inference）v0.2+ |

### 11.3 §4 NLL 算法实现 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §4.1 数据结构 (BorrowSet / MoveTracker) | ✅ 实现 | — | `borrowck::borrow_set` + `borrowck::move_tracker` |
| §4.2 算法三阶段 | ✅ 实现 | B3（简化） | 实现合并 liveness + maybe-init + borrow analysis |
| §4.3 liveness analysis | ✅ 实现 | — | `borrowck::liveness::compute_last_use_map` |
| §4.4 maybe-initialized places | ✅ 实现 | B3（简化） | 通过 `StorageLive/StorageDead` 隐式跟踪 |
| §4.5 move tracking | ✅ 实现 | — | `borrowck::move_tracker::MoveTracker` |
| §4.6 NLL 完整规范（universal region / implied bounds / universe / type tests / SCC） | ❌ 未实现 | B1 | TD-015（Region inference）v0.2+ |

### 11.4 §5 Drop check — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §5.1-5.3 drop check | ❌ 未实现 | B1 | v0.2+（需要 Drop trait 完整实现） |
| §5.4 drop 顺序 | ✅ 实现（简化） | B3 | 实现按 scope 顺序 drop，不做严格 drop check |

### 11.5 §6 借用错误诊断 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §6.1 错误类型 | ✅ 实现 | — | `borrowck::error::BorrowError` + `BorrowErrorKind` |
| §6.2 诊断信息设计 | ✅ 实现 | B3（简化） | 实现提供基本错误信息，无 suggested fix |

### 11.6 §8 Disjoint closure captures — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §8 RFC 2229 disjoint closure captures | ❌ 未实现 | B1 | v0.2+ |

### 11.7 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B1（region inference / two-phase borrows / drop check / disjoint captures） | TD-015 v0.2+ | 需要 region inference 基础设施 |
| B3（lifetime elision / NLL 简化 / maybe-init / drop 顺序 / 诊断） | v0.2+ | 当前简化版满足 MVP |

---

## 12. Stage 7 实现状态更新（v0.14.6，§25.8 回写）

> 本节由 Stage 7.7 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。
> 更新 Stage 6.18 的 §11 偏差清单，反映 Stage 7 的 TD-015 完成。

### 12.1 TD-015 Region inference — 完整实现状态

| 设计 § | Stage 6.18 状态 | Stage 7 状态 | 实现位置 |
|--------|----------------|-------------|---------|
| §4.6.1 Universal region | ❌ B1 | ✅ (7.1) | `RegionInfo::Universal` |
| §4.6.2 Implied bounds | ❌ B1 | ✅ (7.3) | `collect_implied_bounds` |
| §4.6.3 Universe 机制 | ❌ B1 | ✅ (7.4) | `UniverseId` + `check_universe_escapes` |
| §4.6.4 Type tests | ❌ B1 | ✅ (7.3) | `TypeTest` + `infer_regions()` Step 4 |
| §4.6.5 SCC 压缩 | ❌ B1 | ✅ (7.4) | `compute_sccs` (Tarjan) |
| §4.6.6 RegionInferenceContext | ❌ B1 | ✅ (7.1) | 完整数据结构 (1462 LOC) |
| §4.2 不动点迭代 | ❌ B1 | ✅ (7.2) | `infer_regions()` |
| §4.2 Universal check | ❌ B1 | ✅ (7.2) | Step 3 escape detection |
| borrowck 集成 | ❌ B1 | ✅ (7.5) | `run_region_inference` |

### 12.2 偏差处理计划更新

| 偏差 | Stage 6.18 计划 | Stage 7 更新 |
|------|----------------|-------------|
| B1（region inference） | TD-015 v0.2+ | ✅ **已实现** (Stage 7.1-7.5) |
| B1（two-phase borrows） | v0.2+ | 不变 |
| B1（drop check） | v0.2+ | 不变 |
| B1（disjoint closure captures） | v0.2+ | 不变 |
| B3（lifetime elision / NLL 简化 / maybe-init / drop 顺序 / 诊断） | v0.2+ | 不变 |

**关键变化**：Stage 6.18 的 §11.7 中 "B1（region inference）→ TD-015 v0.2+"
已更新为 "✅ **已实现** (Stage 7.1-7.5)"。Region inference 基础设施完整建立
并集成到 borrowck，当前作为附加检查运行（no-op，因 MIR regions 全为 Erased）。

---

## 13. Stage 8 实现状态更新（v0.15.4，§25.8 回写）

> 本节由 Stage 8.6 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 13.1 v0.2 特性实现状态

| 设计 § | 特性 | Stage 7 状态 | Stage 8 状态 | 实现 |
|--------|------|-------------|-------------|------|
| §3.2 | Lifetime elision 规则 | ❌ B1 | ✅ (8.1) | `LifetimeElisionCtxt` + RFC #141 规则 1-4 |
| §5 | Drop elaboration | ❌ B1 | ✅ (8.4) | `DropElaborator` + `needs_drop` + 逆序析构 (§5.4) |
| §5.2 | Drop check (默认) | ❌ B1 | ✅ (8.4) | `needs_drop` 检查 + `register_drop_impl` |
| §5.3 | `#[may_dangle]` 属性 | ❌ B1 | ❌ 未实现 | v0.3+ (需要 unsafe 属性系统) |

### 13.2 偏差处理计划更新

| 偏差 | Stage 7 计划 | Stage 8 更新 |
|------|-------------|-------------|
| B1（lifetime elision） | v0.2+ | ✅ **已实现** (8.1) |
| B1（drop check / drop elaboration） | v0.2+ | ✅ **已实现** (8.4) |
| B1（two-phase borrows） | v0.2+ | 不变 |
| B1（disjoint closure captures） | v0.2+ | 不变 |
| B1（`#[may_dangle]` 属性） | — | v0.3+（新增） |
| B3（NLL 简化 / maybe-init / drop 顺序 / 诊断） | v0.2+ | 部分改善 (8.4: drop 顺序实现) |

**关键变化**: v0.2 路线图 5 项全部实现。Drop elaboration 基础设施完整建立。
