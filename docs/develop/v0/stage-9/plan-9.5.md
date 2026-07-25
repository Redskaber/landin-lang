# Stage 9.5 开发计划: Types conformance 扩展

> **阶段**: Stage 9.5 (Stage 9 第 5 个子阶段)
> **版本**: v0.16.3 → v0.16.4
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.4 完成 conformance 177 → 247 (patterns category)。Stage 9.5 继续扩展
**types** 类别 (per `17-conformance-suite.md` §2 + `02-grammar.md` §3.3)。

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/02-grammar.md` §3.3 (Type):
  - `type := "(" type? "," type? ")" | "!" | "[" type ";" expr "]" | "[" type "]" | "&" lifetime? "mut"? type | "*const" type | "*mut" type | "fn" "(" fn_params? ")" ("->" type)? | "impl" type_bounds | "dyn" type_bounds | type_path | qualified_path`
  - `type_path := (path "::")? ident ("::" type_segment)*`
  - `type_segment := ident generic_args?`
  - `qualified_path := "<" type "as" type_path ">" "::" type_segment`
- `src/parser/ty.rs` (parse_ty — primitive / ref / ptr / tuple / array / slice / fn-ptr / trait-object / impl-trait / path)

## 3. 测试设计 (60 个 .lin tests)

### 3.1 Primitive types (12 tests)

| 类别 | 测试数 | 示例 |
|------|-------|------|
| bool | 1 | `let x: bool = true;` |
| char | 1 | `let x: char = 'a';` |
| i8/i16/i32/i64/i128/isize | 6 | `let x: i32 = 1;` |
| u8/u16/u32/u64/u128/usize | 6 | `let x: u32 = 1;` |
| str (unsized) | 1 | `let x: &str = "hello";` |
| f32/f64 | 2 | `let x: f64 = 3.14;` |
| () unit | 1 | `let x: () = ();` |
| Subtotal | 20 | |

实际上，让我精简至 12 测试: bool, char, i8, i16, i32, i64, i128, isize, u8, u32, u64, usize, f32, f64, str, () → 选 12 个最具代表性的

### 3.2 Reference types (8 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_ref_basic.lin | `let x: &i32 = &1;` |
| ty_ref_mut.lin | `let x: &mut i32 = &mut 1;` |
| ty_ref_ref.lin | `let x: &&i32 = &&1;` |
| ty_ref_str.lin | `let x: &str = "hello";` |
| ty_ref_array.lin | `let x: &[i32; 3] = &[1, 2, 3];` |
| ty_ref_struct.lin | `let x: &P = &p;` |
| ty_ref_mut_struct.lin | `let x: &mut P = &mut p;` |
| ty_ref_static.lin | `let x: &'static str = "hello";` |

### 3.3 Raw pointer types (5 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_ptr_const.lin | `let x: *const i32 = ...;` |
| ty_ptr_mut.lin | `let x: *mut i32 = ...;` |
| ty_ptr_const_void.lin | `let x: *const () = ...;` |
| ty_ptr_const_struct.lin | `let x: *const P = ...;` |
| ty_ptr_mut_array.lin | `let x: *mut [i32; 3] = ...;` |

### 3.4 Array types (8 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_array_basic.lin | `let x: [i32; 3] = [1, 2, 3];` |
| ty_array_2d.lin | `let x: [[i32; 2]; 2] = ...;` |
| ty_array_large.lin | `let x: [u8; 256] = ...;` |
| ty_array_bool.lin | `let x: [bool; 4] = ...;` |
| ty_array_str.lin | `let x: [&str; 2] = ...;` |
| ty_array_struct.lin | `let x: [P; 3] = ...;` |
| ty_array_ref.lin | `let x: [&i32; 3] = ...;` |
| ty_array_empty.lin | `let x: [i32; 0] = [];` |

### 3.5 Slice types (4 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_slice_basic.lin | `let x: &[i32] = &arr;` |
| ty_slice_u8.lin | `let x: &[u8] = &arr;` |
| ty_slice_str.lin | `let x: &[&str] = &arr;` |
| ty_slice_struct.lin | `let x: &[P] = &arr;` |

### 3.6 Tuple types (6 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_tuple_2.lin | `let x: (i32, i32) = (1, 2);` |
| ty_tuple_3.lin | `let x: (i32, i32, i32) = (1, 2, 3);` |
| ty_tuple_mixed.lin | `let x: (i32, &str, bool) = (1, "a", true);` |
| ty_tuple_empty.lin | `let x: () = ();` |
| ty_tuple_single.lin | `let x: (i32,) = (1,);` |
| ty_tuple_nested.lin | `let x: ((i32, i32), i32) = ((1, 2), 3);` |

### 3.7 Function pointer types (5 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_fn_ptr_basic.lin | `let f: fn(i32) -> i32 = ...;` |
| ty_fn_ptr_no_args.lin | `let f: fn() -> i32 = ...;` |
| ty_fn_ptr_no_return.lin | `let f: fn(i32) = ...;` |
| ty_fn_ptr_multi_args.lin | `let f: fn(i32, i32) -> i32 = ...;` |
| ty_fn_ptr_ref_args.lin | `let f: fn(&i32) -> i32 = ...;` |

### 3.8 Path types (5 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_path_simple.lin | `let x: MyType = ...;` |
| ty_path_qualified.lin | `let x: module::MyType = ...;` |
| ty_path_generic.lin | `let x: Vec<i32> = ...;` |
| ty_path_generic_multi.lin | `let x: HashMap<K, V> = ...;` |
| ty_path_nested.lin | `let x: Outer<Inner<i32>> = ...;` |

### 3.9 Trait object types (4 tests)

| 测试文件 | 描述 |
|---------|------|
| ty_dyn_basic.lin | `let x: dyn Trait = ...;` |
| ty_dyn_ref.lin | `let x: &dyn Trait = ...;` |
| ty_impl_basic.lin | `let x: impl Trait = ...;` (may not be supported) |
| ty_dyn_multi.lin | `let x: dyn Trait + Send = ...;` |

### 3.10 边界 & 错误恢复 (3 tests)

| 测试文件 | 描述 |
|---------|------|
| err_ty_missing.lin | `FAIL: let x: = 1;` (missing type) |
| err_ty_unclosed_array.lin | `FAIL: let x: [i32; = ...;` (unclosed array) |
| err_ty_unknown_primitive.lin | `PASS or FAIL: let x: i256 = 1;` (unknown primitive) |

**累计**: 12 + 8 + 5 + 8 + 4 + 6 + 5 + 5 + 4 + 3 = **60 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2152+ tests pass (期望 +13 verification tests = 2165)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 307 passed (247 + 60 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.3 → 0.16.4
- api-naming-standard.md: v2.07 → v2.08

---

**创建日期**: 2026-07-26
