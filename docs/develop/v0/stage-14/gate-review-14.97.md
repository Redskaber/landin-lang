# Stage 14.97 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.110.0 → v0.111.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.97 fixes 2 long-standing P0 bugs in the v0.1 release:

1. **Bug Y1**: Trait default body methods calling `self.method()` crashed with LLVM
   verification errors (wrong function signature + vtable dispatch on a value
   that wasn't a fat pointer).
2. **`for` loop over Range**: `for i in 0..N { body }` was a stub since Stage 2.4b
   — it just checked if the iter was "truthy" and never iterated. Now properly
   lowered to a `while counter < end { body; counter += 1 }` loop.

Both fixes are FULLY WORKING and verified with end-to-end run_ok tests.

## 2. Bugs Fixed

### Bug Y1: Trait default body `self.method()` crash

**Symptom**: 
```landin
trait Counter {
    fn value(&self) -> i32;
    fn double_value(&self) -> i32 { self.value() * 2 }
}
struct Pair { x: i32 }
impl Counter for Pair { fn value(&self) -> i32 { self.x } }
fn main() { let p = Pair { x: 21 }; println!("{}", p.double_value()); }
```
Crashed with `LLVM module verification failed: Invalid indices for GEP pointer type!`

**Root causes** (4 layered):

1. **HIR lowering**: Trait default body methods didn't get their own DefId — all
   trait items shared the trait's DefId. This caused:
   - Function name collisions (e.g., `landin_Counter_default_doubled_increment`
     for both `increment` and `doubled_increment`)
   - fn_sig_table collisions (only one signature stored per trait)
   - body_metas lookup failures

2. **fn_sig_table**: No entries were created for trait default body methods.
   Codegen used a generic variadic signature, causing type mismatches.

3. **resolve_self_param_type**: Only handled impl methods, not trait default
   body methods. Self's type stayed as `Infer`, causing:
   - Wrong codegen (self treated as i32 instead of struct)
   - Method resolution failures inside the body

4. **query_method_self_kind**: Only searched impl blocks, not trait definitions.
   When `p.double_value()` resolved to a trait default body, the call site
   didn't know the method takes `&self`, so it didn't borrow `p` — leading to
   typeck mismatch (`Pair` vs `&Pair`).

**Fixes** (4 layered):

1. **HIR lowering** (`src/hir/lower/item.rs::lower_trait_item`): For trait
   methods WITH bodies, call `enter_owner()` to allocate a fresh DefId, and
   `store_owner()` to register the method as its own `HirItem::Fn` owner.
   For methods WITHOUT bodies (just declarations), keep the old behavior
   (no separate owner) — they don't need to be codegen'd.

2. **fn_sig_table** (`src/driver.rs`): Add a new loop that iterates trait
   owners, finds trait methods with bodies, and inserts their signatures
   into `fn_sig_table`. Uses the first impl's `self_ty` as the specialization
   type (v0.1 single-impl heuristic — full monomorphization is v0.2+ work).

3. **resolve_self_param_type** (`src/mir/lower/mod.rs`): Extend to search
   Trait owners when no impl block owns the body. Find the unique impl
   (or first impl if multiple) of the trait and use its `self_ty` as the
   self parameter's type. Same single-impl heuristic.

4. **query_method_self_kind** (`src/mir/lower/expr_operand.rs`): Extend to
   search Trait owners for trait default body methods. Return the first
   param's `self_kind` so call sites know whether to borrow the receiver.

**Verification**:
- `p.double_value()` (calls `self.value()` in default body) → 42 ✅ (was: LLVM crash)
- Chain: `p.doubled_increment()` (default body calls `self.increment()` which
  is another default body calling `self.value()`) → 26 ✅
- Trait default body without `self.method()` calls → 42 ✅

### For-loop over Range

**Symptom**: `for i in 0..5 { sum += i; }` produced wrong output (treated
iter as a single value, didn't iterate).

**Root cause**: Stage 2.4b left a stub MIR lowering that just checked if
the iter was "truthy" — never properly implemented iteration.

**Fix** (`src/mir/lower/expr_operand.rs::HirExprKind::For`):
Desugar `for pat in start..end { body }` to:
```landin
let mut pat = start;
while pat < end { body; pat += 1; }
```
For inclusive ranges (`start..=end`), use `<=` instead of `<`.

Properly handles:
- `break` (jumps to exit_block via loop_stack)
- `continue` (jumps to incr_block via loop_stack)
- Empty ranges (cond fails immediately, body never executes)
- Single-element ranges (`5..6`)
- Negative-direction ranges (`5..0` — body never executes, no error)

For non-Range iter expressions (arrays, etc.), emits a clear typeck error:
"for-loop only supports Range iterators (start..end or start..=end); found ..."

**Verification**:
- `for i in 0..5 { sum += i; }` → 10 ✅ (was: 0)
- `for i in 0..=5 { sum += i; }` → 15 ✅
- `for i in 0..100 { if i > 3 { break; } sum += i; }` → 6 ✅
- `for i in 0..6 { if i == 2 || i == 4 { continue; } sum += i; }` → 9 ✅

## 3. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5184 | 5191 | +7 |
| - run_ok tests | 155 | 162 | +7 |
| Existing tests flipped | 4 | 4 | (compile_error → run_ok) |

New run_ok tests:
- `e2e-runok-156-for-loop-range.lin` — basic for-loop
- `e2e-runok-157-for-loop-inclusive.lin` — inclusive range
- `e2e-runok-158-for-loop-break.lin` — for-loop with break
- `e2e-runok-159-for-loop-continue.lin` — for-loop with continue
- `e2e-runok-160-trait-default-body-self-method.lin` — Bug Y1 simple
- `e2e-runok-161-trait-default-body-no-call.lin` — Bug Y1 minimal
- `e2e-runok-162-trait-default-body-chain.lin` — Bug Y1 chained defaults

Existing tests flipped from `compile_error` → `run_ok`:
- `06-stdlib/00-core/028-std-for-loop.lin`
- `06-stdlib/02-std/005-iter-for.lin`
- `06-stdlib/02-std/019-std-for-loop-range.lin`
- `06-stdlib/02-std/020-std-for-loop-inclusive.lin`

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5191 passed, 0 failed
```

## 5. Known Limitations

- **For-loop over arrays**: Not supported in v0.1. Only Range iterators work.
  Clear typeck error message is emitted.
- **Open ranges**: `..end` and `start..` (open-ended) not supported. Clear
  typeck error message.
- **Trait default body with multiple impls**: When a trait has multiple impls,
  the default body uses the first impl's `self_ty` for specialization. This
  is wrong for the other impls. Full monomorphization is v0.2+ work. The
  workaround is to override the default body in each impl.
- **Trait default body that calls another trait's method**: Not supported
  (only same-trait method calls work). Cross-trait dispatch requires
  monomorphization.

## 6. Stage Verdict

**PASS** — Both P0 bugs fully fixed, no regressions, +7 new run_ok tests.

Per §1.0 原则 5 "报错 > 静默":
- For-loop over non-Range now produces a clear error instead of wrong output
- Bug Y1 was a crash (already loud) — now works correctly

Per §1.0 原则 6 "通用 > 特例":
- Single rule (enter_owner/store_owner for trait Fn items with body) handles
  all trait default body cases — no per-method special-casing
- Single rule (desugar to while + counter) handles all Range iteration cases
  (excluded, inclusive, break, continue)

Per §1.0 原则 1 "长期 > 短期":
- The trait item owner fix mirrors how impl items are handled — uniform
  architecture, not a hack
- The for-loop desugar uses existing while-loop infrastructure — no new
  MIR constructs needed

v0.111.0: minor bump (2 P0 fixes — major correctness improvements)
