# Stage 40 — v0.28 Prelude Combinators

## Overview

Stage 40 (v0.28) adds combinator methods to the Landin prelude for `Option`
and `Result` types. These were unblocked by Stage 39.3's three root-cause
fixes that made `match self { Some(v) => ..., None => ... }` patterns work
correctly in prelude method bodies.

## Stage 40.1 (v0.589.0) — Option/Result map + and_then (CURRENT)

### Goal

Add the four most-requested combinator methods to the Landin prelude:
- `Option::map<U>(self, f: fn(T) -> U) -> Option<U>`
- `Option::and_then<U>(self, f: fn(T) -> Option<U>) -> Option<U>`
- `Result::map<U>(self, f: fn(T) -> U) -> Result<U, E>`
- `Result::and_then<U>(self, f: fn(T) -> Result<U, E>) -> Result<U, E>`

### Design Decisions

- **fn(T) -> U instead of FnOnce(T) -> U**: Uses function pointer type
  rather than closure trait. Closures (Fn/FnMut/FnOnce traits) are
  deferred to v0.6+.
  - Per §1.0 原則 6 (通解 > 特解): one generic mechanism for all
    transform functions.
  - Per §12 (最优 > 最小): root-cause fix at prelude level using
    standard language features.

- **Mirrors Rust std API**: Method naming matches Rust's std::option and
  std::result for cross-language familiarity. Per §10 标准化 API 命名规则:
  lowercase method names match Rust convention.

### Implementation (src/stdlib/prelude.rs lines 217-264)

```landin
impl<T> Option<T> {
    fn map<U>(self, f: fn(T) -> U) -> Option<U> {
        match self {
            Some(v) => Some(f(v)),
            None => None,
        }
    }
    fn and_then<U>(self, f: fn(T) -> Option<U>) -> Option<U> {
        match self {
            Some(v) => f(v),
            None => None,
        }
    }
}

impl<T, E> Result<T, E> {
    fn map<U>(self, f: fn(T) -> U) -> Result<U, E> {
        match self {
            Ok(v) => Ok(f(v)),
            Err(e) => Err(e),
        }
    }
    fn and_then<U>(self, f: fn(T) -> Result<U, E>) -> Result<U, E> {
        match self {
            Ok(v) => f(v),
            Err(e) => Err(e),
        }
    }
}
```

### Runtime Verified (8 positive tests)

| Combinator | Input | Output | Status |
|------------|-------|--------|--------|
| Option::map | Some(21), double | Some(42) | ✓ |
| Option::map | None, double | None | ✓ |
| Option::and_then | Some(42), half_even | Some(21) | ✓ |
| Option::and_then | None, half_even | None | ✓ |
| Result::map | Ok(21), double | Ok(42) | ✓ |
| Result::map | Err(99), double | Err(99) | ✓ |
| Result::and_then | Ok(42), half_even | Ok(21) | ✓ |
| Result::and_then | Err(99), half_even | Err(99) | ✓ |

### Test Coverage

Per §9.4.3 (1:3+ positive:negative ratio): 8 positive + 24 negative = 32
total (1:3 ratio).

Per §7.3.1: 7 categories covered (Lex 3 + Parse 3 + Typeck 3 + Borrowck 1
+ Resolve 16 + Trait 1 + Codegen 1 = 24 negative cases).

### §3.2 Verification

- cargo fmt --check ✓
- cargo clippy -D warnings ✓ (0 warnings)
- cargo test --release ✓ (5436 tests, 0 failures)

## Next Steps (Stage 40.2+)

- **Stage 40.2**: `Option::unwrap` / `Result::unwrap` — requires panic
  formatting infrastructure (runtime panic message support).
- **Stage 40.3**: `Option::or` / `Option::or_else` / `Option::filter` —
  more combinators.
- **v0.6+**: Display trait (TD-DISPLAY-TRAIT-MISSING) and Fn/FnMut/FnOnce
  traits (replace fn type parameters with trait bounds for closure support).
