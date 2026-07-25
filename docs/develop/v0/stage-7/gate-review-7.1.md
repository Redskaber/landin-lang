# Stage 7 Gate Review Round 1 (7.1) — Region inference 基础设施 (TD-015 step 1)

> **审查日期**: 2026-07-25 | **版本**: v0.14.0 → v0.14.1
> **流程**: stage-committee-process.md v3.21 §13.4（阶段开始设计对齐）+ §14.4（重构即架构设计）+ §1.2 验收
> **审查范围**: Stage 7.1 单一子阶段（Region inference 数据结构 + constraint 收集）

## CI/CD

```
cargo clean: clean
cargo test: 1881 passed + 9 new region_inference tests = 1890 total (107 unit + 1881 integration)
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 阶段开始设计对齐

依据 v3.21 §13.4，本阶段开始时查阅了 `docs/lang-design/04-ownership-borrowing.md`：

- **§3 生命周期系统**：lifetime 标注 / elision / `'static` / bound
- **§4.6 NLL 完整规范**：
  - §4.6.1 Universal region + placeholder
  - §4.6.2 Implied bounds (`&'a T` → `T: 'a`)
  - §4.6.3 Universe 机制 (HRTB)
  - §4.6.4 Type tests (`T: 'a`)
  - §4.6.5 SCC 压缩
  - §4.6.6 RegionInferenceContext 完整数据结构

**偏差**：region inference 完全未实现（TD-015，B1 偏差）。

**决策**：Stage 7.1 只做数据结构 + constraint 收集 API，不做推理算法（分阶段降低风险）。

## §14.4 J1-J6 判据检查

| # | 判据 | 状态 | 说明 |
|---|------|------|------|
| J1 | 架构设计对齐 | ✅ | 新模块按 04-ownership-borrowing.md §4.6 设计 |
| J2 | 单一职责 | ✅ | region_inference.rs = region inference 数据结构 + constraint 收集 |
| J3 | 单向流动 | ✅ | borrowck → region_inference → MirBody，无环 |
| J4 | 编译相关表达完整 | ✅ | RegionInfo / OutlivesConstraint / TypeTest / UniverseCause / RegionInferenceContext 内聚 |
| J5 | 阶段划分清晰 | ✅ | 新模块在 `src/borrowck/` 下，Stage 2 阶段 |
| J6 | 科学合理粒度 | ✅ | region_inference.rs ~370 LOC |

## 新增内容

### `src/borrowck/region_inference.rs` (370 LOC)

| 类型 | 设计 § | 用途 |
|------|--------|------|
| `RegionInfo` (enum) | §4.6.1 | Universal / Inference / Placeholder region 定义 |
| `UniverseId` | §4.6.3 | Universe 标识（HRTB） |
| `OutlivesConstraint` | §4.6.2 | `'a: 'b` 约束 |
| `ConstraintCause` (enum) | — | 约束来源（FnSignature / ImpliedBound / Borrow / TypeTest） |
| `TypeTest` | §4.6.4 | `T: 'a` 验证 |
| `UniverseCause` (enum) | §4.6.3 | Universe 创建原因（Root / Hrtb） |
| `RegionInferenceContext` | §4.6.6 | 完整数据结构（universal_regions + region_defs + constraints + type_tests + universe_causes） |

### API

| 方法 | 用途 |
|------|------|
| `RegionInferenceContext::new()` | 创建空 context（含 `'static` universal region + root universe） |
| `add_universal_region(name)` | 添加函数签名 universal region |
| `add_inference_region(universe)` | 添加 inference region |
| `add_outlives_constraint(sup, sub, cause)` | 添加 `'sup: 'sub` 约束 |
| `add_type_test(universal_region, ty, span)` | 添加 `T: 'a` type test |
| `new_universe(cause)` | 创建新 universe（HRTB） |
| `region_to_vid(region)` | Region → RegionVid 转换 |
| 6 个 getter | universal_regions / region_defs / constraints / type_tests / region_info / num_* |

### 单元测试（9 个）

- `test_new_context_has_static` — 验证初始 context 含 `'static`
- `test_add_universal_region` — 添加 universal region
- `test_add_inference_region` — 添加 inference region
- `test_add_outlives_constraint` — 添加 outlives 约束
- `test_add_type_test` — 添加 type test
- `test_new_universe` — 创建新 universe
- `test_region_to_vid` — Region → RegionVid 转换
- `test_universe_next` — UniverseId::next()
- `test_region_info_predicates` — RegionInfo 谓词

## §23 API 命名合规

- 类型名：`RegionInfo` / `OutlivesConstraint` / `TypeTest` / `UniverseCause` / `RegionInferenceContext` — `<noun>` 模式
- 函数名：`add_*` / `new_*` / `num_*` — `<verb>_<noun>` 模式
- 模块名：`region_inference` — `<noun>_<noun>` 模式
- 所有新增类型 `pub(crate)`（§16 隔离，未来需要时升级）

## §16 接口隔离合规

- 新模块独立于 `BorrowChecker` — 只读 `MirBody` 数据结构
- 不修改现有 borrowck 代码
- 不激活新功能（`#[allow(dead_code)]`，Stage 7.5 集成时激活）
- 1881 原有 tests 零回归

## 七维度审查（精简版）

| 维度 | 状态 |
|------|------|
| D1 架构健康度 | ✅ 新模块独立，未来集成路径清晰 |
| D2 技术债清单 | ✅ TD-015 step 1 完成；step 2-5 待 Stage 7.2-7.5 |
| D3 测试覆盖 | ✅ 9 个新单元测试 + 1881 原有 tests 零回归 |
| D4 下一阶段就绪度 | ✅ Stage 7.2（推理算法）已有数据结构基础 |
| D5 设计合理性 | ✅ §14.4 J1-J6 全部通过，§13.4 设计文档对齐 |
| D6 性能 | ✅ 无性能影响（新功能不激活） |
| D7 文档 | ✅ plan-7.1 + gate-review-7.1 + dev-log + api-naming-standard v1.88 + RELEASE_NOTES + README + worklog |

## 委员会投票

**5/5 GO → PASS**

## 后续行动

| 优先级 | 行动 | 目标阶段 |
|--------|------|---------|
| P2 | TD-015 step 2: Region inference 算法（不动点迭代） | Stage 7.2 |
| P2 | TD-015 step 3: Implied bounds + type tests | Stage 7.3 |
| P2 | TD-015 step 4: Universe 机制 + SCC 压缩 | Stage 7.4 |
| P2 | TD-015 step 5: 集成到 borrowck | Stage 7.5 |
| P3 | TD-018: 用户自定义 trait dyn | Stage 7+ |

---

**审查完成**: 2026-07-25
