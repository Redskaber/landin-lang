# 12 — 路线图

> 本文给出 Landin 项目的 **v0.1/v0.3 分期路线图**（v1.2 修正：不再声称 15 月自举）、每月里程碑、风险登记、v0.2+ 远景。基于 R8 可行性分析报告。

---

## 1. 总体路线图（v1.2 修正）

**前提声明**：本路线图假设 **1 人全职投入**（每周 40+ 小时）或 **2-3 人小团队**（每周 20+ 小时/人）。R4 报告指出："8 门可比语言中只有 Zig 完成自举，耗时 ~7 年"——**v0.1（可用编译器）预期 27-40 月，v0.3（自举完成）预期 43-64 月**，是远高于业界平均的激进目标，需严格 scope 控制。

业余单人开发（每周 10-15 小时）需把时间线 ×2.5-3，即 v0.1 需 60-100 月、v0.3 需 100-180 月。

**重要说明**：本文档 §2 中的"月 1-月 15"指**实现阶段月**（自 stage 0 开发起算），不是日历月。§2 仅描述 stage 0 + 自举验证的实现顺序，总实际工期 = v0.1 (27-40 月) + v0.2 + 自举 (16-24 月) = v0.3 (43-64 月)。

```
月 1          月 2-3        月 4-6         月 7-9        月 10-12      月 13-15+
─────────────────────────────────────────────────────────────────────────
设计冻结  →  Stage 0 前端  →  Stage 0 中端  →  Stage 0 后端  →  Stage 1 重写  →  自举验证
                        ↓                ↓               ↓                ↓
                     Lexer/Parser    Typeck/Borrow    Codegen/Stdlib   Landin 重写    Stage 2 一致性
                                       NLL 算法         mini-cargo       完成自举       v0.1 发布
```

**v0.1 = Stage 0 完整 + conformance 通过（不自举）**：27-40 月
**v0.3 = Stage 1 重写完成 + 自举验证**：43-64 月

---

## 2. 每月里程碑

### 月 1：设计冻结

**目标**：完成本文档集 v1.0，所有设计决策固化。

**产出**：

- ✅ 13 个设计文档（00-overview 到 12-roadmap）
- ✅ BNF 文法定稿
- ✅ MIR 结构定义
- ✅ 自举策略明确
- ✅ RFC 仓库建立（landin-lang/rfcs）

**验收**：

- 设计文档可独立阅读、无矛盾
- 至少 3 位 PL 背景评审者通过（实际若独自开发则自审 3 轮）

---

### 月 2：Lexer + Parser

**目标**：能 parse 全部 conformance 测试源码。

**产出**：

- Lexer（~1,500 行 Rust）：所有 token 类型、错误恢复
- Parser（~4,000 行 Rust）：recursive descent + Pratt，所有产生式
- AST 定义（~2,500 行 Rust）：完整数据结构
- 200 个 parse 测试

**关键决策**：

- 手写 lexer/parser（R3 推荐，不上 generator）
- 错误恢复策略：跳至 sync token（`;`、`}`、`fn`、`struct`）
- Pratt 优先级表固化

**风险**：

- `<<` lexer hack 实现复杂度可能超预期
- 错误恢复质量决定后续体验

**验收**：

- 200 个 conformance parse 测试全通过
- 100 个故意错误的程序能给出合理错误信息

---

### 月 3：HIR + Name Resolution

**目标**：完成 AST → HIR lowering，名字解析正确。

**产出**：

- HIR 定义（~3,000 行 Rust）
- Name resolution（~2,500 行 Rust）：use 导入、可见性、prelude
- Lifetime elision 规则
- 50 个 name resolution 测试

**关键决策**：

- HIR 与 AST 共享约 50% 结构（v1.2.2 修正：R6 指出 v1.0 的"80%"错误，HIR 有 HirId/Body/OwnerNodes 独有机制）
- 不做嵌套 item（与 Rust 不同，简化）

**风险**：

- Glob import 的 shadow 规则容易出错
- prelude 设计可能反复调整

**验收**：

- 50 个 name resolution 测试通过
- 能跑通 hello world 的 name resolution

---

### 月 4：Type Check 基础

**目标**：基本类型推导、unification 工作。

**产出**：

- Type checker 框架（~3,000 行 Rust）
- Unification 算法（constraint-based）
- 基本类型推导（无 trait、无 lifetime）
- 100 个 type check 测试

**关键决策**：

- 用 constraint-based inference（R3 推荐，不用 Algorithm W）
- 整数 fallback 到 i32
- 函数签名必须显式，不做 let-generalization

**风险**：

- Unification 边界 case（recursive types、occurs check）
- 类型推导错误信息难写

**验收**：

- 100 个 type check 测试通过
- 能 typeck hello world 与 fib

---

### 月 5：Trait Resolution

**目标**：完整 trait 系统（impl、coherence、orphan、resolution）。

**产出**：

- Trait resolution（~3,000 行 Rust）
- Coherence check
- Orphan rule 检查
- Trait object 生成
- 100 个 trait 测试

**关键决策**：

- 递归 context reduction + depth limit = 128
- 禁 overlapping impls
- 不上 Chalk/SLG（R3 警告）

**风险**：

- Associated type normalization 容易死循环
- Coherence 跨 crate 检查复杂

**验收**：

- 100 个 trait 测试通过
- 能 typeck Iterator 链式调用

---

### 月 6：MIR Building + Borrow Check (NLL)

**目标**：MIR 构建完整，NLL borrow check 工作。

**产出**：

- MIR 数据结构（~2,000 行 Rust）
- MIR building（~4,000 行 Rust）：HIR → MIR
- NLL 算法（~4,000 行 Rust）：region inference + borrow check
- Liveness analysis
- Maybe-init analysis
- 200 个 borrow check 测试

**关键决策**：

- Day 1 上 MIR-based borrow check（R1 强烈推荐）
- NLL 而非词法 lifetime（R3 推荐）
- MVP 支持两阶段借用（Two-phase borrows）的 method-call 子集（v1.2.2 修正：与 04 §2.4 一致）

**风险**：

- NLL 算法实现复杂度可能超预期 1 个月
- Error 诊断质量决定体验

**验收**：

- 200 个 borrow check 测试通过（含 NLL 用例）
- 能 borrow check 完整 fib 程序

---

### 月 7：LLVM Codegen

**目标**：MIR → LLVM IR，能生成可执行文件。

**产出**：

- Codegen（~4,500 行 Rust）
- Type layout 计算
- Drop glue 生成
- Trait object vtable
- Panic runtime
- 150 个 codegen 测试

**关键决策**：

- 仅 LLVM 后端（R2、R3 共识）
- Local → alloca，依赖 LLVM mem2reg
- 不做 LTO（v0.2）

**风险**：

- LLVM C API / inkwell 学习曲线
- 跨平台 ABI 差异

**验收**：

- 150 个 codegen 测试通过
- 能编译并运行 fib、hello world

---

### 月 8：标准库 core + alloc

**目标**：core 与 alloc 标准库可用。

**产出**：

- core 标准库（~4,000 行 Landin）
- alloc 标准库（~3,000 行 Landin）
- 关键 trait：Clone/Copy/PartialEq/Eq/PartialOrd/Ord/Iterator/IntoIterator/From/Into/AsRef/AsMut/Default/Drop
- 关键类型：Option/Result/Vec/String/Box/Rc/Cell/RefCell

**关键决策**：

- 三层分离从 day 1（R1 教训）
- 编译器自身仅依赖 core+alloc

**风险**：

- Vec/String 实现细节多
- trait 之间的相互依赖容易出循环

**验收**：

- 标准库自身可编译
- 50 个标准库单元测试通过

---

### 月 9：mini-cargo + Test Runner

**目标**：包管理器与测试 runner 工作。

**产出**：

- mini-cargo（~2,500 行 Rust，不参与自举）
- Test runner（~1,500 行 Landin）
- landin.toml manifest 解析
- 路径依赖 + git 依赖
- 100 个集成测试

**关键决策**：

- MVP 不支持 workspace、build script、features
- MVP 仅本地 registry + git URL

**风险**：

- 依赖解析（semver）算法实现
- 多 crate 链接顺序

**验收**：

- 能编译多 crate 项目
- 100 个集成测试通过

---

### 月 10：Stage 0 Conformance 完成

**目标**：Stage 0 通过完整 conformance 套件。

**产出**：

- 完整 conformance 套件（v1.2 修正：3,000-5,000 测试，详见 11/17 文档）
- Bug 修复
- 性能优化（编译速度达基准）
- Stage 0 预编译二进制 + Rust 源码发布（v1.2 修正：不再用 LLVM bitcode）

**关键决策**：

- Stage 0 功能冻结（不增特性）
- 仅修 critical bug

**验收**：

- 5,000 个 conformance 测试全通过（v1.2.2 修正：与 11/17 一致）
- 干净环境 bootstrap 测试通过

---

### 月 11：Stage 1 开发（与月 7-10 并行启动）

**目标**：用 Landin 重写编译器，至少完成 lexer + parser + AST。

**产出**：

- Stage 1 lexer（~1,200 行 Landin）
- Stage 1 parser（~3,500 行 Landin）
- Stage 1 AST（~2,500 行 Landin）
- Stage 1 HIR + Lowering（~2,000 行 Landin）

**关键决策**：

- Stage 1 模块结构与 stage 0 对齐
- 每完成一个模块立即用 stage 0 测试

**风险**：

- Stage 0 可能发现 bug 阻塞 stage 1
- Landin 写编译器体验与 Rust 不同，需适应

**验收**：

- Stage 1 lexer 能 parse 自身源码
- Stage 1 parser 通过 200 个 parse 测试

---

### 月 12：Stage 1 完整 + 首次自举

**目标**：Stage 1 完整，首次成功自举。

**产出**：

- Stage 1 完整实现（~37,000 行 Landin）
- Stage 1 → Stage 2 自举成功
- 自举验证通过

**关键决策**：

- Bug 修复优先级：阻塞自举 > conformance 失败 > 体验问题

**风险**：

- Stage 1 可能在某些 case 上与 stage 0 行为不一致
- LLVM codegen 差异导致 bit-stability 难达

**验收**：

- Stage 1 能编译自身 → Stage 2
- Stage 2 与 Stage 1 行为一致
- 干净环境 bootstrap 成功

---

### 月 13-14：稳定性 + 生态基础

**目标**：v0.1 发布前的最后打磨。

**产出**：

- Feature gate 系统上线
- 30-50 个常见错误代码完善
- landin-doc 简化版（v0.2 完整版）
- 文档：入门教程、语言参考、标准库参考
- 5 个示范项目（hello world、JSON parser、CLI 工具等）
- landin-lang.org 官网

**关键决策**：

- v0.1 仅 nightly channel
- v0.2 计划明确

**风险**：

- 文档工作量可能超预期
- 社区早期反馈可能引发设计动摇

**验收**：

- 文档完整可读
- 5 个示范项目可运行

---

### 月 15：v0.1 发布

**目标**：Landin v0.1 正式发布。

**产出**：

- Landin v0.1.0 release
- 二进制 release（Linux x86_64/ARM64、macOS、Windows）
- 源码发布（含 stage0 Rust 源码 + 预编译二进制，v1.2 修正）
- Release notes
- HN/Reddit/技术博客推广

**验收**：

- 5 个早期用户能成功构建 Landin 项目
- GitHub stars > 500
- 至少 3 个第三方 crate 发布

---

## 3. 风险登记

### 3.1 高风险

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| NLL 算法实现超期 | 高 | 高（阻塞 borrow check） | 月 6 留 2 周 buffer；备选简化 NLL（仅词法 + 1 步 forward） |
| Stage 0 工作量超期 | 中 | 高（整体延迟 3-6 月） | 严格 scope 控制；砍 async/closures 之外的所有非必需 |
| LLVM C API 学习曲线 | 中 | 中 | 用 inkwell crate（Rust binding），避免直接 C API |
| Trait resolution 死循环 | 中 | 中 | 强制 depth limit；CI 加超时测试 |
| 自举时 bit-stability 不达 | 中 | 中 | 接受语义等价，不要求 bit-identical（与 rustc 一致） |

### 3.2 中风险

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| 标准库实现 bug 阻塞 stage 1 | 中 | 中 | 标准库测试覆盖率 > 90% |
| Error message 质量差影响体验 | 高 | 中 | 错误信息快照测试；每月专门 1 天优化 |
| Parser 错误恢复质量 | 中 | 中 | 参考 rustc/TypeScript 的恢复策略 |
| 跨平台 ABI 差异 | 低 | 中 | MVP 仅 5 个目标平台，逐一验证 |
| LLVM 版本升级破坏 bitcode | 低 | 低 | bitcode 标注 LLVM 版本；提供升级脚本 |

### 3.3 低风险

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| 用户反馈设计问题 | 高 | 低（v0.2 改） | v0.1 明确"unstable"，feature gate |
| 性能未达基准 | 中 | 低 | LLVM 后端保证性能下限 |
| 文档不完整 | 高 | 低 | 月 13-14 专门写文档 |

---

## 4. v0.2 远景（v0.1 发布后 6-12 月）

### 4.1 语言特性

- `macro_rules!` 声明宏
- `async fn` + `Future` + `async/await`
- `thread` + `Send`/`Sync` + `Mutex`/`RwLock`
- GATs（generic associated types）
- `?Sized` bound
- Two-phase borrows
- Const generics（基础版）
- `impl Trait` in return position
- `let-else`
- `if let` chains

### 4.2 工具链

- `landin-doc` 完整版
- `landin-fmt` 代码格式化
- `landin-lsp` LSP server
- `landinup` 工具链管理器
- `landin-clippy` lints（v0.3）
- Cranelift 后端（dev 加速）
- Incremental compilation
- LTO（thin LTO）

### 4.3 标准库扩展

- `std::async` 完整
- `std::net::Tcp/Udp/Unix`
- `std::process::Command` 完整
- `std::sync::Mutex/RwLock/Condvar/Barrier`
- `std::time::Instant/Duration/SystemTime`
- `std::collections::VecDeque/LinkedList/BTreeSet`
- `std::path` 跨平台完整版
- `std::os::unix/windows` 平台特定 API

### 4.4 生态

- crates.landin-lang.org 中心 registry
- landin-lang.org 官网 + 文档
- 5 个核心 crate（serde-equivalent、tokio-equivalent、reqwest-equivalent、diesel-equivalent、rand-equivalent）
- 100+ 第三方 crate

---

## 5. 长期愿景（v1.0+，3-5 年）

### 5.1 v1.0 目标

- 完整稳定 API
- 5 个目标平台（Linux x86_64/ARM64、macOS Intel/ARM、Windows）
- WASM 后端（wasm32-unknown-unknown + wasm32-wasi）
- 完整 LSP + IDE 支持
- 至少 1000 个第三方 crate
- 至少 3 个生产用户

### 5.2 v2.0 远景

- Effect system（参考 Koka）
- Algebraic effects + handlers
- Linear types 子集（参考 Austral）
- comptime（参考 Zig）
- Plugin system（基于 proc macro 或 LSP）
- Linux kernel module 支持

### 5.3 不做

- Specialization（R3 陷阱）
- Polonius（复杂度未达收益）
- Full dependent types（不可判定）
- GC（违反系统语言定位）
- Macro 2.0（proc macro 简化版替代）

---

## 6. 关键里程碑检查表

按月检查，确保不偏离路线：

- [ ] 月 1：设计文档 v1.0 完成
- [ ] 月 2：Lexer + Parser 通过 200 测试
- [ ] 月 3：HIR + Name resolution 通过 50 测试
- [ ] 月 4：Type check 通过 100 测试
- [ ] 月 5：Trait resolution 通过 100 测试
- [ ] 月 6：MIR + NLL 通过 200 测试
- [ ] 月 7：Codegen 通过 150 测试
- [ ] 月 8：core + alloc 标准库可用
- [ ] 月 9：mini-cargo + test runner 可用
- [ ] 月 10：Stage 0 conformance 全通过（3,000-5,000 测试）+ 预编译二进制生成（v1.2 修正）
- [ ] 月 11：Stage 1 lexer/parser/AST/HIR 完成
- [ ] 月 12：Stage 1 完整 + 首次自举成功
- [ ] 月 13：Feature gate + error messages 完善
- [ ] 月 14：文档 + 示范项目
- [ ] 月 15+：v0.1 发布（仅 stage 0，预期 27-40 月实际工期）；自举完成在 v0.3（43-64 月）

---

## 7. 自我评估指标

### 7.1 设计质量

- [ ] 设计文档无内部矛盾（自审 3 轮 + 1 轮交叉印证）
- [ ] 所有决策有研究依据（R1/R2/R3/R4 报告引用）
- [ ] 不做的功能有明确理由
- [ ] 风险登记含缓解方案

### 7.2 实现质量

- [ ] Stage 0 conformance 全通过
- [ ] Stage 1 能自编译
- [ ] Stage 2 与 Stage 1 行为一致
- [ ] 干净环境 bootstrap 成功

### 7.3 生态基础

- [ ] landin-lang.org 上线
- [ ] 至少 5 个示范项目
- [ ] 至少 1 篇技术博客介绍 Landin
- [ ] 至少 5 个早期用户

---

### 7.4 应急降级方案（v1.2 新增 6 级降级，R8 报告建议）

1. **降级 1**：放弃 NLL，回退 lexical lifetime（触发：月 22 仍未通过 borrow check 测试；节省 3-4 月）
2. **降级 2**：放弃 stage 1 重写，仅发布 stage 0 编译器（参考 Hare；节省 12-18 月）
3. **降级 3**：放弃 frozen blob，改预编译二进制（v1.2 已默认采用）
4. **降级 4**：放弃 trait object，仅静态分发（节省 1-2 月）
5. **降级 5**：放弃 mini-cargo，仅单文件编译（节省 3-4 月）
6. **降级 6**：放弃 Landin 重写，永久保留 Rust stage 0（参考 Roc FAQ）

---

## 8. 致谢与参考文献

本路线图基于以下研究与资料（v1.2 修正）：

### 研究报告

- R1：Rust 2010-2013 自举史研究
- R2：现代 rustc 架构研究
- R3：编译原理理论研究
- R4：8 门可比语言自举案例研究
- R5：PL 理论一致性审查
- R6：rustc 源码深度对照
- R7：经典书籍章节审查
- R8：自举可行性分析
- R9：文档内部一致性审查
- R10/R11/R12/R13：v1.1 收敛审查

### 一手资料

- Rust 官方仓库与 RFC
- rustc-dev-guide
- Zig 官方博客 "Goodbye to the C++ Implementation of Zig"
- Drew DeVault 语言设计方法论文
- Fernando Borretti Austral 设计文章

### 经典论文与书籍（v1.2 修正参考文献清单）

- Wirth《Compiler Construction》(2005) — 第 1 优先
- Cytron et al. 1991 SSA 论文 + Braun et al. 2013 SSA 替代算法
- Pierce《TAPL》第 11/15/22 章
- Jung et al. 2017 "Understanding and Evolving the Rust Programming Language" §2+§5
- Appel《Modern Compiler Implementation in ML》第 10/11/18 章
- Matsakis RFC 2094 NLL 算法
- Maranget 2007 "Compiling pattern matching"（match exhaustiveness）
- matklad 2020 "Simple but Powerful Pratt Parsing"
- Appel 1998 "SSA is Functional Programming"
- Pierce & Turner 2000 "Local Type Inference"
- Cooper & Torczon《Engineering a Compiler》

---

## 9. 启动指令

设计阶段已完成。下一步进入实现阶段：

### 9.1 创建项目骨架

```bash
# Stage 0 Cargo workspace
mkdir landin && cd landin
cargo new --bin stage0
cd stage0
```

**Cargo.toml 模板**（v1.2 补全，R13 启动性建议）：

```toml
[package]
name = "landin-stage0"
version = "0.1.0"
edition = "2021"
license = "MIT"

[[bin]]
name = "landin-stage0"
path = "src/main.rs"

[lib]
name = "landin_compiler"
path = "src/lib.rs"

[dependencies]
# Arena 分配（模拟 rustc 架构）
la_arena = "0.3"
bumpalo = "3.16"

# LLVM binding
inkwell = { version = "0.5", features = ["llvm18-0"] }

# 字符串 interning
lasso = "0.7"

# 文本处理
unicode-xid = "0.2"

# 序列化（错误信息 JSON 输出）
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"

# 命令行
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
expect-test = "1.5"      # 快照测试
proptest = "1.5"         # 属性测试

[profile.release]
opt-level = 2
debug = false
lto = false               # v0.2 启用 thin LTO
codegen-units = 16

[profile.dev]
opt-level = 0
debug = true
```

**推荐项目结构**：

```
stage0/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI 入口
│   ├── lib.rs               # 库入口
│   ├── lexer/
│   │   ├── mod.rs
│   │   ├── tokenizer.rs
│   │   └── reader.rs
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── expr.rs          # Pratt parser
│   │   ├── item.rs          # 声明
│   │   └── error.rs         # 错误恢复
│   ├── ast/
│   │   ├── mod.rs
│   │   └── visit.rs
│   ├── hir/
│   │   ├── mod.rs
│   │   └── lower.rs
│   ├── resolve/
│   │   └── mod.rs
│   ├── typeck/
│   │   ├── mod.rs
│   │   ├── infer.rs
│   │   └── fulfill.rs
│   ├── traits/
│   │   └── mod.rs
│   ├── mir/
│   │   ├── mod.rs
│   │   ├── build.rs
│   │   └── opt.rs
│   ├── borrowck/
│   │   └── mod.rs
│   ├── codegen/
│   │   ├── mod.rs
│   │   ├── operand.rs
│   │   ├── block.rs
│   │   └── intrinsic.rs
│   ├── diagnostics/
│   │   └── mod.rs
│   └── session/
│       └── mod.rs
└── tests/
    ├── lexer.rs
    ├── parser.rs
    └── typeck.rs
```

### 9.2 创建 conformance 仓库

```bash
mkdir -p tests/conformance/{00-parse,01-typecheck,02-borrowck,03-codegen,04-e2e,05-soundness,06-stdlib,07-integration}
touch tests/conformance/run_all.py
```

参考 [17-conformance-suite.md](./17-conformance-suite.md) 编写测试。

### 9.3 创建 RFC 仓库（v0.2 前）

```bash
# GitHub: landin-lang/rfcs
mkdir rfcs && cd rfcs
echo "# Landin RFCs" > README.md
mkdir text 0000-template.md
```

### 9.4 第一个 PR 验收标准（月 2）

```markdown
## PR: Lexer + Parser 基础

### 验收 checklist
- [ ] Cargo.toml 与项目结构按 §9.1
- [ ] Lexer 实现完整 token 种类（02 §1）
- [ ] Lexer 通过 50 个 token 单元测试
- [ ] Parser 实现 Pratt 优先级（02 §2）
- [ ] Parser 实现完整产生式（02 §3）
- [ ] Parser 通过 200 个 parse 测试
- [ ] 错误恢复策略实现（02 §5.2）
- [ ] AST 数据结构定义（05 §1-§11）
- [ ] 100 个故意错误的程序能给出合理错误信息
- [ ] PR 大小 < 5000 行（不含测试）

### 性能基准
- [ ] Hello world parse < 1ms
- [ ] 1000 行 .lin 文件 parse < 50ms
```

### 9.5 月 2 测试分布

200 parse 测试建议分布：

| 子类 | 数量 | 覆盖 |
| --- | --- | --- |
| 字面量 | 30 | int/float/char/str/byte/raw |
| 运算符 | 25 | 算术/位/比较/逻辑/赋值 |
| 控制流 | 30 | if/while/loop/for/match/if-let/while-let |
| 模式 | 25 | 字面量/结构/enum/元组/数组/范围/或 |
| 类型 | 20 | 基本类型/引用/指针/数组/slice/fn/trait |
| 泛型 | 15 | 类型参数/lifetime/where/bound |
| 表达式 | 20 | 闭包/range/cast/?/method call |
| 声明 | 15 | fn/struct/enum/trait/impl/use/mod |
| 错误恢复 | 20 | 未闭合/缺失 token/错误属性 |
| **合计** | **200** | |

进入实现阶段后，本蓝图作为"参考标准"，所有偏离需通过 RFC 流程。

---

**Landin 设计蓝图 v1.3.2 — 完**

下一步：思考-设计阶段完成，进入实现-测试-报告-修正循环的第一轮。
