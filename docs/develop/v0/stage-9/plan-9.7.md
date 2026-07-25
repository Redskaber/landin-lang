# Stage 9.7 开发计划: Generics conformance 扩展

> **阶段**: Stage 9.7 (Stage 9 第 7 个子阶段)
> **版本**: v0.16.5 → v0.16.6
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.6 完成 conformance 307 → 347 (attributes category)。Stage 9.7 继续扩展
**generics** 类别 (per `17-conformance-suite.md` §2 + `02-grammar.md` §3.2)。

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.2 (generic_params + type_bounds + where_clause):
  - `generic_params := "<" (lifetime_param | type_param)* ">"`
  - `lifetime_param := "'" ident (":" lifetime_bounds)?`
  - `type_param := ident (":" type_bounds)? ("=" type)?`
  - `type_bounds := type_bound ("+" type_bound)*`
  - `type_bound := lifetime | "?" type_path | type_path | "for" generic_params type_path`
  - `where_clause := "where" where_pred ("," where_pred)*`
  - `where_pred := (lifetime ":")? type ":" type_bounds`
- `src/parser/generics.rs` (parse_generics + parse_type_bounds + parse_where_clause)

## 3. 测试设计 (50 个 .lin tests)

### 3.1 Generic type params (12 tests)

| 测试文件 | 描述 |
|---------|------|
| gen_param_single.lin | `struct S<T> { x: T }` |
| gen_param_multi.lin | `struct S<T, U> { x: T, y: U }` |
| gen_param_3.lin | `struct S<T, U, V> { x: T, y: U, z: V }` |
| gen_param_fn.lin | `fn f<T>(x: T) {}` |
| gen_param_impl.lin | `impl<T> S<T> {}` |
| gen_param_trait.lin | `trait T<U> {}` |
| gen_param_enum.lin | `enum E<T> { A(T), B }` |
| gen_param_type_alias.lin | `type T<U> = U;` |
| gen_param_method.lin | `impl S { fn f<T>(&self, x: T) {} }` |
| gen_param_with_default.lin | `struct S<T = i32> { x: T }` |
| gen_param_nested.lin | `struct S<T> { v: Vec<T> }` |
| gen_param_mixed.lin | `struct S<T, U = i32> { x: T, y: U }` |

### 3.2 Lifetime params (8 tests)

| 测试文件 | 描述 |
|---------|------|
| gen_lifetime_basic.lin | `fn f<'a>(x: &'a i32) {}` |
| gen_lifetime_multi.lin | `fn f<'a, 'b>(x: &'a i32, y: &'b i32) {}` |
| gen_lifetime_struct.lin | `struct S<'a> { x: &'a i32 }` |
| gen_lifetime_impl.lin | `impl<'a> S<'a> {}` |
| gen_lifetime_trait.lin | `trait T<'a> {}` |
| gen_lifetime_with_type.lin | `fn f<'a, T>(x: &'a T) {}` |
| gen_lifetime_static.lin | `fn f<'a>(x: &'a i32, y: &'static str) {}` |
| gen_lifetime_bounds.lin | `fn f<'a: 'b>(x: &'a i32) {}` |

### 3.3 Type bounds (10 tests)

| 测试文件 | 描述 |
|---------|------|
| gen_bound_single.lin | `fn f<T: Clone>(x: T) {}` |
| gen_bound_multi.lin | `fn f<T: Clone + Default>(x: T) {}` |
| gen_bound_3.lin | `fn f<T: Clone + Default + Debug>(x: T) {}` |
| gen_bound_lifetime.lin | `fn f<T: 'static>(x: T) {}` |
| gen_bound_mixed.lin | `fn f<T: Clone + 'static>(x: T) {}` |
| gen_bound_struct.lin | `struct S<T: Clone> { x: T }` |
| gen_bound_impl.lin | `impl<T: Clone> S<T> {}` |
| gen_bound_trait.lin | `trait T<U: Clone> {}` |
| gen_bound_question_sized.lin | `fn f<T: ?Sized>(x: &T) {}` |
| gen_bound_for_hrtb.lin | `fn f<T: for<'a> Trait<'a>>(x: T) {}` |

### 3.4 Where clauses (10 tests)

| 测试文件 | 描述 |
|---------|------|
| gen_where_basic.lin | `fn f<T>(x: T) where T: Clone {}` |
| gen_where_multi.lin | `fn f<T, U>(x: T, y: U) where T: Clone, U: Default {}` |
| gen_where_lifetime.lin | `fn f<'a, 'b>(x: &'a i32, y: &'b i32) where 'a: 'b {}` |
| gen_where_mixed.lin | `fn f<'a, T>(x: &'a T) where 'a: 'static, T: Clone {}` |
| gen_where_struct.lin | `struct S<T> where T: Clone { x: T }` |
| gen_where_impl.lin | `impl<T> S<T> where T: Clone {}` |
| gen_where_trait.lin | `trait T<U> where U: Clone {}` |
| gen_where_multi_bound.lin | `fn f<T>(x: T) where T: Clone + Default + Debug {}` |
| gen_where_no_bounds.lin | `fn f<T>(x: T) where {}` (empty where — may be allowed) |
| gen_where_complex.lin | `fn f<T, U, V>(x: T, y: U, z: V) where T: Clone, U: Default + Debug, V: 'static {}` |

### 3.5 Generic args (5 tests)

| 测试文件 | 描述 |
|---------|------|
| gen_args_basic.lin | `let x: S<i32> = S { x: 1 };` |
| gen_args_multi.lin | `let x: S<i32, &str> = ...;` |
| gen_args_nested.lin | `let x: S<Vec<i32>> = ...;` |
| gen_args_lifetime.lin | `let x: S<'a, i32> = ...;` |
| gen_args_mixed.lin | `let x: S<'a, i32, T> = ...;` |

### 3.6 边界 & 错误恢复 (5 tests)

| 测试文件 | 描述 |
|---------|------|
| err_gen_unclosed.lin | `FAIL: struct S<T { x: T }` (unclosed generic) |
| err_gen_no_params.lin | `FAIL: struct S<> { x: i32 }` (empty generics) |
| err_gen_bound_no_type.lin | `FAIL: fn f<T:>(x: T) {}` (bound without type) |
| err_gen_where_no_colon.lin | `FAIL: fn f<T>(x: T) where T Clone {}` (where without colon) |
| err_gen_double_comma.lin | `PASS or FAIL: fn f<T,,>(x: T) {}` (double comma — recovery) |

**累计**: 12 + 8 + 10 + 10 + 5 + 5 = **50 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2176+ tests pass (期望 +12 verification tests = 2188)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 397 passed (347 + 50 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.5 → 0.16.6
- api-naming-standard.md: v2.09 → v2.10

---

**创建日期**: 2026-07-26
