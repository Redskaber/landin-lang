# 05 — AST 结构定义

> 本文定义 Landin 的 AST（Abstract Syntax Tree）数据结构，作为 parser 的输出与后续 pass 的输入。AST **仅承载语法**，不做名称解析与类型检查。所有静态分析在 HIR/MIR 上做。

---

## 1. 设计原则

1. **Span 完备**：每个节点携带 span（`Span` 类型），用于错误信息
2. **Arena 分配**：所有 AST 节点分配在 arena 上，用 `NodeId` 引用，避免生命周期污染（参考 rustc `la_arena` 模式）
3. **不可变**：AST 一旦构建不再修改，所有"修改"通过 HIR lowering 完成
4. **保留宏形状**：MVP 无宏，但 AST 结构预留 `MacroCall` 节点（v0.2 用）

---

## 2. 顶层结构

```rust
// 文件 ID 与位置
struct Span {
    lo: BytePos,        // u32，字节 offset
    hi: BytePos,
    file_id: FileId,    // u32
}

// 整个 crate 的 AST
struct Crate {
    items: Vec<Item>,
    attrs: Vec<Attr>,   // crate-level attrs（如 #![no_std]）
}

// Item ID（arena 引用）
type ItemId = u32;
type NodeId = u32;
```

---

## 3. Item 定义

```rust
enum Item {
    Fn(FnDecl),
    Const(ConstDecl),
    Static(StaticDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    TypeAlias(TypeAliasDecl),
    ExternBlock(ExternBlock),
    Mod(ModDecl),
    Use(UseDecl),
}

struct ItemKind {
    ident: Ident,
    vis: Visibility,
    attrs: Vec<Attr>,
    span: Span,
    kind: Item,
}

struct Ident {
    name: Symbol,       // interned string
    span: Span,
}

type Symbol = u32;       // 字符串池索引

enum Visibility {
    Public,
    Private,
    // v0.2: Crate, Super, InPath(Path)
}

struct Attr {
    path: Path,
    args: Option<AttrArgs>,
    span: Span,
}

enum AttrArgs {
    Empty,
    Literal(LitKind),
    Eq(Expr),
    List(Vec<AttrArg>),
}

struct AttrArg {
    name: Option<Ident>,
    value: Option<Expr>,
}
```

---

## 4. 函数与参数

```rust
struct FnDecl {
    sig: FnSig,
    body: Option<Block>,   // None = extern fn
    generics: Generics,
}

struct FnSig {
    inputs: Vec<Param>,
    output: FnRetTy,
    abi: Abi,                // Landin / C / System
    is_unsafe: bool,
    is_async: bool,          // MVP always false
    is_const: bool,          // v0.2
    span: Span,
}

enum Abi {
    Landin,
    C,
    System,
}

struct Param {
    pat: Pat,
    ty: Ty,
    attrs: Vec<Attr>,
    span: Span,
}

enum FnRetTy {
    Default(Span),           // 返回 ()
    Ty(Ty),
}

struct Block {
    stmts: Vec<Stmt>,
    expr: Option<Expr>,      // 尾表达式
    span: Span,
}
```

---

## 5. Generics 与 bound

```rust
struct Generics {
    params: Vec<GenericParam>,
    where_clause: Vec<WherePredicate>,
    span: Span,
}

enum GenericParam {
    Lifetime(LifetimeParam),
    Type(TypeParam),
    // v0.2: Const(ConstParam)
}

struct LifetimeParam {
    ident: Ident,
    bounds: Vec<Lifetime>,
    attrs: Vec<Attr>,
    span: Span,
}

struct TypeParam {
    ident: Ident,
    bounds: Vec<TypeBound>,
    default: Option<Ty>,    // v0.2: 默认类型参数
    attrs: Vec<Attr>,
    span: Span,
}

struct Lifetime {
    ident: Ident,           // 'a / 'static
    span: Span,
}

enum TypeBound {
    Trait(TraitBound),
    Lifetime(Lifetime),
    // v0.2: MaybeSized,    // ?Sized
}

struct TraitBound {
    is_modified: bool,       // v0.2: ?Trait
    path: Path,
    args: Vec<GenericArg>,
    span: Span,
}

struct WherePredicate {
    lifetime: Option<Lifetime>,
    bounded_ty: Ty,
    bounds: Vec<TypeBound>,
    span: Span,
}
```

---

## 6. Type 定义

```rust
enum Ty {
    // 基本类型
    Bool(Span),
    Char(Span),
    Int(IntTy, Span),
    Uint(UintTy, Span),
    Float(FloatTy, Span),
    Never(Span),               // !
    
    // 复合
    Tuple(Vec<Ty>, Span),
    Array(Box<Ty>, Box<Expr>, Span),   // [T; N]
    Slice(Box<Ty>, Span),              // [T]
    
    // 引用
    Ref(Option<Lifetime>, Mutability, Box<Ty>, Span),
    
    // 裸指针
    Ptr(Mutability, Box<Ty>, Span),
    
    // 函数指针（v1.2 修正：合并 Fn 与 FnPtr 为单一 FnPtr variant）
    FnPtr { inputs: Vec<Ty>, output: Box<Ty>, abi: Abi, is_unsafe: bool, span: Span },
    
    // 用户定义
    Path(QSelf, Path, Span),
    
    // Trait object
    TraitObject {
        bounds: Vec<TypeBound>,
        lifetime: Option<Lifetime>,
        span: Span,
    },
    
    // impl Trait
    ImplTrait(Vec<TypeBound>, Span),
    
    // 推导
    Infer(Span),                // _
}

enum IntTy { I8, I16, I32, I64, I128, Isize }
enum UintTy { U8, U16, U32, U64, U128, Usize }
enum FloatTy { F32, F64 }
enum Mutability { Mutable, Immutable }

struct QSelf {
    ty: Box<Ty>,
    position: usize,   // path 中 < as Trait > 后的位置
}

struct Path {
    segments: Vec<PathSegment>,
    leading_colon: Option<Span>,   // :: 开头
    span: Span,
}

struct PathSegment {
    ident: Ident,
    args: Option<GenericArgs>,
}

enum GenericArgs {
    AngleBracketed(Vec<GenericArg>),
    Parenthesized(Vec<Ty>, Box<Ty>),   // Fn(...) -> U 形式
}

enum GenericArg {
    Lifetime(Lifetime),
    Type(Ty),
    // v0.2: Const(Expr),
    Assoc(AssocBinding),
}

struct AssocBinding {
    ident: Ident,
    ty: Ty,
    span: Span,
}
```

---

## 7. Pattern 定义

```rust
enum Pat {
    Wild(Span),
    Ident(BindingMode, Ident, Option<Box<Pat>>),    // x, x @ pat, mut x
    Struct(Path, Vec<PatField>, bool /* has_rest */, Span),
    TupleStruct(Path, Vec<Pat>, Span),
    Tuple(Vec<Pat>, Span),
    Slice(Vec<Pat>, Option<Box<Pat>>, Span),     // v1.2 新增：[a, b, .., c] 数组/slice 模式
    Or(Vec<Pat>, Span),
    Path(Path, Span),
    Lit(Expr),
    Range(Option<Box<Expr>>, Option<Box<Expr>>, RangeEnd, Span),
    Ref(Box<Pat>, Mutability, Span),
    Box(Box<Pat>, Span),       // v0.2: box pattern
    Rest(Span),                 // ..
    Deref(Span),                // v0.2: deref pattern
}

struct PatField {
    ident: Ident,
    pat: Pat,
    is_shorthand: bool,        // Point { x } 而非 Point { x: x }
}

enum BindingMode {
    ByValue,
    ByRef(Mutability),
}

enum RangeEnd {
    Included,    // ..
    Excluded,    // ..=
}
```

---

## 8. 表达式定义

```rust
enum Expr {
    // 字面量
    Lit(LitKind, Span),
    
    // 路径
    Path(Option<QSelf>, Path, Span),
    
    // 块
    Block(Block, Span),
    
    // 调用
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
        generic_args: Option<GenericArgs>,
        span: Span,
    },
    
    // 字段
    Field {
        receiver: Box<Expr>,
        ident: Ident,
        span: Span,
    },
    
    // 索引
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    
    // 一元
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    
    // 二元
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    
    // 赋值
    Assign {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op: Option<BinOp>,    // += -= etc.
        span: Span,
    },
    
    // 借用
    AddrOf {
        mutability: Mutability,
        expr: Box<Expr>,
        span: Span,
    },
    
    // 解引用（语法糖，自动插入）
    Deref {
        expr: Box<Expr>,
        span: Span,
    },
    
    // 类型转换
    Cast {
        expr: Box<Expr>,
        ty: Ty,
        span: Span,
    },
    
    // 错误传播
    Try {
        expr: Box<Expr>,
        span: Span,
    },
    
    // 控制流
    If {
        cond: Box<Expr>,
        then: Block,
        else_: Option<Box<Expr>>,
        span: Span,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<Arm>,
        span: Span,
    },
    Loop {
        body: Block,
        span: Span,
    },
    While {
        cond: Box<Expr>,
        body: Block,
        span: Span,
    },
    For {
        pat: Pat,
        iter: Box<Expr>,
        body: Block,
        span: Span,
    },
    
    // 闭包
    Closure {
        is_move: bool,                     // v1.2 修正：capture_mode → is_move（move closure v0.2）
        params: Vec<Param>,
        body: Box<Expr>,
        span: Span,
    },
    
    // Return / break / continue
    Return {
        expr: Option<Box<Expr>>,
        span: Span,
    },
    Break {
        label: Option<Ident>,    // MVP: always None
        expr: Option<Box<Expr>>,
        span: Span,
    },
    Continue {
        label: Option<Ident>,
        span: Span,
    },
    
    // Struct 字面量
    Struct {
        path: Path,
        fields: Vec<ExprField>,
        spread: Option<Box<Expr>>,    // v0.2: ..base
        span: Span,
    },
    
    // 数组
    Array {
        elems: Vec<Expr>,        // [a, b, c]
        span: Span,
    },
    Repeat {
        elem: Box<Expr>,
        count: Box<Expr>,         // [elem; count]
        span: Span,
    },
    
    // Tuple
    Tuple {
        elems: Vec<Expr>,
        span: Span,
    },
    
    // Range（v1.2 新增：02 文法支持 range_expr，AST 必须有对应 variant）
    Range {
        start: Option<Box<Expr>>,    // None = ..end
        end: Option<Box<Expr>>,      // None = start..
        end_kind: RangeEnd,           // Included (..=) / Excluded (..)
        span: Span,
    },
    
    // 内建宏调用（v1.2 修正：args 改为 Vec<TokenTree>，TokenStream 是 parser 内部状态）
    MacroCall {
        mac: Path,
        args: Vec<TokenTree>,        // v1.2 修正：TokenStream → Vec<TokenTree>
        span: Span,
    },
    
    // Unsafe
    Unsafe(Block, Span),
    
    // Group (for token tree manipulation)
    Group(Box<Expr>, Span),
}

enum UnaryOp {
    Neg,    // -
    Not,    // !
    Deref,  // *（显式解引用语法）
}

enum BinOp {
    Add, Sub, Mul, Div, Rem,    // + - * / %
    BitAnd, BitOr, BitXor,       // & | ^
    Shl, Shr,                     // << >>
    And, Or,                       // && ||
    Eq, Ne, Lt, Le, Gt, Ge,       // == != < <= > >=
}

struct Arm {
    pat: Pat,
    guard: Option<Expr>,
    body: Box<Expr>,
    span: Span,
}

struct ExprField {
    ident: Ident,
    expr: Option<Expr>,    // None = shorthand
    span: Span,
}

enum LitKind {
    Bool(bool),
    Int(u128, IntTy),    // 整数字面量
    Uint(u128, UintTy),
    Float(f64, FloatTy),
    Char(char),
    Str(Symbol),                  // 字符串字面量
    ByteStr(Vec<u8>),
    RawStr(Symbol, usize),       // r#"..."# 的 hash 数
    Byte(u8),
}
```

---

## 9. Statement 定义

```rust
enum Stmt {
    Local(LocalDecl),
    Expr(Expr, Semicolon),
    Item(ItemId),    // v0.2: 嵌套 item
    Empty(Span),
}

enum Semicolon {
    Yes(Span),
    No,
}

struct LocalDecl {
    pat: Pat,
    ty: Option<Ty>,
    init: Option<Expr>,
    else_block: Option<Block>,    // v0.2: let-else
    attrs: Vec<Attr>,
    span: Span,
}
```

---

## 10. 类型声明

```rust
struct StructDecl {
    ident: Ident,
    generics: Generics,
    fields: Vec<StructField>,
    is_unit: bool,
    is_tuple: bool,
    semi: Option<Span>,    // unit struct 的 ; 
    attrs: Vec<Attr>,
    span: Span,
}

struct StructField {
    vis: Visibility,
    ident: Option<Ident>,   // None for tuple struct
    ty: Ty,
    attrs: Vec<Attr>,
    span: Span,
}

struct EnumDecl {
    ident: Ident,
    generics: Generics,
    variants: Vec<EnumVariant>,
    attrs: Vec<Attr>,
    span: Span,
}

struct EnumVariant {
    ident: Ident,
    data: VariantData,
    discriminant: Option<Expr>,
    attrs: Vec<Attr>,
    span: Span,
}

enum VariantData {
    Unit(Span),
    Tuple(Vec<StructField>, Span),
    Struct(Vec<StructField>, Span),
}

struct TraitDecl {
    ident: Ident,
    generics: Generics,
    supertraits: Vec<TypeBound>,
    items: Vec<TraitItem>,
    attrs: Vec<Attr>,
    span: Span,
}

enum TraitItem {
    Fn(TraitFn),
    Type(TraitType),
    Const(TraitConst),
}

struct TraitFn {
    sig: FnSig,
    body: Option<Block>,   // None = required method
    default: bool,
    span: Span,
}

struct TraitType {
    ident: Ident,
    bounds: Vec<TypeBound>,
    default: Option<Ty>,
    span: Span,
}

struct TraitConst {
    ident: Ident,
    ty: Ty,
    default: Option<Expr>,
    span: Span,
}

struct ImplDecl {
    generics: Generics,
    of_trait: Option<Path>,    // None = inherent impl
    self_ty: Ty,
    items: Vec<ImplItem>,
    attrs: Vec<Attr>,
    span: Span,
}

enum ImplItem {
    Fn(FnDecl),
    Const(ConstDecl),
    Type(TypeAliasDecl),
}

struct ConstDecl {
    ident: Ident,
    ty: Ty,
    expr: Expr,
    vis: Visibility,
    is_const: bool,        // const vs static
    is_mut: bool,           // static mut
    attrs: Vec<Attr>,
    span: Span,
}

// StaticDecl 同 ConstDecl 但 is_const = false

struct TypeAliasDecl {
    ident: Ident,
    generics: Generics,
    ty: Ty,
    vis: Visibility,
    attrs: Vec<Attr>,
    span: Span,
}

struct ExternBlock {
    abi: Abi,
    items: Vec<ExternItem>,
    attrs: Vec<Attr>,
    span: Span,
}

enum ExternItem {
    Fn(FnDecl),     // body = None
    Static(ConstDecl),
}

struct ModDecl {
    ident: Ident,
    kind: ModKind,
    attrs: Vec<Attr>,
    span: Span,
}

enum ModKind {
    Inline(Vec<Item>, Span),
    Loaded,    // 从文件加载
}

struct UseDecl {
    tree: UseTree,
    span: Span,
}

enum UseTree {
    Path {
        prefix: Path,
        children: Vec<UseTree>,
    },
    Leaf(Path, Option<Ident>),    // use a::b as c
    Glob(Path),
}
```

---

## 11. Parser 实现要点

### 11.1 Token 流

```rust
struct TokenStream {
    tokens: Vec<Token>,
    pos: usize,
}

struct Token {
    kind: TokenKind,
    span: Span,
}

enum TokenKind {
    Ident(Symbol),
    Lifetime(Symbol),
    Keyword(Keyword),
    Literal(LitKind),
    Op(Operator),
    Punct(Punct),
    DocComment(Symbol),    // /// doc 或 //! doc
    Eof,
}
```

### 11.2 Parser 结构

```rust
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    session: &'a ParseSession,    // 错误收集
    type_context: bool,            // lexer hack 标志
}

impl<'a> Parser<'a> {
    fn parse_crate(&mut self) -> Crate { ... }
    fn parse_item(&mut self) -> Option<ItemKind> { ... }
    fn parse_fn(&mut self) -> FnDecl { ... }
    // ... 每个产生式一个方法
    
    // Pratt parser
    fn parse_expr(&mut self) -> Expr { self.parse_expr_with_min_bp(0) }
    fn parse_expr_with_min_bp(&mut self, min_bp: u8) -> Expr { ... }
}
```

### 11.3 错误恢复

错误恢复策略：

1. 遇到错误，记录到 session
2. 跳过至下一个 synchronization token：`;`、`}`、`fn`、`struct`、`impl`、`trait`、`pub`、`use`
3. 插入 `Err` 占位节点
4. 继续 parse

错误节点的 span 是"跳过的范围"，方便后续 pass 识别。

---

## 12. HIR 与 AST 的真正差异（v1.2 重写）

R6 报告指出 v1.0/v1.1 的 "HIR 与 AST 共享 80% variant" 是错误描述。HIR 与 AST 的差异远超 name resolution + desugaring。

### 12.1 HIR 独有的核心机制

HIR 引入 AST 没有的三个核心机制：

1. **HirId**：HIR 节点的全局唯一标识。每个 HIR 节点都有 HirId（独立于 NodeId），用于挂载 typeck 结果、borrow check 结果、incremental compilation 缓存。
2. **Body / BodyId**：函数体与 item 分离存储。HIR Item 只存函数签名，函数体单独存储在 `bodies: IndexVec<BodyId, Body>` 中。这让"遍历所有 item"与"遍历函数体"分离，加速 name resolution 等不需函数体的 pass。
3. **OwnerNodes**：每个 item 是一个 "owner"，owner 下有多个 HirId 节点。Owner 与 HirId 形成两层树结构。

### 12.2 HIR 数据结构（关键部分）

```rust
struct Crate<'hir> {
    owners: IndexVec<OwnerId, OwnerNodes<'hir>>,
    bodies: IndexVec<BodyId, Body<'hir>>,
    items: IndexVec<ItemId, Item<'hir>>,
}

struct OwnerNodes<'hir> {
    node: Node<'hir>,                // owner 节点本身
    nodes: IndexVec<ItemLocalId, Node<'hir>>,  // owner 内的所有节点
}

type HirId = (OwnerId, ItemLocalId);  // 两层 ID

struct Body<'hir> {
    params: Vec<Param<'hir>>,
    value: Expr<'hir>,
}

enum Node<'hir> {
    Item(&'hir Item<'hir>),
    Expr(&'hir Expr<'hir>),
    Pat(&'hir Pat<'hir>),
    Ty(&'hir Ty<'hir>),
    Stmt(&'hir Stmt<'hir>),
    // ...
}
```

### 12.3 HIR 与 AST 的实际共享比例

| 部分 | AST | HIR | 共享 |
| --- | --- | --- | --- |
| Expr | `Expr` enum | `Expr` enum（带 HirId） | ~60% variant 共享，但字段不同 |
| Pat | `Pat` enum | `Pat` enum（带 HirId） | ~70% |
| Ty | `Ty` enum | `Ty` enum（带 HirId） | ~80% |
| Item | `ItemKind` enum | `ItemKind` enum（去 Macro, 加 Impl 自带 body 引用） | ~50% |
| Stmt | `Stmt` enum | `Stmt` enum | ~80% |
| Body | 无 | `Body` struct | **HIR 独有** |
| OwnerNodes | 无 | `OwnerNodes` struct | **HIR 独有** |

**总体共享比例约 50%**（按 variant 数加权），不是 80%。

### 12.4 HIR lowering 做的变换

AST → HIR lowering 做以下变换：

1. **Name resolution**：所有路径解析为 fully-qualified（含 DefId）
2. **Lifetime elision**：应用 elision 规则补全 lifetime
3. **Default trait method**：在 trait impl 中插入未 override 方法的默认实现
4. **Desugaring**：
   - `?` → match + `From::from`
   - `for x in iter` → `while let Some(x) = Iterator::next(&mut __it)`
   - `+=` → `AddEq::add_assign`
   - `if let` → `match`
   - `while let` → `loop { match ... }`
   - Range `a..b` → `Range::new(a, b)`
5. **Pattern simplification**：嵌套模式展开为 match 嵌套
6. **HirId 分配**：每个节点分配唯一 HirId
7. **Body 外置**：函数体从 Item 中提取到 `bodies` 表
8. **Attribute 收集**：attribute 收集到 OwnerNodes

### 12.5 MVP 实现

MVP 阶段简化：

- 可省略 incremental compilation 相关的 HirId 缓存（v0.2 加）
- Body 外置必须实现（typeck 与 borrow check 依赖）
- OwnerNodes 必须实现（visibility check 依赖）

参考 rustc `compiler/rustc_hir/src/hir.rs` 完整定义。

---

**下一文档**: [`06-mir.md`](./06-mir.md) — MIR 设计

---

## 13. 实现状态（v0.14.0，§25.8 回写）

> 本节由 Stage 6.18 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 13.1 §2-§8 AST 结构 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §2 顶层结构 (Crate) | ✅ 实现 | — | `ast::kinds::Crate` |
| §3 Item 定义 (fn/const/static/struct/enum/trait/impl/type/extern/mod/use) | ✅ 实现 | — | `ast::kinds::Item` |
| §4 函数与参数 | ✅ 实现 | — | `ast::kinds::FnDecl` + `Param` |
| §5 Generics 与 bound | ✅ 实现 | — | `ast::kinds::GenericParam` + `TypeBound` + `WherePredicate` |
| §6 Type 定义 | ✅ 实现 | — | `ast::kinds::Ty` + `TyKind` |
| §7 Pattern 定义 | ✅ 实现 | — | `ast::kinds::Pat` + `PatKind` |
| §8 表达式定义 (30+ ExprKind) | ✅ 实现 | — | `ast::kinds::Expr` + `ExprKind` |
| §9 Statement 定义 | ✅ 实现 | — | `ast::kinds::Stmt` |
| §10 类型声明 (struct/enum field) | ✅ 实现 | — | `ast::kinds::StructDecl` + `EnumDecl` + `HirVariantData` |

### 13.2 §12 HIR 与 AST 的差异 — 实现状态

| 设计 § | 实现状态 | 偏差类型 | 说明 |
|--------|---------|---------|------|
| §12.1 HIR 独有机制 (Res / OwnerNode / Body) | ✅ 实现 | — | `hir::kinds::Res` + `OwnerNode` + `Body` |
| §12.2 HIR 数据结构 | ✅ 实现 | — | `hir::kinds` + `hir::id` + `hir::map` |
| §12.3 HIR 与 AST 共享比例 | ✅ 实现 | B3（更高） | 实现中 HIR 与 AST 共享更多类型（如 `Path`、`Ident`、`Visibility`），减少重复 |
| §12.4 HIR lowering 变换 | ✅ 实现 | — | `hir::lower::lower_crate` + `HirLowerCtxt` |
| §12.5 MVP 实现 | ✅ 实现 | — | Stage 1 完成 |

### 13.3 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B3（HIR/AST 共享比例更高） | 接受为永久偏差 | 实现更优（DRY），无需重构 |

**B4 补写**：设计文档 §8 表达式定义未描述的 HIR 扩展（`HirExprKind` 比 `ExprKind`
多 `MethodCall` / `Unsafe` / `Try` 等 variant）已在 `hir::kinds::HirExprKind` 实现。

---

## 14. Stage 8 实现状态更新（v0.15.4，§25.8 回写）

> 本节由 Stage 8.6 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。

### 14.1 §8 表达式定义 — Stage 8 扩展

| 设计 § | Stage 7 状态 | Stage 8 状态 | 实现 |
|--------|-------------|-------------|------|
| §8 表达式 (Await) | ❌ 未实现 | ✅ (8.5) | `Expr::Await { expr, span }` + `HirExprKind::Await` |
| §8 表达式 (Async) | ❌ 未实现 | ✅ (8.5) | `Expr::Async { block, span }` + `HirExprKind::Async` |
| §8 表达式 (其他 30+ variant) | ✅ 实现 | 不变 | — |

### 14.2 偏差处理计划更新

| 偏差 | Stage 7 计划 | Stage 8 更新 |
|------|-------------|-------------|
| B4（HIR 扩展: Await/Async） | — | ✅ **已补写** (8.5) |
| B3（HIR/AST 共享比例更高） | 接受为永久偏差 | 不变 |

**结论**: Stage 8.5 新增 `Await`/`Async` 表达式 variant，设计文档 §8 已覆盖。

---

## 15. Stage 12.4 §25.8 追溯回写（v0.21.2，r217 二次审查）

> 本节由 Stage 12.4（r217 二次审查）依据流程 v3.21 §25.8 追溯回写协议生成。
> Stage 8.5 async/await MVP 实现决策仅记录在 `src/ast/async_marker.rs` 模块注释，
> 设计文档未明确"MVP synchronous"语义；本节补写。
> 审计来源: `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §5.4

### 15.1 async/await MVP 同步语义补写（B4 设计灰区）

**实现来源**: Stage 8.5 (async/await 基础设施)
**代码位置**: `src/ast/async_marker.rs` (74 LOC) + `src/parser/expr.rs:667-680` (parser) + `src/mir/lower/expr_operand.rs:1147-1148` (lowering)

**AST/HIR 扩展**:

| 节点 | AST variant | HIR variant | 引入阶段 |
|------|-------------|-------------|---------|
| `async { block }` | `Expr::Async { block }` | `HirExprKind::Async { block }` | 8.5 |
| `expr.await` | `Expr::Await { expr }` | `HirExprKind::Await { expr }` | 8.5 |

**MVP 同步语义**（设计决策，本节明确记录）:

| 表达式 | 完整 async 语义 (v0.2+) | MVP 同步语义 (Stage 8.5, v0.1) |
|---------|----------------------|------------------------------|
| `async { block }` | 编译为 `Future` state machine | 直接 lower 内部 block，无 state machine transform |
| `expr.await` | 挂起当前 task，调度器介入 | 直接求值内部 expr，无挂起 |

**理由**: v0.1 不实现 async runtime（无 `tokio`/`async-std` 等价物）。
MVP 同步语义保证 `async fn` 能 parse + type check + codegen 通过，但实际执行等同同步代码。
这是 v0.2+ async runtime 实现的占位符。

**回写动作**: §8 表达式定义章节新增 `Await` / `Async` variant 的"MVP 同步语义"标注。

### 15.2 设计偏差状态（截至 v0.21.2）

| 偏差 | 类型 | 状态 | 计划 |
|------|------|------|------|
| HIR 扩展: Await/Async | B4 | ✅ 已回写 (8.5) | §14.2 |
| HIR/AST 共享比例更高 | B3（已接受） | — | 永久偏差 |
| async/await MVP 同步语义 | B4 | ✅ 本节回写 | §15.1 |
| async/await 完整 state machine | B1 | ❌ 未实现 | v0.2+ (async runtime) |
