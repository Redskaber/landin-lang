# Stage 14 — v0.1 Release Readiness: Architecture Cleanup + API Standardization + Documentation Sync

> **Author**: redskaber
> **Date**: 2026-07-28
> **Version**: v0.35.0 (baseline) → v0.36.0 (target)
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §23 + §25 + §25.8)
> **Status**: 🔄 In Progress

---

## 1. Stage 14 启动依据

Stage 14 启动基于 Stage 14.1 v0.1 Capability Assessment 的结论
(`docs/develop/v0/stage-14/v0.1-capability-assessment.md`):

- **Verdict**: NO-GO for v0.1 release (prior "v0.1 GATE REACHED" claim
  in `docs/develop/v0/stage-12/v0.1-release.md` is formally superseded)
- **P0 blockers**: 11 (GAP-0 through GAP-30)
- **P1 issues**: 9
- **P2 nice-to-haves**: 11
- **User instruction**: "当前还存在大量问题，远没有达到v0.1发布的需求，
  继续计划推进, API命名标准化，架构清理和优化"

Stage 14 收纳本会话中可完成的"高 ROI 低风险"清理工作:
- API 命名标准化 (§23)
- 架构清理 (§14.4)
- 文档同步 (§17.3 + §18)
- examples/benchmark 标准化 (§17.4)
- README / RELEASE_NOTES 重写
- 版本号同步 (GAP-0)

并将遗留的 P0 blockers (NLL soundness / region inference / drop
elaboration / lifetime elision / self.x crash / stdlib MVP / mini-cargo
CLI / canonical query / disjoint captures / ?Sized + HRTB) 明确记录为
v0.1-rc2 known limitations, 纳入 Stage 14.10+ 后续会话计划.

---

## 2. 子阶段索引

| Sub-stage | Status | Goal | Plan | Gate Review |
|-----------|--------|------|------|-------------|
| 14.1 | ✅ Complete | v0.1 capability assessment + gap analysis | (this file §1) | `v0.1-capability-assessment.md` |
| 14.2 | 🔄 In Progress | Process hygiene: worklog backfill + version sync | §3.1 | (TBD) |
| 14.3 | ✅ Complete | Architecture cleanup: split `trait_dispatch.rs` (962 LOC) per §14.4 | §3.2 | `gate-review-14.3.md` |
| 14.4 | ✅ Complete | API naming audit (§23): fix glob re-exports in `stdlib/mod.rs` | §3.3 | `gate-review-14.4.md` |
| 14.5 | ✅ Complete | examples/ standardization + new `trait_dispatch_emission` example | §3.4 | `gate-review-14.5.md` |
| 14.6 | 🔄 In Progress | Documentation sync (dev-log + tests + matrix) | §3.5 | (TBD) |
| 14.7 | ⏳ Planned | README.md rewrite | §3.6 | (TBD) |
| 14.8 | ⏳ Planned | RELEASE_NOTES.md update | §3.7 | (TBD) |
| 14.9 | ⏳ Planned | Final verification + package zip | §3.8 | (TBD) |
| 14.10+ | ⏳ Deferred | Deep P0 blockers (NLL, region, drop, lifetime, self.x, stdlib, cargo) | §4 | — |

---

## 3. 子阶段计划

### 3.1 Stage 14.2 — Process Hygiene

**Goal**: Close GAP-0 (process gap) — backfill worklog for undocumented
Stages 13.30-13.34 + synchronize version strings.

**MUV**:
- M: Backfill worklog entries for Stages 13.30-13.34 (retrospective —
  based on code state, since worklog was not updated at the time)
- U: Bump Cargo.toml v0.35.0 → v0.36.0 (Stage 14 work)
- V: Mirror `/home/z/my-project/worklog.md` → `docs/worklog.md` (§18.4.0)

**Complexity**: L1.

### 3.2 Stage 14.3 — Architecture Cleanup: `trait_dispatch.rs` Split

**Goal**: Per §14.4, split `src/codegen/trait_dispatch.rs` (962 LOC) into
3 focused sub-modules along the vtable/dynptr/orchestrator boundary.

**MUV**:
- M: Create `src/codegen/trait_dispatch/{mod,vtable,dynptr,orchestrator}.rs`
  with §14.4 J1-J6 compliance table in `mod.rs`
- U: Move functions to sub-modules by responsibility; `mod.rs` re-exports
  all public symbols via explicit list (no glob — §23 compliant)
- V: `cargo test --features llvm-backend` — 1951 tests still pass (zero
  behavior change, pure code reorganization)

**Complexity**: L2.

**6 大判据 (J1-J6) check**: see `src/codegen/trait_dispatch/mod.rs` header.

### 3.3 Stage 14.4 — API Naming Audit (§23)

**Goal**: Scan `src/` for §23 violations + fix all.

**MUV**:
- M: `grep -rn "pub use.*::\*" src/` — found 2 violations in
  `src/stdlib/mod.rs` (lines 34, 35: `pub use trait_methods::*;` +
  `pub use vtable_layout::*;`)
- U: Replace with explicit re-export lists (27 names from `trait_methods`
  + 18 names from `vtable_layout`)
- V: `cargo build --features llvm-backend` + `cargo test` — all green

**Complexity**: L1.

**Audit results**:
- ✅ 0 glob re-exports remaining (only comment references in 6 files)
- ✅ All `#[deprecated]` attributes have `note = "..."` (4 occurrences)
- ✅ All stage entries follow free-function pattern (verified in
  `api-naming-standard.md` §2.2)

### 3.4 Stage 14.5 — examples/ Standardization (§17.4)

**Goal**: Wire `examples/usage/` to be runnable via `cargo run --example`;
add a new Stage 14 example demonstrating the post-split trait dispatch API.

**MUV**:
- M: Add 4 `[[example]]` declarations to `Cargo.toml` (3 existing + 1 new)
- U: Create `examples/usage/trait_dispatch_emission.rs` — demonstrates
  `build_trait_dispatch_emission_plan` + `emit_trait_dispatch_globals_text_batch`
- V: `cargo build --examples --features llvm-backend` — all 4 examples compile

**Complexity**: L1.

### 3.5 Stage 14.6 — Documentation Sync

**Goal**: Per §17.3 + §18, create Stage 14 documentation.

**MUV**:
- M: Create `docs/develop/v0/stage-14/{plan,dev-log,gate-review-14.{3,4,5}}.md`
- U: Create `docs/tests/v0/stage14/{plan,gate}/README.md`
- V: Update `docs/tests/matrix.md` with Stage 14 row

**Complexity**: L2.

### 3.6 Stage 14.7 — README.md Rewrite

**Goal**: Update README from v0.27.1 → v0.36.0; reflect actual state +
v0.1-rc2 known limitations.

**Complexity**: L1.

### 3.7 Stage 14.8 — RELEASE_NOTES.md Update

**Goal**: Add v0.36.0 entry summarizing Stage 14 work.

**Complexity**: L1.

### 3.8 Stage 14.9 — Final Verification + Package

**Goal**: Run §1.2 acceptance checks + package zip.

**MUV**:
- M: `cargo clean && cargo build --lib --features llvm-backend && cargo fmt
  && cargo clippy --all-targets --features llvm-backend -- -D warnings &&
  cargo test --features llvm-backend`
- U: Verify all green (0 warnings, 0 test failures)
- V: `zip -r landin-stage0-v0.36.0-stage14-architecture-cleanup-r253.zip
  landin-stage0/` → save to `/home/z/my-project/`

**Complexity**: L1.

---

## 4. Deferred P0 Blockers (Stage 14.10+)

The following P0 blockers identified in Stage 14.1 assessment are
**deferred to future sessions** (each requires dedicated L2/L3 effort):

| ID | Gap | Est. effort | Notes |
|----|-----|-------------|-------|
| GAP-1 | NLL soundness regression (229 tests unsoundly flipped) | L3 | Needs fixpoint dataflow |
| GAP-2 | Region inference is dead_code (no-op) | L3 | Needs SCC + type tests + universe |
| GAP-3 | Drop elaboration is dead_code | L3 | Needs Drop::drop codegen + dropck |
| GAP-4 | Lifetime elision is dead_code | L2 | Needs 3 rules per §03-type-system.md §5 |
| GAP-5 | `self.x` field access crashes codegen | L2 | Needs typeck writeback |
| GAP-6 | Two-phase borrows (method-call subset) | L2 | RFC-related |
| GAP-8 | `run_ok` conformance tests not actually run | L2 | Runner rewrite |
| GAP-9 | No real standard library | L3 | core + alloc in Landin source |
| GAP-10 | Trait 3-phase canonical query | L3 | Evaluation + Selection + Fulfillment |
| GAP-11 | Associated type normalization | L2 | With depth limit |
| GAP-12 | `?Sized` bound | L2 | Partial support |
| GAP-13 | HRTB `for<'a>` | L2 | For trait impls |
| GAP-14 | Cross-module visibility enforcement | L2 | Activate check_visibility |
| GAP-15 | Mini-cargo CLI (`landinc build/run/test`) | L3 | `landin.toml` parser + multi-crate |
| GAP-7 | Disjoint closure captures (RFC 2229) | L2 | Field-level capture analysis |

**Estimated total**: 6-10 weeks of focused work (per Stage 14.1 assessment).

These are documented in `docs/develop/v0/stage-14/v0.1-capability-assessment.md`
and will be addressed in Stage 14.10+ sessions.

---

## 5. 设计文档对齐 (§13.4)

| 设计文档 | 本阶段对齐 | 偏差 |
|---------|-----------|------|
| `07-codegen.md` §4 vtable emission | ✅ `trait_dispatch/vtable.rs` implements vtable global emission | B4 (设计未明确 vtable/dynptr/orchestrator 拆分, 实现做了) — 补写 §4.X |
| `07-codegen.md` §4 dynptr emission | ✅ `trait_dispatch/dynptr.rs` implements dynptr global emission | 同上 |
| `12-roadmap.md` §1 v0.1 release | ⚠️ v0.1 NOT YET (P0 blockers remain) | B1 (实现 < 设计) — 纳入 Stage 14.10+ |
| `13-stage1-feature-whitelist.md` | ⚠️ Multiple features not yet implemented | B1 — 纳入 Stage 14.10+ |
| `api-naming-standard.md` §2-§4 | ✅ §23 audit passed (0 violations) | 无 |

**§25.8 设计回写**: 本阶段对 `07-codegen.md` 的补写 (vtable/dynptr/orchestrator
拆分) 留待 Stage 14.10+ 的 §25.8 design-writeback 阶段统一执行.

---

## 6. 关键文档

- `docs/develop/v0/stage-14/v0.1-capability-assessment.md` — Stage 14.1 完整评估
- `docs/develop/v0/stage-14/dev-log.md` — Stage 14 开发日志
- `docs/develop/v0/stage-14/gate-review-14.{3,4,5}.md` — 子阶段门审查
- `docs/tests/v0/stage14/plan/README.md` — Stage 14 测试文档
- `docs/tests/matrix.md` — 测试矩阵 (Stage 14 行已添加)
- `docs/worklog.md` — worklog 镜像 (§18.4.0)
- `RELEASE_NOTES.md` — v0.36.0 entry
- `README.md` — v0.36.0 rewrite

---

**创建日期**: 2026-07-28
**Process**: v3.21 (§13.4 + §14.4 + §23 + §25 + §25.8)
