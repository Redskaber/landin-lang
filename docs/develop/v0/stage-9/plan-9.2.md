# Stage 9.2 开发计划: Operators + Pratt precedence conformance 扩展

> **阶段**: Stage 9.2 (Stage 9 第 2 个子阶段)
> **版本**: v0.16.0 → v0.16.1
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.1 完成 conformance 8 → 38 (literals category)。Stage 9.2 继续 conformance
扩展, 聚焦 **operators + Pratt precedence** 类别 (per `17-conformance-suite.md` §2
+ `02-grammar.md` §3.4 + §2 Pratt 优先级表)。

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §1.8 (operator := 28 operators)
- `docs/lang-design/02-grammar.md` §2 (Pratt 优先级表 — 13 levels)
- `docs/lang-design/02-grammar.md` §3.4 (Expression)
- `src/parser/expr.rs` (binop_bp + assign_op + 13 Pratt-level functions)

**Pratt 优先级表** (per §2):

| 优先级 | 运算符 | 结合性 |
|--------|--------|--------|
| 1 (最低) | `\|\|` | 左 |
| 2 | `&&` | 左 |
| 3 | `==` `!=` `<` `>` `<=` `>=` | 需要 |
| 4 | `\|` | 左 |
| 5 | `^` | 左 |
| 6 | `&` | 左 |
| 7 | `<<` `>>` | 左 |
| 8 | `+` `-` | 左 |
| 9 | `*` `/` `%` | 左 |
| 10 | `as` | 左（一元后缀） |
| 11 | `-` `!` `*` `&` `&mut` | 一元前缀 |
| 12 | `(` `.` `[` `?` `!` | 后缀调用/字段/索引 |
| 13 (最高) | 字面量、路径、`(expr)`、`{block}` | 原子 |

## 3. 测试设计 (60 个 .lin tests)

### 3.1 算术运算符 (8 tests)

| 测试文件 | 描述 |
|---------|------|
| arith_add.lin | `1 + 2` |
| arith_sub.lin | `10 - 5` |
| arith_mul.lin | `3 * 4` |
| arith_div.lin | `20 / 5` |
| arith_rem.lin | `17 % 5` |
| arith_chain.lin | `1 + 2 + 3 + 4` (左结合) |
| arith_mixed.lin | `1 + 2 * 3` (优先级) |
| arith_parens.lin | `(1 + 2) * 3` (括号覆盖) |

### 3.2 比较运算符 (6 tests)

| 测试文件 | 描述 |
|---------|------|
| cmp_eq.lin | `a == b` |
| cmp_ne.lin | `a != b` |
| cmp_lt.lin | `a < b` |
| cmp_gt.lin | `a > b` |
| cmp_le.lin | `a <= b` |
| cmp_ge.lin | `a >= b` |

### 3.3 逻辑运算符 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| logic_and.lin | `a && b` |
| logic_or.lin | `a \|\| b` |
| logic_not.lin | `!flag` |
| logic_chain.lin | `a \|\| b && c` (&& 优先于 \|\|) |
| logic_parens.lin | `(a \|\| b) && c` (括号覆盖) |

### 3.4 位运算符 (6 tests)

| 测试文件 | 描述 |
|---------|------|
| bit_and.lin | `a & b` |
| bit_or.lin | `a \| b` |
| bit_xor.lin | `a ^ b` |
| bit_shl.lin | `a << 2` |
| bit_shr.lin | `a >> 2` |
| bit_chain.lin | `a \| b & c` (& 优先于 \|) |

### 3.5 赋值运算符 (12 tests)

| 测试文件 | 描述 |
|---------|------|
| assign_simple.lin | `x = 5` |
| assign_add.lin | `x += 1` |
| assign_sub.lin | `x -= 1` |
| assign_mul.lin | `x *= 2` |
| assign_div.lin | `x /= 2` |
| assign_rem.lin | `x %= 3` |
| assign_and.lin | `x &= 0xff` |
| assign_or.lin | `x \|= 0x10` |
| assign_xor.lin | `x ^= 0xff` |
| assign_shl.lin | `x <<= 4` |
| assign_shr.lin | `x >>= 4` |
| assign_chain.lin | `a = b = c` (右结合) |

### 3.6 一元前缀 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| unary_neg.lin | `-x` |
| unary_not.lin | `!flag` |
| unary_deref.lin | `*ptr` |
| unary_ref.lin | `&x` |
| unary_ref_mut.lin | `&mut x` |

### 3.7 后缀 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| postfix_call.lin | `f(x)` |
| postfix_method.lin | `x.method()` |
| postfix_field.lin | `x.field` |
| postfix_index.lin | `arr[i]` |
| postfix_chain.lin | `obj.method().field` |

### 3.8 Pratt 优先级组合 (10 tests)

| 测试文件 | 描述 |
|---------|------|
| prec_mul_over_add.lin | `1 + 2 * 3` = `1 + (2*3)` |
| prec_add_over_cmp.lin | `a + b < c` = `(a+b) < c` |
| prec_cmp_over_and.lin | `a < b && c < d` = `(a<b) && (c<d)` |
| prec_and_over_or.lin | `a \|\| b && c` = `a \|\| (b&&c)` |
| prec_or_over_assign.lin | `x = a \|\| b` = `x = (a\|\|b)` |
| prec_shift_over_add.lin | `1 + 2 << 3` = `(1+2) << 3`? — 验证实际行为 |
| prec_bit_over_cmp.lin | `a & b < c` = `(a&b) < c`? — 验证实际行为 |
| prec_unary_over_mul.lin | `-a * b` = `(-a) * b` |
| prec_parens_override.lin | `(1 + 2) * 3` (括号优先) |
| prec_nested_parens.lin | `((1 + 2) * (3 + 4))` |

### 3.9 边界 & 错误恢复 (3 tests)

| 测试文件 | 描述 |
|---------|------|
| err_unmatched_paren.lin | `FAIL: (1 + 2` (缺少右括号) |
| err_double_op.lin | `FAIL: 1 + + 2` (双运算符) |
| err_empty_expr.lin | `FAIL: let x = ;` (空表达式) |

**累计**: 8 + 6 + 5 + 6 + 12 + 5 + 5 + 10 + 3 = **60 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2111+ tests pass (期望 +10 verification tests = 2121)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 98 passed (38 + 60 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.0 → 0.16.1
- api-naming-standard.md: v2.04 → v2.05

---

**创建日期**: 2026-07-26
