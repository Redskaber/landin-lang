# Stage 18.45 — 自举 (Self-Hosting) 可行性评估

> **Author**: redskaber + ARCH-A + REV-A + DEV-A + QA-A + PM-A
> **Date**: 2026-08-07
> **Version**: v0.316.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §14.5 (deep review)
> **Status**: Design Assessment — 待委员会审议

## 1. 背景

用户提出："最终目的是编译器自举，请确认设计情况和选择（不要盲目编码）"

自举 (Self-hosting) 意味着 Landin 编译器的源代码用 Landin 语言编写，
用 Landin 编译器自身编译。这是编程语言成熟度的标志性里程碑。

## 2. 当前状态评估

### 2.1 编译器规模

| 维度 | 数值 |
|------|------|
| Rust 源码行数 | ~45,569 LOC |
| 测试数 | 3,144 (607 lib + 2,537 integration) |
| 内置宏数 | 28 |
| 支持的 Item 类型 | 11 (Fn/Const/Static/Struct/Enum/Trait/Impl/TypeAlias/ExternBlock/Mod/Use/MacroRules) |
| MIR 优化 | DCE + ConstProp |
| Codegen | LLVM 19 (text IR + LLVM API) |

### 2.2 语言特性清单

| 特性 | 状态 | 自举需求 |
|------|------|---------|
| fn / struct / enum | ✅ 完整 | **必需** |
| trait / impl | ✅ 完整 | **必需** |
| 泛型 (generics) | ✅ 完整 | **必需** |
| 模式匹配 (match) | ✅ 完整 | **必需** |
| 闭包 (closures) | ✅ 完整 | **必需** |
| 引用 (&T / &mut T) | ✅ 完整 | **必需** |
| macro_rules! | ✅ 28 内置宏 | **必需** |
| where 子句 | ✅ 完整 | **必需** |
| 生命周期 | ✅ 基本支持 | 有用 |
| dyn Trait | ✅ 完整 | 有用 |
| 数组 [T; N] | ✅ 基本支持 | **必需** |
| 元组 (A, B, C) | ✅ 完整 | **必需** |
| extern "C" | ✅ 完整 | **必需** (LLVM 交互) |
| 模块系统 (mod/use) | ⚠️ 基本支持 | **必需** (多文件编译) |
| 字符串方法 | ❌ 无实现 | **必需** (编译器大量使用) |
| Vec/HashMap 实现 | ❌ 仅有类型名 | **必需** (编译器数据结构) |
| 文件 I/O | ❌ 仅有类型名 | **必需** (读源码/写输出) |
| Iterator trait | ❌ 无实现 | **必需** (编译器遍历) |
| Result/Option | ❌ 仅有类型名 | **必需** (错误处理) |
| ? 运算符 | ❌ 无 | 有用 |
| 跨文件编译 | ❌ 仅单文件 | **必需** |
| 标准库 (实际实现) | ❌ 仅注册类型名 | **必需** |

### 2.3 关键差距分析

自举需要 Landin 能表达一个完整的编译器。当前最大的差距是：

**P0 (阻断性)**:
1. **标准库实现**: Vec/String/HashMap/Result/Option 目前只是 interner 中的
   类型名，没有实际实现。编译器需要这些类型来做几乎所有操作。
2. **文件 I/O**: 编译器需要读取源文件、写入 LLVM IR / 目标文件。
   目前 Landin 无法进行任何文件操作。
3. **跨文件编译**: 目前编译器只支持单文件编译。自举需要多文件模块系统。
4. **字符串操作**: 编译器大量使用字符串拼接、分割、格式化。
   目前 Landin 没有任何字符串方法。

**P1 (重要)**:
5. **Iterator trait**: 编译器需要遍历各种数据结构。
6. **错误处理**: Result/Option + ? 运算符，用于编译器错误传播。
7. **内存管理**: 了解 alloc/dealloc 是否可用（目前 extern "C" 可以
   调用 malloc/free，但需要封装）。

**P2 (有益)**:
8. **GATs**: 有助于表达关联类型，但非必需。
9. **增量编译**: 有助于开发效率，但非自举必需。

## 3. 自举路径选择

### 路径 A: 完整自举 (理想但遥远)
- 将整个编译器从 Rust 移植到 Landin
- 需要: 完整标准库 + 文件 I/O + 跨文件编译
- 估计: 50-100 stages
- 风险: 极高，可能遇到 Landin 语言本身的限制

### 路径 B: 渐进式自举 (推荐)
- 分阶段将编译器组件用 Landin 重写
- Phase 1: Lexer (词法分析器) — 最简单，纯字符处理
- Phase 2: Parser (语法分析器) — 中等复杂度
- Phase 3: HIR/MIR — 需要复杂数据结构
- Phase 4: Codegen — 需要文件 I/O + LLVM FFI
- 每个 Phase 都需要先在 Landin 中实现必要的 stdlib
- 估计: 30-50 stages (含 stdlib 实现)

### 路径 C: 最小自举 (快速验证)
- 只将 Lexer + Parser 用 Landin 重写
- 验证 Landin 能否表达编译器前端
- 估计: 15-20 stages
- 好处: 快速验证语言能力，发现限制

### 路径 D: 不自举，专注语言和工具链
- 保持 Rust 实现
- 专注完善 Landin 语言特性和标准库
- 让 Landin 成为一个实用的应用语言
- 估计: 取决于应用场景

## 4. 委员会建议

**建议选择路径 B (渐进式自举)**，理由：

1. **正确 > 妥协**: 渐进式方法允许在每个阶段验证正确性
2. **通解 > 特解**: 渐进式自举是自举问题的通解（而非一次性移植的特解）
3. **风险可控**: 每个 Phase 独立验证，失败可以回退
4. **发现语言限制**: 在移植过程中发现 Landin 语言的不足，及时修补

**前置条件 (Phase 0: 标准库基础)**:
- 实现 Vec<T> (动态数组)
- 实现 String (可增长字符串)
- 实现 HashMap<K, V> (哈希表)
- 实现 Result<T, E> / Option<T>
- 实现基本 Iterator trait
- 实现文件 I/O (File read/write)

**估计**: Phase 0 需要 10-15 stages

## 5. 下一步建议

1. **Stage 18.46**: 创建自举路线图设计文档（路径 B 详细规划）
2. **Stage 18.47**: 委员会审议自举计划
3. **Stage 18.48+**: Phase 0 — 标准库基础实现

## 6. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| Landin 语言能力不足 | 自举无法进行 | 在 Phase 0 中发现并修补 |
| 标准库实现工作量巨大 | 延迟自举 | 渐进式实现，按需添加 |
| LLVM FFI 复杂 | Codegen 难以移植 | 可以保留 Rust codegen，只自举前端 |
| 性能问题 | 自举后编译慢 | 后续优化，先求正确 |

## 7. 结论

自举是正确的长期目标，但当前 Landin 缺乏自举所需的基础设施
（标准库实现、文件 I/O、跨文件编译）。建议采用**渐进式自举**路径，
从 Phase 0（标准库基础）开始。不应继续盲目添加更多宏——
28 个内置宏已足够，现在需要的是**让 Landin 能做实际工作**的能力。
