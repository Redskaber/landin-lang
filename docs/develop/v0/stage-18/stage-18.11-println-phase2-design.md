# Stage 18.11 — println! 通解化 Phase 2 Design + v0.6 P1.5 Review

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.295.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿) + §14.5 (深度审查)
> **Status**: ✅ Complete

## 1. 阶段目标

1. 对 Stage 18.10 (Phase 1) 进行 §14.5 深度审查
2. 设计 Phase 2-4 的详细实现方案
3. 评估是否需要调整 v0.6 路线图

## 2. §14.5 Stage 18.10 深度审查 (D1-D8)

### D1 — 架构健康度 ✅

- built-in macro 注册逻辑封装在 `macro_expand.rs`
- driver 通过 `BUILTIN_MACRO_NAMES` const + `expand_macros_with_errors` 入口交互
- 无跨阶段耦合

### D2 — API 命名 ✅

- `BUILTIN_MACRO_NAMES` (UPPER_SNAKE_CASE const)
- `build_builtin_macro_table` (`<verb>_<noun>_<noun>`)
- `make_builtin_macro_rule` (`<verb>_<noun>_<noun>`)

### D3 — 接口隔离 ✅

- built-in macro 逻辑全部在 `macro_expand.rs`
- driver 只 pre-intern 符号 + 调用 `expand_macros_with_errors`

### D4 — 测试覆盖 ✅

- 8 个新测试，2:6 = 1:3 比例合规

### D5 — 死代码 ✅

- 所有新函数都被 `expand_macros_with_errors` 或测试调用
- `let _ = name;` 是 doc-purpose 参数标记（可接受）

### D6 — 性能 ✅

- built-in macro table 在每次 `expand_macros_with_errors` 调用时重建
- 4 个宏 × 1 rule × ~10 tokens = 微小开销
- 可接受（driver 每次编译只调用一次）

### D7 — 错误处理 ✅

- built-in macro no-op 展开 不会产生错误
- user macro 覆盖 built-in 时，错误由 user macro 的展开路径处理

### D8 — 文档同步 ✅

- 设计文档: stage-18.10-println-tongjie-phase1-design.md
- RELEASE_NOTES + worklog 完整

**委员会投票**: 5/5 GO

## 3. Phase 2 详细设计

### 3.1 目标

将 `println!("x={}", x)` 从 no-op pass-through 改为真正展开为：
```landin
__landin_println("x={}\n", x)
```

其中 `__landin_println` 是一个 extern 函数（封装 C `printf`）。

### 3.2 实现挑战

1. **Format string 转换**: `println!("x={}")` 的格式串需要追加 `\n`。
   - `print!` 不追加 `\n`
   - `eprintln!` / `eprint!` 使用 stderr (需 `__landin_eprint`)

2. **Token 层展开**: macro body 是 token Vec，不能直接构造 AST `Call` 节点。
   - 需要生成 tokens: `__landin_println ( "x={}\n" , x )`
   - parser 解析这些 tokens 为 `Expr::Call`

3. **Span 处理**: 展开的 tokens 需要合理的 span（用 call site span）

4. **Extern 函数声明**: `__landin_println` 需要在 codegen 时被识别为 extern
   - 选项 A: 在 driver 中注入隐式 `extern` item 到 AST
   - 选项 B: 在 codegen 中特解 `__landin_println` 符号
   - 选项 C: 在 stdlib 中声明

### 3.3 推荐方案: 选项 B (codegen 特解)

理由：
- 选项 A 需要修改 AST 结构（注入隐式 item），影响大
- 选项 C 需要 stdlib 支持，但目前 stdlib 还不完善
- 选项 B 是最小改动：codegen 已经有 `Println` 特解，只需把
  `Call(__landin_println, ...)` 当作等价处理

### 3.4 实现步骤 (Phase 2)

1. **修改 built-in macro rule body**:
   - `println` body: `__landin_println ! ( ... )` → 不对，应该是 `__landin_println ( ... )`
   - 实际上，body 应该是 token 序列 `__landin_println ( "fmt\n" , args... )`
   - 但 macro body 是静态 tokens，不能在展开时动态构造 format string
   - **解决方案**: body 仍是 `__landin_println ( $ ( $args ) * )`，
     让 parser 解析为 `Call`，然后在 codegen 中特解 `__landin_println`
     调用（追加 `\n`、使用 stderr 等）

2. **codegen 识别 `__landin_println`**:
   - 在 `codegen_call` 中检查 callee name
   - 如果是 `__landin_println`，追加 `\n` 到 format string
   - 如果是 `__landin_eprintln`，使用 stderr + 追加 `\n`
   - 如果是 `__landin_print` / `__landin_eprint`，不追加 `\n`

3. **移除 parser 特解**:
   - 删除 `expr.rs` 中 `println`/`print`/`eprintln`/`eprint` 的特解分支
   - 这些调用现在走 `MacroCall` → `expand_macros` → `Call` 路径

4. **HIR/MIR/Codegen Println variant 保留**:
   - Phase 2 暂不删除，作为 fallback
   - Phase 3 再删除

### 3.5 风险评估

- **风险 1**: `__landin_println` 调用的 args 类型可能与 `printf` 期望不匹配
  - **缓解**: codegen 特解时复用现有 `Println` 的 codegen 逻辑
- **风险 2**: 用户代码中已有 `__landin_println` 函数定义
  - **缓解**: `__landin_` 前缀是保留命名空间
- **风险 3**: format string 中的 `{}` 占位符需要转换为 `%s`/`%d` 等
  - **缓解**: 复用现有 `Println` codegen 的 format string 转换逻辑

## 4. Phase 3 设计 (移除 Println variant)

### 4.1 目标

移除 AST/HIR/MIR/Codegen 中的 `Println` variant，统一为 `Call`。

### 4.2 步骤

1. 确认 Phase 2 的 `__landin_println` 调用路径稳定
2. 移除 `ast::Expr::Println`
3. 移除 `hir::HirExprKind::Println`
4. 移除 `mir::StatementKind::Println`
5. 移除 `codegen::statement::StatementKind::Println` 处理
6. 移除相关测试（替换为 `__landin_println` 调用测试）

### 4.3 影响范围

- ~50 个测试可能需要更新（所有用到 `println!` 的测试）
- codegen 的 `emit_println` 函数改为 `emit_call_to_landin_println`
- driver 中 `Println` 相关的特殊处理移除

## 5. v0.6 路线图调整

基于 Phase 1-3 的复杂度评估，更新 v0.6 路线图：

| Phase | Stage | 内容 | 估计 |
|-------|-------|------|------|
| Phase 1 | 18.10 (done) | Built-in macro registration | 1 stage |
| Phase 2 | 18.12 | Real expansion + codegen 特解 | 2 stages |
| Phase 3 | 18.13-18.14 | Remove Println variant (AST/HIR/MIR/Codegen) | 2 stages |
| Phase 4 | 18.15 | v0.6 P1.5 final review | 1 stage |

总计 6 stages (18.10-18.15)，比原估计的 2-3 stages 多。原因是
format string 处理和 4 层特解移除比预期复杂。

## 6. 验收

- [x] §14.5 D1-D8 深度审查完成
- [x] Phase 2 详细设计完成
- [x] Phase 3 详细设计完成
- [x] v0.6 路线图更新
- [x] 当前 build/test/clippy 全绿

## 7. 结论

Stage 18.10 (Phase 1) 通过深度审查。Phase 2-3 设计完成，预计
还需 5 stages (18.12-18.15) 完成 println! 通解化。v0.6 路线图
更新为 6 stages (18.10-18.15)。

下一阶段 (Stage 18.12) 将开始 Phase 2 实现：
- 修改 built-in macro body 为 `__landin_println($($args)*)`
- 在 codegen 中特解 `__landin_println` 调用
- 移除 parser 的 println! 特解
