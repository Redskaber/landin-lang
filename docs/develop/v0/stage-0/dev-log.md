# Landin Stage 0 开发日志

> **版本**：v0.9.1 (Stage 3.63-3.67 retroactive updates)
> **状态**：Stage 0 前端闭合完成（0 P0 残留 / 0 警告 / 344 测试通过）
> **最后更新**：2026-07-22 (Stage 3.67 — lexer keyword interning + Span::DUMMY fix)

---

## Retroactive Updates (Stage 3.63-3.67)

Stage 0 received the following improvements during Stage 3.63-3.67
(cross-stage naming standardization + P2 cleanup):

- **Stage 3.63**: `src/lexer/mod.rs` + `src/ast/mod.rs` converted from glob
  (`pub use X::*;`) to explicit re-export lists (completes the Stage 3.57
  P0-3 fix). Added `parser::parse_crate` free function wrapper.
- **Stage 3.64**: `LexError` + `ParseError` now implement `Display` +
  `std::error::Error` (integrates with standard Rust error-handling ecosystem).
  Removed 2 orphaned doc comments in `src/lexer/token.rs`.
- **Stage 3.67**: Lexer now interns keyword strings at tokenization time
  (eliminates `&mut Rodeo` smell in `resolve_crate`). Fixed 11 `Span::DUMMY`
  placeholders in `parser.rs` — top-level declarations now carry their
  keyword's actual span.

**Test count**: 245 → 344 (+99 tests added during Stage 1-3 work + 1 unsafe
impl/trait test in Stage 3.65)

---

## 1. 时间线概览

| 月份 | 阶段 | 产出 |
|---|---|---|
| 月 1 | 设计冻结 | 13 篇设计文档（00-overview 到 12-roadmap），BNF 文法定稿，MIR 结构定义 |
| 月 2 | Lexer + Parser + AST | 手写 lexer（1115 行）、recursive-descent + Pratt parser（1473 行）、AST 定义（619 行），245 测试通过 |
| 月 3+ | HIR + Name Resolution | 计划中（见本文档 §7） |

---

## 2. 月 1：项目骨架创建

### 2.1 目标
- 完成全部设计文档（v1.3.2）
- 固化语法、类型系统、所有权、MIR、codegen 等核心决策
- 建立 RFC 仓库与设计冻结流程

### 2.2 产出
- **13 篇设计文档**（位于 `lang-design/`）：00-overview / 01-language-specification / 02-grammar / 03-type-system / 04-ownership-borrowing / 05-ast / 06-mir / 07-codegen / 08-bootstrap-strategy / 09-stdlib / 10-toolchain / 11-testing / 12-roadmap
- **设计冻结报告**（FREEZE-REPORT.md）：所有 RFC 已合入或拒绝

### 2.3 关键决策
- 手写 lexer/parser（不上 flex/re2c/yacc）— 参考 Rust/Hare 经验
- 显式 allocator，无 GC，无 hidden control flow
- Day 1 上 MIR-based NLL borrow check
- 仅 LLVM 后端
- 自举推迟到 v0.3，v0.1 = Stage 0 完整 + conformance 通过

---

## 3. 月 2：Lexer + Parser + AST 实现

### 3.1 目标
- 实现 Lexer：覆盖全部 token 类型 + 错误恢复
- 实现 Parser：recursive-descent + Pratt，覆盖全部产生式
- 定义 AST：完整数据结构
- 200 个 parse 测试

### 3.2 实际产出
- **Lexer**（`src/lexer/`，1023 + 353 + 39 = 1415 行）
  - `reader.rs`：手写字符级扫描器，支持 maximal munch、嵌套块注释、Unicode 标识符、**raw identifier**、**doc comment**
  - `token.rs`：38 个关键字 + 完整运算符/标点 + 9 种字面量 + RawIdent + DocComment
  - `mod.rs`：`tokenize()` 入口，错误恢复非递归（迭代）
- **Parser**（`src/parser/`，1473 + 39 + 15 = 1527 行）
  - `parser.rs`：recursive-descent + Pratt 优先级表，11 个 item 类型、表达式（13 级优先级）、模式、类型、路径、**RawIdent 在 name 位置**、**DocComment 在 item 位置自动跳过**
  - `error.rs`：`ParseError` 结构
  - `mod.rs`：模块导出
- **AST**（`src/ast/`，619 行）
  - `kinds.rs`：完整的 `Item`/`Expr`/`Pat`/`Ty`/`Stmt`/`Path`/`Ident`/`Generics`/`Attr` 节点
  - `mod.rs`：re-export
- **Session**（`src/session/`，148 行）：`Span`/`BytePos`/`SourceFile`/`FileId`
- **Diagnostics**（`src/diagnostics/`）：占位
- **CLI**（`src/bin/main.rs`，74 行）：`--emit-tokens` / `--emit-ast` 选项
- **测试**：
  - `tests/lexer.rs`：109 测试（字面量 35 + 运算符 25 + 关键字 5 + 标识符 5 + 注释 5 + 错误恢复 5 + 标点 5 + RP0-1/2/4/8 回归 24）
  - `tests/parser.rs`：85 测试（声明 15 + 控制流 15 + 表达式 20 + 类型 10 + 复杂程序 10 + 错误恢复 10 + 边界 5）
  - `tests/ast_structure.rs`：51 测试（含 P0 回归 + AST 结构断言 + 10 Pratt 优先级 + 7 Ty variant + 4 RawIdent 集成 + 3 DocComment 集成 + 边界 case）
  - **总计 245 测试通过**（截至 S0-REV-7）

### 3.3 测试数量对比

| 阶段 | lexer | parser | ast_structure | 总计 |
|---|---|---|---|---|
| S0-REV-1（初版） | 49 | 16 | 0 | 65 |
| S0-REV-2 | 49 | 16 | 0 | 65 |
| S0-REV-3 | 79 | 80 | 0 | 159 |
| S0-REV-4 | 79 | 80 | 0 | 159 |
| S0-REV-5 | 79 | 80 | 0 | 159 |
| S0-REV-6（v0.1.2） | 79 | 80 | 28 | 187 |
| **S0-REV-7（v0.1.3）** | **109** | **85** | **51** | **245** |

超过蓝图 §9.5 的 200 测试目标 22.5%，且测试质量从 87.5% smoke test 提升到 67% smoke + 33% 结构/回归断言。

---

## 4. 审查轮次：S0-REV-1 到 S0-REV-7

### 4.1 S0-REV-1：首轮深度审查
- **范围**：Lexer v1 + Parser v1，65 测试
- **发现**：16 个 P0（dispatch 顺序错、`1f32` 损坏、raw identifier 失效、`0x` 空字面量静默、递归栈溢出风险等）
- **结论**：Stage 0 前端不满足验收标准

### 4.2 S0-REV-2：Parser v1 深度审查
- **范围**：Parser v1，16 测试
- **发现**：23 个 P0（`|` closure bug、RBrace 死循环、BoolLit/Pipe/Bang 死代码、空 token stream panic 等）
- **结论**：测试质量严重不足（100% smoke test，0 个 AST 结构断言）

### 4.3 S0-REV-3：扩展审查
- **范围**：测试扩展到 159 个（lexer 79 + parser 80）
- **发现**：测试数量翻倍，但仍 100% smoke test
- **结论**：可进入下一轮深度审查

### 4.4 S0-REV-4：Lexer v2 深度审查
- **范围**：Lexer v2 全量审查，26 个 probe 验证
- **关键发现**：
  - ✅ 修复完成度 6/16（P0-1 dispatch 顺序、P0-3 byte_escape `\u{}`、P0-5 br"..."、P0-8/9/10 死代码删除）
  - ⚠️ 部分修复 3 项（P0-6 引入栈溢出、P0-12 CRLF、P0-13 BOM）
  - ❌ 未修复 7 项
  - **新引入 RP0-7**：错误恢复用递归 `next_token()`，100k 非法字符即栈溢出
  - **RP0-2 伪装修复**：duplicate arm + 误导注释让人以为 raw identifier 已支持
- **结论**：8 个残留 P0 阻塞 Stage 0 验收

### 4.5 S0-REV-5：Parser v2 + 测试质量审查
- **范围**：Parser v2 + 32 个 probe 验证 + 测试质量评估
- **关键发现**：
  - ✅ P0-1 RBrace 死循环已修复（`recover()` 总是 bump sync token）
  - ✅ closure `|args|` 形式已修复
  - ✅ BoolLit 删除后 parser 正确处理 `KwTrue`/`KwFalse`
  - ❌ closure `||` 空参数形式仍失败（`TokenKind::OrOr` 不被识别）
  - ❌ `&self` / `&mut self` 失败
  - ❌ struct literal、if let、while let、trait body、use group/glob/alias、dyn/impl Trait、attribute、macro call 全部缺失
  - ❌ 基本类型 `bool/char/i32` 等被静默解析为 `Ty::Path`，正确 variant 永不构造
  - ❌ 所有顶层声明 span = `Span::DUMMY`
  - ❌ 空 token stream panic
  - **测试质量缺陷**：100% parser 测试为 smoke test，0 个 AST 结构断言
- **结论**：Parser 仍不满足 Stage 0 验收

### 4.6 S0-REV-6（v0.1.2）：收敛审查 + 开发日志标准化
- **范围**：8 源文件验证 + 4 文档标准化
- **关键发现**：
  - **RP0 修复**：8 个中 **5 个完全修复**（RP0-3/5/6/7 + 部分 RP0-8 文档清理），**3 个未修复**（RP0-1/2/4 + RP0-8 死代码）
  - **S0-REV-5 P0 修复**：7 个全部修复（LBrace 贪婪、parse_path、closure `||`、基本类型、`&self`/`&mut self`、空 token stream、RBrace 死循环）
  - **新增 ast_structure.rs**：28 个测试，包含 P0 回归测试和 AST 结构断言
  - **新发现**：`tests/ast_structure.rs:203-205` 有重复 `#[test]` attribute；`Cargo.toml` 版本仍为 0.1.0
- **结论**：Stage 0 前端**主体功能满足验收**，残留 4 个非阻塞 P0 可在月 3 期间清理；文档标准化完成

### 4.7 S0-REV-7（v0.1.3，本轮）：Stage 0 前端闭合审查
- **范围**：8 源文件验证 + 4 文档更新 + 6 项阻塞性问题修复 + 测试扩展
- **关键修复**：
  - ✅ **RP0-1 验证**：通过 probe 测试确认 v0.1.2 代码已正确处理 `1f32`；新增 3 个回归测试固化
  - ✅ **RP0-2 修复**：新增 `lex_raw_identifier` 方法 + dispatch arm `b'r' if peek_at(1) == '#' && is_ident_start_byte(peek_at(2))`；parser 新增 `expect_ident` helper 接受 `Ident | RawIdent`；6 个回归测试覆盖
  - ✅ **RP0-4 验证**：通过 probe 测试确认 v0.1.2 代码已正确报错；新增 4 个回归测试固化
  - ✅ **RP0-8 修复**：新增 `lex_doc_comment` 方法 + `skip_trivia` 在 `///`/`//!`（4th byte != `/`）处停止；dispatch arm 在 `b'/'` + `//` + (`/` 或 `!`) + 非 `/` 时调用 `lex_doc_comment`；parser 在 `parse_crate` 中自动跳过 DocComment token；7 个回归测试覆盖
  - ✅ **重复 `#[test]` attribute** 删除（`tests/ast_structure.rs:203-205`）
  - ✅ **Cargo.toml 版本号** 0.1.2 → 0.1.3
  - ✅ **12 个编译警告** 全部清理：`lex_count` 加 `#[allow(dead_code)]`；10 处 `let (krate, ...)` → `let (_krate, ...)`；1 处重复 `#[test]` 删除
- **测试扩展**：
  - lexer.rs 从 79 → 109（+30 个测试：3 RP0-1 回归 + 6 RP0-2 回归 + 4 RP0-4 回归 + 7 RP0-8 回归 + 10 既有边界扩展）
  - ast_structure.rs 从 28 → 51（+23 个测试：10 Pratt 优先级 + 7 Ty variant + 4 RawIdent 集成 + 3 DocComment 集成）
  - parser.rs 保持 85（既有覆盖已足够）
  - 总计：187 → 245（+58 个测试，超出蓝图 §9.5 目标 22.5%）
- **结论**：Stage 0 前端**完整闭合**，0 P0 残留 / 0 编译警告 / 245 测试通过，准备进入 Stage 1（月 3 HIR + Name Resolution）

---

## 5. 当前状态：v0.1.3

### 5.1 通过的验收标准

- ✅ 245 个测试全部通过（lexer 109 + parser 85 + ast_structure 51）
- ✅ Lexer 覆盖全部 token 类型（关键字/字面量/运算符/标点/RawIdent/DocComment）+ 错误恢复
- ✅ Parser 覆盖全部 item 类型 + 基本表达式 + 基本类型 + 基本模式 + 错误恢复 + RawIdent 在 name 位置 + DocComment 跳过
- ✅ AST 节点结构完整（对照 05-ast.md §2-§11）
- ✅ CLI 工具可用（`--emit-tokens` / `--emit-ast`）
- ✅ 错误恢复迭代化（无栈溢出风险）
- ✅ RBrace 死循环已修复
- ✅ `&self` / `&mut self` 已支持
- ✅ closure `||` 空参数已支持
- ✅ 基本类型 `bool/char/i32/u64/f32/f64` 等构造正确 AST variant
- ✅ 空 token stream 不 panic
- ✅ `1f32` 纯后缀浮点正确产生 `FloatLit(1.0, Some(F32))`
- ✅ `r#name` 正确产生 `RawIdent` token + parser 在 name 位置接受
- ✅ `0x`/`0o`/`0b` 空字面量报错 + 返回恢复 token
- ✅ `///` 和 `//!` 正确产生 `DocComment(sym, is_inner)` token
- ✅ `////` 和 `//!/` 正确识别为普通注释（非 doc）
- ✅ 0 编译警告
- ✅ 0 P0 残留

### 5.2 已知限制（Stage 0 范围内）

#### 5.2.1 P0 残留（0 个）

✅ 全部 4 个 RP0 已修复（见 §4.7）。

#### 5.2.2 P1（11 个，可推迟到 Stage 1+）

- CRLF 在 string literal 内部未归一化（spec §1.1 要求）
- BOM 无专用错误消息
- `RawByteStrLit` variant 缺失（hash count 信息丢失）
- `LexError` 未实现 `std::error::Error` + `Display`
- 14 个弱保留关键字未覆盖
- Display impl `_` fallback 不强制 exhaustiveness
- `PathLeading::Crate/Super/Self_` variant 未构造
- 顶层声明 span = `Span::DUMMY`（P1-16）
- closure `move` 关键字未支持
- `TokenKind` 未派生 `Eq`（实际 PartialEq 已派生）
- 整数溢出 `unwrap_or(u128::MAX)` 静默 clamp

#### 5.2.3 Stage 0 范围外（推迟到 Stage 1+ HIR 阶段）

- 完整泛型参数解析（type bounds `T: Clone + Default`、`for<>` HRTB）
- 完整 where 子句解析
- 复杂模式匹配（嵌套 struct/tuple 模式、@-binding、range pattern）
- 完整属性解析（`#[derive(...)]` / `#![inner]` / meta 形式）+ DocComment 挂载为 attribute
- 内建宏调用（26 个）
- `pub(crate)` / `pub(super)` / `pub(in path)` 完整 visibility
- `Span.file_id` 字段（月 3+ 多文件时加）
- Property-based testing（proptest）
- struct literal / if let / while let / macro call 表达式
- Block doc comments (`/** ... */` / `/*! ... */`)

### 5.3 文档标准化产出

- `docs/development-log.md`（本文件）
- `docs/stage0-status.md`：Stage 0 状态报告 v0.1.3
- `docs/testing-guide.md`：测试指南
- `docs/build-guide.md`：构建指南

---

## 6. 下一步：月 3 HIR + Name Resolution

### 6.1 月 3 目标
- 完成 AST → HIR lowering
- 名字解析正确（use 导入、可见性、prelude）
- Lifetime elision 规则
- 50 个 name resolution 测试

### 6.2 关键决策
- HIR 与 AST 共享约 50% 结构（v1.2.2 修正：HIR 有 HirId/Body/OwnerNodes 独有机制）
- 不做嵌套 item（与 Rust 不同，简化）

### 6.3 Stage 1 启动条件 — 全部满足 ✅

1. ✅ Stage 0 前端闭合（0 P0 残留）
2. ✅ 0 编译警告
3. ✅ 测试数量超过蓝图目标（245/200）
4. ✅ 测试质量达标（33% 结构断言）
5. ✅ 文档标准化完成

### 6.4 Stage 1 期间补足的测试

距离蓝图 §9.5 的 200 测试目标已超出 22.5%，但仍有以下质量维度需在 Stage 1 早期补足：
- AST 结构断言测试（约 35 个，覆盖 28 种 Expr + 16 种 Ty + 12 种 Pat）
- span 正确性测试（顶层 item span 非 DUMMY）
- 错误信息内容测试
- conformance 套件建立（蓝图 §17 要求 600 个 parse 测试）

### 6.5 Stage 1 详细任务拆分（月 3-月 4）

详见 §7。

---

## 7. Stage 1 详细任务拆分

Stage 1（月 3-月 4）目标是完成 HIR + Name Resolution。为避免"一下子迈太大步伐"，将月 3 拆分为 4 个子阶段，每个子阶段有明确产出和验收标准。

### 7.1 子阶段 1.1（月 3 上半月，第 1-2 周）：HIR 数据结构定义

**目标**：定义 HIR 所有数据结构（不动 AST lowering 逻辑）

**任务清单**：
1. 新建 `src/hir/` 模块（mod.rs / kinds.rs / map.rs / id.rs）
2. 定义 `HirId`（DefId + ItemLocalId）+ `HirIdMap` / `HirIdSet`
3. 定义 `OwnerNodes`（Fn/Const/Static/Struct/Enum/Trait/Impl/TypeAlias/Mod/Use/Extern 等所有 item 的 HIR 形式）
4. 定义 `Body`（函数体 / 常量初始化 / 静态变量初始化）
5. 定义 `HirExpr` / `HirStmt` / `HirPat` / `HirTy` / `HirPath`（与 AST 70% 同构，但带 HirId 与类型占位）
6. 定义 `GenericParam` / `WherePredicate`（从 AST 蓝图迁移，但更严格）
7. 定义 `InferTy` 占位（用于类型推导，Stage 2 完整实现）

**验收标准**：
- HIR 数据结构定义完整（对照 05-ast.md §2-§11 与 06-mir.md §3）
- 20+ HIR 单元测试（构造 + Debug + Eq + Hash）
- 0 编译警告
- 既有 245 测试不受影响

### 7.2 子阶段 1.2（月 3 下半月，第 3-4 周）：AST → HIR Lowering

**目标**：实现 `lower_crate(AstCrate) -> HirCrate`，转换所有 AST 节点

**任务清单**：
1. 新建 `src/hir/lower/` 模块
2. 实现 `LowerCtxt`（持有 interner + HirId 分配器 + 错误累积器）
3. 实现 `lower_item` / `lower_fn` / `lower_struct` / `lower_enum` / `lower_trait` / `lower_impl` / `lower_const` / `lower_static` / `lower_type_alias` / `lower_mod` / `lower_use` / `lower_extern`
4. 实现 `lower_body`（表达式 + 语句递归下降）
5. 实现 `lower_expr`（28 种 ExprKind）
6. 实现 `lower_pat`（Wild/Ident/Ref 三种，其余留 Stage 2）
7. 实现 `lower_ty`（16 种 TyKind）
8. 实现 `lower_path`（含 RawIdent 段处理）
9. 实现 `lower_generics` / `lower_where_clause`
10. DocComment token 在 lowering 时挂载为 `Attribute::Doc(Symbol)`（部分实现属性系统）

**验收标准**：
- `lower_crate(parse_crate(...))` 对所有 245 个 parse 测试用例不 panic
- 30+ lowering 测试（AST → HIR 结构等价断言）
- 0 编译警告

### 7.3 子阶段 1.3（月 4 上半月，第 1-2 周）：Name Resolution 基础

**目标**：实现 module-level name resolution（不处理 scope 嵌套）

**任务清单**：
1. 新建 `src/resolve/` 模块（mod.rs / namespace.rs / resolver.rs / errors.rs）
2. 定义 `Namespace`（TypeNamespace / ValueNamespace / MacroNamespace）
3. 定义 `Resolver`（持有 module tree + glob imports + errors）
4. 实现 module tree 构造（从 HIR crate 顶层 items 推导）
5. 实现 `use` 声明解析（简单形式 `use a::b::c;`，glob `use a::*;`，alias `use a::b as c;` 留 Stage 2）
6. 实现顶层 item 注册（fn/const/static/struct/enum/trait/impl/type/mod/use）
7. 实现 prelude 注入（`std::prelude::v1` 隐式 use）
8. 实现可见性检查（仅 `pub` / `pub(crate)` 两种；其余 P1）
9. 实现 RawIdent 解析（`r#match` 在 value/type namespace 都注册为 `match`）
10. 实现 duplicate definition 检测

**验收标准**：
- 50+ name resolution 测试（覆盖 use / prelude / pub / duplicate / RawIdent）
- 0 编译警告

### 7.4 子阶段 1.4（月 4 下半月，第 3-4 周）：Scope-based Name Resolution

**目标**：实现完整 scope嵌套 name resolution（block / fn / closure / match arm）

**任务清单**：
1. 实现 `Scope` 链表（block scope / fn scope / closure scope / loop scope / match arm scope）
2. 实现 `let` 绑定注册 + shadowing 检测
3. 实现路径解析（`a::b::c` 在所有 scope 链中查找）
4. 实现裸 ident 解析（在 scope 链中查找）
5. 实现 `self` / `Self` / `super` / `crate` 关键字解析
6. 实现 lifetime 解析（`'a` 在 scope 链中查找）
7. 实现 label 解析（`'lbl:` for loop / break 'lbl）
8. 实现 forward reference 检测（fn 内部不能 forward reference 同 fn 中的 let）
9. 实现 unused variable 警告（P1，可推迟）
10. 整合到 HIR lowering 流水线（lowering 时调用 resolver）

**验收标准**：
- 50+ scope resolution 测试
- 综合：所有 245 个 parse 用例的 HIR + resolve 都不 panic
- 0 编译警告
- Stage 1 整体验收：245 + 80 + 50 + 50 = 425+ 测试通过

---

## 8. 版本历史

| 版本 | 日期 | 变更 |
|---|---|---|
| v0.1.0 | 月 2 末 | 初版 Lexer + Parser + AST，65 测试，S0-REV-1/2 发现 39 P0 |
| v0.1.2 | S0-REV-6 | 修复 7/8 S0-REV-5 关键 P0 + 5/8 S0-REV-4 残留 P0；扩展 ast_structure 测试到 28 个；总计 187 测试通过；4 个 P0 残留不阻塞月 3 启动 |
| v0.1.3 | S0-REV-7 | **Stage 0 前端闭合**：修复全部 4 个 RP0（含 RP0-2 raw identifier 与 RP0-8 doc comment 的完整 lexer+parser 支持）；清理 12 个编译警告；测试扩展到 245 个（+58，含 28 RP0 回归 + 10 Pratt 优先级 + 7 Ty variant + 4 RawIdent 集成 + 3 DocComment 集成）；测试质量从 87.5% smoke 提升到 67% smoke + 33% 结构断言；0 P0 残留 / 0 警告 / 245 测试通过 |

---

**Landin Stage 0 开发日志 v0.1.3 — 完（Stage 0 前端闭合）**
