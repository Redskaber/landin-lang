//! Stage 15.6 — `method_return_type_cache` activation tests.
//!
//! These tests verify the cache infrastructure added in Stage 15.4 and
//! activated in Stage 15.6. The cache lives on `MirLowerCtxt` as a
//! `RefCell<HashMap<DefId, Option<Ty>>>` and is consulted by
//! `MirLowerCtxt::query_method_return_type` (the public method).
//!
//! Coverage:
//! 1. Cache starts empty (no spurious entries).
//! 2. Cache populates on lookup (None results are also cached).
//! 3. Repeated lookups of the same DefId are memoized (single HIR scan).
//! 4. Different DefIds get independent cache entries.
//! 5. Cache returns same result as direct uncached call (correctness).
//! 6. Cache hit on real HIR (verifies cache is actually consulted).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): tests verify both correctness
//! (cache returns the same value as direct HIR scan) and the performance
//! invariant (cache prevents re-scanning).
//!
//! Per §23 (API Naming): the test module name follows
//! `<feature>_<noun>_<noun>_tests` pattern (matches the feature name).

#![cfg(test)]

use landin_compiler::hir::DefId;
use landin_compiler::mir::lower::MirLowerCtxt;
use landin_compiler::session::Span;
use lasso::Rodeo;

/// Stage 15.6 test 1: cache starts empty.
///
/// Per §29.1.3 (Design-Impl-Test): verifies the cache infrastructure
/// added in Stage 15.4 has no spurious initial entries.
#[test]
fn stage15_6_cache_starts_empty() {
    let interner = Rodeo::new();
    let cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    assert!(
        cx.method_return_type_cache.borrow().is_empty(),
        "cache must start empty before any lookup"
    );
}

/// Stage 15.6 test 2: cache populates on lookup (even when no HIR is
/// attached — None should be cached to prevent repeated HIR scanning).
///
/// Per §1.0 原则 3 "显式 > 隐式": caching None is intentional and tested.
#[test]
fn stage15_6_cache_populates_on_miss_with_no_hir() {
    let interner = Rodeo::new();
    let cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    // No HIR attached — query should return None and cache None.
    let result1 = cx.query_method_return_type(DefId(42));
    assert!(result1.is_none(), "query without HIR must return None");

    // Cache should now have one entry (DefId(42) → None).
    assert_eq!(
        cx.method_return_type_cache.borrow().len(),
        1,
        "cache must populate on miss"
    );
    assert!(
        cx.method_return_type_cache
            .borrow()
            .get(&DefId(42))
            .is_some(),
        "cache must store None result explicitly"
    );
}

/// Stage 15.6 test 3: repeated lookups hit cache (no re-scan).
///
/// This is the performance invariant — the whole point of the cache.
/// Per §29.4 (performance baseline): cache hit is O(1), miss is O(n) HIR scan.
#[test]
fn stage15_6_repeated_lookups_are_cached() {
    let interner = Rodeo::new();
    let cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    // First lookup: populates cache (None because no HIR).
    let _ = cx.query_method_return_type(DefId(7));
    assert_eq!(cx.method_return_type_cache.borrow().len(), 1);

    // Second lookup: cache hit, no new entry.
    let _ = cx.query_method_return_type(DefId(7));
    assert_eq!(
        cx.method_return_type_cache.borrow().len(),
        1,
        "repeated lookup must hit cache, not add new entry"
    );

    // Different DefId: cache miss, adds new entry.
    let _ = cx.query_method_return_type(DefId(8));
    assert_eq!(cx.method_return_type_cache.borrow().len(), 2);
}

/// Stage 15.6 test 4: different DefIds get independent cache entries.
///
/// Per §1.0 原则 6 "通用 > 特例": one cache handles all DefIds uniformly.
#[test]
fn stage15_6_distinct_defids_get_distinct_entries() {
    let interner = Rodeo::new();
    let cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    for i in 0..5u32 {
        let _ = cx.query_method_return_type(DefId(i));
    }
    assert_eq!(
        cx.method_return_type_cache.borrow().len(),
        5,
        "each distinct DefId must get its own cache entry"
    );
}

/// Stage 15.6 test 5: cache returns same result as direct uncached call.
///
/// This is the correctness invariant. Per §29.1.3: test → design coverage.
/// The cached method must return exactly what the uncached inner function
/// would return. We use the real driver entry point to get a real HIR.
#[test]
fn stage15_6_cached_matches_uncached_semantics() {
    // Compile a small program with an impl method.
    let src = r#"
        struct Counter { v: i32 }
        impl Counter {
            fn new() -> Counter { Counter { v: 0 } }
            fn get(self) -> i32 { self.v }
        }
        fn main() -> i32 {
            let c = Counter::new();
            c.get()
        }
    "#;

    let result = landin_compiler::compile(src);
    assert!(
        result.errors.is_empty(),
        "test program must compile cleanly (errors: {})",
        result.errors.total_count()
    );

    let hir = result.hir.as_ref().expect("HIR must be present");

    // Find the DefId of `Counter::get` by scanning owners.
    // The method's DefId is `f.hir_id.owner` (per the uncached function's
    // lookup key — see `query_method_return_type_uncached`).
    let get_spur = result.interner.get("get").expect("`get` must be interned");
    let mut get_did: Option<DefId> = None;
    for (_, owner) in &hir.owners {
        if let landin_compiler::hir::OwnerNode::Item(landin_compiler::hir::HirItem::Impl(
            impl_block,
        )) = owner
        {
            for item in &impl_block.items {
                if let landin_compiler::hir::HirImplItem::Fn(f) = item {
                    if f.ident.name == get_spur {
                        get_did = Some(f.hir_id.owner);
                    }
                }
            }
        }
    }
    let get_did = get_did.expect("`Counter::get` DefId must be found");

    // Uncached: call the inner function directly.
    let uncached = landin_compiler::mir::lower::query_method_return_type_uncached(hir, get_did);
    // Sanity: uncached should return Some (the method has explicit return type i32).
    assert!(uncached.is_some(), "uncached lookup must find the method");

    // Cached: call through MirLowerCtxt with HIR attached.
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    cx.hir = Some(hir);
    let cached_first = cx.query_method_return_type(get_did);
    let cached_second = cx.query_method_return_type(get_did);

    // First cached call should match uncached.
    assert_eq!(
        uncached, cached_first,
        "cached first call must match uncached semantics"
    );
    // Second cached call should also match (cache hit, same value).
    assert_eq!(
        uncached, cached_second,
        "cached second call (cache hit) must match uncached semantics"
    );
}

/// Stage 15.6 test 6: cache hit on real HIR (verifies cache is actually
/// consulted, not bypassed).
///
/// Per §29.1.1 (data flow coverage): explicit cache-hit verification.
#[test]
fn stage15_6_cache_hit_on_real_hir() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn new(x: i32, y: i32) -> Point { Point { x: x, y: y } }
            fn x(self) -> i32 { self.x }
        }
        fn main() -> i32 {
            let p = Point::new(1, 2);
            p.x()
        }
    "#;

    let result = landin_compiler::compile(src);
    assert!(
        result.errors.is_empty(),
        "test program must compile cleanly (errors: {})",
        result.errors.total_count()
    );

    let hir = result.hir.as_ref().expect("HIR must be present");

    // Find any impl method DefId (use f.hir_id.owner per uncached lookup key).
    let mut method_did: Option<DefId> = None;
    'outer: for (_, owner) in &hir.owners {
        if let landin_compiler::hir::OwnerNode::Item(landin_compiler::hir::HirItem::Impl(
            impl_block,
        )) = owner
        {
            for item in &impl_block.items {
                if let landin_compiler::hir::HirImplItem::Fn(f) = item {
                    method_did = Some(f.hir_id.owner);
                    break 'outer;
                }
            }
        }
    }
    let method_did = method_did.expect("at least one impl method must exist");

    // Build MirLowerCtxt with HIR attached.
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    cx.hir = Some(hir);

    // First call: miss → populate.
    let _ = cx.query_method_return_type(method_did);
    assert_eq!(cx.method_return_type_cache.borrow().len(), 1);

    // Second call: hit (verified by cache still having 1 entry, not 2).
    let _ = cx.query_method_return_type(method_did);
    assert_eq!(
        cx.method_return_type_cache.borrow().len(),
        1,
        "cache hit must not add duplicate entry"
    );
}
