# 17 — Conformance 测试套件规范

> 本文定义 Landin conformance 测试套件的目录结构、测试用例格式、runner 实现、通过率标准。v1.2 新增（R12 完备性审查建议）。

---

## 1. 测试套件目标

Conformance 套件的目标：

1. **门神**：stage 0 必须通过完整套件才能进入 stage 1 开发
2. **回归保护**：任何 stage 0 修改后必须仍通过全部套件
3. **行为规范**：测试用例即语言规范的可执行版本
4. **跨编译器一致性**：stage 1 重写后必须通过同一套件，保证行为一致

---

## 2. 目录结构

```
tests/
├── conformance/
│   ├── 00-parse/                          # Parse 测试（600 个）
│   │   ├── 00-literals/
│   │   │   ├── 001-integer-dec.lin
│   │   │   ├── 002-integer-hex.lin
│   │   │   ├── ...
│   │   │   └── 100-float-edge.lin
│   │   ├── 01-operators/
│   │   ├── 02-control-flow/
│   │   ├── 03-patterns/
│   │   ├── 04-types/
│   │   ├── 05-attributes/
│   │   └── 99-error-recovery/
│   ├── 01-typecheck/                      # Type check 测试（1000 个）
│   │   ├── 00-basic-inference/
│   │   ├── 01-trait-resolution/
│   │   ├── 02-generics/
│   │   ├── 03-closures/
│   │   ├── 04-lifetimes/
│   │   └── 99-error-cases/
│   ├── 02-borrowck/                       # Borrow check 测试（800 个）
│   │   ├── 00-nll-basic/
│   │   ├── 01-nll-advanced/
│   │   ├── 02-move-semantics/
│   │   ├── 03-closure-capture/
│   │   ├── 04-two-phase-borrows/
│   │   ├── 05-disjoint-captures/
│   │   └── 99-error-cases/
│   ├── 03-codegen/                        # Codegen 测试（600 个）
│   │   ├── 00-llvm-ir-output/
│   │   ├── 01-abi/
│   │   ├── 02-type-layout/
│   │   ├── 03-drop-glue/
│   │   ├── 04-vtable/
│   │   └── 99-panic-paths/
│   ├── 04-e2e/                            # End-to-End 测试（500 个）
│   │   ├── 00-hello-world/
│   │   ├── 01-fib/
│   │   ├── 02-string-ops/
│   │   ├── 03-vec-ops/
│   │   ├── 04-hashmap/
│   │   ├── 05-error-handling/
│   │   ├── 06-traits/
│   │   ├── 07-closures/
│   │   ├── 08-ffi/
│   │   ├── 09-real-world/                 # 真实程序（JSON parser 等）
│   │   └── 99-edge-cases/
│   ├── 05-soundness/                      # Soundness 测试（500 个）
│   │   ├── 00-r5-regression/              # R5 报告 7 个漏洞反例
│   │   ├── 01-rustc-soundness-holes/      # Rust 历史 soundness bug
│   │   ├── 02-drop-check/
│   │   ├── 03-lifetime-edge/
│   │   ├── 04-trait-coherence/
│   │   └── 05-unsafe-boundary/
│   ├── 06-stdlib/                         # stdlib 测试（500 个）
│   │   ├── 00-core/
│   │   ├── 01-alloc/
│   │   └── 02-std/
│   ├── 07-integration/                    # 集成测试（500 个）
│   │   ├── 00-multi-crate/
│   │   ├── 01-cross-module/
│   │   └── 02-feature-gate/
│   ├── run_all.py                         # 主 runner
│   ├── run_category.py                    # 单类别 runner
│   ├── expected/                          # 期望输出快照
│   │   └── ...
│   └── README.md                          # 套件说明
└── fuzz/
    ├── fuzz_parser.py
    ├── fuzz_typecheck.py
    └── fuzz_borrowck.py
```

总计 **5,000 测试**（v1.2 修正：与 §5.1 表格累加一致）。

---

## 3. 测试用例格式

### 3.1 Pass 测试（应编译通过）

```landin
// tests/conformance/00-parse/00-literals/001-integer-dec.lin
// CATEGORY: parse
// DESCRIPTION: Decimal integer literals
// EXPECTED: compile_ok

fn main() {
    let a: i32 = 42;
    let b: i32 = 0;
    let c: i64 = 1_000_000;
    let d: u8 = 255;
    
    assert_eq!(a, 42);
    assert_eq!(b, 0);
    assert_eq!(c, 1_000_000);
    assert_eq!(d, 255);
}
```

### 3.2 Fail 测试（应编译失败）

```landin
// tests/conformance/01-typecheck/99-error-cases/E0308-mismatched-types.lin
// CATEGORY: typecheck
// DESCRIPTION: E0308 mismatched types
// EXPECTED: compile_error
// ERROR_CODE: E0308
// ERROR_PATTERN: expected `i32`, found `&str`

fn main() {
    let x: i32 = "hello";   // E0308
}
```

### 3.3 Run 测试（应编译并运行）

```landin
// tests/conformance/04-e2e/01-fib/001-fib-30.lin
// CATEGORY: e2e
// DESCRIPTION: Recursive fib(30)
// EXPECTED: run_ok
// EXPECTED_STDOUT: 832040
// EXPECTED_EXIT_CODE: 0

fn fib(n: i64) -> i64 {
    if n < 2 { return n; }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    println!("{}", fib(30));
}
```

### 3.4 Panic 测试（应运行时 panic）

```landin
// tests/conformance/04-e2e/99-edge-cases/001-panic-oob.lin
// CATEGORY: e2e
// DESCRIPTION: Panic on out of bounds
// EXPECTED: run_panic
// PANIC_PATTERN: index out of bounds

fn main() {
    let v = vec![1, 2, 3];
    let _ = v[10];
}
```

### 3.5 MIR dump 测试

```landin
// tests/conformance/03-codegen/00-llvm-ir-output/001-arith.lin
// CATEGORY: codegen
// DESCRIPTION: Check LLVM IR for arithmetic
// EXPECTED: compile_ok
// DUMP_MIR: true
// DUMP_LLVM_IR: true
// LLVM_IR_PATTERN: add i32

fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### 3.6 Soundness 测试

```landin
// tests/conformance/05-soundness/00-r5-regression/001-nll-universal-region.lin
// CATEGORY: soundness
// DESCRIPTION: R5 #1 NLL universal region must reject UAF
// EXPECTED: compile_error
// ERROR_CODE: E0719
// REFERENCES: 14-soundness §3.1

// 此程序试图构造 use-after-free，必须被编译器拒绝
fn foo<'a, 'b>(x: &'a &'b u8) -> &'a &'b u8 { x }
// ...
```

### 3.7 Header 字段完整清单

| 字段 | 必需 | 说明 |
| --- | --- | --- |
| `CATEGORY` | ✅ | parse/typecheck/borrowck/codegen/e2e/soundness/stdlib/integration |
| `DESCRIPTION` | ✅ | 测试描述 |
| `EXPECTED` | ✅ | compile_ok / compile_error / run_ok / run_panic |
| `ERROR_CODE` | compile_error 时必需 | E0xxx |
| `ERROR_PATTERN` | compile_error 时必需 | 正则 |
| `EXPECTED_STDOUT` | run_ok 时可选 | 期望 stdout |
| `EXPECTED_STDERR` | run_ok 时可选 | 期望 stderr |
| `EXPECTED_EXIT_CODE` | run_ok 时可选 | 期望退出码 |
| `PANIC_PATTERN` | run_panic 时必需 | panic 信息正则 |
| `DUMP_MIR` | 可选 | true 时 dump MIR |
| `DUMP_LLVM_IR` | 可选 | true 时 dump LLVM IR |
| `LLVM_IR_PATTERN` | 可选 | LLVM IR 应包含的模式 |
| `MIR_PATTERN` | 可选 | MIR 应包含的模式 |
| `REFERENCES` | 可选 | 相关文档章节 |
| `ISSUE` | 可选 | rustc issue 编号（用于 soundness 测试） |
| `IGNORE` | 可选 | true 时跳过（含原因） |

---

## 4. Runner 实现

### 4.1 Python runner

```python
# tests/conformance/run_all.py
import os
import re
import subprocess
import sys
import argparse
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional

@dataclass
class TestResult:
    name: str
    passed: bool
    message: str

@dataclass
class TestCase:
    path: Path
    category: str
    description: str
    expected: str
    error_code: Optional[str]
    error_pattern: Optional[str]
    expected_stdout: Optional[str]
    expected_exit_code: Optional[int]
    panic_pattern: Optional[str]
    dump_mir: bool
    dump_llvm_ir: bool
    llvm_ir_pattern: Optional[str]
    references: Optional[str]
    ignore: Optional[str]

def parse_test(path: Path) -> TestCase:
    content = path.read_text()
    
    def extract(field: str) -> Optional[str]:
        m = re.search(rf'// {field}: (.+)', content)
        return m.group(1).strip() if m else None
    
    return TestCase(
        path=path,
        category=extract('CATEGORY') or 'unknown',
        description=extract('DESCRIPTION') or '',
        expected=extract('EXPECTED') or 'compile_ok',
        error_code=extract('ERROR_CODE'),
        error_pattern=extract('ERROR_PATTERN'),
        expected_stdout=extract('EXPECTED_STDOUT'),
        expected_exit_code=int(extract('EXPECTED_EXIT_CODE')) if extract('EXPECTED_EXIT_CODE') else None,
        panic_pattern=extract('PANIC_PATTERN'),
        dump_mir=extract('DUMP_MIR') == 'true',
        dump_llvm_ir=extract('DUMP_LLVM_IR') == 'true',
        llvm_ir_pattern=extract('LLVM_IR_PATTERN'),
        references=extract('REFERENCES'),
        ignore=extract('IGNORE'),
    )

def run_test(test: TestCase, landin_bin: str) -> TestResult:
    if test.ignore:
        return TestResult(test.path.name, True, f"ignored: {test.ignore}")
    
    # 编译
    cmd = [landin_bin, 'compile', str(test.path), '-o', '/tmp/landin_test_bin']
    if test.dump_mir:
        cmd.append('--emit mir')
    if test.dump_llvm_ir:
        cmd.append('--emit llvm-ir')
    
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    
    if test.expected == 'compile_error':
        if result.returncode == 0:
            return TestResult(test.path.name, False, "expected compile error but succeeded")
        if test.error_code and test.error_code not in result.stderr:
            return TestResult(test.path.name, False, f"expected {test.error_code} not in stderr")
        if test.error_pattern and not re.search(test.error_pattern, result.stderr):
            return TestResult(test.path.name, False, f"pattern {test.error_pattern} not found")
        return TestResult(test.path.name, True, "ok")
    
    if test.expected == 'compile_ok':
        if result.returncode != 0:
            return TestResult(test.path.name, False, f"compile failed: {result.stderr}")
        return TestResult(test.path.name, True, "ok")
    
    if test.expected == 'run_ok':
        if result.returncode != 0:
            return TestResult(test.path.name, False, f"compile failed: {result.stderr}")
        run_result = subprocess.run(['/tmp/landin_test_bin'], capture_output=True, text=True, timeout=10)
        if test.expected_stdout and run_result.stdout.strip() != test.expected_stdout:
            return TestResult(test.path.name, False, f"stdout mismatch: got {run_result.stdout!r}, expected {test.expected_stdout!r}")
        if test.expected_exit_code is not None and run_result.returncode != test.expected_exit_code:
            return TestResult(test.path.name, False, f"exit code: got {run_result.returncode}, expected {test.expected_exit_code}")
        return TestResult(test.path.name, True, "ok")
    
    if test.expected == 'run_panic':
        if result.returncode != 0:
            return TestResult(test.path.name, False, f"compile failed: {result.stderr}")
        run_result = subprocess.run(['/tmp/landin_test_bin'], capture_output=True, text=True, timeout=10)
        if run_result.returncode == 0:
            return TestResult(test.path.name, False, "expected panic but ran successfully")
        if test.panic_pattern and not re.search(test.panic_pattern, run_result.stderr):
            return TestResult(test.path.name, False, f"panic pattern {test.panic_pattern} not found")
        return TestResult(test.path.name, True, "ok")
    
    return TestResult(test.path.name, False, f"unknown EXPECTED: {test.expected}")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--landin', default='landin')
    parser.add_argument('--category', default=None)
    parser.add_argument('--verbose', '-v', action='store_true')
    args = parser.parse_args()
    
    conformance_dir = Path('tests/conformance')
    tests = []
    
    for landin_file in conformance_dir.rglob('*.lin'):
        test = parse_test(landin_file)
        if args.category and test.category != args.category:
            continue
        tests.append(test)
    
    results = []
    for test in tests:
        result = run_test(test, args.lin)
        results.append(result)
        if args.verbose or not result.passed:
            status = "PASS" if result.passed else "FAIL"
            print(f"{status}: {test.path} - {result.message}")
    
    passed = sum(1 for r in results if r.passed)
    failed = sum(1 for r in results if not r.passed)
    print(f"\nResults: {passed} passed, {failed} failed, {len(results)} total")
    
    sys.exit(0 if failed == 0 else 1)

if __name__ == '__main__':
    main()
```

### 4.2 Rust 单元测试（per-pass）

每个 pass（lexer/parser/typeck 等）有自己的 Rust 单元测试，直接调用 pass 函数：

```rust
// 在 landin-lexer/src/lib.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_integer_literals() {
        let cases = vec![
            ("42", 42i128),
            ("0xff", 255),
            ("0b1010", 10),
            ("1_000_000", 1000000),
            ("42i64", 42),
        ];
        for (src, expected) in cases {
            let tokens = lex(src);
            assert_eq!(tokens.len(), 1);
            assert!(matches!(tokens[0].kind, 
                TokenKind::Literal(LitKind::Int(v, _)) if v == expected as u128));
        }
    }
}
```

Rust 单元测试与 conformance 测试互补：单元测试快速验证 pass 内部，conformance 测试验证端到端行为。

---

## 5. 通过率标准

### 5.1 Stage 0 通过标准

| 类别 | 测试数 | 通过率要求 |
| --- | --- | --- |
| 00-parse | 600 | 100% |
| 01-typecheck | 1000 | 100% |
| 02-borrowck | 800 | 100% |
| 03-codegen | 600 | 100% |
| 04-e2e | 500 | 100% |
| 05-soundness | 500 | 100% |
| 06-stdlib | 500 | 100% |
| 07-integration | 500 | 100% |
| **合计** | **5,000** | **100%** |

任何 < 100% 的类别阻塞 stage 1 开发。

### 5.2 Stage 1 通过标准

Stage 1 必须通过 stage 0 的同一套件（行为一致性）。

### 5.3 Fuzzing 标准

- 每夜运行 8 小时 fuzzing
- 不应有 crash（panic = ICE = bug）
- 不应有 false positive（sound 程序被错误拒绝）

---

## 6. 测试用例编写规范

### 6.1 命名规范

```
<EOMETRY>-<description>.lin
```

例：`001-integer-dec.lin`、`E0308-mismatched-types.lin`

### 6.2 Header 规范

每个测试文件**必须**含完整 header（见 §3.7）。

### 6.3 一个测试一个特性

每个测试聚焦**单一特性**或**单一错误**。多特性测试拆为多个测试。

### 6.4 错误测试需含修复版本

```landin
// E0308-mismatched-types.lin
// EXPECTED: compile_error
fn main() {
    let x: i32 = "hello";
}

// E0308-mismatched-types-fixed.lin
// EXPECTED: compile_ok
fn main() {
    let x: &str = "hello";
}
```

### 6.5 Soundness 测试需含引用

每个 soundness 测试必须 `REFERENCES` 字段指向 14-soundness 章节或 rustc issue。

---

## 7. 测试覆盖率

### 7.1 Coverage 工具

使用 `llvm-cov` 收集覆盖率：

```bash
landinc build --coverage
landinc test
llvm-cov show target/debug/landin -instr-profile=landin.profdata > coverage.txt
```

### 7.2 覆盖率目标

| 组件 | 目标覆盖率 |
| --- | --- |
| Lexer | 95% |
| Parser | 90% |
| Type checker | 85% |
| Borrow checker | 85% |
| Codegen | 75% |
| 标准库 | 90% |
| 错误诊断 | 80% |

### 7.3 PR 验收

PR 不强制覆盖率提升，但不应降低现有覆盖率。

---

## 8. CI 集成

### 8.1 GitHub Actions

```yaml
name: Conformance
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build landin
        run: cd stage0 && cargo build --release
      - name: Run conformance
        run: python3 tests/conformance/run_all.py --landin target/release/landin
      - name: Run unit tests
        run: cd stage0 && cargo test --release
      - name: Upload coverage
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: coverage
          path: coverage.txt

  fuzz:
    runs-on: ubuntu-latest
    if: github.event.schedule == '0 0 * * *'
    steps:
      - uses: actions/checkout@v3
      - name: Run fuzzing
        run: python3 tests/fuzz/run_all.py --duration 8h
```

### 8.2 多平台矩阵

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
```

---

## 9. 测试用例贡献流程

### 9.1 添加测试

1. 在对应类别目录下创建 `.lin` 文件
2. 编写 header（必填字段）
3. 编写测试代码
4. 本地运行 `python3 tests/conformance/run_all.py --category <cat>`
5. 提交 PR

### 9.2 测试评审

PR 必须包含：

- 测试本身
- 测试描述（DESCRIPTION）
- 期望行为说明
- 若是 soundness 测试，需引用相关文档

### 9.3 测试废弃

废弃测试需 `IGNORE` 字段说明原因：

```landin
// IGNORE: v0.2 will redesign this feature
```

---

**下一文档**: [`18-glossary.md`](./18-glossary.md) — 术语表
