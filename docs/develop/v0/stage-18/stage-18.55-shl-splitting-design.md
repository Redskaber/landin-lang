# Stage 18.55 — `<<` Splitting + GAT Phase 3 E2E Tests

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.321.0 → v0.322.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §13.5 (design-review agent cycle)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 上一阶段成果 (Stage 18.52-18.54 GATs Phase 1+2 + Generic Param Resolution)

完成 GATs 三阶段基础设施:
- Stage 18.52: AST/Parser/HIR 基础设施 — `type Item<'a, T> where Self: 'a;`
- Stage 18.53: Qualified path 解析 — `<T as Trait>::Item` + `>>` splitting + `&'a mut self`
- Stage 18.54: Generic type param resolution — `Res::GenericParam` + scope 栈 + typeck unify

**审查发现**: GATs 现可端到端工作 — `<S as C>::Item` 在 MIR 中正确解析为 `i32`。但有一个已知限制: `<<` (Shl) splitting 未实现, 导致 `Vec<<T as Trait>::Item>` 解析失败。

### 1.2 已知限制清单 (Stage 18.54 末尾记录)

| 限制 | 严重性 | 本阶段处理 |
|------|--------|-----------|
| `<<` (Shl) splitting | P2 (GAT 完整性) | ✅ 本阶段修复 |
| GAT e2e 测试覆盖 | P1 (验证) | ✅ 本阶段补全 |
| `find_assoc_type_def_id` 按 name 查找 | P3 (Phase 4 改进) | ⏳ 推迟 |
| GAT variance 检查 | P3 (远期) | ⏳ 推迟 |

### 1.3 本阶段目标

**目标**: 完成 GATs 最后一个解析限制 (`<<` splitting), 并补全 GAT e2e 测试套件, 标记 GATs v0.7 P1 任务完成。

**做**:
- Parser 新增 `shl_split` 字段 + `eat_lt_or_split()` 方法 (mirror of `eat_gt_or_split`)
- `try_parse_generic_args` lookahead 识别 `<<` 作为 generic args 开始
- `parse_generics` 与 `try_parse_generic_args` 使用 `eat_lt_or_split` 处理嵌套 `<`
- 新增 GAT Phase 3 e2e 测试: 完整 GAT 使用场景 (声明 + impl + 使用 + 运行)
- 新增 conformance 测试

**不做** (留待后续):
- ❌ GAT monomorphization 特殊优化 (当前 substitute 已正确处理 Projection)
- ❌ GAT variance 检查
- ❌ 增量编译 (下一 P1 任务)

### 1.4 设计文档参考

| 文档 | 章节 | 关键约束 |
|------|------|---------|
| `docs/develop/v0/stage-18/stage-18.53-gats-phase2-design.md` | §3.1 | `>>` splitting 设计 (本阶段 mirror) |
| `docs/develop/v0/stage-18/stage-18.54-generic-param-resolution-design.md` | §1.3 | 已知限制清单 |
| `src/parser/parser.rs:140-185` | `eat_gt_or_split` | 现有 `>>` splitting 实现 (本阶段参考) |
| `src/lexer/token.rs:109` | `Shl` token | lexer 已产生 `<<` token |

---

## 2. §1.0 设计原则遵循

| 原则 | 本阶段如何遵循 |
|------|---------------|
| 1. 长期 > 短期 | `<<` splitting 是 GAT 完整性的最后一块拼图 |
| 2. 整体 > 局部 | Parser + lookahead + e2e 测试协同 |
| 3. 显式 > 隐式 | `shl_split` 字段显式跟踪 split 状态 |
| 4. 报错 > 静默 | `<<` 不再静默失败, 而是正确 split |
| 5. 去除兼容思维 | 不保留 " Vec<<T as C>::Item> 不支持" 的旧行为 |
| 6. 通用 > 特例 | 一个 `eat_lt_or_split` 处理所有 `<<` 场景, mirror `eat_gt_or_split` |
| 7. API 命名标准化 | `eat_lt_or_split` / `shl_split` 命名对称 |
| 8. 设计驱动测试 | e2e 测试覆盖 GAT 完整使用路径 |
| 9. 正确 > 妥协 | 不假装支持, 实际实现 |

---

## 3. 技术设计

### 3.1 Parser 新增 `shl_split` 字段 (src/parser/parser.rs)

**Mirror of `shr_split`**:

```rust
pub struct Parser<'a> {
    // ... existing fields ...
    pub(super) shr_split: u32,  // existing (Stage 18.53)
    /// Stage 18.55: `<<` splitting state.
    ///
    /// Mirror of `shr_split` for `<<` (Shl) tokens. When the parser is
    /// inside nested generics and encounters `<<` (e.g.,
    /// `Vec<<T as Trait>::Item>`), the lexer produces a single `<<` token
    /// where two `<` are needed. This field tracks the split state.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": mirror of `shr_split` for symmetry.
    pub(super) shl_split: u32,
}
```

### 3.2 新增 `eat_lt_or_split()` 方法 (src/parser/parser.rs)

**Mirror of `eat_gt_or_split()`**:

```rust
/// Stage 18.55: Try to consume a `<` token, splitting a `<<` if necessary.
///
/// Mirror of `eat_gt_or_split`. When the parser is inside nested generics
/// (e.g., `Vec<<T as Trait>::Item>`), the lexer produces a single `<<`
/// (Shl) token where two `<` are needed. This method handles that split.
///
/// - If the next token is `<`, consume it and return true.
/// - If the next token is `<<` and `shl_split > 0`, decrement and return true.
/// - If the next token is `<<` and `shl_split == 0`, set `shl_split = 1` and return true.
/// - Otherwise return false.
///
/// Per §10 naming: `eat_lt_or_split` mirrors `eat_gt_or_split`.
pub(super) fn eat_lt_or_split(&mut self) -> bool {
    match self.peek() {
        TokenKind::Lt => {
            self.bump();
            true
        }
        TokenKind::Shl => {
            if self.shl_split > 0 {
                self.shl_split -= 1;
                if self.shl_split == 0 {
                    self.bump();
                }
                true
            } else {
                self.shl_split = 1;
                true
            }
        }
        _ => false,
    }
}
```

### 3.3 `try_parse_generic_args` lookahead 更新 (src/parser/path.rs)

**当前** (Stage 18.53 已添加 `TokenKind::Lt` to lookahead):
```rust
let looks_like_generic = matches!(
    next,
    TokenKind::Ident(_) | ... | TokenKind::Lt // `<<T as Trait>::Item>`
);
```

**修改**: 当看到 `<<` (Shl) 时也识别为 generic args 开始:
```rust
| TokenKind::Shl // `<<T as Trait>::Item>` (nested qualified path)
```

### 3.4 `try_parse_generic_args` 内部使用 `eat_lt_or_split`

**当前** (line 245):
```rust
self.bump(); // <
```

**修改**:
```rust
// Stage 18.55: Use `eat_lt_or_split` to handle `<<` in nested generics
// like `Vec<<T as Trait>::Item>`. Per §1.0 原則 6 "通用 > 特例".
if !self.eat_lt_or_split() {
    return None; // shouldn't happen (lookahead already verified `<`)
}
```

### 3.5 `parse_generics` 使用 `eat_lt_or_split` (src/parser/generics.rs)

**当前** (line 158):
```rust
self.bump(); // <
```

**修改**:
```rust
// Stage 18.55: Use `eat_lt_or_split` for consistency with try_parse_generic_args.
self.eat_lt_or_split();
```

### 3.6 测试设计 (§9.4.3 1:3+ ratio)

**测试文件**: `tests/v0/stage18/plan/stage18_55_gats_phase3_e2e_tests.rs` (≥8 测试: 2 正 + 6 负)

**正向测试** (2):
1. `nested_qualified_path_in_generic` — `Vec<<T as Trait>::Item>` 解析成功
2. `gat_e2e_full_pipeline` — GAT 声明 + impl + 使用 + 运行验证

**负向测试** (6):
1. `shl_unbalanced_extra_lt` — `Vec<<T>::Item>` (extra `<`) 报错
2. `shl_missing_close_gt` — `Vec<<T as Trait::Item>` (missing `>`) 报错
3. `gat_undefined_in_nested` — `Vec<<T as Undefined>::Item>` 报错
4. `shl_eof_mid_parse` — `Vec<<T as Trait` EOF 报错
5. `gat_mismatched_arity` — trait 声明 `type Item<T>` 但 impl `type Item` (无 generics) 报错
6. `shl_garbage_after_lt` — `Vec<<@>::Item>` 报错

**Conformance 测试**:
- `0386-gat-nested-qualified-path.lin` — 正向: `Vec<<T as Trait>::Item>`
- `0387-gat-e2e-lending-iterator.lin` — 正向: 完整 LendingIterator trait
- `err-0332-gat-shl-unbalanced.lin` — 负向
- `err-0333-gat-shl-missing-close.lin` — 负向

---

## 4. §13.5 设计-审查 Agent 循环

### 4.1 Round 1 自审

| 维度 | 自审结论 | 状态 |
|------|---------|------|
| 设计偏差 | `<<` splitting 是 GAT 完整性的最后限制; e2e 测试验证完整性 | ✅ |
| §1.0 原则 1 长期 > 短期 | 不修则 `Vec<<T as C>::Item>` 永远失败 | ✅ |
| §1.0 原则 6 通用 > 特例 | mirror `eat_gt_or_split` 设计, 无新概念 | ✅ |
| §1.0 原则 7 API 命名 | `eat_lt_or_split` / `shl_split` 对称命名 | ✅ |
| §9.4.3 1:3+ 测试 | 8 unit (2:6) + 4 conformance = 1:3 ✓ | ✅ |
| 向后兼容 | 现有 `<<` 作为 Shl token 的行为不变; 只在 generics 上下文 split | ✅ |
| 死代码 | 无; `eat_lt_or_split` 复用现有 split 模式 | ✅ |

### 4.2 §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | mirror 设计降低风险, e2e 测试补全 |
| DEV-A | GO | `eat_lt_or_split` 与 `eat_gt_or_split` 对称, 易实现 |
| QA-A | GO | 1:3+ 比例; e2e 测试覆盖完整 GAT 路径 |
| REV-A | GO | 设计原则 1, 6, 7 遵循; GAT v0.7 P1 任务完成 |
| PM-A | GO | GATs 完成后可进入增量编译 (下一 P1) |

**5/5 GO** ✅

---

## 5. 实施步骤

1. ✅ 写设计文档 (本文件)
2. ⏳ 新增 `shl_split` 字段到 Parser (src/parser/parser.rs)
3. ⏳ 新增 `eat_lt_or_split()` 方法 (mirror of `eat_gt_or_split`)
4. ⏳ 更新 `try_parse_generic_args` lookahead 识别 `Shl`
5. ⏳ 更新 `try_parse_generic_args` 与 `parse_generics` 使用 `eat_lt_or_split`
6. ⏳ 新增 e2e 测试 (tests/v0/stage18/plan/stage18_55_gats_phase3_e2e_tests.rs)
7. ⏳ 新增 conformance 测试
8. ⏳ 验收: cargo clean + build + fmt + clippy + test
9. ⏳ worklog + 版本 bump v0.321.0 → v0.322.0
10. ⏳ 打包 tar.gz

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `<<` splitting 破坏现有 Shl 作为 shift operator 的语义 | `eat_lt_or_split` 只在 generics 上下文调用; expression 中的 `<<` 仍走 binop 路径 |
| lookahead 误判 `<<` 为 generic args | `try_parse_generic_args` 已有 lookahead 检查; 新增 `Shl` 到 lookahead 列表是安全的 (Shl 在 expression 上下文不会出现) |
| e2e 测试依赖 stdlib 类型 (Vec) | 测试使用 user-defined types 避免 stdlib 依赖 |

---

## 7. 结论

Stage 18.55 设计完成。`<<` splitting 是 GAT 完整性的最后一块拼图, mirror `>>` splitting 设计降低风险。e2e 测试补全验证 GAT 端到端正确性。完成后 GATs v0.7 P1 任务标记完成, 可进入增量编译 (下一 P1)。

5/5 GO, 进入实施阶段。
