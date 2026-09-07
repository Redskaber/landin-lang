# Stage 145 开发日志 — TD-CODEGEN-CAST-UNSIGNED 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.668.0 → v0.669.0 |
| 测试数 | 5892 → 5918 (+26 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 145 完整修复 TD-CODEGEN-CAST-UNSIGNED — `emit_cast` 添加 `src_signed: bool` 参数，
让 caller (有 MIR `Ty` 访问权) 传递 source 类型的 signedness。

### WHY (根因分析, §2.2 根因思维)

**根因**: `EmitType` 只携带 integer *width* (I8/I16/I32/I64), 不携带 *signedness* —
`i8` 和 `u8` 都映射到 `EmitType::I8`。对于 widening casts (`u8 as i64`), sext (sign-extend, 
signed) vs zext (zero-extend, unsigned) 的选择依赖 source 类型的 signedness, 
而 `EmitType` 无法提供。

**之前的行为**: `emit_cast` 硬编码 `is_signed=1` (LLVMSysEmitter) / "sext" (TextEmitter) —
所有整数按有符号处理。这导致 `b'\xFF' as i64` 返回 -1 而非 255 (违反 Rust 语义)。

**架构选项分析**:
- Option A (添加 U8/U16/U32/U64 variants 到 EmitType): ~500+ LOC, 178 match arms 需更新 — 高风险
- Option B (添加 src_signed 参数到 emit_cast): ~30 LOC, focused change — 低风险 ✓
- Option C (bypass emit_cast, 直接调用 LLVMBuildZExt): ~50 LOC, 3 paths for same op — §1.0 原則 6 violation

### HOW (核心修复)

#### 修复 1: trait 方法签名修改
```rust
// Before:
fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

// After:
fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, src_signed: bool, val: &EmitValue) -> EmitValue;
```

#### 修复 2: LLVMSysEmitter 用 src_signed 选择 is_signed
```rust
LLVMBuildIntCast2(self.builder, v, dst_ty, if src_signed { 1 } else { 0 }, name_c.as_ptr())
```

#### 修复 3: TextEmitter 用 src_signed 选择 sext vs zext
```rust
if sw < dw {
    if src_signed { "sext" } else { "zext" }
}
```

#### 修复 4: 添加 is_mir_type_signed + operand_is_signed helpers
```rust
pub fn is_mir_type_signed(ty: &Ty) -> bool {
    matches!(&ty.kind, TyKind::Int(_))  // true for Int, false for Uint/Bool/others
}
pub fn operand_is_signed(mir: &MirBody, op: &Operand) -> bool { ... }
```

#### 修复 5: 更新 7 个调用点
- `rvalue.rs:777` (Rvalue::Cast): 用 `operand_is_signed(mir, op)`
- `rvalue.rs:238-241` (float bitwise): `true` (irrelevant for float)
- `operand.rs:364` (constant cast): `matches!(&c.val, ConstVal::Int(_))`
- `statement.rs:122` (i32→i1 trunc): `true` (irrelevant for narrowing)
- `statement.rs:558` (println int widening): `true` (signed path)
- `statement.rs:574` (float→double): `true` (irrelevant for float)
- `statement.rs:609` (ptr deref→int): `true` (fallback, documented)
- `places.rs:1429` (OOB bounds check): query `mir.local_decls[idx].ty`

### 重要副作用: bool as i64 行为修正
之前 `bool as i64` 用 sext → `true` (i1=1) 变成 -1 (i64=0xFFFF...FFFF)。
现在 bool 不是 `TyKind::Int`, 所以 `is_mir_type_signed` 返回 false → zext → `true` 变成 1。
这是 **正确的 Rust 语义** (`true as i64 == 1`)。Stage 143/144 测试期望 `-1` 是 bug,
本阶段更新为 `1`。

### 决策点 (§12 最优 > 最小, §1.0 原則 5/6/9/10)

1. **选 src_signed: bool 参数** 不选新增 U8/U16/U64 variants — §1.0 原則 6 (通解 > 特解) +
   §12 (最优 > 最小: 30 LOC vs 500 LOC)
2. **选 caller 查询 MIR** 不选 emitter 推断 — §1.0 原則 10 (唯一可信数据源: MIR Ty 是 source of truth)
3. **选 bool 参数** 不选 typed enum — 简洁性, signedness 是二元概念
4. **选修改 trait 签名** 不选新增 emit_cast_unsigned — §1.0 原則 5 (去除兼容思维) +
   §1.0 原則 6 (一个 cast 方法处理所有情况)
5. **选 bool 默认 true (signed)** for 未知类型 — §1.0 原則 9 (正确 > 妥协: 文档化 fallback)
6. **更新 Stage 143/144 测试期望** 从 -1 改为 1 — 修正之前的 bug 期望 (bool 是 unsigned)

### 裁剪点 (§1.2.1)
L3 任务 — codegen 跨模块变更 (7 调用点 + 2 emitter impls + 1 helper module), 
但单轮收敛 (根因清晰, 修复方案明确)。保留 §14.5 深度审查的关键检查点。

### 下一步 (MUV)
- Stage 146: TD-TYPECK-ASSOC-TYPE-PROJECTION (解锁 Iterator trait) 或
  TD-TYPECK-LIFETIME-ELISION (3 条 elision rules) 或
  TD-LEX-DOC-COMMENT (P4, v0.16+)

## 文件清单

### 修改文件 (7)
- `src/codegen/emitter/arithmetic.rs` — trait 方法签名 (+src_signed + 文档注释)
- `src/codegen/llvm/arithmetic.rs` — LLVMSysEmitter impl 用 src_signed
- `src/codegen/text/arithmetic.rs` — TextEmitter impl 用 src_signed (sext vs zext)
- `src/codegen/rvalue.rs` — 4 处 emit_cast 调用更新
- `src/codegen/operand.rs` — 1 处 emit_cast 调用更新 (ConstVal::Int check)
- `src/codegen/statement.rs` — 4 处 emit_cast 调用更新
- `src/codegen/mir_translation/places.rs` — 1 处 emit_cast 调用更新 (OOB bounds check)
- `src/codegen/mir_translation/types.rs` — 添加 is_mir_type_signed + operand_is_signed helpers
- `src/codegen/mir_translation/mod.rs` — re-export 新 helpers
- `src/codegen/llvm/tests.rs` — 1 处 emit_cast 调用更新 (lib test)

### 修改测试文件 (2)
- `tests/v0/stage143/plan/string_methods_tests.rs` — bool as i64 期望 -1 → 1 (修正 bug 期望)
- `tests/v0/stage144/plan/lex_literals_tests.rs` — bool as i64 期望 -1 → 1 (修正 bug 期望)

### 新建文件 (1)
- `tests/v0/stage145/plan/cast_unsigned_tests.rs` — 26 tests
