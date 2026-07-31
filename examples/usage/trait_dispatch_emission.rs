//! API usage demo: trait dispatch emission — vtable + dynptr global inspection.
//!
//! Run with: `cargo run --example trait_dispatch_emission --features llvm-backend`
//!
//! Stage 14.5: Demonstrates the post-§14.4-split trait dispatch API.
//! Shows how to:
//!   1. `compile(src)` → `CompileResult` (driver orchestrates all passes)
//!   2. Inspect `result.trait_resolver` for vtable/dynptr emission data
//!   3. `build_trait_dispatch_emission_plan(&resolver, &interner)` → plan
//!   4. `emit_trait_dispatch_globals_text_batch(&plan)` → LLVM IR text lines
//!
//! This example exercises the §14.4 architectural split of
//! `codegen/trait_dispatch/` into `vtable/`, `dynptr/`, and `orchestrator/`
//! sub-modules. It does NOT emit a full module — it only shows the
//! trait-dispatch globals in isolation, which is useful for debugging
//! vtable layout and dynptr structure.

use landin_compiler::codegen::{
    build_trait_dispatch_emission_plan, emit_trait_dispatch_globals_text_batch,
};
use landin_compiler::driver::compile;

fn main() {
    // A small Landin program that defines a trait + impl so the trait
    // resolver builds a vtable. The `fn main()` body calls the method so
    // the codegen pipeline reaches the trait-dispatch emission phase.
    let src = "\
struct Pair { x: i32, y: i32 }

impl Pair {
    fn sum(self) -> i32 { self.x + self.y }
}

fn main() -> i32 {
    let p = Pair { x: 10, y: 20 };
    p.sum()
}
";

    let result = compile(src);

    if !result.errors.is_empty() {
        eprintln!("compile errors ({} total):", result.errors.total_count());
        eprintln!(
            "{}",
            result.errors.format_for_user(None, Some(&result.interner))
        );
        std::process::exit(1);
    }

    // The trait resolver collects inherent impls (no user-defined traits
    // here, but the inherent method `sum` is still registered). For a
    // program with `impl Trait for Type`, the resolver would build a
    // vtable entry; for inherent methods only, the vtable map is empty
    // but the resolver is still populated.
    let resolver = &result.trait_resolver;
    let interner = &result.interner;

    println!("=== Trait Resolver State ===");
    println!("trait defs: {}", resolver.traits.len());
    println!("impl blocks: {}", resolver.impls.len());
    println!("vtables:     {}", resolver.vtables.len());
    println!();

    // Build the trait-dispatch emission plan. For inherent-only programs
    // this will be empty (no vtable/dynptr globals). For programs with
    // `impl Trait for Type`, this would produce vtable + dynptr specs.
    let plan = build_trait_dispatch_emission_plan(resolver, interner);

    println!("=== Trait-Dispatch Emission Plan ===");
    println!("vtable_specs: {}", plan.vtable_specs.len());
    println!("dynptr_specs: {}", plan.dynptr_specs.len());
    println!(
        "summary: {} vtable globals, {} dynptr globals, {} total method slots",
        plan.summary.vtable_count, plan.summary.dynptr_count, plan.summary.total_method_slots
    );
    println!();

    // Emit the LLVM IR text for all trait-dispatch globals. Empty for
    // inherent-only programs; non-empty for trait-dispatching programs.
    let ir_lines = emit_trait_dispatch_globals_text_batch(&plan);

    println!("=== Trait-Dispatch LLVM IR ({} lines) ===", ir_lines.len());
    for line in &ir_lines {
        println!("{line}");
    }
    if ir_lines.is_empty() {
        println!("(no trait-dispatch globals — program uses only inherent methods)");
    }
}
