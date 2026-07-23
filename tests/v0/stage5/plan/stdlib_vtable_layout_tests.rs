//! Stage 5.37: Stdlib vtable slot layout tests
//!
//! Tests `StdlibVtableSlot` + `stdlib_trait_method_index()` +
//! `stdlib_vtable_layout()` + `stdlib_vtable_slot_count()` +
//! `is_stdlib_marker_trait()` + `stdlib_traits_with_vtable()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    is_stdlib_marker_trait, stdlib_trait_method_index, stdlib_traits_with_vtable,
    stdlib_vtable_layout, stdlib_vtable_slot_count, StdlibSelfKind, StdlibVtableSlot,
};

// ---------------------------------------------------------------------------
// stdlib_trait_method_index
// ---------------------------------------------------------------------------

/// Clone: clone → slot 0, clone_from → slot 1.
#[test]
fn test_stdlib_trait_method_index_clone() {
    assert_eq!(stdlib_trait_method_index("Clone", "clone"), Some(0));
    assert_eq!(stdlib_trait_method_index("Clone", "clone_from"), Some(1));
}

/// Drop has one method → slot 0.
#[test]
fn test_stdlib_trait_method_index_drop() {
    assert_eq!(stdlib_trait_method_index("Drop", "drop"), Some(0));
}

/// PartialEq: eq → 0, ne → 1.
#[test]
fn test_stdlib_trait_method_index_partial_eq() {
    assert_eq!(stdlib_trait_method_index("PartialEq", "eq"), Some(0));
    assert_eq!(stdlib_trait_method_index("PartialEq", "ne"), Some(1));
}

/// Add: add → slot 0 (single-method trait).
#[test]
fn test_stdlib_trait_method_index_add() {
    assert_eq!(stdlib_trait_method_index("Add", "add"), Some(0));
    // Sub has its own slot 0 (separate trait)
    assert_eq!(stdlib_trait_method_index("Sub", "sub"), Some(0));
}

/// Unknown trait → None.
#[test]
fn test_stdlib_trait_method_index_unknown_trait() {
    assert_eq!(stdlib_trait_method_index("BogusTrait", "clone"), None);
    assert_eq!(stdlib_trait_method_index("From", "from"), None); // not registered
    assert_eq!(stdlib_trait_method_index("", "clone"), None);
}

/// Unknown method on a known trait → None.
#[test]
fn test_stdlib_trait_method_index_unknown_method() {
    assert_eq!(stdlib_trait_method_index("Clone", "bogus"), None);
    assert_eq!(stdlib_trait_method_index("Clone", "next"), None);
    // Add doesn't have `sub` (different op trait)
    assert_eq!(stdlib_trait_method_index("Add", "sub"), None);
}

/// Marker traits have no slots → method index returns None.
#[test]
fn test_stdlib_trait_method_index_marker() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        assert_eq!(
            stdlib_trait_method_index(trait_name, "clone"),
            None,
            "{trait_name} is a marker — should have no slots"
        );
    }
}

// ---------------------------------------------------------------------------
// stdlib_vtable_layout
// ---------------------------------------------------------------------------

/// Clone layout has 2 slots: clone@0 + clone_from@1.
#[test]
fn test_stdlib_vtable_layout_clone() {
    let layout = stdlib_vtable_layout("Clone").expect("Clone should be registered");
    assert_eq!(layout.len(), 2);
    assert_eq!(layout[0].slot_index, 0);
    assert_eq!(layout[0].method.name, "clone");
    assert_eq!(layout[1].slot_index, 1);
    assert_eq!(layout[1].method.name, "clone_from");
}

/// Drop layout has 1 slot: drop@0.
#[test]
fn test_stdlib_vtable_layout_drop() {
    let layout = stdlib_vtable_layout("Drop").expect("Drop should be registered");
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].slot_index, 0);
    assert_eq!(layout[0].method.name, "drop");
}

/// Marker trait layout is empty Vec (not None).
#[test]
fn test_stdlib_vtable_layout_marker_empty() {
    let layout = stdlib_vtable_layout("Copy").expect("Copy should be registered");
    assert!(layout.is_empty());
}

/// Unknown trait layout is None.
#[test]
fn test_stdlib_vtable_layout_unknown() {
    assert_eq!(stdlib_vtable_layout("BogusTrait"), None);
    assert_eq!(stdlib_vtable_layout(""), None);
}

/// Vtable layout is deterministic — repeated calls return same order.
#[test]
fn test_stdlib_vtable_layout_deterministic() {
    let layout1 = stdlib_vtable_layout("Clone").unwrap();
    let layout2 = stdlib_vtable_layout("Clone").unwrap();
    assert_eq!(layout1.len(), layout2.len());
    for (s1, s2) in layout1.iter().zip(layout2.iter()) {
        assert_eq!(s1.slot_index, s2.slot_index);
        assert_eq!(s1.method.name, s2.method.name);
    }
}

/// Vtable layout for arith op preserves method name.
#[test]
fn test_stdlib_vtable_layout_arith() {
    let layout = stdlib_vtable_layout("Add").expect("Add should be registered");
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].method.name, "add");
    let layout2 = stdlib_vtable_layout("Sub").expect("Sub should be registered");
    assert_eq!(layout2[0].method.name, "sub");
}

// ---------------------------------------------------------------------------
// stdlib_vtable_slot_count
// ---------------------------------------------------------------------------

/// `stdlib_vtable_slot_count` matches expected counts.
#[test]
fn test_stdlib_vtable_slot_count() {
    assert_eq!(stdlib_vtable_slot_count("Clone"), Some(2));
    assert_eq!(stdlib_vtable_slot_count("Drop"), Some(1));
    assert_eq!(stdlib_vtable_slot_count("Default"), Some(1));
    assert_eq!(stdlib_vtable_slot_count("PartialEq"), Some(2));
    assert_eq!(stdlib_vtable_slot_count("Add"), Some(1));
    assert_eq!(stdlib_vtable_slot_count("Iterator"), Some(1));
    // Markers have 0 slots (but registered)
    assert_eq!(stdlib_vtable_slot_count("Copy"), Some(0));
    assert_eq!(stdlib_vtable_slot_count("Send"), Some(0));
    assert_eq!(stdlib_vtable_slot_count("Eq"), Some(0));
    // Unknown traits
    assert_eq!(stdlib_vtable_slot_count("BogusTrait"), None);
    assert_eq!(stdlib_vtable_slot_count(""), None);
}

// ---------------------------------------------------------------------------
// is_stdlib_marker_trait
// ---------------------------------------------------------------------------

/// Markers return true.
#[test]
fn test_is_stdlib_marker_trait_true() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        assert!(
            is_stdlib_marker_trait(trait_name),
            "{trait_name} should be a marker trait"
        );
    }
}

/// Non-marker registered traits return false.
#[test]
fn test_is_stdlib_marker_trait_false() {
    assert!(!is_stdlib_marker_trait("Clone"));
    assert!(!is_stdlib_marker_trait("Drop"));
    assert!(!is_stdlib_marker_trait("Default"));
    assert!(!is_stdlib_marker_trait("Add"));
    assert!(!is_stdlib_marker_trait("Iterator"));
}

/// Unknown traits return false (not registered → not a marker).
#[test]
fn test_is_stdlib_marker_trait_unknown() {
    assert!(!is_stdlib_marker_trait("BogusTrait"));
    assert!(!is_stdlib_marker_trait("From")); // not registered
    assert!(!is_stdlib_marker_trait(""));
}

// ---------------------------------------------------------------------------
// stdlib_traits_with_vtable
// ---------------------------------------------------------------------------

/// `stdlib_traits_with_vtable` should include Clone (has 2 slots).
#[test]
fn test_stdlib_traits_with_vtable_includes_clone() {
    let traits = stdlib_traits_with_vtable();
    assert!(
        traits.contains(&"Clone"),
        "expected Clone in traits with vtable, got: {traits:?}"
    );
    assert!(traits.contains(&"Drop"));
    assert!(traits.contains(&"Add"));
    assert!(traits.contains(&"Iterator"));
}

/// `stdlib_traits_with_vtable` should exclude all marker traits.
#[test]
fn test_stdlib_traits_with_vtable_excludes_markers() {
    let traits = stdlib_traits_with_vtable();
    for marker in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        assert!(
            !traits.contains(marker),
            "{marker} should NOT be in traits with vtable (it's a marker)"
        );
    }
}

/// `stdlib_traits_with_vtable` count is non-trivial (≥ 20).
#[test]
fn test_stdlib_traits_with_vtable_count() {
    let traits = stdlib_traits_with_vtable();
    // 13 core + 2 I/O + 2 unary + 10 binary + 10 assign = 37 traits with methods
    assert!(
        traits.len() >= 20,
        "expected at least 20 traits with vtable, got {} ({traits:?})",
        traits.len()
    );
}

// ---------------------------------------------------------------------------
// StdlibVtableSlot struct
// ---------------------------------------------------------------------------

/// `StdlibVtableSlot` field access works as expected.
#[test]
fn test_stdlib_vtable_slot_struct() {
    let layout = stdlib_vtable_layout("PartialEq").unwrap();
    assert_eq!(layout.len(), 2);

    let slot0: &StdlibVtableSlot = &layout[0];
    assert_eq!(slot0.slot_index, 0);
    assert_eq!(slot0.method.name, "eq");
    assert_eq!(slot0.method.self_kind, StdlibSelfKind::SelfByRef);

    let slot1: &StdlibVtableSlot = &layout[1];
    assert_eq!(slot1.slot_index, 1);
    assert_eq!(slot1.method.name, "ne");
}

/// `StdlibVtableSlot` derives PartialEq/Eq.
#[test]
fn test_stdlib_vtable_slot_eq() {
    let layout = stdlib_vtable_layout("Clone").unwrap();
    let s1 = layout[0];
    let s2 = layout[0];
    assert_eq!(s1, s2);
    assert_ne!(layout[0], layout[1]);
}
