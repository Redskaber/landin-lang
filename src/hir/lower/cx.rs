//! Lowering context: holds the HIR crate being built + ID allocators.

use crate::hir::id::{DefId, DefIdCounter, HirId, ItemLocalIdCounter, OwnerId};
use crate::hir::kinds::{Body, BodyId, HirCrate, OwnerNode};
use crate::hir::lower::error::LowerError;
use crate::session::Span;
use lasso::Rodeo;

/// The lowering context. Holds:
/// - A reference to the interner (for symbol lookup, though lowering
///   rarely needs to intern new symbols — most are copied from AST)
/// - DefId and ItemLocalId counters for fresh ID allocation
/// - The current owner (DefId) — set when entering an owner's body
/// - A stack of previous owners (for nested owner lowering, e.g. trait items)
/// - The HIR crate being built (owners + bodies)
/// - Errors encountered (non-fatal)
pub struct HirLowerCtxt<'a> {
    pub interner: &'a Rodeo,
    pub def_id_counter: DefIdCounter,
    /// Per-owner ItemLocalId counter; reset when entering a new owner.
    pub local_id_counter: ItemLocalIdCounter,
    /// The current owner (DefId). `None` at crate level.
    pub current_owner: Option<DefId>,
    /// Stack of previous owners (for nesting). Push on enter_owner,
    /// pop on exit_owner.
    pub owner_stack: Vec<Option<DefId>>,
    /// The HIR crate being built.
    pub hir: HirCrate,
    /// Errors encountered during lowering (non-fatal: continue).
    pub errors: Vec<LowerError>,
}

impl<'a> HirLowerCtxt<'a> {
    pub fn new(interner: &'a Rodeo) -> Self {
        Self {
            interner,
            def_id_counter: DefIdCounter::new(),
            local_id_counter: ItemLocalIdCounter::new(),
            current_owner: None,
            owner_stack: Vec::new(),
            hir: HirCrate::new(),
            errors: Vec::new(),
        }
    }

    /// Allocate the next DefId. Used when entering a new owner.
    pub fn fresh_def_id(&mut self) -> DefId {
        self.def_id_counter.fresh()
    }

    /// Allocate the next HirId within the current owner.
    /// Panics if `current_owner` is None (i.e., at crate level — only
    /// owners themselves can be at crate level, and they get HirId
    /// via `enter_owner`).
    pub fn fresh_hir_id(&mut self) -> HirId {
        let owner = self
            .current_owner
            .expect("fresh_hir_id called outside an owner context");
        let local_id = self.local_id_counter.fresh();
        HirId::new(owner, local_id)
    }

    /// Enter an owner context. Allocates a DefId, sets `current_owner`,
    /// resets the local ID counter, and returns the DefId.
    ///
    /// The owner's own HirId (local_id = 0) is allocated automatically
    /// and can be retrieved via [`owner_hir_id`] after this call.
    ///
    /// Use [`exit_owner`] to restore the previous owner context.
    pub fn enter_owner(&mut self) -> DefId {
        let prev = self.current_owner;
        let def_id = self.fresh_def_id();
        self.current_owner = Some(def_id);
        // Reset the local ID counter and allocate local_id=0 for the owner
        // node itself.
        self.local_id_counter = ItemLocalIdCounter::new();
        let _owner_local = self.local_id_counter.fresh_owner_local();
        self.owner_stack.push(prev);
        def_id
    }

    /// The HirId of the current owner node itself (local_id = 0).
    pub fn owner_hir_id(&self) -> HirId {
        HirId::new(
            self.current_owner
                .expect("owner_hir_id called outside an owner context"),
            crate::hir::id::ItemLocalId(0),
        )
    }

    /// Exit the current owner context, restoring the previous owner.
    pub fn exit_owner(&mut self) {
        self.current_owner = self
            .owner_stack
            .pop()
            .expect("exit_owner called without matching enter_owner");
    }

    /// Register a body in the HIR crate. Returns the BodyId for later
    /// reference.
    pub fn store_body(&mut self, body: Body) -> BodyId {
        let body_id = BodyId {
            owner: OwnerId::new(
                self.current_owner
                    .expect("store_body called outside an owner context"),
            ),
        };
        self.hir.bodies.push((body_id, body));
        body_id
    }

    /// Register an owner node in the HIR crate.
    pub fn store_owner(&mut self, def_id: DefId, node: OwnerNode) {
        self.hir.owners.push((def_id, node));
    }

    /// Record a non-fatal lowering error.
    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(LowerError::new(message, span));
    }

    /// Consume the context and return the completed HIR crate + any errors.
    ///
    /// Stage 18.78 P0-A: Changed from returning just `HirCrate` to returning
    /// `(HirCrate, Vec<LowerError>)` so the driver can populate
    /// `CompileErrors.lower`. Previously, `into_hir()` silently discarded
    /// `self.errors`, making `CompileErrors.lower` always empty.
    ///
    /// Per §1.0 原則 4 "报错 > 静默": lowering errors must reach the user.
    pub fn into_hir(self) -> (HirCrate, Vec<LowerError>) {
        (self.hir, self.errors)
    }

    /// The current owner's DefId, if any.
    pub fn current_owner(&self) -> Option<DefId> {
        self.current_owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_exit_owner_basic() {
        let mut interner = Rodeo::new();
        let _ = &mut interner;
        let mut cx = HirLowerCtxt::new(&interner);
        assert_eq!(cx.current_owner(), None);

        let d1 = cx.enter_owner();
        assert_eq!(cx.current_owner(), Some(d1));
        assert_eq!(d1.as_u32(), 0);

        // HirId allocation within owner 1
        let h1 = cx.fresh_hir_id();
        assert_eq!(h1.owner, d1);
        assert_eq!(h1.local_id.as_u32(), 1); // 0 is the owner itself

        let h2 = cx.fresh_hir_id();
        assert_eq!(h2.local_id.as_u32(), 2);

        // Nested owner
        let d2 = cx.enter_owner();
        assert_eq!(cx.current_owner(), Some(d2));
        assert_eq!(d2.as_u32(), 1);

        let h3 = cx.fresh_hir_id();
        assert_eq!(h3.owner, d2);
        assert_eq!(h3.local_id.as_u32(), 1); // reset for new owner

        cx.exit_owner();
        assert_eq!(cx.current_owner(), Some(d1));

        cx.exit_owner();
        assert_eq!(cx.current_owner(), None);
    }

    #[test]
    fn store_body_and_owner() {
        let interner = Rodeo::new();
        let mut cx = HirLowerCtxt::new(&interner);

        let d = cx.enter_owner();
        let hir_id = cx.owner_hir_id();
        assert_eq!(hir_id.local_id.as_u32(), 0);

        // Store a dummy body
        let body = Body {
            hir_id: cx.fresh_hir_id(),
            params: vec![],
            value: crate::hir::kinds::HirExpr {
                hir_id: cx.fresh_hir_id(),
                kind: crate::hir::kinds::HirExprKind::Unit,
                span: Span::DUMMY,
            },
            span: Span::DUMMY,
        };
        let body_id = cx.store_body(body);
        assert_eq!(body_id.owner.0, d);

        // Store a dummy owner
        let node = OwnerNode::Item(crate::hir::kinds::HirItem::Fn(crate::hir::kinds::HirFn {
            hir_id,
            ident: crate::ast::Ident::new(lasso::Spur::default(), Span::DUMMY),
            generics: crate::hir::kinds::HirGenerics::default(),
            sig: crate::hir::kinds::HirFnSig {
                inputs: vec![],
                output: crate::hir::kinds::HirFnRetTy::Default(Span::DUMMY),
                abi: crate::ast::Abi::Landin,
                is_unsafe: false,
                span: Span::DUMMY,
            },
            body: Some(body_id),
            vis: crate::ast::Visibility::Private,
            attrs: vec![],
            span: Span::DUMMY,
        }));
        cx.store_owner(d, node);

        cx.exit_owner();

        let (hir, _errors) = cx.into_hir();
        assert_eq!(hir.owner_count(), 1);
        assert_eq!(hir.body_count(), 1);
    }
}
