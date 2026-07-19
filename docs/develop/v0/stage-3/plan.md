# Stage 3 Plan — LLVM Codegen

> **Date**: 2026-07-19
> **Stage**: 3 (LLVM codegen)
> **Complexity**: L3 (core architecture, cross-module)
> **Baseline rounds**: 8-15 (per §3.1)
> **Process**: v3.4 (with §9.1.1 negative-test matrix + §9.3.1 expanded audit + §9.3.3 convergence rule)

---

## 1. 目标

将 Stage 2.x 产出的 MIR (已通过 typeck + borrowck) 编译为 LLVM IR，
最终生成可执行文件。

**MVP 目标**: `fn main() { 42 }` → 可运行的二进制

---

## 2. 子阶段拆分 (MUV)

### Stage 3.1 — 基础 codegen (返回字面量)
- 目标: `fn main() -> i32 { 42 }` → LLVM IR → 可执行
- 涉及: 函数 prologue/epilogue, return, i32 常量
- 验收: `lli` 能执行生成的 .ll, 返回 42

### Stage 3.2 — 算术运算
- 目标: `fn main() -> i32 { 1 + 2 * 3 }` → 正确的 LLVM IR
- 涉及: BinaryOp (Add/Sub/Mul/Div), Operand::Copy/Constant
- 验收: 计算结果正确

### Stage 3.3 — 变量与赋值
- 目标: `fn f(x: i32) -> i32 { let y = x + 1; y }` → 正确
- 涉及: LocalDecl → alloca, Store/Load, StorageLive/Dead
- 验收: 变量读写正确

### Stage 3.4 — 控制流
- 目标: if/while/match → LLVM basic blocks + branch
- 涉及: SwitchInt, Goto, BasicBlockId → LLVM label
- 验收: 控制流正确

### Stage 3.5 — 函数调用
- 目标: 递归 fibonacci → 正确的 call 指令
- 涉及: FnDef → LLVM function, Call terminator
- 验收: 递归调用正确

### Stage 3.6 — 借用与引用
- 目标: `let x = 1; let r = &x; *r` → 正确的 getelementptr/load
- 涉及: Rvalue::Ref, Projection::Deref
- 验收: 引用解引用正确

### Stage 3.7 — Assert + overflow check
- 目标: 算术溢出检查 → LLVM intrinsic + panic
- 涉及: Assert terminator → llvm.sadd.with.overflow
- 验收: 溢出检测正确

### Stage 3.8 — Stage 3 门审查
- 全量集成测试
- §9.3 阶段门审查

---

## 3. 复杂度预评估

| 指标 | 评估 |
|------|------|
| 代码变动量 | 大 (新增 codegen 模块, ~2000-3000 LOC) |
| 依赖风险 | 高 (LLVM C API 绑定, inkwell/llvm-sys) |
| 历史缺陷密度 | N/A (新阶段) |
| **等级** | **L3** |
| **基准轮次** | 8-15 轮 |

---

## 4. 集成测试计划

### §9.1.1 负向测试覆盖
| 类别 | 示例 | Stage 3 检测点 |
|------|------|---------------|
| Type mismatch | 已在 Stage 2 覆盖 | N/A (codegen 不做 typeck) |
| Codegen 错误 | 无法 codegen 的 MIR | codegen panic/error |
| 输出验证 | 错误的 LLVM IR | `lli` 执行失败 |
| 功能正确性 | 计算结果错误 | 断言执行结果 |

### §9.3.1 审计集
- ≥30 case, 覆盖 Stage 3.1-3.7 所有子阶段
- 包含正向 (编译+执行+验证结果) 和负向 (codegen 错误)

---

## 5. 技术选型

### LLVM 绑定
- **选项 A**: `inkwell` (Rust LLVM bindings, 安全包装)
- **选项 B**: `llvm-sys` (直接 C API 绑定)
- **选项 C**: 直接生成文本 `.ll` 文件 (无 LLVM 依赖)

**初始选择**: **选项 C** (直接生成 .ll 文本)
- 理由: 无外部依赖, 可立即开始, 用 `lli` 验证
- 后续: 如果需要 JIT 或优化, 切换到 inkwell

### 验证工具
- `lli` (LLVM interpreter) — 执行 .ll 文件
- `llc` (LLVM compiler) — .ll → .o → 可执行
- `opt` (LLVM optimizer) — 验证 IR 正确性
