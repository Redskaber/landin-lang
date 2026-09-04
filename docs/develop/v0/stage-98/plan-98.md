# Stage 98 开发计划 — Trait impl method symbol mangling 修复

> **阶段**: v0.9 (Prelude Trait Coverage Wave)
> **TD**: TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH (FIXED)
> **复杂度**: L3 (跨模块, 4 源文件 + 32+ 测试文件)
> **版本基线**: v0.636.0 (Stage 97, 5580 tests)
> **目标版本**: v0.637.0

## 一、5W2H 启动分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 修复 trait impl method symbol mangling — `impl Display for i32 { fn fmt }` 和 `impl Debug for i32 { fn fmt }` 产生 `landin_i32_fmt` 冲突 |
| **WHY** | LLVM module 有两个同名不同签名函数 → SIGSEGV/stack smashing。这是 P2 阻断项, 阻止添加 Debug/PartialOrd impls |
| **WHO** | ARCH-A 设计 mangling scheme; DEV-A 实施 4 源文件 + 32+ 测试更新; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 98 完成 → 进入 Stage 99 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH) |
| **WHERE** | `src/driver/driver_codegen_prep.rs`, `src/traits/resolver.rs`, `src/stdlib/vtable_layout.rs`, `src/codegen/drop_glue.rs` + 32+ 测试文件 |
| **HOW** | mangling 中包含 trait 名: `landin_Display_i32_fmt` vs `landin_Debug_i32_fmt` |
| **HOW MUCH** | 4 源文件 + 32+ 测试文件, ~500 LOC 变更 |

## 二、根因

### Symbol collision
```rust
impl Display for i32 { fn fmt(&self) -> String { ... } }  // → landin_i32_fmt
impl Debug for i32 { fn fmt(&self) -> String { ... } }     // → landin_i32_fmt  ← COLLISION
```

LLVM module 收到两个 `landin_i32_fmt` 定义, 但签名不同 (Display 返回 String 用 sret; Debug 返回 String 用 sret, 但 vtable 中可能不同) → LLVM verifier 通过但 codegen 阶段 stack smashing / SIGSEGV.

### Mangling scheme 修复

**Before** (4 处):
- `fn_name_by_def_id`: `<type>_<method>`
- `VtableEntry.fn_name`: `<type>_<method>`
- `stdlib_vtable_method_symbols`: `<type>_<method>`
- `drop_glue.rs`: `<type>_<method>`

**After**:
- `fn_name_by_def_id`: `<Trait>_<type>_<method>`
- `VtableEntry.fn_name`: `<Trait>_<type>_<method>`
- `stdlib_vtable_method_symbols`: `<Trait>_<type>_<method>`
- `drop_glue.rs`: `<Trait>_<type>_<method>`

## 三、决策点 (§12 最优>最小, §1.0 原则 6 通解>特解)

### 决策 1: mangling 中包含 trait 名

**选择**: `<Trait>_<type>_<method>` (e.g. `landin_Display_i32_fmt`).

**替代方案 (拒绝)**:
- ❌ 限制用户不能用同名 method (违反 Rust 设计)
- ❌ 用 hash 或 DefId (不可读, 调试困难)

**理由** (§1.0 原则 6 通解>特解):
- 一个 mangling 规则适用于所有 trait impl methods (任何 trait, 任何 type, 任何 method).
- 与 Rust RFC 2603 (Rust mangling v0) 一致 — trait name 包含在 mangled name 中.

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 98.1 | 设计 mangling scheme (trait 名包含) | ARCH-A review |
| 98.2 | 更新 `driver_codegen_prep.rs` (fn_name_by_def_id) | 函数定义名正确 |
| 98.3 | 更新 `traits/resolver.rs` (VtableEntry.fn_name) | vtable 符号正确 |
| 98.4 | 更新 `stdlib/vtable_layout.rs` (stdlib_vtable_method_symbols) | stdlib vtable 正确 |
| 98.5 | 更新 `codegen/drop_glue.rs` (Drop impl method) | Drop 调用正确 |
| 98.6 | 更新 32+ 测试文件 (旧 mangled name → 新) | cargo test 全绿 |
| 98.7 | §3.2 验收 + worklog + tech-debt-register | 全绿 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (5589 tests, 0 failures, 9 ignored)

## 六、新发现 — 新 TD

### TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+)

**现象**: Mangling 修复后, user code 中 impl method returning String 工作正常 (`test_sret2.landin → 42`). 但 **prelude impl bodies 在 LLVM integration tests 中触发 stack smashing**.

**根因**: 待 Stage 99 调查。可能是:
- LLVM module verification 对 prelude impl method bodies 的处理不完整
- prelude impl method body 的 MIR 结构与 user code 不同
- codegen 对 prelude impl method 的 alloca/load 顺序有 bug

**修复路径**: Stage 99 → 5W2H 根因分析 → 修复 → 重新添加 Debug impls.

## 七、下一步

- Stage 99: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因调查 + 修复
- Stage 100: 重新添加 Debug impls (returning String) + PartialOrd impls (returning Option)
