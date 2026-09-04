# Stage 100 开发计划 — monomorphization 跳过 prelude generic function

> **阶段**: v0.10 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 修复 - Layer 1)
> **TD**: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+) — Stage 99 RCA Layer 1 修复
> **复杂度**: L3 (跨模块: codegen + driver + 修改 CompileResult 字段)
> **版本基线**: v0.638.0 (Stage 99 RCA, 5594 tests)
> **目标版本**: v0.639.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 在 `codegen_from_mir` 中跳过 prelude generic function (DefId >= user_item_count 且 MIR 含 Param type)。仅 codegen_mono_functions 在实例化时 emit 这些函数 |
| **WHY** | prelude generic function (Option::map, Box::new, Vec::push) 的 MIR 含 Param type，codegen_from_mir 当前为所有 MIR bodies 生成 LLVM IR，Param 被 fallback 到 i32 → 不正确 LLVM IR → 累积触发 SIGSEGV/SIGABRT (Stage 99 RCA 4-layer 根因链 Layer 1+2+3) |
| **WHO** | ARCH-A 设计；DEV-A 实施；REV-A 审查；QA-A 测试 |
| **WHEN** | Stage 100 完成 → 进入 Stage 101 (修复 Param fallback 返回 Error) |
| **WHERE** | `src/codegen/function.rs:154 codegen_from_mir`, `src/driver/mod.rs CompileResult`, `src/driver/compile_inner.rs`, `src/codegen/pipeline.rs:168` |
| **HOW** | 1) CompileResult 添加 user_item_count; 2) codegen_from_mir 接收 user_item_count + 跳过 prelude generic; 3) pipeline.rs 传 user_item_count |
| **HOW MUCH** | ~5 文件, ~30 LOC, 1:3+ 正负测试 |

## 二、对齐设计文档 (§13.1 / §8.4.5)

### docs/lang-design/06-mir.md 对齐
Rust 设计: rustc monomorphization pass 只为 `MonoItem::Fn` (具体实例化) 生成 codegen。generic function 定义本身不 emit LLVM IR，只有调用点的实例化版本 emit。

### docs/graph/mir/data-flow.md 对齐
data flow: MIR lower → driver mirs (含 prelude generic bodies) → codegen_from_mir (所有 bodies) + codegen_mono_functions (MonoItem::Fn 实例化)

当前 codegen_from_mir 错误地为 prelude generic bodies 生成 IR，违反 data flow — prelude generic 应仅被 codegen_mono_functions 实例化路径 emit。

## 三、决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

### 决策 1: 跳过 prelude generic function (基于 DefId + Param 类型双重检查)

**选择**: 在 `codegen_from_mir` 中跳过 DefId >= user_item_count 且 MIR 含 Param type 的 body。

**替代方案 (拒绝)**:
- ❌ 跳过所有 prelude items — 错误，prelude non-generic function (String::from_str, String::new) 仍需 codegen
- ❌ 在 MIR lower 阶段不生成 prelude generic bodies — 错误，破坏 MIR lower 单一职责
- ❌ 在 codegen_from_mir 中尝试解析 Param 类型 — 违反 §16 (codegen 不访问 HIR)

**理由** (§1.0 原则 6 通解>特解):
- 一条规则适用于所有 prelude items，不区分具体 trait/method
- 双重检查 (DefId 边界 + Param 存在) 避免误跳过 prelude non-generic function
- 与 rustc 设计一致 — generic function 定义不 emit，只实例化版本 emit

### 决策 2: 把 user_item_count 存到 CompileResult

**选择**: 在 CompileResult 添加 `user_item_count: usize` 字段。

**理由** (§1.0 原则 10 唯一可信数据源):
- `user_item_count` 已在 `compile_inner.rs:79` 计算
- 当前传递路径: compile_inner → driver_codegen_prep (trait_resolver)
- 新增传递路径: compile_inner → CompileResult → codegen_from_mir
- codegen 不访问 HIR，需要通过 CompileResult 传递 user_item_count

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 100.1 | CompileResult 添加 user_item_count 字段 | 编译通过 |
| 100.2 | compile_inner 设置 user_item_count 到 CompileResult | 字段正确填充 |
| 100.3 | codegen_from_mir 接收 user_item_count + 跳过 prelude generic | 跳过逻辑正确 |
| 100.4 | pipeline.rs 传 user_item_count 到 codegen_from_mir | 调用链通 |
| 100.5 | 添加 4 个 stage100 测试 (1 positive + 3 negative) | cargo test 全绿 |
| 100.6 | §3.2 验收 + 验证 Param warnings 减少 | fmt/clippy/test 全绿 |
| 100.7 | 更新 worklog + tech-debt-register + README + RELEASE_NOTES | 文档同步 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- [ ] `cargo test --release --features llvm-backend --lib` ✓ (898+ tests, 0 failures)
- [ ] `cargo test --release --features llvm-backend --test all_tests` ✓ (5598+ tests, 0 failures, 9 ignored)
- [ ] Param warnings 数量显著减少 (基线 1360 → 目标 < 200)

## 六、参考

- Stage 99 RCA: `docs/develop/v0/stage-99/dev-log.md` (4-layer 根因链)
- Rust 设计: rustc monomorphization — generic function 定义不 emit, 只实例化版本 emit
- docs/lang-design/06-mir.md: MIR 设计文档
- docs/graph/mir/data-flow.md: MIR 数据流图
