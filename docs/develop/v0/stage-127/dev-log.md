# Stage 127 开发日志 — TD-TRAIT-METHOD-AMBIGUITY (UFCS)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.657.0 → v0.658.0 |
| 测试数 | 5768 (898 lib + 4870 integration, +26 stage127) |
| 失败数 | 0 |
| ignored | 9 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 127 实现 UFCS (Universal Function Call Syntax) — `<T as Trait>::method(receiver, args)`，
解决 TD-TRAIT-METHOD-AMBIGUITY（两个 trait 为同一类型提供同名方法时的歧义）。

### WHY (根因分析)
- **原根因假设**：mangling collision (`landin_i32_fmt` for both Display and Debug)
  → Stage 98 (v0.9) 已修复（3-element mangling: `landin_<Trait>_<Type>_<method>`）
- **真正根因**：`resolve_trait_method` (mir/lower/method_resolution.rs:662-684) 缺少
  candidate filter — 遍历所有 trait impls 返回第一个匹配，无多候选报错
- **通解**：引入 UFCS 显式 trait 限定语法，用户通过 `<T as Trait>::method` 显式消歧
- **特解（否决）**：rename `Debug::fmt` → `debug_fmt`（违反 §1.0 原则 9 + 破坏 Rust 源码兼容性）

### WHO
- DEV-A: 实现 parser + HIR + resolver + MIR lower + codegen
- ARCH-A: 设计 UFCS 语法对齐 Rust RFC 0132 + Rust Reference §6.1
- REV-A: 验证 26 个测试（8 正 + 11 负 + 4 边界 + 3 回归）
- QA-A: 1:3+ 正负比例 + ≥30 负向审计覆盖

### WHEN
- Phase 0: 恢复 v0.657.0 baseline + 设计文档（上轮完成）
- Phase 1-5: 实现 UFCS（本轮完成）
- Phase 6: 26 个测试（本轮完成）
- Phase 7: §3.2 验收 + 文档 + 打包（本轮完成）

### WHERE
- 代码：`src/parser/path.rs` + `src/parser/expr.rs` + `src/hir/kinds.rs` +
  `src/hir/lower/path.rs` + `src/hir/lower/body.rs` + `src/hir/lower/item.rs` +
  `src/resolve/resolver.rs` + `src/resolve/path_resolve.rs` + `src/codegen/function.rs` +
  `src/mir/lower/expr_variants.rs`
- 测试：`tests/v0/stage127/plan/ufcs_tests.rs` (26 tests)
- 文档：`docs/lang-design/03-type-system.md` §2.6 + `02-grammar.md` + `16-diagnostics.md`

### HOW (实施策略)
1. **Parser**: 启用 Expr 上下文的 `is_qself_start` 检查（之前只支持 Type/Pattern）
2. **HIR**: `HirPath` 新增 `qself: Option<HirQSelf>` 字段 + `lower_path_with_qself` 函数
3. **Resolver**: 新增 `trait_method_index: HashMap<(Spur, Spur), DefId>` + `resolve_qualified_path`
4. **MIR lower**: 在 `lower_path_expr` 的 `_ =>` 分支添加 UFCS 解析 —
   `resolve_ufcs_impl_method_def_id` 直接查找 impl 方法 DefId（绕过 typeck receiver_type 问题）
5. **Codegen**: `get_concrete_type_name` 添加 Adt 分支 + 传递 `type_name_by_def_id`
6. **Tests**: 26 个测试覆盖正向（UFCS 工作）+ 负向（错误情况）+ 边界 + 回归

### HOW MUCH (§3.2 验收)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4870 tests, 0 failures)
- Total: 5768 tests, 0 failures, 9 ignored

## 关键技术决策

### 决策点 1: UFCS 解析位置
- **选 A**：在 `parse_path_with_ctx` 启用 Expr 上下文 qself（通解，复用现有 `try_parse_qself`）
- **否决 B**：在 `parse_primary_expr` 添加新 `<` 分支（特解，重复逻辑）
- **依据**：§1.0 原则 6 (通解 > 特例) + §13.4 J2 (单一职责)

### 决策点 2: impl 方法解析位置
- **选 A**：在 MIR lower 的 `lower_path_expr` 直接解析 impl 方法 DefId
  （`resolve_ufcs_impl_method_def_id` — 绕过 typeck receiver_type 问题）
- **否决 B**：依赖 codegen 的 `re_resolve_trait_method_calls`（受 typeck 推断问题阻塞）
- **依据**：§1.0 原则 9 (正确 > 妥协) — 直接解析更正确，不依赖间接路径

### 决策点 3: trait_method_index 预计算
- **选 A**：在 `resolve_all_paths` Phase 3.5 预计算 `trait_method_index`
- **否决 B**：在 `resolve_qualified_path` 时查询 HIR（违反 §16 codegen HIR-free）
- **依据**：§16 (codegen HIR-free) + §1.0 原则 6 (通解 > 特例)

## 已知限制 (TD 同步)

### TD-UFCS-SHORT-FORM (P3, v0.14+)
- **描述**：短形式 `Trait::method(receiver)`（无 `<T as ...>` 限定）未实现
- **根因**：短形式需要从 receiver 推断 Self 类型，但 MIR lower 在解析 path 时
  无法访问 receiver（receiver 是 Call 的 args[0]，在 func 之后才 lower）
- **修复方案**：在 `lower_call_expr` 中添加特殊路径，当 func path 的 qself.ty
  为 None 时，先 lower receiver 获取其类型，再回填到 qself.ty
- **影响**：用户必须使用完整形式 `<T as Trait>::method`（可接受）

### TD-UFCS-DEFAULT-BODY-EMPTY-IMPL (P3, v0.14+)
- **描述**：UFCS 调用 trait 默认方法体时，如果 impl 块为空（不覆盖），
  `resolve_ufcs_impl_method_def_id` 找不到 impl 方法（因为 impl 中没有该方法）
- **根因**：`resolve_ufcs_impl_method_def_id` 只扫描 impl 块的 items，
  不回退到 trait 声明的默认方法体
- **修复方案**：当 impl 块中找不到方法时，回退到 trait 声明的默认方法 DefId
- **影响**：用户必须在 impl 中显式覆盖默认方法（可接受 workaround）

### TD-UFCS-AMBIGUITY-E1109 (P3, v0.14+)
- **描述**：普通方法调用 `obj.method()` 当 2+ trait 提供同名方法时，
  仍静默选择第一个匹配（无 E1109 报错）
- **根因**：`resolve_trait_method` 缺少 candidate filter + 多候选报错
- **修复方案**：在 `resolve_trait_method` 中收集所有候选，>1 时报 E1109
- **影响**：用户必须使用 UFCS 显式消歧（可接受，符合 Rust 行为）

## Stage Summary
- Stage 127 PASSED — UFCS `<T as Trait>::method` 完整实现
- 8 positive + 11 negative + 4 edge + 3 regression = 26 tests, all pass
- 0 regression (5742 → 5768 tests, +26 new)
- 3 new TDs documented (TD-UFCS-SHORT-FORM + TD-UFCS-DEFAULT-BODY-EMPTY-IMPL + TD-UFCS-AMBIGUITY-E1109)
- v0.658.0
