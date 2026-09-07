# Stage 144 开发日志 — TD-LEX-RAW-STRING + TD-LEX-BYTE-LITERAL 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.668.0 (不变 — TD 实际范围比描述小，无版本变更) |
| 测试数 | 5857 → 5892 (+35 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 144 完整修复 2 个 P3 技术债：
1. **TD-LEX-RAW-STRING**: raw string literals (`r"..."`, `r#"..."#`) 完整支持
2. **TD-LEX-BYTE-LITERAL**: byte literals (`b'A'`, `b"..."`, `br"..."`) 完整支持

### WHY (根因分析, §2.2 根因思维)

**TD 描述与实际不符**:
- TD-LEX-RAW-STRING 描述 "Lexer 不支持 raw string 语法" — 但实际 **Lexer 已实现**
  (Stage 6.13 中 `lex_raw_string` + `lex_raw_string_hash` 已存在)
- TD-LEX-BYTE-LITERAL 描述 "Lexer 不支持 byte literal 语法" — 但实际 **Lexer + Parser + Typeck + Codegen 全链路已实现**
  (Stage 6.13 中 `lex_byte` + `lex_byte_string` + `lex_raw_byte_string` 已存在)

**真正缺失的部分**:
- TD-LEX-RAW-STRING: parser/expr.rs 中 `RawStrLit(sym, hashes)` token 没有 literal expression arm
- TD-LEX-BYTE-LITERAL: `lex_byte` 在缺少 closing `'` 时不报错 (§1.0 原則 4 violation)

### HOW (核心修复)

#### 修复 1: parser 添加 RawStrLit arm
```rust
TokenKind::RawStrLit(sym, _hashes) => {
    self.bump();
    Expr::Lit(LitKind::Str(sym), span)
}
```
- raw string 与 regular string 都产生 `LitKind::Str` (相同 `&'static str` 类型)
- Per §1.0 原則 6 (通解 > 特解): 一个 `LitKind::Str` 处理两种 token

#### 修复 2: lex_byte 添加未闭合错误
```rust
if self.peek() == Some(b'\'') {
    self.bump();
} else {
    self.errors.push(LexError {
        message: "unterminated byte literal — missing closing `'`".into(),
        ...
    });
}
```
- Per §1.0 原則 4 (报错 > 静默): 缺少闭合 `'` 必须 push 错误
- Per §1.0 原則 9 (正确 > 妥协): 修复根因 (push error) 而非症状 (静默接受)

### 发现的新 TD (待 Stage 145+ 处理)
- **TD-CODEGEN-CAST-UNSIGNED**: `b'\xFF' as i64` 返回 -1 而非 255。codegen 的 `emit_cast`
  使用 `LLVMBuildIntCast2(is_signed=1)`，对所有整数按有符号处理。u8 值 ≥ 128 时
  sign-extended 到负数。需要区分有符号/无符号。本阶段测试用 `0x7F` (127) 绕过。

### 决策点 (§12 最优 > 最小)

1. **选 RawStrLit → LitKind::Str** 不选新增 AST 节点 — §1.0 原則 6 (通解 > 特解):
   raw string 与 regular string 在 type system 中相同 (都是 `&'static str`)
2. **选 push lex error** 不选静默接受 — §1.0 原則 4 (报错 > 静默) + §1.0 原則 9 (正确 > 妥协)
3. **选修复 lex_byte** 不选只改 parser — 完整修复 TD-LEX-BYTE-LITERAL 的 lex 层缺陷
4. **选 byte literal `0x7F`** 不选 `0xFF` — 测试用例避开已知的 TD-CODEGEN-CAST-UNSIGNED
   缺陷，stage 144 范围内不修

### 裁剪点 (§1.2.1)
L2 任务 — 跳过跨阶段深度审查 (§14.6) — 单文件 + 单功能 + 35 tests 已覆盖。
保留 §14.5 深度审查的关键检查点 (D1-D8 摘要见 worklog)。

### 下一步 (MUV)
- Stage 145: TD-TYPECK-ASSOC-TYPE-PROJECTION (解锁 Iterator trait) 或
  TD-CODEGEN-CAST-UNSIGNED (本阶段发现的新 TD) 或
  TD-LEX-DOC-COMMENT (P4, v0.16+)
- v0.15 阶段剩余 TD: TD-TYPECK-LIFETIME-ELISION, TD-BORROWCK-NLL, TD-TRAIT-BLANKET-IMPL 等

## 文件清单

### 修改文件 (2)
- `src/parser/expr.rs` — 添加 `RawStrLit(_, _)` arm 到 literal expression parser
- `src/lexer/string.rs` — `lex_byte` 添加未闭合错误 push (§1.0 原則 4)

### 新建文件 (1)
- `tests/v0/stage144/plan/lex_literals_tests.rs` — 35 tests

### 修改测试文件 (1)
- `tests/all_tests.rs` — 添加 stage144 模块注册
