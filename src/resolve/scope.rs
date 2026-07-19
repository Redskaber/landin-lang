//! Scope chain for local variable resolution.
//!
//! Per Stage 1.4 plan: a linked-list of scopes (Fn → Block → Closure →
//! MatchArm → Loop) that tracks `let` bindings, closure params, match
//! arm pattern bindings, and for-loop bindings.

use crate::hir::HirId;
use lasso::Spur;
use std::collections::HashMap;

/// The kind of a scope. Used for diagnostics and forward-ref detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Function body scope. Holds fn params.
    Fn,
    /// `{ ... }` block scope.
    Block,
    /// Closure `|args| body` scope.
    Closure,
    /// Match arm pattern scope.
    MatchArm,
    /// Loop / while / for body scope.
    Loop,
}

/// A single scope frame in the scope chain.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Bindings in this scope: name → HirId of the binding.
    pub bindings: HashMap<Spur, HirId>,
    /// Parent scope (None for the root/fn scope).
    pub parent: Option<Box<Scope>>,
    /// Scope kind.
    pub kind: ScopeKind,
}

impl Scope {
    pub fn new(kind: ScopeKind) -> Self {
        Self {
            bindings: HashMap::new(),
            parent: None,
            kind,
        }
    }

    /// Create a child scope of this scope.
    pub fn child(&self, kind: ScopeKind) -> Self {
        Self {
            bindings: HashMap::new(),
            parent: Some(Box::new(self.clone())),
            kind,
        }
    }

    /// Insert a binding into this scope. If the name already exists in
    /// THIS scope (not parent), it's a shadowing within the same scope —
    /// the new binding replaces the old (Rust allows re-binding).
    pub fn insert(&mut self, name: Spur, hir_id: HirId) {
        self.bindings.insert(name, hir_id);
    }

    /// Look up a name, walking the scope chain from inner to outer.
    /// Returns the HirId of the binding if found.
    pub fn lookup(&self, name: Spur) -> Option<HirId> {
        if let Some(hir_id) = self.bindings.get(&name) {
            return Some(*hir_id);
        }
        self.parent.as_ref().and_then(|p| p.lookup(name))
    }
}

/// A managed scope stack. Wraps the current scope and provides
/// push/pop operations.
pub struct ScopeStack {
    current: Scope,
}

impl ScopeStack {
    pub fn new(kind: ScopeKind) -> Self {
        Self {
            current: Scope::new(kind),
        }
    }

    /// Push a new child scope.
    pub fn push(&mut self, kind: ScopeKind) {
        let new_scope = self.current.child(kind);
        self.current = new_scope;
    }

    /// Pop the current scope, returning to the parent.
    /// Panics if there's no parent (root scope can't be popped).
    pub fn pop(&mut self) {
        self.current = *self
            .current
            .parent
            .take()
            .expect("ScopeStack::pop called on root scope");
    }

    /// Insert a binding into the current scope.
    pub fn insert(&mut self, name: Spur, hir_id: HirId) {
        self.current.insert(name, hir_id);
    }

    /// Look up a name in the scope chain.
    pub fn lookup(&self, name: Spur) -> Option<HirId> {
        self.current.lookup(name)
    }

    /// Get the current scope kind.
    pub fn kind(&self) -> ScopeKind {
        self.current.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{DefId, ItemLocalId};
    use lasso::Rodeo;

    fn hir_id(n: u32) -> HirId {
        HirId::new(DefId(0), ItemLocalId(n))
    }

    #[test]
    fn scope_basic_lookup() {
        let mut interner = Rodeo::new();
        let name = interner.get_or_intern("x");
        let mut scope = Scope::new(ScopeKind::Fn);
        scope.insert(name, hir_id(1));
        assert_eq!(scope.lookup(name), Some(hir_id(1)));
        let other = interner.get_or_intern("y");
        assert_eq!(scope.lookup(other), None);
    }

    #[test]
    fn scope_child_inherits_parent() {
        let mut interner = Rodeo::new();
        let name = interner.get_or_intern("x");
        let mut parent = Scope::new(ScopeKind::Fn);
        parent.insert(name, hir_id(1));
        let child = parent.child(ScopeKind::Block);
        assert_eq!(child.lookup(name), Some(hir_id(1)));
    }

    #[test]
    fn scope_shadowing() {
        let mut interner = Rodeo::new();
        let name = interner.get_or_intern("x");
        let mut parent = Scope::new(ScopeKind::Fn);
        parent.insert(name, hir_id(1));
        let mut child = parent.child(ScopeKind::Block);
        child.insert(name, hir_id(2)); // shadows parent
        assert_eq!(child.lookup(name), Some(hir_id(2))); // inner wins
    }

    #[test]
    fn scope_stack_push_pop() {
        let mut interner = Rodeo::new();
        let name1 = interner.get_or_intern("x");
        let name2 = interner.get_or_intern("y");
        let mut stack = ScopeStack::new(ScopeKind::Fn);
        stack.insert(name1, hir_id(1));
        stack.push(ScopeKind::Block);
        stack.insert(name2, hir_id(2));
        assert_eq!(stack.lookup(name1), Some(hir_id(1))); // from parent
        assert_eq!(stack.lookup(name2), Some(hir_id(2))); // from current
        stack.pop();
        assert_eq!(stack.lookup(name2), None); // popped
        assert_eq!(stack.lookup(name1), Some(hir_id(1))); // still there
    }
}
