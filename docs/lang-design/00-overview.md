# Landin 语言设计蓝图 v1.3.2 (Final) — 总览

> **版本**: v1.3.2 (Final) · **日期**: 2026-07-18 · **状态**: ✅ **设计冻结，可进入实现阶段**
>
> **文档定位**: 本文档集是 Landin 语言的"标准编译器文档集合"。v1.3.2 经 25 路研究 + 9 轮迭代审查 + 100+ 项问题修正（含 v1.2.1 残留 P0 全部修复），于 2026-07-18 **真正正式冻结**。变更详情见 [CHANGELOG.md](./CHANGELOG.md) 与 [FREEZE-REPORT.md](./FREEZE-REPORT.md)。

---

## 1. Landin 是什么

**Landin** 是一门**静态类型、编译型、内存安全的系统级编程语言**，定位介于 C 与 Rust 之间。其核心目标是在不牺牲性能的前提下，提供现代语言的人机工程学。

### 一句话定义

> Landin = Rust 的安全模型（所有权 + 借用 + trait）+ Zig 的简洁语法（无隐藏控制流 + 显式分配器）+ 现代 IR 设计（MIR-first），分阶段交付：v0.1 = 可用编译器（Rust 实现），v0.3 = 自举完成。

### 设计哲学三原则

1. **MIR-first，AST-second**
   借鉴 Rust 1.0 之前在 AST 上做 borrow check 的痛苦教训（RFC #1211）。Landin 从 day 1 就把所有静态分析（borrow check、liveness、初始化检查、drop 顺序）放在 MIR 上做。

2. **拒绝"语言层特判"**
   借鉴 Rust 移除 `~T` sigil、`Box` 特判（RFC #59、#130）的教训。Landin 中 `Box`/`Vec`/`String`/`Rc` 全部是普通泛型库类型，编译器对任何类型一视同仁。

3. **渐进式自举**
   借鉴 Zig 与 Hare 的对照案例（R4 报告）。Stage 0 (Rust) → v0.1 发布 → stage 1 (Landin) 重写 → v0.3 自举完成。不要求 v0.1 自举，避免重蹈"声称 15 月自举但 5 年仍未完成"的覆辙。

---

## 2. 与现有语言的差异定位

| 维度 | C | Rust | Zig | **Landin** |
| --- | --- | --- | --- | --- |
| 内存安全 | ❌ | ✅ borrow checker | ⚠️ 显式 allocator 但无别名分析 | ✅ borrow checker (NLL) |
| 数据竞争安全 | ❌ | ✅ Send/Sync | ❌ | ⚠️ MVP 单线程，v0.2 加并发 |
| 泛型 | ❌ | ✅ monomorphization | ✅ comptime | ✅ monomorphization |
| 错误处理 | errno | Result + ? | error union | Result + ? |
| 元编程 | #define | macro_rules! + proc macro | comptime | 内建宏集（26 个），macro_rules! 推 v0.2 |
| 自举 | ❌ (gcc 自举) | ✅ (OCaml stage-0 冻结) | ✅ (WASM blob) | ✅ (预编译二进制 + 源码双备份) |
| 标准库规模 | 小 | 大 | 中 | **小** (核心 25k-40k 行) |
| 编译速度 | 快 | 慢 | 中 | **中** (LLVM 单后端) |

### 不做什么

- ❌ 反射 / 运行时类型信息（Rust RFC #379 教训）
- ❌ struct 继承 / virtual structs（Rust RFC #341 教训）
- ❌ sigil 内建类型（Rust RFC #59 教训）
- ❌ M:N green thread 运行时（Rust RFC #230 教训）
- ❌ Box 在 borrow checker 中的特判（Rust RFC #130 教训）
- ❌ `int`/`uint` 这种"看起来像默认"的命名（Rust RFC #544 教训）
- ❌ `macro_rules!` 宏（MVP 推迟到 v0.2）
- ❌ async/await（MVP 单线程，v0.2 加）
- ❌ specialization / overlapping impls（trait 系统稳定性优先）
- ❌ proc macro（永久不做，v0.2 仅 macro_rules!）
- ❌ GATs / async fn in trait（v0.2+）
- ❌ const generics（v0.2+）
- ❌ Send/Sync/Unpin（v0.2 加并发）
- ❌ Two-phase borrows 显式形式（MVP 仅支持 method-call 子集）

### MVP vs v0.2 特性对比表

| 特性 | v0.1 (MVP) | v0.2 | v0.3+ |
| --- | --- | --- | --- |
| 并发 | 单线程 | thread + Send/Sync | async runtime |
| 宏 | 内建宏集（26 个） | `macro_rules!` | - |
| async/await | 无 | 有 | stream/combinators |
| GATs | 无 | 有 | - |
| Const generics | 无 | 基础 | 完整 |
| `?Sized` | 部分支持（str/[T]/dyn Trait） | 完整 | - |
| Two-phase borrows | method-call 子集 | 完整 | - |
| Specialization | 永久不做 | - | - |
| Polonius | 永久不做 | - | - |
| LSP server | 无 | 有 | - |
| Cranelift 后端 | 无 | 有 | - |
| Incremental compilation | 无 | 有 | - |
| LTO | 无 | thin LTO | full LTO |
| landin-doc | 无 | 有 | - |
| landin-fmt | 无 | 有 | - |
| Effect system | 无 | 无 | v2.0 探索 |

---

## 3. 核心技术栈决策

经过 25 路研究（R1-R23 + N1-N6）交叉印证，做出以下核心技术决策：

| 决策点 | 选择 | 理由来源 |
| --- | --- | --- |
| 宿主语言（stage-0） | **Rust** | R3 推荐；用 `la_arena` + `bumpalo` 模拟 rustc arena 架构 |
| 后端 | **LLVM only** | R2、R3 共识；通过 inkwell crate 集成 |
| IR 设计 | **AST → HIR → MIR → LLVM IR**（4 层） | R1 强调 MIR 是灵魂；R6 指出 HIR 必须用 HirId/Body 外置存储，与 AST 共享 < 50% |
| 借用检查 | **NLL on MIR**（含 universal region + type tests + universe 机制 + SCC 压缩） | R5 指出 v1.0 算法不健全；R6 指出缺 SCC 压缩 |
| 类型推导 | **constraint-based**，函数签名显式 | R7 指出 v1.0 §4.5 伪代码实为 Algorithm W，需重写 |
| 泛型实现 | **monomorphization only** | R3 强制 |
| Trait 解析 | **三阶段（Evaluation + Selection + Fulfillment）+ Canonical query + depth=128**（参考 rustc **老 solver**，非 next-gen） | R6 指出 v1.0 仅两阶段不可行；R19 修正归因 |
| 寄存器分配 | **不做，交给 LLVM** | R3 强烈建议 |
| 宏系统 | **MVP 内建宏集（26 个）**（不开放自定义），`macro_rules!` 推迟 v0.2 | R9 指出 v1.0 内部矛盾 |
| 并发模型 | **MVP 单线程**，v0.2 加 Send/Sync + thread | R1/R4 共识 |
| 错误处理 | **Result + ? + `From` trait（要求唯一 impl）**，panic = abort | R5 指出多 impl 歧义 |
| 包管理器 | **v0.1 内置 mini-cargo**（path 依赖 + 简化 semver） | R1 教训 |
| 库分层 | **core / alloc / std**（第一天就分） | R1 强调 |
| Feature gate | **v0.1 上 `#[unstable]` + `#![feature(...)]`** | R1 教训 |
| 自举策略 | **stage-0 = 预编译二进制 + Rust 源码双备份**（v0.1 不自举，v0.3 自举） | R8 指出 LLVM bitcode 不稳定 |
| Unsized 类型 | **MVP 部分支持**（str / [T] / dyn Trait + ?Sized bound） | R9 指出 v1.0 矛盾 |
| Two-phase borrows | **MVP 支持子集**（method-call auto-ref） | R6 指出 v1.0 矛盾 |
| Drop check | **MVP 实现 `#[may_dangle]`** | R5 指出 v1.0 缺失 |
| Disjoint closure captures | **MVP 实现 RFC 2229** | R6 指出 stage 1 自举需要 |

---

## 4. 文档集导航（v1.3.2）

本蓝图共 **23 个文档**（16 核心 + 5 补充 + 3 元文档），按阅读顺序：

| # | 文档 | 内容 |
| --- | --- | --- |
| 00 | `00-overview.md` | 本文档：总览、设计哲学、决策摘要 |
| 01 | `01-language-specification.md` | 语言规范 |
| 02 | `02-grammar.md` | 完整 EBNF 文法、词法结构、Pratt 优先级表 |
| 03 | `03-type-system.md` | 类型系统、trait 三阶段 resolution、constraint-based 推导 |
| 04 | `04-ownership-borrowing.md` | 所有权、NLL 算法（含 universal region + SCC + Drop check + Two-phase + Disjoint closure） |
| 05 | `05-ast.md` | AST + HIR 数据结构（含 HirId/Body/OwnerNodes + Range/Slice variant） |
| 06 | `06-mir.md` | MIR 完整定义（与 rustc master 一致：Fake(FakeBorrowKind) / Coroutine / RawPtr(RawPtrKind) 等） |
| 07 | `07-codegen.md` | MIR → LLVM IR 映射（含 OperandValue 4 形态 + FunctionCx） |
| 08 | `08-bootstrap-strategy.md` | 渐进式自举策略（预编译二进制 frozen blob） |
| 09 | `09-stdlib.md` | core / alloc / std 三层标准库 API |
| 10 | `10-toolchain.md` | landin / landinc / landin-test 工具链 |
| 11 | `11-testing.md` | 测试金字塔（5,000 测试 + Soundness 套件） |
| 12 | `12-roadmap.md` | 修正后路线图（v0.1/v0.3 分期）+ 6 级应急降级 + Cargo.toml 模板 |
| 13 | `13-stage1-feature-whitelist.md` | Stage 1 源码特性白皮书（26 宏 / 22 属性） |
| 14 | `14-soundness-considerations.md` | Soundness 论证 |
| 15 | `15-attributes.md` | 属性系统完整清单（22 个 MVP 属性 + pipeline + derive 展开） |
| 16 | `16-diagnostics.md` | 诊断系统与 12 类错误代码注册表 |
| 17 | `17-conformance-suite.md` | Conformance 测试套件规范（5,000 测试） |
| 18 | `18-glossary.md` | 术语表（90+ 术语统一定义） |
| 19 | `19-project-meta.md` | 项目元信息 SSOT（编译器名/文件后缀/CLI/目录约定/工具链命名/lang items/intrinsics/ABI/target triple/命名决策依据） |
| - | `CHANGELOG.md` | v1.0 → v1.1 变更日志 |
| - | `FREEZE-REPORT.md` | v1.3.2 冻结报告 |
| - | `README.md` | 文档集入口 |

---

## 5. 研究基础

本蓝图基于 **25 路研究**（详见 `/home/z/my-project/worklog.md`）：

| 研究 ID | 主题 | 关键产出 |
| --- | --- | --- |
| R1 | Rust 2010-2013 自举史 | 8 条建议、7 个 Rust 后悔决策 |
| R2 | 现代 rustc 架构 | MVP 必须保留的 8 子系统 |
| R3 | 编译原理理论 | Top 5 必读书、8 个理论陷阱 |
| R4 | 8 门可比语言自举 | 横向对比表、5 条反例 |
| R5 | PL 理论一致性审查 | 7 个 soundness 漏洞 |
| R6 | rustc 源码深度对照 | 25 个实现细节遗漏 |
| R7 | 经典书籍章节审查 | 10 处引用错误 |
| R8 | 自举可行性分析 | 工作量低估 2.5-3.5x |
| R9 | 文档内部一致性审查 | 13 处严重矛盾 |
| R10 | v1.1 收敛审查 | 49 项承诺落实核查 |
| R11 | rustc 终验 | 11 项事实错误 |
| R12 | 文档完备性 | 57 项主题清单 |
| R13 | 工程启动性 | 综合 4.4/10 |
| R14 | v1.2 零残留审查 | 5 P0 + 18 P1 |
| R15 | v1.2 rustc 终验 | 9 P0 + 6 P1 |
| R16 | v1.2 完备性 | 79% 覆盖率 |
| R17 | v1.2 启动性 | 7.4/10 |
| R18 | v1.2.1 P0 验证 | 3 P0 + 19 P1 |
| R19 | v1.2.1 rustc 复核 | 4 P0 + 6 P1 |
| R20 | v1.2.1 综合审查 | 0 P0 + 4 P1 + 11 P2 |

### 关键洞察

1. **8 门可比语言中只有 Zig 完成自举**（R4）—— v0.3 43-64 月是现实估算
2. **MIR 是 Rust 编译器的灵魂**（R2）—— MVP 必须从 day 1 上 MIR
3. **rustc MIR 数据结构在 2023-2025 大幅演化**（R15/R19）—— v1.3.2 已与 rustc master 对齐
4. **Algorithm W + subtyping 会不终止**（R3 陷阱 #1）—— 必须用 constraint-based inference
5. **rustc 默认 recursion_limit = 128**（R11 验证）—— Landin 取相同值
6. **NLL 比 lexical lifetime 划算**（R3）—— MVP 必须有
7. **永久维护 C bootstrap 拖累 Hare 5 年**（R4）—— Landin 用预编译二进制 + 源码双备份
8. **Stage 1 自举需要 disjoint closure captures**（R6）—— MVP 必须实现 RFC 2229
9. **rustc 老 solver 三阶段**（R19 修正）—— 不是 next-gen solver
10. **BorrowKind::Shallow 应为 Fake(FakeBorrowKind)**（R19）—— v1.3.2 已修正

---

## 6. 如何阅读本文档集

- **设计评审者**：00 → 01 → 14 → 04 → 03 → 06，覆盖语义、soundness、核心算法
- **实现者**：00 → 12 → 08 → 13 → 02 → 05 → 06 → 07，按里程碑顺序
- **潜在用户**：00 → 01 → 02 → 09，判断语言是否满足需求
- **PL 研究者**：03 → 04 → 14 → 06，关注类型系统与 soundness

---

## 7. 设计变更管理

进入实现阶段后，所有设计变更必须通过 **RFC 流程**：

- v0.1 → v0.2 的破坏性变更必须有 RFC
- 新加语言特性必须有 RFC
- RFC 必须包含：动机、详细设计、替代方案、对现有代码的影响
- RFC 仓库：`landin-lang/rfcs`（v0.2 发布前建立）

---

## 8. 致谢

本蓝图基于以下一手资料：Rust 官方仓库与 RFC、rustc-dev-guide、Graydon Hoare `rust-prehistory`、Zig 官方博客、Drew DeVault 语言设计博客、Fernando Borretti Austral 设计、Cytron 1991 SSA 论文、Damas-Milner 1982 HM 论文、Jung et al. 2017 Rust 形式化、Pierce《TAPL》、Cooper & Torczon《Engineering a Compiler》、Harper《PFPL》、Braun et al. 2013 SSA、Appel 1998 "SSA is FP"、RFC 2094 NLL、RFC 1211 MIR、RFC 1327 dropck、RFC 2229 disjoint closure captures、rustc master 源码（mir/syntax.rs / hir.rs 等）。

---

**下一文档**: [`01-language-specification.md`](./01-language-specification.md) — 语言规范
