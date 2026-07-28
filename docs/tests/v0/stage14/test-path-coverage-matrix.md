# Stage 14 — Test Path Coverage Matrix

> **Author**: redskaber
> **Date**: 2026-07-28
> **Version**: v0.42.0
> **Process**: stage-committee-process.md v3.21 §17.5 (测试矩阵覆盖率)

## 1. Methodology

Per user instruction: "先解决现有的问题，再根据测试理论去阶段审查去做测试路径表格统计（清晰明确）、所有分支流的测试解决问题"

This matrix systematically tests every feature × every branch flow to identify
remaining bugs. Each cell is a test case that exercises a specific code path.

**Legend**:
- ✅ = verified at runtime (run_ok test or manual `--run` verification)
- ⚠️ = compiles but runtime not verified
- ❌ = broken (segfault, wrong output, or compile error)
- ⏳ = not yet tested

## 2. Feature × Branch Flow Matrix

### 2.1 Arithmetic Operators

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `+` | positive + positive | `1 + 2 = 3` | ✅ |
| `+` | positive + negative | `5 + (-3) = 2` | ✅ |
| `+` | negative + negative | `(-5) + (-3) = -8` | ⏳ |
| `-` | positive - positive | `10 - 3 = 7` | ✅ |
| `-` | positive - negative | `5 - (-3) = 8` | ⏳ |
| `*` | positive * positive | `3 * 4 = 12` | ✅ |
| `*` | positive * negative | `3 * (-4) = -12` | ⏳ |
| `*` | negative * negative | `(-3) * (-4) = 12` | ⏳ |
| `/` | positive / positive | `10 / 3 = 3` | ✅ |
| `/` | positive / negative | `10 / (-3) = -3` | ⏳ |
| `/` | negative / positive | `(-10) / 3 = -3` | ⏳ |
| `%` | positive % positive | `10 % 3 = 1` | ✅ |
| `%` | positive % negative | `10 % (-3) = 1` | ⏳ |

### 2.2 Comparison Operators

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `==` | equal | `5 == 5 → true` | ✅ |
| `==` | not equal | `5 == 6 → false` | ⏳ |
| `!=` | not equal | `5 != 6 → true` | ✅ |
| `!=` | equal | `5 != 5 → false` | ⏳ |
| `<` | less | `3 < 7 → true` | ✅ |
| `<` | not less | `7 < 3 → false` | ⏳ |
| `>` | greater | `10 > 5 → true` | ✅ |
| `>` | not greater | `5 > 10 → false` | ⏳ |
| `<=` | less | `3 <= 7 → true` | ⏳ |
| `<=` | equal | `5 <= 5 → true` | ⏳ |
| `>=` | greater | `10 >= 5 → true` | ⏳ |
| `>=` | equal | `5 >= 5 → true` | ⏳ |

### 2.3 Logical Operators

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `&&` | true && true | `true && true → true` | ⏳ |
| `&&` | true && false | `true && false → false` | ⏳ |
| `&&` | false && _ (short-circuit) | `false && (1/0 == 0) → false` | ⏳ |
| `\|\|` | true \|\| _ (short-circuit) | `true \|\| (1/0 == 0) → true` | ⏳ |
| `\|\|` | false \|\| true | `false \|\| true → true` | ⏳ |
| `\|\|` | false \|\| false | `false \|\| false → false` | ⏳ |

### 2.4 Bitwise Operators

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `&` (bitand) | `0b1100 & 0b1010` | `12 & 10 = 8` | ⏳ |
| `\|` (bitor) | `0b1100 \| 0b1010` | `12 \| 10 = 14` | ⏳ |
| `^` (bitxor) | `0b1100 ^ 0b1010` | `12 ^ 10 = 6` | ⏳ |
| `<<` (shl) | `1 << 4` | `1 << 4 = 16` | ⏳ |
| `>>` (shr) | `256 >> 4` | `256 >> 4 = 16` | ⏳ |

### 2.5 Control Flow

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `if` | true branch | `if true { 1 } else { 2 }` → 1 | ✅ |
| `if` | false branch | `if false { 1 } else { 2 }` → 2 | ✅ |
| `if-else if` | first branch | `if true { 1 } else if true { 2 }` → 1 | ✅ |
| `if-else if` | second branch | `if false { 1 } else if true { 2 }` → 2 | ✅ |
| `if-else if-else` | else branch | `if false { 1 } else if false { 2 } else { 3 }` → 3 | ✅ |
| `while` | zero iterations | `while false { }` → no iterations | ⏳ |
| `while` | multiple iterations | `while i < 5 { i += 1; }` → 5 iterations | ✅ |
| `loop` | with break | `loop { break; }` | ✅ |
| `loop` | with break value | `loop { break 42; }` → 42 | ⏳ |
| `match` | first arm | `match 0 { 0 => 1, _ => 2 }` → 1 | ✅ |
| `match` | default arm | `match 99 { 0 => 1, _ => 2 }` → 2 | ✅ |
| `match` | with binding | `match Shape::Circle(5) { Circle(r) => r }` → 5 | ✅ |
| `return` | return value | `return 42;` → 42 | ✅ |
| `return` | return after if | `if x { return -1; } return 1;` → -1 | ✅ |
| `return` | multiple returns | 3 returns in different branches | ✅ |

### 2.6 Data Types

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `i32` | literal | `42` | ✅ |
| `i32` | negative | `-42` | ✅ |
| `i64` | literal | `42i64` | ⏳ |
| `bool` | true | `true` | ✅ |
| `bool` | false | `false` | ✅ |
| `bool` | print | `println!("{}", true)` → "true" | ✅ |
| `&str` | literal | `"hello"` | ✅ |
| `&str` | print | `println!("{}", "hello")` → "hello" | ✅ |
| `()` | unit | `()` | ✅ |
| tuple | construct | `(1, 2, 3)` | ✅ |
| tuple | field access | `t.0` | ✅ |
| array `[a,b,c]` | construct | `[10, 20, 30]` | ✅ |
| array `[val; N]` | repeat | `[0; 3]` | ✅ |
| array | index read | `arr[0]` | ✅ |
| array | index write | `arr[0] = 10` | ✅ |
| struct | construct | `Point { x: 1, y: 2 }` | ✅ |
| struct | field access | `p.x` | ✅ |
| struct | nested construct | `Rect { tl: Point { x: 0, y: 0 } }` | ✅ |
| struct | nested field access | `r.tl.x` | ✅ |
| enum | unit variant | `Shape::Circle` | ⏳ |
| enum | data variant | `Shape::Circle(5)` | ✅ |
| enum | match binding | `match s { Circle(r) => r }` | ✅ |

### 2.7 Functions & Methods

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| fn | no params | `fn f() -> i32 { 42 }` | ✅ |
| fn | one param | `fn f(x: i32) -> i32 { x }` | ✅ |
| fn | multiple params | `fn f(a: i32, b: i32, c: i32) -> i32 { a+b+c }` | ✅ |
| fn | recursion | `fn fact(n: i32) -> i32 { ... }` | ✅ |
| fn | early return | `return 42;` | ✅ |
| impl | `self` by value | `fn get(self) -> i32 { self.x }` | ✅ |
| impl | `&self` | `fn get(&self) -> i32 { self.x }` | ✅ |
| impl | `&mut self` | `fn inc(&mut self) { self.val += 1; }` | ✅ |
| impl | `&self` + array field | `fn get(&self, i: i32) -> i32 { self.data[i] }` | ✅ |
| impl | `&mut self` + array field | `fn push(&mut self, v: i32) { self.data[self.top] = v; }` | ✅ |

### 2.8 Compound Assignment

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `+=` | integer | `x += 5` | ✅ |
| `-=` | integer | `x -= 2` | ✅ |
| `*=` | integer | `x *= 3` | ⏳ |
| `/=` | integer | `x /= 2` | ⏳ |
| `%=` | integer | `x %= 20` | ✅ |

### 2.9 I/O

| Feature | Branch | Test | Status |
|---------|--------|------|--------|
| `println!` | string literal | `println!("hello")` | ✅ |
| `println!` | format args | `println!("x = {}", 42)` | ✅ |
| `println!` | multiple args | `println!("{}, {}", a, b)` | ✅ |
| `println!` | bool | `println!("{}", true)` → "true" | ✅ |
| `println!` | negative | `println!("{}", -5)` → "-5" | ✅ |
| `print!` | no newline | `print!("hello")` | ✅ |
| `eprintln!` | stderr | `eprintln!("err")` | ✅ |

## 3. Summary

| Category | Tested | Total | Coverage |
|----------|--------|-------|----------|
| Arithmetic | 5 | 13 | 38% |
| Comparison | 4 | 12 | 33% |
| Logical | 0 | 6 | 0% |
| Bitwise | 0 | 5 | 0% |
| Control Flow | 11 | 15 | 73% |
| Data Types | 18 | 21 | 86% |
| Functions & Methods | 10 | 10 | 100% |
| Compound Assignment | 3 | 5 | 60% |
| I/O | 7 | 7 | 100% |
| **Total** | **58** | **94** | **62%** |

## 4. Priority Fix Order

Based on coverage gaps and feature importance:

1. **Logical operators (0%)** — `&&`/`||` with short-circuit evaluation
2. **Bitwise operators (0%)** — `&`/`|`/`^`/`<<`/`>>`
3. **Arithmetic edge cases (38%)** — negative number arithmetic
4. **Comparison edge cases (33%)** — `<=`/`>=` branches
5. **Compound assignment (60%)** — `*=`/`/=`
6. **Control flow gaps (73%)** — `loop` with break value, `while` zero iterations
7. **Data types (86%)** — `i64`, enum unit variants

---

**Created**: 2026-07-28
**Process**: v3.21 §17.5

---

## 5. Stage 14.40 Update — Method Chain Coverage (2026-07-28)

**Stage 14.40 closes the method chain resolution saga (Stages 14.38-14.40).**

### New E2E Test Paths Added

| Test ID | Feature | Expected | Status |
|---------|---------|----------|--------|
| E-055 | Multi-step method chain (`a.add(b).scale(2).add(...).get()`) | `50` | ✅ |
| E-056 | Inline chained method call (`V::new(1,2).add(...).get()`) | `10` | ✅ |

### Coverage Delta

| Metric | Before (Stage 14.39) | After (Stage 14.40) | Delta |
|--------|----------------------|---------------------|-------|
| run_ok tests | 54 | 56 | +2 |
| conformance total | 5080 | 5082 | +2 |
| Method chain paths covered | 0 | 2 (multi-step + inline) | +2 |
| Pipeline coverage | 99.7% (618/620) | 99.7% (620/620 + 2 new) | +2 paths |

### Resolver Bug Fixed

The root cause of method chain failure was a resolver bug: impl/trait method
signatures stored inside `HirImpl.items` / `HirTrait.items` were left with
`path.res = Res::Unknown` because the resolver only processed the OWNER copies
(separate `HirItem::Fn` owners) but not the inline clones inside Trait/Impl
blocks. After Stage 14.40, the resolver processes both copies uniformly via
`resolve_trait_item_paths` and `resolve_impl_item_paths` helpers.

**Last updated**: 2026-07-28 (Stage 14.40)
