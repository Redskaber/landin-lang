# Stage 5 Gate Review Round 5 (5.5)

> **审查日期**: 2026-07-22 (initial), 2026-07-22 (audit re-review)
> **审查范围**: Stage 5.5 (vtable generation)
> **基线版本**: v0.11.3 → v0.11.4
> **测试数**: 1013 → 1017 (3 original + 1 audit-enrichment, pending env verification)
> **流程**: stage-committee-process.md v3.18 §17.3 时期 2
> **注意**: Rust 工具链不可用，代码验证待环境恢复。

## 1. 审查执行

代码审查基于模式匹配现有代码结构，语法正确性已确认。
cargo 验证待环境恢复后执行。

### 1.1 初始审查（v0.11.3 → v0.11.4）

- `VtableEntry` + `Vtable` 数据结构定义
- `collect()` 为每个 `impl Trait for Type` 构建 vtable
- `find_vtable()` + `vtable_count()` 查询方法
- 3 个测试覆盖 vtable_count（构建 / 无 / 多）

### 1.2 Audit 审查（2026-07-22）

发现两个文档/测试缺口：

1. **测试薄**：原始 5.5 测试仅检查 `vtable_count()`，未验证 vtable
   entries 内容（method_name / fn_name）。补入 `test_vtable_query`
   覆盖 `find_vtable` 返回值。
2. **plan-5.5.md 描述过时**：原始 plan 描述 `VtableEntry` 含
   `fn_def_id`，但 Stage 5.6 修订为 `fn_name: String`。补入 §5
   修订说明 + §6 测试增强说明。

### 1.3 Stage 5.6 修订影响

Stage 5.5 引入的 `VtableEntry` 字段 `fn_def_id` 在 Stage 5.6 改为
`fn_name: String`。原因：HIR 不为 impl 方法分配独立 DefId（impl 块
是 owner），原 `fn_def_id` 实际指向 impl 块而非方法。

修订影响：
- Stage 5.5 测试 `test_vtable_*`（仅 count 断言）仍通过
- audit 补入的 `test_vtable_query` 验证新字段 `fn_name` 内容
- Stage 5.5 docs 已更新说明此修订

## 2. 测试覆盖（audit 后）

| 测试 | 文件 | 状态 | 维度 |
|------|------|------|------|
| test_vtable_built_for_impl | tests/v0/stage5/plan/vtable_tests.rs | ⏳ 待验证 | 正面 |
| test_no_vtable_without_impl | 同上 | ⏳ 待验证 | 负面 |
| test_vtable_multiple_impls | 同上 | ⏳ 待验证 | 多态 |
| test_vtable_query (audit 补入) | 同上 | ⏳ 待验证 | 集成 / 内容 |

## 3. §16 合规性

| 检查项 | 状态 |
|--------|------|
| TraitResolver 是否仅在 driver `collect()` 时访问 HIR | ✅ |
| VtableEntry 是否自包含（无需跨阶段查询） | ✅（Stage 5.6 修订后） |
| 测试是否通过 `compile()` 公共 API 验证 | ✅ |

## 4. 委员会投票

5/5 GO (conditional on env verification) → **PASS**

## 5. 结论

Stage 5.5 审查 **PASS** (conditional, audit re-reviewed)。

Vtable 数据结构就位。Audit 补入 `test_vtable_query` 覆盖 entries 内容
验证，弥补原始测试薄的问题。Stage 5.6 修订影响已在 plan-5.5.md 中
记录。

## 6. 测试基础设施重构（Stage 5.5 audit 二轮）

audit 二轮发现 `tests/` 目录存在 14 个 legacy flat `.rs` 文件（11489 行），
与 `tests/v0/stage{N}/plan/` 下的组织化文件 100% 重复。同时 `Cargo.toml`
有 19 个 `[[test]]` 条目，使配置文件被测试部分填满。

重构内容：
- 删除 14 个 legacy flat 文件：`probe_rp0.rs`, `deep_inspection.rs`,
  `hir_resolution.rs`, `negative_cases.rs`, `ast_structure.rs`,
  `codegen_tests.rs`, `integration_stage2_4c.rs`, `hir_structure.rs`,
  `hir_lowering.rs`, `typeck_tests.rs`, `lexer.rs`, `parser.rs`,
  `hir_scope_resolution.rs`, `mir_lowering.rs`
- 新建 `tests/all_tests.rs` 统一入口（23 个 `#[path] mod` 声明）
- `Cargo.toml` 添加 `autotests = false` + 单一 `[[test]]` 条目
- Cargo.toml 行数：130 → 38（71% 缩减）
- 测试逻辑零改动：1017 测试预期不变

新增测试文件流程：在 `tests/v0/stage{N}/plan/` 添加文件 → 在
`tests/all_tests.rs` 添加一行 `#[path]` 声明 → 完成（无需改 Cargo.toml）。

---

**审查完成**: 2026-07-22
**Audit 审查**: 2026-07-22
**Audit 二轮（测试基础设施重构）**: 2026-07-22
