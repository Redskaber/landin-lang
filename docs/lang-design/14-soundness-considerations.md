# 14 — Soundness 论证

> 本文集中论证 Landin 类型系统的 soundness。基于 R5（理论一致性审查）报告发现的 7 个 soundness 漏洞及其修复方案。本文不提供完整 Coq 证明，仅给出关键定理声明与论证思路。

---

## 1. Soundness 目标

### 1.1 Safe 子集的健全性

Landin safe 子集（不含 `unsafe`）必须满足：

1. **Progress**：well-typed 程序不会"卡住"（stuck）—— 任一 well-typed 表达式要么是值，要么可 step
2. **Preservation**：若 `e: T` 且 `e → e'`，则 `e': T`
3. **内存安全**：无 use-after-free、无 double-free、无未初始化内存读取、无数据竞争（v0.2 加并发后）
4. **类型安全**：无 type confusion，无 null 解引用（safe 子集中）

### 1.2 Unsafe 子集的责任

`unsafe` 子集允许打破上述保证，但 unsafe 代码作者必须手动维护以下不变量：

1. **非空指针**：`&T` 与 `Box<T>` 不为 null（unsafe 代码不可构造 null 引用）
2. **有效对齐**：解引用的指针必须满足类型对齐要求
3. **初始化**：读取的内存必须已初始化为正确类型
4. **生命周期**：引用不超过被引用者
5. **无数据竞争**（v0.2 加并发后）
6. **`unsafe trait` 契约**：实现 unsafe trait 必须满足其文档化契约

---

## 2. 关键定理

### 2.1 类型 soundness（声明，不带证明）

**定理 1（Type Soundness）**：若 `e: T` 且 `e →* v`（e 求值为 v），则 `v: T`。

**论证思路**：

- Progress + Preservation ⇒ Type Soundness（Wright-Felleisen 1994 标准方法）
- 关键 case：match 表达式穷尽性、trait method 调用、closure 调用、`?` 操作符
- 推导参考 Jung et al. 2017 "Understanding and Evolving the Rust Programming Language" §3

### 2.2 借用 soundness

**定理 2（Borrow Soundness）**：对任意 well-typed 程序，运行时不存在：

- (a) 同时活跃的 `&mut` 与任何其他借用
- (b) 活跃的 `&` 与原 place 的写访问
- (c) move 后的 place 访问

**论证思路**：

- NLL 在 MIR 上做 dataflow 分析，证明 borrow 活跃区间与冲突访问不重叠
- 算法 soundness 证明参考 RFC 2094 + Jung 2017 §4
- 关键引理：liveness analysis 的不动点正确性（Cooper-Harvey-Kennedy 2004）

### 2.3 Drop soundness

**定理 3（Drop Soundness）**：每个非 Copy 值恰好在离开作用域时被 drop 一次，且 drop 期间引用数据有效。

**论证思路**：

- Drop elaboration 在 MIR 上插入 `Drop` terminator，保证 drop 次序
- Drop check (`#[may_dangle]`)：保证 Drop impl 不会访问已 drop 的引用字段
- 算法参考 RFC 1327 "dropck-param-eyepatch"

### 2.4 内存安全

**定理 4（Memory Safety）**：safe 子集程序运行时不存在：

- (a) use-after-free
- (b) double-free
- (c) 未初始化内存读取
- (d) 越界访问（运行时检查）
- (e) 整数溢出 UB（debug panic，release wrapping）

**论证思路**：

- (a)(b) 由所有权 + 借用 + drop soundness 保证
- (c) 由 maybe-init dataflow analysis 保证
- (d) 由运行时 bounds check 保证
- (e) 由 `Assert` terminator + LLVM overflow intrinsics 保证

---

## 3. R5 发现的 7 个 Soundness 漏洞与修复

### 3.1 漏洞 1：NLL region inference 不完整

**反例**（R5 §1.1）：

```landin
fn foo<'a, 'b>(x: &'a &'b u8) -> &'a &'b u8 { x }
// 'a: 'b implied bound 未捕获，可构造 UAF
```

**修复**（v1.1 已纳入 04 文档 §4.6）：

- 引入 **universal region**：函数签名的 `'a`/`'b` 是 universal，所有引用参数的 lifetime ⊆ universal region
- 引入 **implied bounds**：`&'a T` 隐含 `T: 'a`，所有 `'b` 出现在 `T` 中则 `'b: 'a`
- 引入 **type tests**：验证 `T: 'a` 在每个借用点
- 引入 **universe**：HRTB `for<'a>` 创建新 universe，避免变量捕获

**soundness 论证**：

- Universal region 不动点求解后，每个 non-universal region 的点集 ⊆ 某 universal region
- Type tests 在 borrow check 阶段验证
- Implied bounds 保证引用字段的有效性

### 3.2 漏洞 2：Associated type normalization 无终止保证

**反例**（R5 §1.2）：

```landin
trait T { type Item; }
impl T for i32 { type Item = <i32 as T>::Item; }  // 自引用
```

**修复**（v1.1 已纳入 03 文档 §7.1）：

- Normalization depth limit = 32
- Normalization stack cycle 检测
- Placeholder types 避免产生新 inference variable

**soundness 论证**：

- Depth limit 保证终止
- Cycle 检测报错（不允许自引用 associated type）
- Placeholder 保证 normalization 结果是 closed term

### 3.3 漏洞 3：Drop check 缺失

**反例**（R5 §1.3）：

```landin
struct Inspector<'a>(&'a u8);
impl<'a> Drop for Inspector<'a> {
    fn drop(&mut self) {
        println!("{}", self.0);  // 可能访问已 drop 的数据
    }
}

struct World<'a> {
    inspector: Inspector<'a>,
    data: Box<u8>,  // 可能先于 inspector 被 drop
}
```

**修复**（v1.1 已纳入 04 文档 §5）：

- 默认 Drop impl 要求所有 lifetime 参数 `: 'static`
- `#[may_dangle]` 标注的 lifetime 可放宽
- Drop elaboration 按"逆字段序"插入 Drop terminator

**soundness 论证**：

- 默认情况下，Drop impl 不能 outlive 任何字段（因要求 `'static`）
- `#[may_dangle]` 是 unsafe 标注，作者承诺 Drop impl 不访问该字段
- Drop 顺序保证字段在父结构之后析构（逆序）

### 3.4 漏洞 4：FalseEdge 省略导致 match guard 错误

**反例**（R5 §1.4）：

```landin
match x {
    Some(y) if y > 0 => positive(),
    Some(_) => zero_or_negative(),
    None => empty(),
}
```

若 guard `y > 0` 为 false，需要从 `positive()` 的 CFG 跳回 match 的下一个 arm。无 FalseEdge 无法精确建模。

**修复**（v1.1 已纳入 06 文档 §7）：

- 恢复 `FalseEdge { real_target, imaginary_target }` 到 TerminatorKind
- Match lowering：每个 arm 的 guard 失败时跳到 imaginary_target（下一个 arm 的入口）
- Real_target 是 guard 成功时跳转点

**soundness 论证**：

- FalseEdge 精确建模 match 语义，CFG 与实际控制流一致
- Borrow check 在 FalseEdge 的 imaginary_target 处保守处理借用

### 3.5 漏洞 5：Lifetime elision 边界不健全

**反例**（R5 §1.5）：

```landin
fn f(x: &Box<&u8>) -> &u8 { x }  // 嵌套引用，elision 规则不适用
```

**修复**（v1.1 已纳入 04 文档 §3.2）：

- 明确三条 elision 规则的边界 case
- 嵌套引用 `&'a &'b T` 不应用 elision（要求显式）
- `Box<Self>` 方法的 elision 取 self 的 lifetime
- 不应用的 case 报"missing lifetime specifier"

**soundness 论证**：

- Elision 仅是语法糖，展开后的 lifetime 标注必须通过 borrow check
- 不应用的 case 强制显式，避免 silently 错误

### 3.6 漏洞 6：整数 fallback 与 trait selection 交互

**反例**（R5 §1.6）：

```landin
trait Trait { fn method(self); }
impl Trait for i32 { fn method(self) {} }
impl Trait for i64 { fn method(self) {} }

fn main() {
    ().method(42);  // 42 fallback 到 i32，静默选第一个 impl
}
```

**修复**（v1.1 已纳入 03 文档 §4.6）：

- 整数 fallback **仅在无 trait constraint 时触发**
- 若 inference variable 同时有 trait bound，不触发 fallback
- 报错"type annotations needed"

**soundness 论证**：

- 避免 fallback 静默选择 trait impl，保证 coherence
- 用户必须显式指定类型

### 3.7 漏洞 7：`?` 与 From trait 多 impl 选择

**反例**（R5 §1.7）：

```landin
impl From<ErrorA> for Box<dyn Error> { ... }
impl From<ErrorB> for Box<dyn Error> { ... }

fn f() -> Result<(), Box<dyn Error>> {
    let x: Result<(), ErrorA> = ...;
    x?;  // 应转换 ErrorA → Box<dyn Error>，但 From 选择歧义
}
```

**修复**（v1.1 已纳入 03 文档 §5）：

- `?` 上下文要求 `From<E1> for E2` 唯一 impl
- 多 impl 候选报错"ambiguous From implementation"
- 用户必须显式 `.map_err()`

**soundness 论证**：

- 避免 silently 选择错误的 From impl
- Coherence 保证不冲突，但 `Box<dyn Error>` 是 trait object，多 From 是合法的

---

## 4. 与 Rust 已知 Soundness Hole 的对比

### 4.1 Rust soundness hole 历史

| CVE/Issue | 描述 | Landin 状态 |
| --- | --- | --- |
| #25860 | Implied bounds not enforced | 已修复（v1.1 §3.1） |
| #135011 | Recursive associated type normalization | 已修复（v1.1 §3.2） |
| #56254 | Drop check bypass | 已修复（v1.1 §3.3） |
| #34761 | Match guard CFG imprecision | 已修复（v1.1 §3.4） |
| #42729 | Lifetime elision unsoundness | 已修复（v1.1 §3.5） |
| #152589 | Integer fallback + trait selection | 已修复（v1.1 §3.6） |
| #29149 | `?` with multiple From impls | 已修复（v1.1 §3.7） |
| #73294 | Variance of `PhantomData<...>` | 不适用（Landin 不做 variance 推导） |
| #84570 | Specialization soundness | 不适用（Landin 永久不做 specialization） |

### 4.2 Landin 暂未覆盖的 Rust soundness 风险

以下 Rust soundness 风险在 Landin MVP 不适用（因功能未实现）：

- `unsafe` trait 的 Send/Sync 错误实现（v0.2 加并发后才需考虑）
- async fn 的 cancellation safety（v0.2 加 async 后才需考虑）
- const eval UB（v0.2 加 const eval 后才需考虑）
- GATs 的 variance（v0.2 加 GATs 后才需考虑）

v0.2 实现这些功能时，必须重新评估 soundness。

---

## 5. `unsafe` 边界规范

### 5.1 `unsafe` 的责任

`unsafe` 代码作者必须保证：

1. **指针有效**：解引用前确保非 null、对齐、有效内存
2. **初始化**：`ptr::read` 前确保内存已初始化为正确类型
3. **生命周期**：构造的引用不超过被引用者
4. **无竞争**（v0.2）：跨线程访问需要同步
5. **`unsafe trait` 契约**：实现 unsafe trait 必须满足其文档

### 5.2 `unsafe` 不能做的事

即使 `unsafe` 也不能：

1. 构造未对齐的 `&T` 引用（但可以构造未对齐的 `*const T`）
2. 违反 trait 的 coherence（unsafe impl 仍受 orphan rule 约束）
3. 跳过 Drop（值离开作用域必 drop，但可通过 `ManuallyDrop` 延迟）
4. 关闭 bounds check（可通过 `get_unchecked` 跳过，但仍是 unsafe）

### 5.3 `unsafe` 边界与抽象

`unsafe` 代码必须封装为 safe API：

```landin
// 正确：unsafe 封装在 safe fn 内
pub fn new_vec<T>(capacity: usize) -> Vec<T> {
    let mut v = Vec::with_capacity(capacity);
    unsafe {
        // 内部 unsafe 操作，但外部 API safe
    }
    v
}

// 错误：unsafe 暴露给调用者
pub unsafe fn new_vec<T>(capacity: usize) -> Vec<T> { ... }
```

---

## 6. 未定义行为清单

### 6.1 Safe 子集中的运行时错误（非 UB）

- 整数溢出：debug panic / release wrapping
- 越界访问：panic（含 stack trace）
- 除以零：panic
- 解引用 null 裸指针：safe 子集不可达
- `unwrap` on None/Err：panic

这些不是 UB，编译器不能假设它们不发生。

### 6.2 Unsafe 子集中的 UB

即使 `unsafe`，以下仍是 UB：

1. 解引用未对齐指针
2. 解引用悬垂指针（已 free 或从未分配）
3. 读取未初始化内存（`MaybeUninit` 之外）
4. 数据竞争（v0.2）
5. 违反 `unsafe trait` 契约
6. 错误的 lifetime 标注（绕过 borrow checker）
7. `transmute` 到无效类型（如 `transmute::<i32, &i32>(42)`）
8. 调用函数指针类型不匹配
9. 写入 `&T` 内存（通过别名）
10. 通过 `Union` 读取错误 variant（v0.2）

### 6.3 编译器对 UB 的处理

编译器可假设 UB 不发生，做以下优化：

- 假设指针有效（可省略 null 检查）
- 假设无整数溢出（release 模式可做 `n < n + 1` 假设，但 Landin 默认 wrapping）
- 假设 `unsafe trait` 契约满足（可内联 trait method）

---

## 7. Soundness 测试套件

### 7.1 必须包含的测试类别

| 类别 | 测试数 | 来源 |
| --- | --- | --- |
| 7 个修复漏洞的反例 | 7+ | R5 报告 |
| Rust soundness issue 对应测试 | 20+ | rustc issue tracker |
| Memory safety 测试 | 50+ | fuzzing + 手工 |
| Drop order 测试 | 30+ | 边界 case |
| Borrow check 边界 | 100+ | NLL + two-phase |
| Trait coherence | 30+ | orphan + overlapping |
| Lifetime 边界 | 50+ | HRTB + elision |
| `unsafe` 边界 | 50+ | 各类 UB |
| **合计** | **340+** | |

### 7.2 测试方法

- **静态测试**：编译期必须报错的程序（应被拒绝）
- **动态测试**：运行时必须 panic 或正确行为的程序
- **Fuzzing**：自动生成程序，验证编译器不 crash + sound 程序不被错误拒绝
- **Differential testing**：与 rustc 对照（v0.2 加）

---

## 8. 已知限制

### 8.1 Landin MVP 不形式化证明 soundness

完整 Coq 证明需要：

- RustBelt 风格的语义模型
- Iris 分离逻辑
- 数人月 PL 研究者工作

MVP 阶段仅做"工程 soundness"——通过测试 + 与 rustc 对照。完整形式化证明推迟到 v1.0+。

### 8.2 已知风险

- **NLL 算法实现 bug**：rustc NLL 用了 5 年稳定，Landin 实现可能复现类似 bug
- **Trait resolution 边界**：HRTB + associated type + coherence 组合的复杂 case
- **跨 crate soundness**：orphan rule 跨 crate 检查的正确性

### 8.3 缓解

- Conformance 套件 ≥ 3,000 测试
- Soundness 测试 ≥ 340
- 持续 fuzzing
- 与 rustc differential testing（v0.2）

---

## 9. 总结

v1.0/v1.1 存在 7 个 soundness 漏洞，v1.2 全部修复（v1.1 的 CHANGELOG 声称已修复但实际未贯穿到源文档，v1.2 真正落实）：

| 漏洞 | 修复文档章节（v1.2 实际位置） | soundness 保证 |
| --- | --- | --- |
| NLL region inference | 04 §4.6 NLL 算法完整规范 | universal region + type tests + universe + SCC 压缩 |
| Normalization 终止 | 03 §7.1 Normalization 算法与终止性保证 | depth limit=32 + cycle detection + placeholder |
| Drop check | 04 §5 Drop check | `#[may_dangle]` + 逆序 drop + RFC 1327 |
| FalseEdge | 06 §7 Terminator（FalseEdge variant 恢复）+ §12 差异表 | match guard CFG 精确建模 |
| Lifetime elision 边界 | 04 §3.2 Lifetime elision 规则（含 5 类边界 case） | 强制显式，不静默选择 |
| 整数 fallback + trait | 03 §4.6 整数 fallback（仅无 trait constraint 时触发） | fallback 不触发 trait selection |
| `?` From 歧义 | 03 §5.10 `?` 操作符与 From trait 唯一性 | 唯一 impl 要求 |

Landin MVP 不提供完整形式化 soundness 证明，但通过：

1. 工程实现的严格审查（v1.1 修复）
2. Conformance 套件 ≥ 3,000 测试
3. Soundness 套件 ≥ 340 测试
4. 持续 fuzzing
5. 与 Rust soundness hole 历史对照

达成"工程 soundness"——safe 子集程序不会因编译器 bug 而产生 UB。

完整形式化证明（RustBelt 风格）推迟到 v1.0+ 学术合作。

---

**Landin 蓝图 v1.3.2 — 完**

下一步：思考-设计阶段完成，可进入实现-测试-报告-修正循环的第一轮（Stage 0 Lexer + Parser 实现）。
