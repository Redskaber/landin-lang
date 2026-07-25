# Stage 9.1 — Systematic Review + v0.1 Conformance Kickoff

> **阶段**: Stage 9.1 (Stage 9 启动 — 系统性审查 + 阶段性规划)
> **版本**: v0.15.6 → v0.16.0
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §25 (深度审查) + §13.4 (阶段开始设计对齐) + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 8.7 完成 §17 文档标准化后，项目到达 v0.2 路线图全部完成 + 文档全合规的
重大里程碑。本阶段 (Stage 9.1) 是 Stage 9 的启动阶段，承担两大任务：

1. **系统性审查** — 对当前项目状态做全面体检 (架构 / 技术债 / 测试 / 设计文档 / CLI)
2. **阶段性规划** — 决定 Stage 9 方向，制定可执行的子阶段计划

## 2. §25 系统性审查（七维度）

### 2.1 D1 架构健康度 ✅

- **源代码**: 90 files, 32,014 LOC (avg 356 LOC/file)
- **超过 1000 LOC 的文件**: 7 个
  - `borrowck/region_inference.rs` (1462 LOC, incl. ~600 LOC tests)
  - `mir/lower/expr_operand.rs` (1279 LOC, TD-019 OPEN)
  - `borrowck/mod.rs` (1205 LOC, incl. ~600 LOC tests)
  - `typeck/checker.rs` (1156 LOC)
  - `stdlib/trait_methods.rs` (1103 LOC)
  - `codegen/mod.rs` (1058 LOC)
  - `parser/expr.rs` (1047 LOC)
- **模块组织**: 50+ 模块, 单一职责, 数据流单向 ✅
- **§14.4 J1-J6 判据**: 全部满足

### 2.2 D2 技术债清单 ✅

| ID | 描述 | 状态 |
|----|------|------|
| TD-019 | `expr_operand.rs` 巨型 match (1279 LOC) | 🟡 OPEN (用户指示暂不拆) |
| TD-011 ~ TD-018, TD-022 ~ TD-026 | 全部已偿还 | ✅ CLOSED |

**新增技术债**: 0。**结论**: 仅 TD-019 OPEN，且为用户主动 hold，无需偿还。

### 2.3 D3 测试覆盖深度 ✅

| 测试类型 | 数量 |
|---------|------|
| Unit tests (inline) | 146 |
| Integration tests (`tests/v0/stage{0-8}/plan/`) | 1954 |
| Doc-tests (ignored) | 2 |
| Conformance tests (`tests/conformance/`) | 8 |
| Benchmarks | 5 |
| **总计** | **2100 tests + 5 bench + 8 conformance** |

**测试增长曲线**: 993 (Stage 4) → 1881 (Stage 5.99) → 2100 (Stage 8.6) → 2100 (Stage 9.1)

**测试覆盖盲区**:
- Conformance suite 仅 8 个 (目标 600) — Stage 9 主战场
- 无 fuzzing 套件 (设计文档 §10 提及但未实现)
- 无 multi-platform CI 矩阵 (仅有 Linux)

### 2.4 D4 下一阶段就绪度 ✅

**v0.1 release gate** (per `12-roadmap.md` §1):
- ✅ Stage 0 完整 (lexer + parser + AST)
- ✅ Stage 1-8 完整 (HIR + MIR + typeck + borrowck + codegen + traits + stdlib + v0.2 features)
- ❌ Conformance 通过 (8/600 ≈ 1.3%) — **主要 gap**

**v0.3 bootstrap gate** (远期):
- ✅ Stage 0 实现完整
- ❌ Stage 1 重写规划 (远期)
- ❌ 自举验证

### 2.5 D5 设计合理性 ✅

设计文档 (19 份, v1.3.2 Final, frozen) 全部已通过 §25.8 同步:
- `01-language-specification.md` §13 ✅
- `02-grammar.md` §5 ✅
- `03-type-system.md` §10+§11+§12 ✅
- `04-ownership-borrowing.md` §11+§12+§13 ✅
- `05-ast.md` §13+§14 ✅
- `06-mir.md` §14 ✅
- `07-codegen.md` §14+§15 ✅
- `09-stdlib.md` §11 ✅

**新发现偏差**: 0 (Stage 8.7 已清理所有 B1/B3/B4)

### 2.6 D6 性能与可扩展性 ✅

- Lexer/Parser: O(n) 线性
- HIR lowering: O(n)
- MIR lowering: O(n)
- Typeck unification: O(n × α(n)) (union-find)
- Borrow checker NLL: O(R²×P) + Tarjan SCC O(V+E)
- Codegen: O(n) per function
- 无 O(n²) 或更差算法 ✅

### 2.7 D7 文档与知识传承 ✅

- §17.1/§17.2/§17.3/§18.4 全合规 (Stage 8.7 完成)
- 9 个 stage 目录 (stage-0 到 stage-8) + 新增 stage-9
- 9 个 tests/v0/stageN 目录 + 新增 stage9
- worklog.md 完整 (7497 lines, 191 Task IDs, stage5.99-r148 → stage8.7-r183 无缺口)
- RELEASE_NOTES.md + README.md + api-naming-standard.md (v2.03) 全部最新

## 3. 战略决策（§15 长期 > 短期）

### 3.1 三个候选方向

| 方向 | 描述 | 长期价值 | 短期成本 | 风险 |
|------|------|---------|---------|------|
| **A. v0.1 Conformance** | 扩展 conformance 套件到 600 parse tests | 极高 — v0.1 release gate | 中 — 每测试 ~5-10 行 .lin | 低 |
| B. v0.3 Bootstrap Prep | Stage 1 重写规划 | 高 — 但远期 | 高 — 需 Stage 0 全功能 | 高 |
| C. v0.2+ Features | macro_rules!/Send/Sync/GATs | 中 — 功能扩展 | 中-高 | 中 |

### 3.2 决策: 方向 A — v0.1 Conformance

**理由** (per §15 长期 > 短期):

1. **明确的 release gate** — `12-roadmap.md` §1 明确 "v0.1 = Stage 0 完整 + conformance 通过"
2. **可执行的语言规范** — 测试用例即语言规范的可执行版本 (`17-conformance-suite.md` §1.3)
3. **回归保护** — 任何 stage 0 修改后必须仍通过全部套件 (`17-conformance-suite.md` §1.2)
4. **跨编译器一致性** — stage 1 重写后必须通过同一套件 (`17-conformance-suite.md` §1.4) — 为 v0.3 铺路
5. **低风险高回报** — 每个测试独立、增量、可并行
6. ** unlocks v0.1 release** — 唯一明确的下一步 milestone

**对比方向 B/C**:
- 方向 B 风险过高 — Stage 1 重写需要 v0.1 稳定作为参照
- 方向 C 功能扩展 — 但缺 conformance 验证的功能扩展会累积隐藏 bug

### 3.3 Stage 9 子阶段计划

**目标**: conformance tests 8 → 600+ (per `17-conformance-suite.md` §2)

| 子阶段 | 主题 | 测试数 | 累计 |
|--------|------|-------|------|
| 9.1 | Systematic review + conformance kickoff + literals expansion | +30 | 38 |
| 9.2 | Operators + Pratt precedence | +60 | 98 |
| 9.3 | Control flow (if/while/for/loop/match/break/continue) | +80 | 178 |
| 9.4 | Patterns (wild/ident/lit/struct/tuple/or/range) | +70 | 248 |
| 9.5 | Types (primitives/refs/ptrs/arrays/generics) | +60 | 308 |
| 9.6 | Attributes (#[derive]/#![inner]/meta) | +40 | 348 |
| 9.7 | Generics (type params/bounds/where) | +50 | 398 |
| 9.8 | Closures (||/|args|/move ||) | +40 | 438 |
| 9.9 | Modules (mod/use/visibility) | +60 | 498 |
| 9.10 | Error recovery (malformed programs) | +50 | 548 |
| 9.11 | Realistic programs (fib/iterators/traits) | +52 | 600 |
| 9.12 | §25 deep review + v0.1 release candidate | — | 600 |

**每子阶段交付**:
- N 个 `.lin` 测试文件 in `tests/conformance/00-parse/<category>/`
- 1 个 plan 文档 in `docs/develop/v0/stage-9/`
- 1 个 gate-review 文档
- 1 个测试计划文档 in `docs/tests/v0/stage9/plan/`
- worklog 条目 + RELEASE_NOTES 更新
- CI/CD 全绿 (cargo test + fmt + clippy + conformance run_all.py)

## 4. Stage 9.1 具体计划

### 4.1 Systematic review 文档

创建 `docs/develop/v0/stage-9/systematic-review-v0156.md` (本文件的扩展版)

### 4.2 Conformance 测试扩展 (literals category)

新增 30 个 `.lin` 测试到 `tests/conformance/00-parse/00-literals/`:

| 类别 | 测试数 | 示例 |
|------|-------|------|
| Integer decimal | 5 | `42`, `0`, `999_999`, `1_000_000`, `007` |
| Integer hex | 4 | `0xff`, `0xDEAD_BEEF`, `0x0`, `0x` (FAIL) |
| Integer octal | 3 | `0o777`, `0o0`, `0o123_456` |
| Integer binary | 3 | `0b1010`, `0b0`, `0b1111_0000` |
| Integer suffix | 4 | `42i32`, `42u64`, `42isize`, `42usize` |
| Float | 5 | `3.14`, `1.0e10`, `1_000.000_001`, `0.0`, `1f64` |
| Char | 3 | `'a'`, `'\n'`, `'\\'` |
| String | 3 | `"hello"`, `""`, `"with\nescape"` |

### 4.3 验证

```bash
cargo clean && cargo test
cargo fmt --check
cargo clippy --all-targets
python3 tests/conformance/run_all.py  # 期望: 38 passed, 0 failed
```

### 4.4 文档

- `docs/develop/v0/stage-9/plan-9.1.md` (本文件)
- `docs/develop/v0/stage-9/systematic-review-v0156.md`
- `docs/develop/v0/stage-9/gate-review-9.1.md`
- `docs/tests/v0/stage9/plan/systematic_review_v0156.md`
- `docs/tests/v0/stage9/plan/conformance_literals.md`
- `tests/v0/stage9/plan/systematic_review_v0156_tests.rs` (验证脚本)
- `docs/develop/v0/stage-9/README.md` (Stage 9 索引)

## 5. 验收标准

- ✅ `cargo test`: 2100+ tests pass (期望 +5 systematic review tests = 2105)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 38 passed (8 + 30 new)
- ✅ Stage 9 方向决定 (v0.1 Conformance)
- ✅ Stage 9 子阶段计划制定
- ✅ 所有文档创建 (§17.3 三阶段文档协议)

## 6. 版本

- Cargo.toml: 0.15.6 → 0.16.0 (Stage 9 启动, minor bump)
- api-naming-standard.md: v2.03 → v2.04

---

**创建日期**: 2026-07-26
