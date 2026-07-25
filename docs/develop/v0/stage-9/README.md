# Stage 9 — v0.1 Conformance Suite Expansion

> **阶段范围**: Stage 9.1 - 9.12 (12 sub-stages planned)
> **版本范围**: v0.15.6 → v0.20.x (target v0.1 release candidate)
> **流程**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §17.1/§17.2/§17.3 + §25 + §25.8)
> **状态**: 🟡 In Progress (Stage 9.1 complete)

## 阶段目标

按 `docs/lang-design/17-conformance-suite.md` §2 规范, 将 conformance 测试从 8 个
扩展到 600+ 个, 通过 v0.1 release gate (`12-roadmap.md` §1: "v0.1 = Stage 0 完整
+ conformance 通过")。

## 子阶段索引

| 子阶段 | 主题 | 新增测试 | 累计 | 状态 |
|--------|------|---------|------|------|
| 9.1 | Systematic review + literals expansion | +30 | 38 | ✅ Complete |
| 9.2 | Operators + Pratt precedence | +60 | 98 | 🟡 Planned |
| 9.3 | Control flow (if/while/for/loop/match/break/continue) | +80 | 178 | 🟡 Planned |
| 9.4 | Patterns (wild/ident/lit/struct/tuple/or/range) | +70 | 248 | 🟡 Planned |
| 9.5 | Types (primitives/refs/ptrs/arrays/generics) | +60 | 308 | 🟡 Planned |
| 9.6 | Attributes (#[derive]/#![inner]/meta) | +40 | 348 | 🟡 Planned |
| 9.7 | Generics (type params/bounds/where) | +50 | 398 | 🟡 Planned |
| 9.8 | Closures (||/|args|/move ||) | +40 | 438 | 🟡 Planned |
| 9.9 | Modules (mod/use/visibility) | +60 | 498 | 🟡 Planned |
| 9.10 | Error recovery (malformed programs) | +50 | 548 | 🟡 Planned |
| 9.11 | Realistic programs (fib/iterators/traits) | +52 | 600 | 🟡 Planned |
| 9.12 | §25 deep review + v0.1 release candidate | — | 600 | 🟡 Planned |

## 战略理由 (§15 长期 > 短期)

1. **明确的 release gate** — `12-roadmap.md` §1: "v0.1 = Stage 0 完整 + conformance 通过"
2. **可执行的语言规范** — `17-conformance-suite.md` §1.3: "测试用例即语言规范的可执行版本"
3. **回归保护** — `17-conformance-suite.md` §1.2: "任何 stage 0 修改后必须仍通过全部套件"
4. **跨编译器一致性** — `17-conformance-suite.md` §1.4: "stage 1 重写后必须通过同一套件" — 为 v0.3 铺路
5. **低风险高回报** — 每个测试独立、增量、可并行
6. **unlocks v0.1 release** — 唯一明确的下一步 milestone

## 关键里程碑

- 🎯 Stage 9.12 完成 = v0.1 release candidate
- 🎯 600+ conformance tests 全绿
- 🎯 §25.8 设计回写 `17-conformance-suite.md` +§11 (实现状态)

## 技术债状态

| ID | 描述 | 状态 |
|----|------|------|
| TD-019 | expr_operand 巨型 match | 🟡 OPEN (user-directed hold) |

## 关联测试

- `tests/conformance/00-parse/00-literals/*.lin` (33 files after 9.1)
- `tests/conformance/00-parse/{01-10}-*/` (to be expanded in 9.2-9.11)
- `tests/v0/stage9/plan/systematic_review_v0156_tests.rs` (10 tests)
