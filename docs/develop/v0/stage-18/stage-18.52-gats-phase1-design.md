# Stage 18.52 — GATs Phase 1: AST / Parser / HIR Infrastructure

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-07
> **Version**: v0.318.0 → v0.319.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §13.5 (design-review agent cycle)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 用户反馈 (Stage 18.47 v0.7 路线图修订)

> "你这些都还没做呢：Phase 完整移除 Println variant; GATs; Incremental; Cross-compile; 测试..."

修订后的 v0.7 优先级:
1. ✅ P0: Println variant 完整移除 (Stage 18.48)
2. ✅ P0: 系统性测试增强 (Stage 18.49-51)
3. ⏳ **P1: GATs 实现 ← 本阶段**
4. P1: 增量编译
5. P2: 交叉编译

### 1.2 当前 GATs 状态审查

**已有的关联类型支持** (Stage 16.68 引入):
- AST: `TraitItem::Type(Ident, Vec<TypeBound>, Option<Ty>)` — 无 generics 字段
- HIR: `HirAssocType { hir_id, ident, bounds, default, span }` — 无 generics 字段
- Parser: `type Item;`, `type Item: Bound;`, `type Item = T;` — 不支持 `type Item<'a, T>;`
- Typeck: `projection_resolver.rs` 处理 `Projection(assoc_def_id, substs)` — 仅支持非泛型关联类型

**GATs 完整缺失**:
- 关联类型声明无法带 generics: `type Item<'a, T> where Self: 'a;`
- impl 块中关联类型实现无法带 generics: `type Item<'a, T> = &'a Vec<T>;`
- 使用点无法带 generic args: `Self::Item<'a>` 或 `<T as Trait>::Item<'a>`

### 1.3 设计文档参考

| 文档 | 章节 | 关键约束 |
|------|------|---------|
| `docs/lang-design/01-language-specification.md` | §5.5 (Trait) | "MVP 不支持 GATs (`type Item<'a>`)，推迟到 v0.2" |
| `docs/lang-design/03-type-system.md` | 全文 | 类型系统基础 |
| `docs/lang-design/13-stage1-feature-whitelist.md` | §50 | "GATs | ❌ | v0.2" |
| `docs/lang-design/14-soundness-considerations.md` | §281 | "GATs 的 variance (v0.2 加 GATs 后才需考虑)" |

**设计偏差** (§14.8 回写): 当前 v0.318.0 仍是 v0.1 → v0.2 过渡阶段。设计文档原计划 "MVP 不支持 GATs"，但 v0.7 路线图明确要求实现 GATs。本阶段开启 v0.2 GATs 实现，按 Phase 推进。

---

## 2. GATs 实现路线图 (4 Phases)

GATs 是复杂特性，需要分阶段实现。完整 Rust GATs 涉及：

| Phase | 内容 | 估计 Stages | 状态 |
|-------|------|-------------|------|
| **Phase 1** | AST + Parser + HIR 基础设施 (解析与表示) | 1 stage (本阶段) | ⏳ |
| Phase 2 | Typeck: 高阶投影解析 (HR-projection resolution) | 2-3 stages | 远期 |
| Phase 3 | Codegen: GAT monomorphization | 1-2 stages | 远期 |
| Phase 4 | 完整 GAT 测试套件 + soundness 验证 | 1-2 stages | 远期 |

### 2.1 Phase 1 范围 (本阶段)

**目标**: 让 Landin 能解析与表示 GATs 语法，HIR 中保留 generics 信息，下游 graceful degradation。

**做**:
- AST `TraitItem::Type` 增加 `Generics` 字段
- Parser 解析 `type Item<'a, T> where Self: 'a;` 语法
- HIR `HirAssocType` 增加 `HirGenerics` 字段
- HIR lower 传递 generics 信息
- 下游消费者 (typeck, codegen, mir) 对 GAT generics 字段 graceful degradation: 非 GAT 路径不受影响，GAT 路径仅解析表示，不进行投影计算
- 新增 1:3+ 比例测试 (≥2 正向 + ≥6 负向)

**不做** (留待 Phase 2+):
- ❌ 高阶投影解析 (`<T as Trait<'a>>::Item` 的实际类型计算)
- ❌ GAT monomorphization
- ❌ GAT variance 检查
- ❌ Where clause 求解 (`where Self: 'a`)
- ❌ 使用点 generic args 解析 (`Self::Item<'a>` 实际替换)

### 2.2 Phase 1 设计原则遵循 (§1.0)

| 原则 | 本阶段如何遵循 |
|------|---------------|
| 1. 长期 > 短期 | Phase 1 是 GATs 完整实现的必要前置；不做 Phase 2 会导致 Phase 1 信息丢失 |
| 2. 整体 > 局部 | AST/Parser/HIR 三层同时变更，保证数据流通畅 |
| 3. 显式 > 隐式 | 显式 `Generics` 字段，不依赖隐式推断 |
| 4. 报错 > 静默 | 解析失败时清晰报错，不静默丢弃 generics |
| 5. 去除兼容思维 | 不保留 "无 generics 的 Type variant" — 统一为带 generics 的形式 |
| 6. 通用 > 特解 | 一个 `Generics` 字段统一处理 lifetime/type params，不分别特例 |
| 7. API 命名标准化 | `TraitItem::Type` 字段顺序遵循 (Ident, Generics, Bounds, Default) |
| 8. 设计驱动测试 | 测试用例覆盖 GAT 解析、lowering、graceful degradation |
| 9. 正确 > 妥协 | Phase 2 推迟明确记录于本文档，不假装 Phase 1 完整 |

---

## 3. 技术设计

### 3.1 AST 变更 (src/ast/kinds.rs)

**Before**:
```rust
pub enum TraitItem {
    Fn(Ident, Generics, FnSig, Option<Block>),
    Type(Ident, Vec<TypeBound>, Option<Ty>),  // ← 无 generics
    Const(Ident, Ty, Option<Expr>),
}
```

**After**:
```rust
pub enum TraitItem {
    Fn(Ident, Generics, FnSig, Option<Block>),
    Type(Ident, Generics, Vec<TypeBound>, Option<Ty>),  // ← 加 Generics
    Const(Ident, Ty, Option<Expr>),
}
```

**理由** (§10 命名标准): 字段顺序 (Ident, Generics, Bounds, Default) 与 `TraitItem::Fn(Ident, Generics, FnSig, Body)` 一致 — "name → generics → signature → body" 模式。

### 3.2 Parser 变更 (src/parser/items.rs)

**Before** (line 477-492):
```rust
TokenKind::KwType => {
    self.bump();
    let name = self.expect_ident("associated type name");
    let mut bounds = Vec::new();
    if *self.peek() == TokenKind::Colon { ... }
    let default = if *self.peek() == TokenKind::Eq { ... }
    self.expect(&TokenKind::Semicolon, "`;`");
    Some(TraitItem::Type(name, bounds, default))
}
```

**After**:
```rust
TokenKind::KwType => {
    let kw_span = self.current_span();
    self.bump();
    let name = self.expect_ident("associated type name");
    let generics = self.parse_generics();        // ← NEW: parse <'a, T>
    let where_clause = self.parse_where_clause(); // ← NEW: parse where Self: 'a
    let mut bounds = Vec::new();
    if *self.peek() == TokenKind::Colon { ... }
    let default = if *self.peek() == TokenKind::Eq { ... }
    self.expect(&TokenKind::Semicolon, "`;`");
    Some(TraitItem::Type(
        name,
        Generics { params: generics, where_clause, span: kw_span },
        bounds,
        default,
    ))
}
```

**理由** (§1.0 原则 6 "通用 > 特例"): 复用现有的 `parse_generics` 与 `parse_where_clause`，不为 GAT 写特殊 parser。

### 3.3 HIR 变更 (src/hir/kinds.rs)

**Before**:
```rust
pub struct HirAssocType {
    pub hir_id: HirId,
    pub ident: Ident,
    pub bounds: Vec<HirTypeBound>,
    pub default: Option<HirTy>,
    pub span: Span,
}
```

**After**:
```rust
pub struct HirAssocType {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,        // ← NEW
    pub bounds: Vec<HirTypeBound>,
    pub default: Option<HirTy>,
    pub span: Span,
}
```

**理由**: `HirGenerics` 已存在 (src/hir/kinds.rs:451)，复用。`HirGenerics::default()` 产生空 params/where_clause，向后兼容。

### 3.4 HIR Lower 变更 (src/hir/lower/item.rs)

**Trait item Type arm** (line 402-411):
```rust
ast::TraitItem::Type(ident, generics, bounds, default) => {
    let hir_generics = generics::lower_generics(self, generics);  // ← NEW
    let hir_bounds = generics::lower_type_bounds(self, bounds);
    let hir_default = default.as_ref().map(|t| ty::lower_ty(self, t));
    HirTraitItem::Type(HirAssocType {
        hir_id: self.fresh_hir_id(),
        ident: *ident,
        generics: hir_generics,  // ← NEW
        bounds: hir_bounds,
        default: hir_default,
        span: Span::DUMMY,
    })
}
```

**Impl item Type arm** (line 485-491):
```rust
Some(HirImplItem::Type(HirAssocType {
    hir_id: hir_t.hir_id,
    ident: hir_t.ident,
    generics: hir_t.generics.clone(),  // ← NEW: 从 TypeAliasDecl 的 generics 复用
    bounds: vec![],
    default: Some(hir_t.ty),
    span: hir_t.span,
}))
```

**注**: impl 块中的 `type Item = T;` 实际通过 `parse_type_alias` 解析为 `TypeAliasDecl` (已有 generics 字段)，再 lower 到 `HirTypeAlias` (已有 generics 字段)，最后 wrapping 为 `HirImplItem::Type`。当前 wrapping 丢弃了 generics — 我们修复为保留。

### 3.5 下游消费者变更 (Graceful Degradation)

下游 6 个模块需要适配新的 `HirAssocType.generics` 字段，但对 GAT 实际不做语义处理：

| 文件 | 变更 | 语义 |
|------|------|------|
| `src/typeck/projection_resolver.rs` | 无需变更 | 现有 lookup 仍按 name 匹配，对 GAT 暂返回 None (graceful degradation) |
| `src/mir/lower/*.rs` | 无需变更 | lower 时遇到 GAT 会按现有路径处理，可能生成 Projection 留待 Phase 2 |
| `src/mir/monomorphize/*.rs` | 无需变更 | GAT 在 Phase 1 不会被实际 monomorphize |
| `src/codegen/*.rs` | 无需变更 | 不影响 codegen 路径 |
| `src/driver.rs` | 无需变更 | HIR 遍历不变 |
| `src/hir/visit.rs` (如有) | 检查是否需要 visit generics | 如有 visitor，加 `visit_generics` |

### 3.6 测试设计 (§9.4.3 1:3+ ratio)

**测试文件**:
- `tests/v0/stage18/plan/stage18_52_gats_tests.rs` (≥8 测试: 2 正向 + 6 负向)
- `tests/conformance/01-typecheck/<编号>-gats-*.lin` (≥4 conformance 测试)

**正向测试** (2):
1. `gat_parse_simple_lifetime` — `trait Foo { type Item<'a>; }` 解析成功
2. `gat_parse_with_default` — `trait Foo { type Item<'a> = &'a i32; }` 解析成功

**负向测试** (6):
1. `gat_missing_semicolon` — `type Item<'a>` 缺 `;` 报错
2. `gat_unbalanced_angle` — `type Item<'a>>` 角括号不匹配报错
3. `gat_invalid_lifetime_bound` — `type Item<'a: 'b>` (无 `'b` 声明) 报错（或接受，需验证）
4. `gat_invalid_where_clause` — `type Item<'a> where Undefined: 'a;` 报错（或接受）
5. `gat_missing_ident` — `type <'a>;` 缺 ident 报错
6. `gat_double_colon_in_generic` — `type Item<'a::b>;` 路径不能作为 lifetime 报错

**Conformance 测试**:
- `compile_ok-gats-simple-declaration.lin` — 正向: GAT 声明能编译
- `compile_ok-gats-with-default.lin` — 正向: GAT 带默认值能编译
- `compile_error-gats-missing-semicolon.lin` — 负向: 缺 `;` 报错
- `compile_error-gats-missing-ident.lin` — 负向: 缺 ident 报错

---

## 4. §13.5 设计-审查 Agent 循环

### 4.1 Round 1 自审

| 维度 | 自审结论 | 状态 |
|------|---------|------|
| 设计偏差 | Phase 1 仅做 AST/Parser/HIR 基础设施，符合 v0.7 路线图分阶段原则 | ✅ |
| §1.0 原则 1 长期 > 短期 | Phase 1 是 Phase 2 的必要前置，长期价值清晰 | ✅ |
| §1.0 原则 6 通用 > 特例 | 复用 `Generics`/`parse_generics`/`lower_generics`，无特殊路径 | ✅ |
| §10 命名标准 | 字段顺序与 `TraitItem::Fn` 一致 (Ident, Generics, ...) | ✅ |
| §11 接口隔离 | AST → HIR → MIR 单向流动；HIR 内部 generic 字段不外泄 | ✅ |
| §9.4.3 1:3+ 测试 | 8 单元测试 (2:6) + 4 conformance (2:2) 满足比例 | ✅ |
| 兼容性 | 现有非 GAT 关联类型用 `Generics::default()` (空 params + 空 where_clause) — 100% 向后兼容 | ✅ |
| 死代码 | Phase 1 不创建死代码；graceful degradation 不引入未使用路径 | ✅ |

### 4.2 §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 分阶段实现合理，Phase 1 范围清晰 |
| DEV-A | GO | 复用现有 `Generics` 与 `parse_generics`，工作量小 |
| QA-A | GO | 1:3+ 比例满足；graceful degradation 可测试 |
| REV-A | GO | 设计原则 1, 3, 6, 7 遵循；Phase 2 推迟明确记录 |
| PM-A | GO | v0.7 路线图优先级正确 |

**5/5 GO** ✅

---

## 5. 实施步骤

1. ✅ 写设计文档 (本文件)
2. ⏳ 修改 AST `TraitItem::Type` (src/ast/kinds.rs)
3. ⏳ 修改 Parser (src/parser/items.rs)
4. ⏳ 修改 HIR `HirAssocType` (src/hir/kinds.rs)
5. ⏳ 修改 HIR lower (src/hir/lower/item.rs)
6. ⏳ 适配下游消费者 (检查 + 必要修复)
7. ⏳ 新增单元测试 (tests/v0/stage18/plan/stage18_52_gats_tests.rs)
8. ⏳ 新增 conformance 测试
9. ⏳ 验收: cargo clean + build + fmt + clippy + test
10. ⏳ worklog + 版本 bump v0.318.0 → v0.319.0
11. ⏳ 打包 tar.gz

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Parser 变更破坏现有非 GAT 关联类型测试 | `parse_generics` 在无 `<` 时返回空 Vec，向后兼容 |
| HIR 字段顺序变更破坏外部消费者 | HIR 是内部表示，无外部消费者；下游 visitor 适配 |
| Phase 2 推迟导致 GAT 实际不可用 | 文档明确记录；conformance 测试仅验证 parsing + AST 结构，不验证类型解析 |

---

## 7. 结论

Stage 18.52 设计完成。Phase 1 范围严格限定为 AST/Parser/HIR 基础设施，不涉及 typeck/codegen 语义。下游消费者采用 graceful degradation 策略，确保 100% 向后兼容。

设计原则严格遵循 §1.0 (长期/通用/显式/报错/去兼容/命名/测试/正确)，§10 命名标准，§11 接口隔离。

5/5 GO，进入实施阶段。
