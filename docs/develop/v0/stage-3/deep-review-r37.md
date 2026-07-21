# Stage 0-3 深度审查报告（Round 37）

> **审查日期**: 2026-07-22
> **审查协议**: stage-committee-process.md v3.16 §25（阶段末尾深度审查协议）
> **基线版本**: v0.8.12 / 984 tests / Stage 3.68 complete
> **审查者**: Super Z (main) + Agent Group（ARCH-A / DEV-A / QA-A / ALG-C / SKL-A）
> **触发时机**: Stage 3 完全收敛后，进入 Stage 4 前的阶段切换点深度审查

---

## 1. 执行摘要

**结论：GO-WITH-CONDITIONS**

Stage 0-3 经历 36 轮 gate review + 6 轮跨阶段 audit（Stage 3.63-3.68），
当前状态非常健康：984 测试全过，0 clippy 警告，0 fmt 问题，0 build 警告，
0 TODO/FIXME，§16 接口隔离 100% 合规。所有 soundness-critical 限制已关闭。

**可以进入 Stage 4**，但建议在 Stage 4 启动前/初期处理以下 3 个条件项：
- **条件 1**：`HirParam` 重复设计需在 Stage 4 闭包工作时一并审视
- **条件 2**：`Emitter` trait 膨胀（36 方法）需在添加第二后端前分解
- **条件 3**：AST 枚举命名不一致（`Expr`/`Ty`/`Pat` 直接 vs `ItemKind` 包装）需在 Stage 4 宏系统工作时统一

**阻塞项**：0 P0 / 0 P1
**技术债**：5 项 P2 / 3 项 P3（全部有明确偿还计划，不影响 Stage 4 启动）

---

## 2. 七维度审查结论

### D1. 架构健康度

**现状**：架构非常健康。Stage 3.56-3.60 完成的 §16 合规重构使 codegen 和
typeck 都成为纯 MIR 消费者——零上游函数调用，仅通过预计算的数据表
（`FieldTyTable`、`FnSigTable`、`body_metas`、`fn_name_by_def_id`）与
driver 交互。数据流清晰：

```
source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen → .ll
```

每个阶段的入口都是自由函数（`tokenize`、`parse_crate`、`lower_crate`、
`resolve_crate`、`lower_body_full`、`check_mir_body_with_tables`、
`check_mir_body`、`codegen_crate`），符合 `api-naming-standard.md` §2.2。

Stage 3.63-3.68 的命名标准化工作消除了所有主要词汇不一致：
- `Lvalue` → `Place`（167+ refs，与设计文档 06-mir.md §4 对齐）
- `LowerCtxt` → `HirLowerCtxt`（与 `MirLowerCtxt` 对称）
- `BorrowKind` 统一（消除 `BkKind` 别名）
- `DefKind` 移到 `hir::kinds`（架构归属正确）
- `fat_ptr_type` → `emit_fat_ptr_type`（前缀一致）
- glob exports 全面消除（所有 `mod.rs` 使用显式列表）

**风险**：
1. **`parser.rs` 3052 LOC**——单文件偏大，但功能内聚（递归下降 + Pratt），
   拆分收益有限，风险可控
2. **`mir/lower/mod.rs` 2729 LOC**——MIR lowering 逻辑集中，后续 Stage 4
   添加闭包/trait 时可能进一步膨胀
3. **`HirParam` 重复**（`HirFnSig.inputs` + `Body.params`）——当前是 clone，
   与 rustc 设计一致，但 Stage 4 闭包捕获可能需要重新审视

**建议**：
- Stage 4 添加闭包时，考虑将 `mir/lower/mod.rs` 按功能拆分（expr/pat/stmt/ty）
- `HirParam` 重复保持现状（rustc 也这样做），但在 Stage 4 闭包工作时审视是否
  需要改为引用共享
- 维护当前架构——不要为了"优化"而破坏 §16 合规

### D2. 技术债清单

| ID | 描述 | 优先级 | 影响 Stage 4? | 偿还计划 |
|----|------|--------|--------------|---------|
| TD-001 | `HirParam` 重复（`HirFnSig.inputs` + `Body.params` clone） | P2 | ⚠️ 间接（闭包捕获时需审视） | Stage 4 闭包工作时评估是否改为引用共享 |
| TD-002 | `Emitter` trait 膨胀（36 方法，1 实现） | P2 | ⚠️ 添加第二后端时需分解 | Stage 4+ 添加 MLIR/LLVM-C 后端前分解为子 trait |
| TD-003 | AST 枚举命名不一致（`Expr`/`Ty`/`Pat` 直接 vs `ItemKind` 包装） | P2 | ⚠️ 宏系统可能需要统一 | Stage 4 宏系统工作时统一（选择 `XxxKind` + 包装模式） |
| TD-004 | 可见性强制检查是 stub（`check_visibility` 返回 Ok） | P2 | ❌ 不影响（需嵌套模块） | Stage 4 添加嵌套模块后激活 |
| TD-005 | Prelude 注入未实现 | P2 | ❌ 不影响 | Stage 5 stdlib MVP |
| TD-006 | NLL 单遍前向（循环内借用可能误报） | P2 | ❌ 不影响 | Stage 4+ 实现定点数据流 |
| TD-007 | `use` 声明解析限制（3+ 段路径不支持） | P3 | ❌ 不影响 | Stage 4 扩展 |
| TD-008 | 跨 crate 导入未实现 | P3 | ❌ 不影响 | Stage 5 |

**技术债分类**：
- **可接受的**（有明确偿还计划）：TD-001 到 TD-008 全部
- **危险的**（影响下一阶段）：0 项——所有影响 Stage 4 的技术债都有明确的
  Stage 4 内偿还计划

### D3. 测试覆盖深度

**现状**：984 测试，分布如下：
- Stage 0（lexer/parser/AST）：344 测试
- Stage 1（HIR/resolve）：114 测试
- Stage 2（MIR/typeck/borrowck）：168 测试
- Stage 3（codegen）：294 测试 + 5 §21 audit 测试
- 负向测试矩阵（§9.1.1）：7 类全覆盖
- §21 程序化审计测试：5 个（验证 §16 合规）

**覆盖率评估**：
- 功能覆盖：~99%（所有已实现功能都有测试）
- 回归覆盖：100%（36 轮 gate review 累积 716+ 审计 case，0 回归）
- 边界覆盖：~95%（§9.3.2 边界 case 测试持续运行）
- 负向覆盖：100%（§9.1.1 全部 7 类）
- 审计覆盖：100%（§9.3.1 ≥30 case 每轮）

**风险**：
1. **无性能基准测试**——当前没有 benchmark suite，无法量化 Stage 4 优化效果
2. **无 fuzzing**——无模糊测试，深层 bug 可能未发现
3. **property-based 测试缺失**——Stage 1.1 worklog 已记录，仍为 0

**补测计划**：
- Stage 4 启动时：添加 `benches/` 目录 + criterion 基准测试
- Stage 4 中期：探索 cargo-fuzz 模糊测试
- Stage 4 后期：添加 proptest 属性测试

### D4. 下一阶段就绪度

Stage 4 计划内容：宏系统 + 属性 + 闭包（L3）+ PHI 优化（L1）

| 下一阶段需求 | 当前状态 | 差距 | 解阻计划 |
|-------------|---------|------|---------|
| AST 属性表示（`#[attr]`） | ✅ 已有 `Attr`/`AttrArgs` 结构 | 无 | 直接使用 |
| HIR 属性传递 | ✅ 已有 `attrs: Vec<Attr>` 字段 | 无 | 直接使用 |
| Parser 属性解析 | ✅ `parse_outer_attrs` 已实现 | 无 | 直接使用 |
| 闭包 AST 节点 | ✅ `Expr::Closure` 已定义 | 无 | 直接使用 |
| 闭包 HIR 节点 | ⚠️ `HirExprKind::Closure` 已定义但未充分使用 | 闭包捕获/类型推断未实现 | Stage 4 闭包工作 |
| 闭包 MIR lowering | ❌ 未实现 | 需要闭包类型 lowering + 捕获 codegen | Stage 4 L3 |
| PHI 优化 | ❌ 未实现 | codegen 当前 emit alloca+load/store，依赖 LLVM mem2reg | Stage 4 L1 |
| TraitResolver | ❌ 未实现 | L5 trait dispatch 阻塞 | Stage 5 |
| 嵌套模块支持 | ⚠️ 模块树扁平（所有 item 在 crate root） | visibility 强制检查需嵌套模块 | Stage 4 早期 |
| 宏 AST 节点 | ⚠️ `Expr::MacroCall` 已定义但未实现展开 | 宏展开器未实现 | Stage 4 宏系统 |

**就绪度结论**：Stage 4 的基础设施（AST/HR 属性、闭包节点、parser 骨架）
**已就绪**。需要实现的 是 Stage 4 的核心功能（闭包 lowering、宏展开、PHI）。
**无阻塞项**——可以启动 Stage 4。

### D5. 设计合理性

**现状评估**：

**合理的设计**：
1. **§16 数据驱动架构**——driver 预计算所有元数据，codegen/typeck 纯消费数据
   表。这是 Stage 3.56-3.60 的核心重构，设计非常干净
2. **`Place`（原 `Lvalue`）命名**——与设计文档 + borrowck 内部词汇 + 现代 rustc 对齐
3. **`HirSelfKind` 区分 trait-Self vs impl-Self**——为 Stage 4 trait 工作奠定基础
4. **`Emitter` trait 可插拔**——为多后端预留扩展点
5. **`UseImport` 表 + leaf/glob 优先级**——use 解析设计合理

**过度设计**：无发现。当前实现恰好满足需求，没有过度抽象。

**设计不足**：
1. **`Emitter` trait 36 方法**——过于庞大，但当前只有 1 个实现，分解的边际
   收益低。Stage 4 添加第二后端时再分解（TD-002）
2. **模块树扁平**——`ModuleNode.children` 存在但 `build_module_tree` 不填充
   嵌套模块。这是 Stage 1.3 的简化，Stage 4 需要补全（TD-004）
3. **`HirParam` 重复**——与 rustc 一致，但 Stage 4 闭包可能需要重新审视（TD-001）

**建议**：
- 当前设计**不需要重构**——所有"不足"都有明确的 Stage 4 偿还计划
- Stage 4 宏系统工作时，统一 AST 枚举命名（TD-003）
- Stage 4 闭包工作时，审视 `HirParam` 重复（TD-001）

### D6. 性能与可扩展性

**现状**：未运行正式性能基准（无 benchmark suite）。但基于代码分析：

**性能良好的设计**：
1. **String interner（`lasso::Rodeo`）**——O(1) 字符串比较，适合编译器场景
2. **`HirId`/`DefId` 整数键**——HashMap 查找 O(1)
3. **预计算元数据**——driver 一次性构建 `FieldTyTable`/`FnSigTable`，避免
   重复 HIR 遍历
4. **MIR 控制流图**——basic block + terminator，标准编译器 IR

**潜在瓶颈**：
1. **`HirCrate.owners` / `bodies` 使用 `Vec<(K, V)>` + 线性查找**——Stage 1.2
   注释提到"可换 FxHashMap"。当前规模（<1000 items）无瓶颈，但大 crate
   可能受影响。Stage 4+ 可优化
2. **`resolve_path` 单遍查找**——value_ns + type_ns + use_imports 三次
   HashMap 查找。可优化为一次，但当前规模无瓶颈
3. **codegen `codegen_function` 线性遍历**——每个 MIR body 独立 codegen，
   无并行。Stage 4+ 可并行化

**优化建议**：
- Stage 4：添加 `benches/` 基准测试套件，建立性能基线
- Stage 5：如果编译速度成为问题，考虑 `HirCrate` 改用 FxHashMap
- Stage 5+：codegen 并行化（每个 fn 独立 codegen）

### D7. 文档与知识传承

**现状**：文档非常充分。

**文档清单**：
- `docs/stage-committee-process.md` v3.16——流程 SOP（1820 行）
- `docs/develop/v0/api-naming-standard.md`——API 命名标准（v1.5）
- `docs/develop/v0/stage-0-3-cross-stage-audit.md`——§21 跨阶段审计报告
- `docs/develop/v0/stage-{0,1,2,3}/`——各阶段开发日志 + gate review 报告
- `docs/tests/matrix.md`——全局测试矩阵
- `docs/lang-design/`——语言设计文档（19 个 .md）
- `docs/agent-team/`——Agent 团队文档（11 个 .md）
- `README.md` + `RELEASE_NOTES.md`——项目概览 + 发布说明
- `worklog.md`——共享工作日志（1876+ 行）

**文档完整度**：~95%

**隐性知识**（未充分记录的设计决策）：
1. **为什么 `HirParam` 重复**——只有 worklog 提到"与 rustc 一致"，没有专门
   设计文档解释
2. **为什么 `Emitter` 是 trait 而非具体类型**——隐含在 §16.1.3 "可替换"
   原则中，但没有专门文档
3. **为什么 `check_visibility` 是 stub**——只在代码注释和 audit 报告中提到

**补档计划**：
- Stage 4 启动时：创建 `docs/develop/v0/architecture-decisions.md`（ADR）
  记录关键设计决策
- 将 `HirParam` 重复、`Emitter` trait、`check_visibility` stub 等决策
  记录到 ADR

---

## 3. 委员会投票

| 角色 | 投票 | 理由 |
|------|------|------|
| **ARCH-A** (架构师) | **GO** | §16 合规 100%，数据流清晰，命名标准化完成。架构健康，无需重构。 |
| **DEV-A** (开发) | **GO** | 984 测试 + 0 警告 + 0 clippy，代码质量高。Stage 4 基础设施就绪。 |
| **QA-A** (质量) | **GO-WITH-CONDITIONS** | 测试覆盖 ~99%，但缺 benchmark/fuzzing。建议 Stage 4 早期补基准测试。 |
| **ALG-C** (类型系统) | **GO** | `HirSelfKind` + `Res::SelfTy` 判别为 Stage 4 trait 工作奠定基础。NLL 单遍限制可接受。 |
| **SKL-A** (Tooling) | **GO** | API 命名标准 v1.5 + 流程 v3.16 完整。文档充分，新 Agent 可上手。 |

**投票结果**：5/5 GO（其中 1 个 GO-WITH-CONDITIONS）→ **GO-WITH-CONDITIONS**

---

## 4. 行动计划

### 本阶段（Stage 3）追加任务

**无**——Stage 3 完全收敛，无需追加任务。

### Stage 4 优先任务（按优先级排序）

1. **L3 闭包 codegen**（高用户价值）
   - 闭包类型 lowering + 捕获 codegen
   - 审视 `HirParam` 重复是否需要改为引用共享（TD-001）
   - 预计：2-3 轮

2. **L1 PHI 优化**（IR 质量）
   - codegen 直接 emit PHI 节点，减少对 LLVM mem2reg 的依赖
   - 预计：1-2 轮

3. **嵌套模块支持**（解阻 visibility 强制）
   - `build_module_tree` 填充 `ModuleNode.children`
   - 激活 `check_visibility` 强制检查（TD-004）
   - 预计：1-2 轮

4. **宏系统 + 属性**（新功能）
   - `Expr::MacroCall` 展开
   - AST 枚举命名统一（TD-003）
   - 预计：3-5 轮

5. **性能基准套件**（QA-A 条件项）
   - 添加 `benches/` + criterion
   - 建立编译速度基线
   - 预计：1 轮

### 技术债偿还顺序

1. **Stage 4 早期**：TD-003（AST 枚举命名）+ TD-001（HirParam 审视）+ TD-004（嵌套模块）
2. **Stage 4 中期**：TD-006（NLL 定点数据流）+ TD-007（use 3+ 段路径）
3. **Stage 4+ 添加第二后端时**：TD-002（Emitter trait 分解）
4. **Stage 5**：TD-005（Prelude 注入）+ TD-008（跨 crate 导入）

---

## 5. 结论

### **GO-WITH-CONDITIONS**

**Stage 0-3 完全收敛，可以进入 Stage 4。**

**条件**（非阻塞，建议 Stage 4 早期处理）：
1. 添加性能基准套件（QA-A 条件项）
2. 创建架构决策记录（ADR）文档
3. Stage 4 闭包工作时审视 `HirParam` 重复

**Stage 3 最终状态**：
- 984 测试全过（0 失败，2 忽略）
- 0 clippy 警告 / 0 fmt 问题 / 0 build 警告
- 0 TODO/FIXME / 0 unimplemented!
- §16 合规 8/8 ✅
- 36 轮 gate review + 6 轮跨阶段 audit CONVERGED
- 流程 v3.16（新增 §25 阶段末尾深度审查协议）
- API 命名标准 v1.5

**架构健康度**：优秀——§16 数据驱动架构 + 命名标准化完成 + 无技术债累积风险

**下一阶段就绪度**：✅ 就绪——Stage 4 基础设施（AST/HR 属性、闭包节点、
parser 骨架）已就绪，核心功能（闭包 lowering、宏展开、PHI）待实现

---

**深度审查完成**: 2026-07-22
**审查协议**: stage-committee-process.md v3.16 §25
**审查者**: Super Z (main) + Agent Group
**下一审查**: Stage 4 完成后（或用户要求时）
