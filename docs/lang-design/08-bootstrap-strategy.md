# 08 — 自举策略

> 本文定义 Landin 的 **渐进式自举策略**（v1.2 修正：v0.1 不自举，v0.3 自举完成）。设计基于 R1（Rust 自举史）+ R4（可比语言案例）+ R8（可行性分析）的研究结论，避免 Hare 那种"5 年仍维护 C bootstrap"的反模式。

---

## 1. 自举目标定义

### 1.1 什么是"自举完成"

Landin 的自举完成定义为：

1. **可自编译**：用 stage-1 编译器能成功编译 Landin 编译器自身的 stage-2 源码
2. **可重生成**：stage-1 编译的 stage-2 二进制与 stage-2 自编译产生的二进制行为完全一致
3. **可分发**：stage-2 二进制可作为生产编译器分发
4. **可 bootstrap from source**：从 stage-0 + Landin 源码，可在干净环境重建完整工具链

### 1.2 不要求的事

- 不要求 stage-2 与 stage-1 二进制 bit-identical
- 不要求编译速度与 stage-0 相当
- 不要求实现 stage-0 的所有特性

### 1.3 v1.2 重大修正：分阶段交付

**v1.0 错误**：声称 15 月完成自举。

**v1.2 修正**（基于 R8 报告）：

- **v0.1**（20-40 月）：交付 stage 0 编译器（Rust 实现），可编译第三方 Landin 程序，**不要求自举**
- **v0.2**（24 月）：标准库扩展 + 工具链完善
- **v0.3**（31-64 月）：完成自举（stage 1 用 Landin 重写 + stage 2 验证）

**理由**：

- R8 工作量重估：stage 0 实际需 130-180k 行 Rust，v1.0 估算的 53k 行低估 2.5-3.5x
- R8 时间线重估：1 人全职实际需 30-54 月
- R4 印证：Hare 5 年仍未完成编译器自举

---

## 2. 三阶段 Bootstrap 流程

### 2.1 阶段概览

```
┌────────────────────────────────────────────────────────────────┐
│  Stage 0: Rust 实现的 Landin 编译器                            │
│  - 完整功能（覆盖所有 v0.1 特性）                              │
│  - 写完即冻结为 frozen blob                                    │
│  - 永久维护：仅修 critical bug，不增功能                       │
└────────────────────────────────────────────────────────────────┘
                              ↓ 编译
┌────────────────────────────────────────────────────────────────┐
│  Stage 1: 用 Stage 0 编译 Landin 写的 Landin 编译器            │
│  - 功能可与 Stage 0 等价                                       │
│  - 这是"第一个自举 Landin 编译器"                              │
│  - 输出：可执行二进制                                          │
└────────────────────────────────────────────────────────────────┘
                              ↓ 自编译
┌────────────────────────────────────────────────────────────────┐
│  Stage 2: 用 Stage 1 编译同一份 Landin 编译器源码              │
│  - 验证：Stage 2 行为应与 Stage 1 一致                         │
│  - 自举完成                                                    │
└────────────────────────────────────────────────────────────────┘
                              ↓ 滚动
┌────────────────────────────────────────────────────────────────┐
│  Stage N: 用 Stage (N-1) 编译当前源码 → Stage N                │
│  - 正常迭代流程                                                │
│  - Stage 0 永远不动                                            │
└────────────────────────────────────────────────────────────────┘
```

### 2.2 Stage 0 冻结机制（v1.2 修正）

**v1.0 错误**：选 LLVM bitcode 作 stage 0 frozen blob。

**v1.2 修正**（基于 R8 报告）：改用 **预编译二进制 + Rust 源码双备份** 策略（参考 Rust 模式）：

- 每个目标平台提供预编译 `landin-stage0` 二进制（约 15-30 MB）
- 同时提交 stage 0 Rust 源码（用户可用系统 Rust 工具链重建）
- SHA256 校验保证完整性

**理由**（R8 报告）：

- LLVM bitcode **backward compatible but NOT forward compatible**，major 版本升级允许破坏
- llvm-sys.rs 强制 LLVM 版本严格匹配，跨版本不可用
- 5 年内必有 3-4 次破坏性 LLVM 升级
- Rust 自身用预编译二进制而非 bitcode

### 2.3 干净环境 bootstrap（v1.2 修正）

用户在干净环境（仅有 `rustc` + `cargo` + LLVM 工具链）下构建 Landin：

```bash
# 方法 A：使用预编译二进制
wget https://landin-lang.org/dist/stage0-x86_64-linux.tar.gz
tar xzf stage0-x86_64-linux.tar.gz
./landin-stage0 compile landin-compiler/ -o landin-stage1
./landin-stage1 compile landin-compiler/ -o landin-stage2

# 方法 B：从源码重建 stage 0
git clone https://github.com/landin-lang/landin-stage0
cd landin-stage0
cargo build --release
target/release/landin-stage0 compile ../landin-compiler/ -o landin-stage1
```

方法 B 要求用户有 Rust 工具链，方法 A 不要求。两种方法都能达到"从源码重建"的目标。

### 2.4 Stage 0 永久维护原则

R4 报告指出 Hare 的教训：永久维护 C bootstrap 5 年仍卡住。Landin 的策略：

- Stage 0 写完即冻结，**只修 critical bug**（不阻塞 stage-1 的 bug 才修）
- **不增功能**：stage 0 不支持 v0.2+ 特性
- **明确弃用时间表**：v0.5（约 24 个月后）评估是否替换为更小的"stage -1"（如微型解释器）

这与 Hare 的根本差异：Hare 让 stage 0 持续追上语言演化，最终变成"两份编译器并行维护"。Landin 把 stage 0 视为"不可变历史档案"，所有演化在 stage-1+ 进行。

### 2.5 Stage 0 与 Stage 1 的功能对齐

Stage 1 必须能编译自身，因此 stage 0 必须支持 stage 1 源码用到的**所有语言特性**。

策略：

- Stage 0 在 v0.1 设计阶段就规划好"stage 1 源码需要的特性集"
- Stage 1 源码严格使用该子集
- Stage 0 不实现"v0.1 中 stage 1 不需要"的特性

这要求 stage 0 的功能集合 = stage 1 源码所需特性集，**不能多也不能少**。

---

## 3. Stage 0 实现计划

### 3.1 宿主语言：Rust

经 R3、R4 综合分析，选用 **Rust** 作为 stage 0 宿主语言，理由：

| 优势 | 劣势（及缓解） |
| --- | --- |
| inkwell/melior 提供成熟 LLVM binding | borrow checker 在 arena-based 代码中价值低（用 `la_arena` + `bumpalo` 缓解，正是 rustc 自身做法） |
| logos/chumsky/rowan 等 crate 加速开发 | Rust 编译速度慢（一次性投入，可接受） |
| 我作为 AI 在 Rust 上的产出效率高 | Roc 案例（R4）证明 Rust 不一定适合写编译器，但 Roc 是函数式语言场景，与 Landin 不同 |
| 与 Landin 类型系统相似，便于交叉验证 | |

考虑过的备选：

- **OCaml**（R1、R4 推荐）：sum type + pattern matching 舒适，但 LLVM binding 不成熟
- **Zig**（R4 中 Roc 迁移目标）：comptime + 显式 allocator 适合编译器，但 Zig 生态小
- **C++**（Odin 选择）：与 LLVM 原生集成，但开发体验差

**最终决策：Rust + `bumpalo` + `la_arena`**，模拟 rustc 自身的 arena-based 编译器架构。

### 3.2 Stage 0 实现里程碑

按 R8 报告"v0.1/v0.3 分期"路线（v1.2 修正，参考 08-bootstrap-strategy）：

| 月 | 里程碑 | 产物 |
| --- | --- | --- |
| 1 | 语言设计冻结（本文档集 v1.0） | 设计文档 |
| 2 | Lexer + Parser + AST | 能 parse 全部 stage1 源码 |
| 3 | HIR + Name resolution + Type check | 通过 stage1 的 typeck |
| 4 | MIR building + Borrow check (NLL) | 通过 stage1 的 borrowck |
| 5 | LLVM codegen + 链接 | 能编译并运行 stage1 hello world |
| 6 | core/alloc 标准库 MVP | Vec/String/Box/Result/Option 可用 |
| 7 | trait resolution + monomorphization 完整 | trait 与泛型全场景通过 |
| 8-9 | mini-cargo + 测试 runner | 能编译多文件项目 |
| 10 | 用 stage 0 编译 stage 1（首次成功） | 自举完成第一次 |
| 11 | bug 修复 + conformance 测试套件 | stage1 通过所有 stage1 自测 |
| 12 | 自举验证：stage1 → stage2 → 一致 | 自举正式完成 |

### 3.3 Stage 0 代码规模估算（v1.2 修正）

**v1.0 估算**：53,000 行 Rust（R8 报告证明低估 2.5-3.5x）

**v1.2 修正**：130,000-180,000 行 Rust

| 组件 | v1.0 估算 | v1.2 估算 | 依据 |
| --- | --- | --- | --- |
| Lexer | 1,500 | 3,000-4,000 | rustc lexer 3,500 行 |
| Parser | 4,000 | 12,000-18,000 | rustc parser 25,000 行 |
| AST + HIR + Lowering | 5,000 | 15,000-25,000 | rustc hir+ast 30,000 行 |
| Name resolution | 2,500 | 8,000-12,000 | rustc resolve 15,000 行 |
| Type checker | 6,000 | 20,000-35,000 | rustc typeck 50,000 行 |
| Trait resolution | 3,000 | 15,000-25,000 | rustc traits 40,000 行 |
| MIR building | 4,000 | 10,000-15,000 | rustc mir_build 12,000 行 |
| Borrow checker (NLL) | 4,000 | 12,000-18,000 | rustc borrowck+region 28,000 行 |
| MIR optimization | 2,000 | 6,000-10,000 | rustc mir_opts 10,000 行 |
| LLVM codegen | 4,500 | 20,000-30,000 | rustc codegen-llvm+ssa 45,000 行 |
| Monomorphization | 2,000 | 4,000-6,000 | rustc monomorphize 5,000 行 |
| Errors + diagnostics | 2,500 | 8,000-15,000 | rustc diagnostics 20,000+ 行 |
| 标准库 core+alloc | 8,000 (Landin) | 25,000-40,000 (Landin) | Rust core+alloc 子集 30,000 行 |
| mini-cargo | 2,500 (Rust) | 6,000-9,000 (Rust) | cargo 核心 30,000 行 |
| Test runner | 1,500 (Landin) | 4,000-6,000 (Landin) | libtest 10,000 行 |
| 内建宏展开器 | - | 3,000-5,000 | v1.2 新增 |
| **合计** | **~53,000** | **~130,000-180,000**（v1.2.3 修正：与 §1.3 一致，v1.2 表格曾误标 170-260k） | **2.5-3.5x** |

### 3.4 Stage 0 不实现的功能

明确不实现（推到 stage 1+ 或永久不做）：

- Lints（warning 系统）：MVP 仅 error
- Incremental compilation：每次全量
- Proc macro：永久不做（v0.2 仅 macro_rules!）
- Coverage instrumentation
- Sanitizers
- Profile-guided optimization
- Cranelift 后端
- LTO
- Cross-language LTO
- Debug info > -g1
- 多线程并行 typeck
- IDE/LSP（v0.2 单独项目）

---

## 4. Stage 1 实现计划

### 4.1 Stage 1 与 Stage 0 的关系

Stage 1 是 **stage 0 的 Landin 重写**，功能等价但代码用 Landin 写。设计原则：

- **结构对齐**：stage 1 源码模块结构与 stage 0 对齐，便于交叉验证
- **特性对齐**：stage 1 用到的所有语言特性 stage 0 必须支持
- **测试对齐**：stage 1 必须通过 stage 0 的全部测试套件

### 4.2 Stage 1 写作策略

按 stage 0 的模块顺序逐个重写：

1. Lexer（最先，依赖最少）
2. Parser
3. AST
4. HIR + Lowering
5. Name resolution
6. Type checker
7. Trait resolution
8. MIR building
9. Borrow checker
10. MIR optimization
11. LLVM codegen
12. Monomorphization
13. mini-cargo（v0.1 用 Rust 写，不参与自举；v0.2 用 Landin 重写）

每个模块完成后，立即跑 stage 0 的对应测试。

### 4.3 Stage 1 与 Stage 0 的差异

允许的差异：

- **更高效的数据结构**：Landin 有 `Box<T>`、`Vec<T>` 等，可比 stage 0 Rust 更直接表达
- **更简洁的错误处理**：Landin 的 `?` 操作符
- **更现代的 trait 用法**：associated type、where clause 等

不允许的差异：

- **算法改变**：必须用相同的 NLL 算法、相同的 trait resolution 算法
- **特性集改变**：stage 1 不能引入 stage 0 不支持的特性
- **ABI 改变**：必须保持 stage 0 的 ABI 兼容性

### 4.4 Stage 1 代码规模估算

| 组件 | 行数（Landin） | 对应 stage 0 行数（Rust） |
| --- | --- | --- |
| Lexer | 1,200 | 1,500 |
| Parser | 3,500 | 4,000 |
| AST + HIR | 4,500 | 5,000 |
| Name resolution | 2,200 | 2,500 |
| Type checker | 5,500 | 6,000 |
| Trait resolution | 2,800 | 3,000 |
| MIR building | 3,800 | 4,000 |
| Borrow checker | 3,700 | 4,000 |
| MIR optimization | 1,800 | 2,000 |
| LLVM codegen | 4,200 | 4,500 |
| Monomorphization | 1,800 | 2,000 |
| Errors + diagnostics | 2,200 | 2,500 |
| **合计** | **~37,000 行 Landin** | **~41,000 行 Rust** |

Landin 代码预期比 Rust 略短（5-10%），因 Landin 类型系统更现代。

---

## 5. 自举验证

### 5.1 Conformance 测试

自举完成前，stage 0 必须通过 **完整 conformance 套件**：

| 测试类别 | 数量 | 覆盖 |
| --- | --- | --- |
| Parse 测试 | ~200 | 所有语法产生式 |
| Type check 测试 | ~300 | 类型推导、coherence、trait resolution |
| Borrow check 测试 | ~200 | NLL、move、init |
| Codegen 测试 | ~150 | LLVM IR 输出 |
| End-to-end 测试 | ~100 | 编译运行 + 输出校验 |
| **合计** | **~5,000 个测试**（v1.2.3 修正：与 11/17 一致） | |

### 5.2 Stage 1 自测

stage 1 完成后，必须能编译自身：

```bash
./landin-stage0 compile landin-compiler/ -o landin-stage1
./landin-stage1 compile landin-compiler/ -o landin-stage2

# 验证：stage2 与 stage1 行为一致
diff <(./landin-stage1 compile test_suite/ -o /tmp/s1) \
     <(./landin-stage2 compile test_suite/ -o /tmp/s2)
# 应无差异（或仅时间戳）
```

### 5.3 Bit-stability 测试

```bash
# 同一源码用 stage1 编译两次
./landin-stage1 compile landin-compiler/ -o landin-stage2a
./landin-stage1 compile landin-compiler/ -o landin-stage2b

sha256sum landin-stage2a landin-stage2b
# 应一致（除非 LLVM 版本变化）
```

### 5.4 干净环境 bootstrap 测试

在 Docker 容器（含 rustc + cargo + LLVM 工具链）中（v1.2 修正：不再用 LLVM bitcode，改用源码重建）：

```dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y rustc cargo llvm lld python3
COPY landin-stage0/ landin-compiler/ /work/
WORKDIR /work
RUN cd landin-stage0 && cargo build --release && \
    ./target/release/landin-stage0 compile ../landin-compiler/ -o landin-stage1 && \
    ./landin-stage1 compile ../landin-compiler/ -o landin-stage2 && \
    ./landin-stage2 test tests/conformance/
```

成功 = 自举完成。

---

## 6. 版本发布

### 6.1 版本号策略

参考 Semantic Versioning：

- **v0.x.y**: MVP 阶段，可能破坏性变更
- **v1.0.0**: 第一个稳定版本（自举完成 + 通过 conformance）
- **v1.x.y**: 向后兼容的特性添加
- **v2.0.0**: 破坏性变更（v0.2 特性落地）

### 6.2 Release channel

参考 Rust release channel（R1 报告）：

- **nightly**: 每日构建，含 unstable 特性
- **beta**: 6 周一次，从 nightly 拣选
- **stable**: 6 周一次，从 beta 拣选

MVP 阶段仅 nightly，v0.5 后启用 beta/stable。

### 6.3 Feature gate

每个 unstable 特性必须 `#[feature(...)]` 才能在 nightly 用：

```landin
#![feature(generic_associated_types)]

fn foo<T: for<'a> Trait<'a>>() { ... }
```

stable channel 拒绝编译含 `#![feature(...)]` 的代码（R1 教训：feature gate 必须从 v0.1 上）。

---

## 7. 风险与缓解

### 7.1 自举风险登记

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| Stage 0 实现工作量超预期 | 中 | 高（延迟 3-6 月） | 严格 scope 控制，砍非核心功能 |
| Stage 1 写到一半发现 stage 0 缺特性 | 中 | 高（返工） | Stage 0 与 stage 1 同步开发，每月联调 |
| LLVM 版本升级破坏 inkwell 兼容 | 中 | 中 | 锁定 inkwell 对应 LLVM 版本，提供升级脚本 |
| Borrow checker 边界 case 错误 | 高 | 低（debug 模式 abort） | Conformance 套件 + fuzzing |
| Trait resolution 死循环 | 中 | 中 | 强制 depth limit = 128 |
| Stage 0 预编译二进制太大 | 低 | 低 | 提供 Linux/macOS/Windows 三平台二进制，每平台 15-30 MB |

### 7.2 应急方案

若 v0.1 时间线（27-40 月）无法达成：

- **方案 A**：放宽 v0.1 特性集，砍 async / closures / `?` 之外的所有"非必需"
- **方案 B**：推迟 stage 1 重写到 v0.2，先发布 stage 0 作为生产编译器（参考 Hare 模式，但明确 v0.2 完成自举）
- **方案 C**：缩减 conformance 套件，仅核心特性自举

---

## 8. 时间线总结（v1.2 修正）

| 阶段 | v1.0 估算 | v1.2 乐观 | v1.2 现实 |
| --- | --- | --- | --- |
| Stage 0 开发 | 9 月 | 18-24 月 | 24-36 月 |
| Stage 0 conformance 通过 | 1 月 | 2-3 月 | 3-4 月 |
| Stage 1 重写 | 2-3 月 | 8-12 月 | 12-18 月 |
| 自举验证 + 发布 | 2-3 月 | 3-4 月 | 4-6 月 |
| **v0.1（仅 stage 0）** | - | **20-27 月** | **27-40 月** |
| **v0.3（自举完成）** | **15 月** | **31-43 月** | **43-64 月** |

**对照业界**：

- Rust：2010 → 2013 自宿主跑通 → 2015 1.0 = 36-60 月，团队 5-10 人
- Zig：2015 → 2022 自宿主默认 = 84 月，Andrew Kelley 全职 + 社区
- Landin 现实 43-64 月：1 人全职，与 Zig 自举周期相当

R8 报告核心结论：v1.0 声称的 15 月自举在 1 人全职条件下不可行。

---

**下一文档**: [`09-stdlib.md`](./09-stdlib.md) — 标准库设计
