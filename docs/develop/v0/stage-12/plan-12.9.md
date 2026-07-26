# Stage 12.9 — Plan: Polish Backfill (deferred P2/P3 items from gate-review-12.8.md)

> **版本**: v0.21.3 → v0.21.4 (target) | **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.21 (§25.7 P2/P3 problem handling + §15 long-term > short-term)
> **基于**: gate-review-12.8.md §"Stage 13.1 immediate actions" item 4 (deferred P2/P3 follow-ups)
> **创建日期**: 2026-07-26

---

## 1. 阶段定位

### 1.1 背景

Stage 12 在 Stage 12.8 marked COMPLETE (v0.21.3)，但 gate-review-12.8.md §"Stage 13.1
immediate actions" item 4 明确列出 3 项 P2/P3 polish items 作为 "Stage 13.1-adjacent,
non-blocking, ~4-6 hours total" 推迟处理：

1. Stage 5 develop-side `README.md` creation (D7 gap from r217 stages-5-8 audit §5.5)
2. Stage 6 `plan-6.{4,5,6}.md` retroactive backfill (per r217 stages-5-8 audit §7 P2 item 6)
3. api-naming-standard v2.36 record correction (+11 → +12 tests for Stage 12.2)

### 1.2 §15 决策

用户指示"继续推进 Stage 12"。依据 §15「最优 > 最小」（长期 > 短期），应在 Stage 13
启动前关闭所有 P2/P3 polish items，避免技术债累积。Stage 12.9 即为此目的设立的
polish 子阶段。

### 1.3 不影响 Stage 13 启动

per gate-review-12.8.md，这些 polish items 是 non-blocking 的：
- Stage 13 launch criteria (5 项) 全部已 closed
- Stage 12.9 是 "nice-to-have" polish，不是 Stage 13 启动前置条件
- Stage 12.9 完成后，Stage 13 launch 仍然 ✅ AUTHORIZED

---

## 2. MUV 拆分

### MUV-1: Stage 5 develop README

- 输入：Stage 5 是最大阶段（99 sub-stages, 977 rust tests, 502 conformance, 200 dev docs）但 develop-side 无 README.md
- 输出：`docs/develop/v0/stage-5/README.md` 创建（mirror stage-6/README.md 结构）
- 验收：文件存在 + 结构符合 §17.3 + 引用 Stage 12.4 §25.8 retroactive backfill
- Task ID: stage12.9-muv1-r221 ✅ DONE (subagent)

### MUV-2: Stage 6 plan-6.{4,5,6}.md retroactive backfill

- 输入：Stage 6 has 18 gate-review files but only 15 plan files (6.4, 6.5, 6.6 missing)
- 输出：3 retroactive plan files 从对应 gate reviews 重建
- 验收：3 files exist + 每个 ≤150 行 + 包含 §14.4 J1-J6 判据 + 标注"retroactive backfill"
- Task ID: stage12.9-muv2-r221 ✅ DONE (subagent)

### MUV-3: api-naming-standard v2.36 record correction

- 输入：v2.36 record says "+10 rust (2325 → 2335)" — actual stage12_2_tests.rs has 12 tests
- 输出：v2.36 record corrected to "+12 rust (2325 → 2337)" + correction note
- 验收：record matches actual test count + correction note explains the delta
- Task ID: stage12.9-muv3-r221 ✅ DONE (main agent)

### MUV-4: Stage 12.9 verification tests

- 输入：Stage 12.9 polish work 需 verification tests 确保文件存在 + 结构正确
- 输出：`tests/v0/stage12/plan/stage12_5_tests.rs` 创建
- 验收：cargo test 全绿
- Task ID: stage12.9-muv4-r221 ✅ DONE (main agent)

---

## 3. 验收标准 (per §3.3 + §1.2)

| 维度 | 标准 | Stage 12.9 |
|------|------|-----------|
| `cargo test` | 0 failed | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| Conformance gate | 5026+ (no regression) | ✅ |
| 文档完整性 | Stage 5 README + plan-6.{4,5,6}.md + v2.36 correction | ✅ |

---

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Retroactive plan-6.{4,5,6}.md 内容不准确 | 低 | 低（仅文档） | 从 gate reviews 完整重建，标注 retroactive |
| Stage 5 README 计数错误 | 低 | 低 | 用 Grep/Bash 实际计数，引用 r217 已验证数字 |
| v2.36 correction 引入新错误 | 极低 | 极低 | 仅修改一行 + 添加 correction note |

---

## 5. 文档同步计划 (per §17.3 + §18)

### 5.1 开发轮文档
- `docs/develop/v0/stage-12/plan-12.9.md` (本文件)
- `docs/develop/v0/stage-12/gate-review-12.9.md` (Stage 12.9 完成后)

### 5.2 测试文档
- `tests/v0/stage12/plan/stage12_5_tests.rs` (Stage 12.9 验证测试)

### 5.3 同步更新
- `README.md` — Stage 12.9 sub-stage added
- `RELEASE_NOTES.md` — v0.21.4 entry
- `docs/develop/v0/api-naming-standard.md` — v2.39 entry + v2.36 correction
- `docs/tests/matrix.md` — Stage 12.9 row added
- `docs/worklog.md` — Stage 12.9 entry appended

---

**Stage 12.9 完成日期**: 2026-07-26
**Next**: Stage 13.1 launch (TD-028 §16 fix + TD-029 TyKind::Dynamic refactor)
