//! Stage 5.36: Stdlib trait method signature tests
//!
//! Tests `StdlibTraitMethod` + `StdlibSelfKind` + `stdlib_trait_methods()` +
//! `stdlib_trait_method_count()` + `find_stdlib_trait_method()` +
//! `is_stdlib_trait_method()` + `stdlib_traits_with_method()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    find_stdlib_trait_method, is_stdlib_trait_method, stdlib_trait_method_count,
    stdlib_trait_methods, stdlib_traits_with_method, StdlibSelfKind, StdlibTraitMethod,
    StdlibTypeKind,
};

// ---------------------------------------------------------------------------
// stdlib_trait_methods — slice lookup
// ---------------------------------------------------------------------------

/// Clone should have exactly 2 methods: clone + clone_from.
#[test]
fn test_stdlib_trait_methods_clone() {
    let methods = stdlib_trait_methods("Clone").expect("Clone should be registered");
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "clone");
    assert_eq!(methods[1].name, "clone_from");
}

/// Drop should have exactly 1 method: drop(&mut self).
#[test]
fn test_stdlib_trait_methods_drop() {
    let methods = stdlib_trait_methods("Drop").expect("Drop should be registered");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "drop");
    assert_eq!(methods[0].self_kind, StdlibSelfKind::SelfByMutRef);
    assert_eq!(methods[0].return_kind, StdlibTypeKind::Unit);
}

/// Default::default is a NoSelf associated function returning Self.
#[test]
fn test_stdlib_trait_methods_default() {
    let methods = stdlib_trait_methods("Default").expect("Default should be registered");
    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "default");
    assert_eq!(m.self_kind, StdlibSelfKind::NoSelf);
    assert!(!m.has_self());
}

/// Display has fmt(&self, f: &mut Formatter) -> Result<(), Error>.
#[test]
fn test_stdlib_trait_methods_display() {
    let methods = stdlib_trait_methods("Display").expect("Display should be registered");
    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "fmt");
    assert_eq!(m.self_kind, StdlibSelfKind::SelfByRef);
    assert_eq!(m.param_count, 1);
    assert_eq!(m.return_kind, StdlibTypeKind::StdType);
}

/// PartialEq has eq + ne, both `fn(&self, &Self) -> bool`.
#[test]
fn test_stdlib_trait_methods_partial_eq() {
    let methods = stdlib_trait_methods("PartialEq").expect("PartialEq should be registered");
    assert_eq!(methods.len(), 2);
    for m in methods {
        assert_eq!(m.self_kind, StdlibSelfKind::SelfByRef);
        assert_eq!(m.param_count, 1);
        assert_eq!(m.return_kind, StdlibTypeKind::Bool);
    }
}

/// Ord has cmp(&self, &Self) -> Ordering.
#[test]
fn test_stdlib_trait_methods_ord() {
    let methods = stdlib_trait_methods("Ord").expect("Ord should be registered");
    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "cmp");
    assert_eq!(m.self_kind, StdlibSelfKind::SelfByRef);
}

/// Markers (Copy/Send/Sync/Sized/Unpin/Eq) should be in the registry
/// but with empty method tables.
#[test]
fn test_stdlib_trait_methods_marker_empty() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        let methods = stdlib_trait_methods(trait_name)
            .unwrap_or_else(|| panic!("{trait_name} should be in the stdlib trait registry"));
        assert_eq!(
            methods.len(),
            0,
            "{trait_name} should have 0 methods (marker)"
        );
    }
}

/// Add::add is `fn(self, rhs: Rhs) -> Self::Output` — by-value self.
#[test]
fn test_stdlib_trait_methods_add() {
    let methods = stdlib_trait_methods("Add").expect("Add should be registered");
    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "add");
    assert_eq!(m.self_kind, StdlibSelfKind::SelfByValue);
    assert_eq!(m.param_count, 1);
    assert!(m.has_self());
}

/// Sub::sub — separate table from Add::add, correct name.
#[test]
fn test_stdlib_trait_methods_sub() {
    let methods = stdlib_trait_methods("Sub").expect("Sub should be registered");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "sub");
}

/// Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr — all have one method
/// with the correct name and by-value self.
#[test]
fn test_stdlib_trait_methods_all_arith_binary() {
    for (trait_name, expected_method_name) in [
        ("Add", "add"),
        ("Sub", "sub"),
        ("Mul", "mul"),
        ("Div", "div"),
        ("Rem", "rem"),
        ("BitAnd", "bitand"),
        ("BitOr", "bitor"),
        ("BitXor", "bitxor"),
        ("Shl", "shl"),
        ("Shr", "shr"),
    ] {
        let methods = stdlib_trait_methods(trait_name)
            .unwrap_or_else(|| panic!("{trait_name} should be registered"));
        assert_eq!(methods.len(), 1, "{trait_name} should have 1 method");
        assert_eq!(
            methods[0].name, expected_method_name,
            "{trait_name} method name mismatch"
        );
        assert_eq!(
            methods[0].self_kind,
            StdlibSelfKind::SelfByValue,
            "{trait_name} should be by-value self"
        );
    }
}

/// AddAssign/SubAssign/... — `fn(&mut self, rhs: Rhs) -> ()`.
#[test]
fn test_stdlib_trait_methods_all_arith_assign() {
    for (trait_name, expected_method_name) in [
        ("AddAssign", "add_assign"),
        ("SubAssign", "sub_assign"),
        ("MulAssign", "mul_assign"),
        ("DivAssign", "div_assign"),
        ("RemAssign", "rem_assign"),
        ("BitAndAssign", "bitand_assign"),
        ("BitOrAssign", "bitor_assign"),
        ("BitXorAssign", "bitxor_assign"),
        ("ShlAssign", "shl_assign"),
        ("ShrAssign", "shr_assign"),
    ] {
        let methods = stdlib_trait_methods(trait_name)
            .unwrap_or_else(|| panic!("{trait_name} should be registered"));
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, expected_method_name);
        assert_eq!(methods[0].self_kind, StdlibSelfKind::SelfByMutRef);
        assert_eq!(methods[0].return_kind, StdlibTypeKind::Unit);
    }
}

/// Iterator::next is `fn(&mut self) -> Option<Self::Item>`.
#[test]
fn test_stdlib_trait_methods_iterator() {
    let methods = stdlib_trait_methods("Iterator").expect("Iterator should be registered");
    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "next");
    assert_eq!(m.self_kind, StdlibSelfKind::SelfByMutRef);
    assert_eq!(m.return_kind, StdlibTypeKind::StdType);
}

/// Unknown trait should return None.
#[test]
fn test_stdlib_trait_methods_none() {
    assert_eq!(stdlib_trait_methods("BogusTrait"), None);
    assert_eq!(stdlib_trait_methods("From"), None); // not registered yet
    assert_eq!(stdlib_trait_methods(""), None);
}

// ---------------------------------------------------------------------------
// stdlib_trait_method_count
// ---------------------------------------------------------------------------

/// `stdlib_trait_method_count` matches slice length.
#[test]
fn test_stdlib_trait_method_count() {
    assert_eq!(stdlib_trait_method_count("Clone"), Some(2));
    assert_eq!(stdlib_trait_method_count("Drop"), Some(1));
    assert_eq!(stdlib_trait_method_count("Copy"), Some(0));
    assert_eq!(stdlib_trait_method_count("BogusTrait"), None);
}

// ---------------------------------------------------------------------------
// find_stdlib_trait_method
// ---------------------------------------------------------------------------

/// `find_stdlib_trait_method` should hit when (trait, method) is valid.
#[test]
fn test_find_stdlib_trait_method_hit() {
    let m =
        find_stdlib_trait_method("Clone", "clone").expect("Clone::clone should be in the registry");
    assert_eq!(m.name, "clone");
    assert_eq!(m.self_kind, StdlibSelfKind::SelfByRef);

    let m2 = find_stdlib_trait_method("Iterator", "next")
        .expect("Iterator::next should be in the registry");
    assert_eq!(m2.name, "next");
}

/// `find_stdlib_trait_method` should miss on unknown method or trait.
#[test]
fn test_find_stdlib_trait_method_miss() {
    assert!(find_stdlib_trait_method("Clone", "bogus").is_none());
    assert!(find_stdlib_trait_method("BogusTrait", "clone").is_none());
    // Clone doesn't have `next`
    assert!(find_stdlib_trait_method("Clone", "next").is_none());
}

/// `find_stdlib_trait_method` for arithmetic ops should match exact op name.
#[test]
fn test_find_stdlib_trait_method_arith() {
    assert!(find_stdlib_trait_method("Add", "add").is_some());
    assert!(find_stdlib_trait_method("Sub", "sub").is_some());
    assert!(find_stdlib_trait_method("Mul", "mul").is_some());
    // Add doesn't have `sub` — must be exact
    assert!(find_stdlib_trait_method("Add", "sub").is_none());
    assert!(find_stdlib_trait_method("Add", "add_assign").is_none());
}

// ---------------------------------------------------------------------------
// is_stdlib_trait_method
// ---------------------------------------------------------------------------

/// `is_stdlib_trait_method` returns true for known pairs.
#[test]
fn test_is_stdlib_trait_method_true() {
    assert!(is_stdlib_trait_method("Clone", "clone"));
    assert!(is_stdlib_trait_method("Clone", "clone_from"));
    assert!(is_stdlib_trait_method("Iterator", "next"));
    assert!(is_stdlib_trait_method("Default", "default"));
    assert!(is_stdlib_trait_method("Add", "add"));
}

/// `is_stdlib_trait_method` returns false for unknown pairs.
#[test]
fn test_is_stdlib_trait_method_false() {
    assert!(!is_stdlib_trait_method("Clone", "next"));
    assert!(!is_stdlib_trait_method("Iterator", "clone"));
    assert!(!is_stdlib_trait_method("Bogus", "clone"));
    assert!(!is_stdlib_trait_method("Copy", "clone")); // markers have no methods
}

// ---------------------------------------------------------------------------
// stdlib_traits_with_method
// ---------------------------------------------------------------------------

/// `stdlib_traits_with_method("clone")` should include at least Clone.
#[test]
fn test_stdlib_traits_with_method_clone() {
    let traits = stdlib_traits_with_method("clone");
    assert!(
        traits.contains(&"Clone"),
        "expected Clone in traits with method `clone`, got: {traits:?}"
    );
}

/// `stdlib_traits_with_method("fmt")` should include Display + Debug (both
/// declare `fmt`).
#[test]
fn test_stdlib_traits_with_method_fmt() {
    let traits = stdlib_traits_with_method("fmt");
    assert!(traits.contains(&"Display"));
    assert!(traits.contains(&"Debug"));
}

/// `stdlib_traits_with_method("bogus")` should be empty.
#[test]
fn test_stdlib_traits_with_method_bogus() {
    let traits = stdlib_traits_with_method("bogus_method");
    assert!(traits.is_empty(), "got: {traits:?}");
}

// ---------------------------------------------------------------------------
// StdlibTraitMethod helpers
// ---------------------------------------------------------------------------

/// `StdlibTraitMethod::has_self()` returns false only for NoSelf methods.
#[test]
fn test_stdlib_trait_method_has_self() {
    let default = find_stdlib_trait_method("Default", "default").unwrap();
    assert!(!default.has_self(), "Default::default is NoSelf");

    let clone = find_stdlib_trait_method("Clone", "clone").unwrap();
    assert!(clone.has_self(), "Clone::clone takes self by ref");

    let drop = find_stdlib_trait_method("Drop", "drop").unwrap();
    assert!(drop.has_self(), "Drop::drop takes self by mut ref");

    let add = find_stdlib_trait_method("Add", "add").unwrap();
    assert!(add.has_self(), "Add::add takes self by value");
}

/// `StdlibTraitMethod` should derive PartialEq/Eq for direct comparison.
#[test]
fn test_stdlib_trait_method_partial_eq() {
    let m1 = StdlibTraitMethod {
        name: "test",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 0,
        return_kind: StdlibTypeKind::Bool,
        param_kinds: &[],
        is_unsafe: false,
    };
    let m2 = StdlibTraitMethod {
        name: "test",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 0,
        return_kind: StdlibTypeKind::Bool,
        param_kinds: &[],
        is_unsafe: false,
    };
    let m3 = StdlibTraitMethod {
        name: "test",
        self_kind: StdlibSelfKind::SelfByValue, // different
        param_count: 0,
        return_kind: StdlibTypeKind::Bool,
        param_kinds: &[],
        is_unsafe: false,
    };
    assert_eq!(m1, m2);
    assert_ne!(m1, m3);
}
