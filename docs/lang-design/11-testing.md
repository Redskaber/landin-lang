# 11 — 测试策略

> 本文定义 Landin 项目的测试金字塔、conformance 套件、fuzzing、CI 流程。测试是自举验证的基础——stage 0 必须通过完整 conformance 才能进入 stage 1 开发。

---

## 1. 测试金字塔

**v1.2 修正**（基于 R8 报告）：测试数量从 v1.0 的 950 扩展到 3,000-5,000，新增 soundness 测试类别。

```
                  ┌───────────────┐
                  │  End-to-End   │  ~500 个：编译运行真实小程序
                  └───────────────┘
                ┌───────────────────┐
                │  Integration      │  ~500 个：跨 pass 集成
                └───────────────────┘
              ┌───────────────────────────┐
              │  Unit / Conformance       │  ~2,000 个：每个 pass 单独测试
              └───────────────────────────┘
            ┌───────────────────────────────────┐
            │  Soundness + Fuzzing              │  ~500 + 持续：自动生成
            └───────────────────────────────────┘
```

| 层级 | 数量 | 目的 | 运行频率 |
| --- | --- | --- | --- |
| Unit / Conformance | ~2,000 | 验证单 pass 正确性 | 每次 commit |
| Integration | ~500 | 验证 pass 间协作 | 每次 commit |
| End-to-End | ~500 | 验证完整程序行为 | 每次 commit |
| Soundness | ~500 | 验证 R5 找出的 7 个漏洞反例 + rustc soundness hole | 每次 commit |
| Fuzzing | 持续 | 发现边界 case | 每夜 CI |

---

## 2. Conformance 套件

Conformance 套件是 Landin 自举的"门神"——stage 0 必须通过完整套件才能开始 stage 1 开发。

### 2.1 测试组织

```
tests/
├── conformance/
│   ├── 00-parse/
│   │   ├── 001-literals.lin
│   │   ├── 002-operators.lin
│   │   ├── ...
│   │   └── 200-...
│   ├── 01-typecheck/
│   │   ├── 001-basic-inference.lin
│   │   ├── ...
│   │   └── 300-...
│   ├── 02-borrowck/
│   │   ├── 001-nll-basic.lin
│   │   ├── ...
│   │   └── 200-...
│   ├── 03-codegen/
│   │   ├── 001-arith.lin
│   │   ├── ...
│   │   └── 150-...
│   ├── 04-e2e/
│   │   ├── 001-hello-world.lin
│   │   ├── 002-fib.lin
│   │   ├── ...
│   │   └── 100-...
│   └── run_all.py
└── fuzz/
    ├── fuzz_parser.py
    ├── fuzz_typecheck.py
    └── fuzz_borrowck.py
```

### 2.2 测试用例格式

每个 conformance 测试是一个 `.lin` 文件，含：

- 程序源码
- 期望输出（注释形式）
- 期望错误（可选）

```landin
// tests/conformance/04-e2e/002-fib.lin
// EXPECTED_STDOUT: 832040
// EXPECTED_EXIT: 0

fn fib(n: i32) -> i32 {
    if n < 2 { return n; }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    let result = fib(30);
    println(result.to_string().as_str());
}
```

```landin
// tests/conformance/02-borrowck/001-nll-basic.lin
// EXPECTED_ERROR: E0502
// ERROR_PATTERN: cannot borrow .* as mutable because it is also borrowed as immutable

fn main() {
    let mut v = vec![1, 2, 3];
    let r = &v[0];
    println(r);
    v.push(4);    // ERROR: v is borrowed
}
```

### 2.3 Runner 实现

```python
# tests/conformance/run_all.py
import os
import re
import subprocess
import sys

def run_test(test_file):
    source = open(test_file).read()
    
    expected_stdout = re.search(r'// EXPECTED_STDOUT: (.+)', source)
    expected_exit = re.search(r'// EXPECTED_EXIT: (\d+)', source)
    expected_error = re.search(r'// EXPECTED_ERROR: (\w+)', source)
    
    # 编译
    result = subprocess.run(
        ['landin', 'compile', test_file, '-o', '/tmp/test_bin'],
        capture_output=True, text=True
    )
    
    if expected_error:
        if result.returncode == 0:
            return False, "expected error but compile succeeded"
        if expected_error.group(1) not in result.stderr:
            return False, f"expected error {expected_error.group(1)} not found"
        return True, "ok"
    
    if result.returncode != 0:
        return False, f"compile failed: {result.stderr}"
    
    # 运行
    run_result = subprocess.run(['/tmp/test_bin'], capture_output=True, text=True, timeout=10)
    
    if expected_stdout and run_result.stdout.strip() != expected_stdout.group(1):
        return False, f"stdout mismatch: got {run_result.stdout!r}, expected {expected_stdout.group(1)!r}"
    
    if expected_exit and run_result.returncode != int(expected_exit.group(1)):
        return False, f"exit code mismatch: got {run_result.returncode}, expected {expected_exit.group(1)}"
    
    return True, "ok"

def main():
    test_dir = 'tests/conformance'
    passed = 0
    failed = 0
    failures = []
    
    for root, dirs, files in os.walk(test_dir):
        for f in files:
            if f.endswith('.lin'):
                test_file = os.path.join(root, f)
                ok, msg = run_test(test_file)
                if ok:
                    passed += 1
                else:
                    failed += 1
                    failures.append((test_file, msg))
    
    print(f"passed: {passed}, failed: {failed}")
    for f, msg in failures:
        print(f"  FAIL {f}: {msg}")
    
    sys.exit(0 if failed == 0 else 1)

if __name__ == '__main__':
    main()
```

---

## 3. 各 pass 的单元测试

### 3.1 Lexer 测试

```rust
#[test]
fn test_integer_literals() {
    let cases = vec![
        ("42", 42i128),
        ("0xff", 255),
        ("0b1010", 10),
        ("1_000_000", 1000000),
        ("42i64", 42),
        ("-42", -42),    // 负号作为一元 op，不在此测试
    ];
    for (src, expected) in cases {
        let tokens = lex(src);
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, TokenKind::Literal(LitKind::Int(v, _)) if v == expected as u128));
    }
}
```

### 3.2 Parser 测试

```rust
#[test]
fn test_if_expr() {
    let src = "if x > 0 { 1 } else { 2 }";
    let ast = parse(src);
    assert!(matches!(ast, Ast::Expr(Expr::If { .. })));
}

#[test]
fn test_precedence() {
    let src = "1 + 2 * 3";
    let ast = parse(src);
    // 应解析为 1 + (2 * 3)
    match ast {
        Ast::Expr(Expr::Binary { op: BinOp::Add, rhs, .. }) => {
            assert!(matches!(*rhs, Expr::Binary { op: BinOp::Mul, .. }));
        }
        _ => panic!("expected Add at top"),
    }
}
```

### 3.3 Type check 测试

```rust
#[test]
fn test_unify_int_i32() {
    // i32 应与 i32 unify
    let ty1 = Ty::Int(IntTy::I32);
    let ty2 = Ty::Infer(var(1));
    unify(ty1, ty2);
    assert_eq!(infer_var_resolution(1), Some(Ty::Int(IntTy::I32)));
}

#[test]
fn test_int_fallback() {
    // let x = 42; 应 fallback 到 i32
    let src = "fn f() { let x = 42; }";
    let hir = lower(parse(src));
    let ty = type_check(hir);
    assert_eq!(ty_of("x"), Ty::Int(IntTy::I32));
}

#[test]
fn test_trait_resolution() {
    // i32: Display 应能 resolve
    let src = "fn f<T: Display>(x: T) { x.fmt(); }";
    let hir = lower(parse(src));
    let result = type_check(hir);
    assert!(result.errors.is_empty());
}
```

### 3.4 Borrow check 测试

```rust
#[test]
fn test_nll_basic() {
    // NLL 应让此代码通过
    let src = r#"
        fn main() {
            let mut v = vec![1, 2, 3];
            let r = &v[0];
            println!("{}", r);
            v.push(4);  // OK with NLL
        }
    "#;
    let result = borrow_check(lower(parse(src)));
    assert!(result.errors.is_empty());
}

#[test]
fn test_double_mut_borrow() {
    let src = r#"
        fn main() {
            let mut v = vec![1];
            let r1 = &mut v;
            let r2 = &mut v;  // ERROR
            r1.push(1);
            r2.push(2);
        }
    "#;
    let result = borrow_check(lower(parse(src)));
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].code.starts_with("E05"));
}
```

### 3.5 Codegen 测试

```rust
#[test]
fn test_arith_codegen() {
    let src = "fn f(a: i32, b: i32) -> i32 { a + b }";
    let mir = compile_to_mir(src);
    let llvm_ir = codegen(mir);
    assert!(llvm_ir.contains("add i32"));
}

#[test]
fn test_bounds_check_codegen() {
    let src = r#"
        fn f(v: &[i32], i: usize) -> i32 { v[i] }
    "#;
    let mir = compile_to_mir(src);
    let llvm_ir = codegen(mir);
    assert!(llvm_ir.contains("__landin_panic_bounds_check"));
}
```

---

## 4. End-to-End 测试

### 4.1 经典程序测试

```
tests/e2e/
├── 001-hello-world.lin
├── 002-fib.lin
├── 003-factorial.lin
├── 004-string-reverse.lin
├── 005-vec-operations.lin
├── 006-hashmap-usage.lin
├── 007-file-io.lin
├── 008-error-propagation.lin
├── 009-trait-objects.lin
├── 010-closures.lin
├── 011-generics-deep.lin
├── 012-pattern-matching.lin
├── 013-recursive-types.lin
├── 014-ffi-c.lin
├── 015-iterators.lin
├── 016-box-vec-string.lin
├── 017-result-option.lin
├── 018-cell-refcell.lin
├── 019-rc-arc.lin        (rc only in MVP)
├── 020-drop-order.lin
├── ...
└── 100-real-world-json-parser.lin
```

### 4.2 真实世界测试

最后 10 个 e2e 测试用真实程序：

- JSON parser（500 行）
- HTTP client（200 行，仅 GET）
- Markdown to HTML（300 行）
- 简易 shell（200 行）
- Brainfuck 解释器（100 行）
- 正则引擎（500 行，子集）
- Tetris 游戏（800 行，终端）
- 链表 / 树 / 图数据结构（500 行）
- 加密库（AES，500 行）
- 编译器子集（500 行）

每个真实程序测试覆盖多个语言特性组合。

---

## 5. Fuzzing

### 5.1 Parser fuzzing

```python
# tests/fuzz/fuzz_parser.py
import hypothesis.strategies as st
from hypothesis import given

@given(st.text(alphabet="abcdefghijklmnopqrstuvwxyz (){}[];,.0123456789+-*/", min_size=1, max_size=100))
def test_parser_does_not_crash(s):
    """Parser 不应 crash，可报错但必须返回 AST。"""
    try:
        result = parse(s)
        assert result is not None
    except LandinParseError:
        pass    # 合法报错
    except Exception as e:
        assert False, f"parser crashed on {s!r}: {e}"
```

### 5.2 Type system fuzzing

```python
@given(st.lists(st.sampled_from(["i32", "u64", "bool", "&str", "Vec<i32>", "f64"])))
def test_type_inference_terminates(types):
    """类型推导必须在 depth limit 内终止。"""
    src = generate_program_with_types(types)
    result = type_check(parse(src), timeout=5)
    assert result.terminated
```

### 5.3 Borrow check fuzzing

```python
@given(borrow_check_program_strategy)
def test_borrowck_terminates(program):
    """Borrow check 必须在合理时间内完成。"""
    result = borrow_check(parse(program), timeout=10)
    assert result.terminated
```

### 5.4 持续 fuzzing

CI 每夜运行 fuzzing 8 小时，发现 crash 自动建 issue。

---

## 6. 性能基准

### 6.1 编译速度基准

```bash
# tests/benchmarks/compile_time/
```

| 基准 | 期望（stage 0） | 期望（stage 1） |
| --- | --- | --- |
| Hello world | < 0.5s | < 1s |
| Fib (30 行) | < 1s | < 2s |
| Vec 操作（100 行） | < 2s | < 4s |
| JSON parser（500 行） | < 5s | < 10s |
| Stage 1 自编译（30k 行） | < 60s | < 120s |

### 6.2 运行时性能基准

```landin
// tests/benchmarks/runtime/fib.lin
fn fib(n: i64) -> i64 {
    if n < 2 { return n; }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    let start = std::time::Instant::now();   // v0.2
    let result = fib(40);
    let elapsed = start.elapsed();
    println!("fib(40) = {} in {:?}", result, elapsed);
}
```

预期：Landin `fib(40)` 与 Rust `fib(40)` 性能差距 < 5%（同样使用 LLVM O2）。

### 6.3 二进制大小基准

| 程序 | Hello world | Fib | JSON parser |
| --- | --- | --- | --- |
| Landin (debug) | < 500 KB | < 500 KB | < 700 KB |
| Landin (release) | < 100 KB | < 100 KB | < 200 KB |
| Rust (debug) | ~3.5 MB | ~3.5 MB | ~3.7 MB |
| Rust (release) | ~300 KB | ~300 KB | ~400 KB |
| C (gcc -O2) | < 20 KB | < 20 KB | < 50 KB |

Landin 目标：release 二进制比 Rust 小 2x（因 std 库小），比 C 大 3-5x（含 panic runtime + allocator）。

---

## 7. CI 流程

### 7.1 GitHub Actions 配置

```yaml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:

jobs:
  test-stage0:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build stage 0
        run: cd stage0 && cargo build --release
      - name: Run unit tests
        run: cd stage0 && cargo test --release
      - name: Run conformance
        run: python3 tests/conformance/run_all.py
      - name: Run e2e
        run: python3 tests/e2e/run_all.py
  
  test-bootstrap:
    runs-on: ubuntu-latest
    needs: test-stage0
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v3
      - name: Bootstrap from scratch
        run: |
          # 1. Build stage 0 from source（v1.2 修正：不再用 bitcode）
          cd stage0 && cargo build --release
          cd .. && cp stage0/target/release/landin-stage0 ./landin-stage0
          
          # 2. Compile stage 1
          ./landin-stage0 compile landin-compiler/ -o landin-stage1
          
          # 3. Verify self-bootstrap
          ./landin-stage1 compile landin-compiler/ -o landin-stage2
          ./landin-stage2 compile landin-compiler/ -o landin-stage2-verify
          
          # 4. Check bit-stability
          diff landin-stage2 landin-stage2-verify
  
  fuzz:
    runs-on: ubuntu-latest
    if: github.event.schedule == '0 0 * * *'    # nightly
    steps:
      - uses: actions/checkout@v3
      - name: Run fuzzing
        run: python3 tests/fuzz/run_all.py --duration 8h
```

### 7.2 多平台测试矩阵

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    include:
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
      - os: ubuntu-latest
        target: aarch64-unknown-linux-gnu
      - os: macos-latest
        target: x86_64-apple-darwin
      - os: macos-latest
        target: aarch64-apple-darwin
      - os: windows-latest
        target: x86_64-pc-windows-msvc
```

---

## 8. 测试覆盖率

### 8.1 目标

| 组件 | 覆盖率目标 |
| --- | --- |
| Lexer | 95% |
| Parser | 90% |
| Type checker | 85% |
| Borrow checker | 85% |
| Codegen | 75% |
| 标准库 | 90% |

### 8.2 工具

使用 `llvm-cov`（v0.2 集成）：

```bash
landinc build --coverage
landinc test
llvm-cov show target/debug/myapp -instr-profile=myapp.profdata
```

MVP 不强制覆盖率，但 PR 鼓励不低于项目平均。

---

## 9. 错误信息质量测试

错误信息是用户体验关键。MVP 加 **错误信息快照测试**：

```rust
#[test]
fn test_borrow_error_message() {
    let src = r#"
        fn main() {
            let mut v = vec![1];
            let r = &v;
            v.push(2);
            println!("{}", r);
        }
    "#;
    let result = compile(src);
    let expected = r#"error[E0502]: cannot borrow `v` as mutable because it is also borrowed as immutable
 --> <unknown>:4:13
  |
3 |         let r = &v;
  |                  - immutable borrow occurs here
4 |         v.push(2);
  |         ^^^^^^^^^ mutable borrow occurs here
5 |         println!("{}", r);
  |                          - immutable borrow later used here
"#;
    assert_eq!(result.stderr, expected);
}
```

错误信息变更必须显式更新快照，避免静默退化。

---

## 10. 自举验证测试

### 10.1 三阶段验证

```bash
# 1. stage0 编译 stage1 源码
./landin-stage0 compile landin-compiler/ -o landin-stage1

# 2. stage1 自编译
./landin-stage1 compile landin-compiler/ -o landin-stage2

# 3. stage2 自编译
./landin-stage2 compile landin-compiler/ -o landin-stage3

# 4. 验证 stage2 与 stage3 行为一致
./landin-stage2 test tests/conformance/ > /tmp/s2.txt
./landin-stage3 test tests/conformance/ > /tmp/s3.txt
diff /tmp/s2.txt /tmp/s3.txt
```

### 10.2 Bit-stability

```bash
# 编译两次，二进制应一致（或仅时间戳差异）
./landin-stage1 compile landin-compiler/ -o landin-stage2a
./landin-stage1 compile landin-compiler/ -o landin-stage2b

sha256sum landin-stage2a landin-stage2b
```

LLVM 版本变化时可能不一致，需用相同 LLVM 重新构建 stage0。

### 10.3 干净环境测试

```dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y rustc cargo llvm lld python3
COPY . /work
WORKDIR /work
RUN cd stage0 && cargo build --release && \
    cp target/release/landin-stage0 ../landin-stage0 && \
    cd .. && \
    ./landin-stage0 compile landin-compiler/ -o landin-stage1 && \
    ./landin-stage1 compile landin-compiler/ -o landin-stage2 && \
    ./landin-stage2 test tests/conformance/
```

成功 = 自举完成（v1.2 修正：不再用 LLVM bitcode，改用源码重建）。

---

**下一文档**: [`12-roadmap.md`](./12-roadmap.md) — 路线图
