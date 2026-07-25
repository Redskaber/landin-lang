# Stage 8.7 开发计划: 文档目录标准化 + worklog 同步 + §17.1/§17.2/§18.4 合规修复

> **阶段**: Stage 8.7 (Stage 8 文档收尾里程碑)
> **版本**: v0.15.5 → v0.15.6
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §17.1 (tests 目录标准化) + §17.2 (docs/tests 目录标准化) + §17.3 (三阶段文档协议) + §18.4 (worklog 协议) + §1.2 验收

## 1. 背景

Stage 8.1-8.6 完成 v0.2 路线图全部 5 项特性 + §25.8 设计回写 + §25 深度审查 GO。
但在 Stage 6/7/8 推进过程中，文档组织出现了长期偏差：

1. **§17.1 / §17.2 违规**: Stage 6/7/8 的 plan + gate-review + deep-review 文档
   全部 misplaced 在 `docs/develop/v0/stage-5/` 目录 (64 个文件); `docs/tests/v0/`
   目录缺少 stage6/7/8 子目录; `tests/v0/` 缺少 stage6 子目录
2. **§18.4 worklog 协议违规**: `docs/worklog.md` 只记录到 stage6.9-r157, 而
   项目实际已推进到 r182 (v0.15.5); stage6.10-r158 到 stage8.6-r182 共 24 个
   Task ID 条目缺失
3. **§17.3 三阶段文档协议违规**: plan-8.6.md 缺失 (只有 gate-review-8.6.md);
   docs/tests/v0/stage{6,7,8}/plan/ 缺少与测试代码双向印证的 .md 文档
4. **README.md / RELEASE_NOTES.md 未反映 v0.15.5 最新状态**

本轮作为 Stage 8 文档收尾，集中修复上述偏差，使项目文档体系达到 §17/§18 全合规状态。

## 2. §17.1 tests/ 目录标准化

### 2.1 缺失目录创建

```
tests/v0/stage6/plan/  ← 新建 (Stage 6 是纯重构, 无新增测试, 仅放 README)
```

### 2.2 已有目录验证

- `tests/v0/stage7/plan/` ✅ (Stage 7.5-7.9 已用)
- `tests/v0/stage8/plan/` ✅ (Stage 8.1-8.6 已用)

## 3. §17.2 docs/tests/ 目录标准化

### 3.1 缺失目录创建

```
docs/tests/v0/stage6/plan/  ← 新建 (放 README, Stage 6 无测试计划)
docs/tests/v0/stage7/plan/  ← 新建 (Stage 7 已有测试代码, 缺文档)
docs/tests/v0/stage8/plan/  ← 新建 (Stage 8 已有测试代码, 缺文档)
```

### 3.2 测试计划文档补建 (与测试代码双向印证)

| 测试代码 | 测试文档 (新建) |
|---------|----------------|
| tests/v0/stage7/plan/region_inference_tests.rs | docs/tests/v0/stage7/plan/region_inference.md |
| tests/v0/stage7/plan/user_defined_trait_dyn_tests.rs | docs/tests/v0/stage7/plan/user_defined_trait_dyn.md |
| tests/v0/stage7/plan/design_writeback_verification_tests.rs | docs/tests/v0/stage7/plan/design_writeback_verification.md |
| tests/v0/stage7/plan/deep_review_tests.rs | docs/tests/v0/stage7/plan/deep_review.md |
| tests/v0/stage7/plan/systematic_review_v014_tests.rs | docs/tests/v0/stage7/plan/systematic_review_v014.md |
| tests/v0/stage8/plan/lifetime_elision_tests.rs | docs/tests/v0/stage8/plan/lifetime_elision.md |
| tests/v0/stage8/plan/object_safety_tests.rs | docs/tests/v0/stage8/plan/object_safety.md |
| tests/v0/stage8/plan/extern_c_abi_tests.rs | docs/tests/v0/stage8/plan/extern_c_abi.md |
| tests/v0/stage8/plan/drop_elaboration_tests.rs | docs/tests/v0/stage8/plan/drop_elaboration.md |
| tests/v0/stage8/plan/async_await_tests.rs | docs/tests/v0/stage8/plan/async_await.md |
| tests/v0/stage8/plan/deep_review_tests.rs | docs/tests/v0/stage8/plan/deep_review.md |

## 4. §17.3 docs/develop/v0/ 目录标准化

### 4.1 缺失目录创建

```
docs/develop/v0/stage-6/  ← 新建
docs/develop/v0/stage-7/  ← 新建
docs/develop/v0/stage-8/  ← 新建
```

### 4.2 文件迁移 (64 个文件)

从 `docs/develop/v0/stage-5/` 迁移到对应 stage 目录:

- 33 个 stage 6 文件 (plan-6.{1..18}.md + gate-review-6.{1..18}.md, 缺 plan-6.4/6.5/6.6 — 实际 33 个)
- 19 个 stage 7 文件 (plan-7.{1..9}.md + gate-review-7.{1..9}.md + deep-review-stage7-r173.md)
- 12 个 stage 8 文件 (plan-8.{1..5}.md + gate-review-8.{1..6}.md + deep-review-stage8-r181.md + 新增 plan-8.6.md)

### 4.3 README.md 创建

每个新目录创建 README.md 作为索引:

- `docs/develop/v0/stage-6/README.md` — Stage 6 索引 (18 子阶段, 47 模块拆分)
- `docs/develop/v0/stage-7/README.md` — Stage 7 索引 (9 子阶段, TD-015/018)
- `docs/develop/v0/stage-8/README.md` — Stage 8 索引 (7 子阶段, v0.2 路线图)

## 5. §17.3 plan-8.6.md 补建

之前 Stage 8.6 只有 gate-review-8.6.md, 缺 plan-8.6.md (违反 §17.3 时期 1)。
本轮补建 plan-8.6.md, 内容描述 §25.8 + §25 计划 (虽为事后补建, 但符合协议要求)。

## 6. §18.4 worklog 同步

### 6.1 缺失 Task ID 条目 (24 个)

从 stage6.10-r158 到 stage8.6-r182, 共 24 个 Task ID 条目缺失:

- stage6.10-r158 ~ stage6.18-r166 (9 个)
- stage7.1-r167 ~ stage7.9-r175 (9 个)
- stage8.1-r176 ~ stage8.5-r180 (5 个)
- stage8.6-r182 (1 个, 含 r181 deep review)

### 6.2 同步策略

基于现有 plan + gate-review 文档反推每轮的 Work Log + Stage Summary, 使用
与现有 worklog 一致的格式 (Task ID / Agent / Task / Work Log / Stage Summary / Next)。

## 7. README.md + RELEASE_NOTES.md 更新

### 7.1 README.md

- 版本号: v0.15.5 → v0.15.6
- Stage 8.7 添加到 milestones
- 文档结构说明 (新增 stage-6/7/8 目录)
- 测试目录结构说明 (新增 stage6/7/8 目录)

### 7.2 RELEASE_NOTES.md

新增 v0.15.6 section, 描述 Stage 8.7 文档标准化工作。

## 8. api-naming-standard.md 更新

- v2.02 → v2.03
- 新增条目: 文档组织相关 API (目录结构 / 命名约定)

## 9. docs/tests/matrix.md + README.md 更新

更新全局测试矩阵, 包含 stage 6/7/8 测试统计。

## 10. 验收标准

- `cargo clean && cargo test`: 2100 passed (0 failed, 0 regressions)
- `cargo fmt --check`: clean
- `cargo clippy --all-targets`: 0 warnings, 0 errors
- §17.1 / §17.2 / §17.3 / §18.4 全合规
- 64 个文档迁移完成
- 11 个新测试计划文档创建完成
- 24 个 worklog 条目同步完成
- README.md / RELEASE_NOTES.md / api-naming-standard.md 更新完成

## 11. 版本

- Cargo.toml: 0.15.5 → 0.15.6
- api-naming-standard.md: v2.02 → v2.03

---

**创建日期**: 2026-07-25
