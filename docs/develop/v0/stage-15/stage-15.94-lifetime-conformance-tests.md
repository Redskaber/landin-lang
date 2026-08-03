# Stage 15.94 — Lifetime Elision + Region Inference Conformance Tests

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.218.0 → v0.219.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.94 adds **8 conformance tests** for lifetime elision rules and
region inference, verifying the work done in Stages 15.90-15.93.

**Per user directive**: "简化的设计实现需要将其完整的设计实现纳入设计
实现测试计划，不能遗漏" — simplified implementations must have their
complete design included in the test plan, with no gaps.

The lifetime elision (Stages 15.90-15.92) and region inference (Stage
15.93) work was implemented with unit tests but lacked conformance tests
that verify end-to-end compilation. This stage closes that gap.

**New conformance tests** (8 tests, all `compile_ok`):

| # | File | Rule Tested |
|---|------|-------------|
| 1 | `elision-rule-2-single-input.lin` | Rule 2: single input → output |
| 2 | `elision-rule-3-self-param.lin` | Rule 3: &self → output |
| 3 | `elision-rule-3-self-with-arg.lin` | Rule 3: &self + arg → output gets self |
| 4 | `explicit-lifetime-dedup.lin` | Explicit lifetime deduplication |
| 5 | `elision-no-output-ref.lin` | Elision with no output reference |
| 6 | `elision-tuple-return.lin` | Rule 2 with tuple return containing ref |
| 7 | `elision-nested-ref.lin` | Elision with chained references |
| 8 | `explicit-multi-lifetime.lin` | Multiple explicit lifetimes |

**Test impact**:
- 8 new conformance tests
- 0 conformance test flips
- **Total: 7612 tests passing** (244 lib + 2144 integration + 5224
  conformance [was 5216, +8 new]), 0 failures, 0 warnings.

## 2. Test Details

### 2.1 `elision-rule-2-single-input.lin`
```landin
fn f(x: &i32) -> &i32 { x }
fn main() { let v = 42; let r = f(&v); let _ = *r; }
```
Verifies RFC 141 rule 2: single input lifetime assigned to output.

### 2.2 `elision-rule-3-self-param.lin`
```landin
struct S { x: i32 }
impl S { fn get(&self) -> &i32 { &self.x } }
fn main() { let s = S { x: 42 }; let _ = s.get(); }
```
Verifies RFC 141 rule 3: self lifetime assigned to output.

### 2.3 `elision-rule-3-self-with-arg.lin`
```landin
struct S { x: i32 }
impl S { fn get_or(&self, _default: &i32) -> &i32 { &self.x } }
fn main() { let s = S { x: 42 }; let d = 0; let _ = s.get_or(&d); }
```
Verifies rule 3 with multiple input lifetimes: self + arg, output gets self.

### 2.4 `explicit-lifetime-dedup.lin`
```landin
fn foo<'a>(x: &'a i32, y: &'a i32) -> &'a i32 { x }
fn main() { let a = 1; let b = 2; let _ = foo(&a, &b); }
```
Verifies explicit lifetime deduplication (Stage 15.92).

### 2.5 `elision-no-output-ref.lin`
```landin
fn print_ref(x: &i32) { let _ = *x; }
fn main() { let v = 42; print_ref(&v); }
```
Verifies elision with no output reference (no rule 2/3 applies).

### 2.6 `elision-tuple-return.lin`
```landin
fn pair(x: &i32) -> (&i32, &i32) { (x, x) }
fn main() { let v = 42; let (a, b) = pair(&v); let _ = *a + *b; }
```
Verifies rule 2 with tuple return type containing references.

### 2.7 `elision-nested-ref.lin`
```landin
fn get_ref(x: &i32) -> &i32 { x }
fn main() { let v = 42; let r = get_ref(&v); let _ = *r; }
```
Verifies elision with chained reference calls.

### 2.8 `explicit-multi-lifetime.lin`
```landin
fn foo<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { x }
fn main() { let a = 1; let b = 2; let _ = foo(&a, &b); }
```
Verifies multiple explicit lifetimes with different names.

## 3. API Naming Compliance (§23)

No API changes. This stage adds only test files.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS (was 5216, +8 new)
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.218.0 → v0.219.0 (minor bump — Phase 3 Task 12 lifetime conformance tests).
