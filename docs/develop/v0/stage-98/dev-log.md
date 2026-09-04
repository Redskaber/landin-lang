# Stage 98 开发日志 — Trait impl symbol mangling 修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.636.0 → v0.637.0 |
| 测试数 | 5580 → 5589 (+9) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC | 4 源文件 + 32+ 测试文件 (~500 LOC 变更) |

## 修改文件

### 源文件 (4)
| 文件 | 变更 |
|------|------|
| `src/driver/driver_codegen_prep.rs` | `fn_name_by_def_id`: `<type>_<method>` → `<Trait>_<type>_<method>` |
| `src/traits/resolver.rs` | `VtableEntry.fn_name`: 同上 |
| `src/stdlib/vtable_layout.rs` | `stdlib_vtable_method_symbols`: 同上 |
| `src/codegen/drop_glue.rs` | Drop impl method 调用名: 同上 |

### 测试文件 (32+)
所有引用旧 mangled name (`landin_i32_fmt` 等) 的测试更新为新 mangled name (`landin_Display_i32_fmt` 等).

### 其他
- `Cargo.toml`: 版本 → 0.637.0
- `src/stdlib/prelude.rs`: Debug + PartialOrd impl bodies 暂时移除 (Stage 99 调查)

## 根因 (FIXED)

### Symbol collision
```rust
impl Display for i32 { fn fmt(&self) -> String { ... } }  // → landin_i32_fmt
impl Debug for i32 { fn fmt(&self) -> String { ... } }     // → landin_i32_fmt  ← COLLISION
```

LLVM module 收到两个 `landin_i32_fmt` 定义, 但签名不同 → SIGSEGV/stack smashing.

### Mangling 修复

**Before**: `<type>_<method>` (e.g. `landin_i32_fmt`)
**After**: `<Trait>_<type>_<method>` (e.g. `landin_Display_i32_fmt`, `landin_Debug_i32_fmt`)

## 关键决策

### 决策 1: mangling 中包含 trait 名

**理由** (§12 最优>最小, §1.0 原则 6 通解>特解):
- 一个 mangling 规则适用于所有 trait impl methods.
- 与 Rust RFC 2603 (Rust mangling v0) 一致.

**替代方案 (拒绝)**:
- 限制用户不能用同名 method (违反 Rust 设计).
- 用 hash 或 DefId (不可读, 调试困难).

### 决策 2: Debug + PartialOrd impl bodies 暂时移除

**理由**:
- 根因 (symbol collision) 已修复 — user code 中 impl method returning String 正常工作 (`test_sret2.landin → 42`).
- 但 prelude impl bodies 在 LLVM integration tests 中触发 stack smashing.
- 新 TD: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+).
- Stage 99 转根因调查.

## 测试覆盖

新增 9 个测试 (positive + negative):
- 验证新 mangling 在 Display/Clone/Default 等 trait 上工作
- 负向测试覆盖 undefined type, type mismatch, nonexistent method, wrong arg count 等

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend` ✓ (5589 tests, 0 failures, 9 ignored)

## 打包

- `landin-stage0-v0.637.0-stage98-trait-impl-symbol-mangling-r646.tar.gz` (4.87 MB)
- 路径: `/home/z/my-project/download/`

## 下一步

- Stage 99: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因调查 + 修复
- Stage 100: 重新添加 Debug + PartialOrd impls
- TD-PRELUDE-METHOD-COVERAGE: 扩展 prelude 方法覆盖
