//! Stage 147 (v0.15 — TD-ASSOC-TYPE-MULTI-RUNTIME): Fix multi-assoc-type
//! trait method resolution. Bodyless trait methods now get unique DefIds.
//!
//! ## Root cause
//!
//! Bodyless trait methods (e.g., `fn key(&self) -> Self::Key;` in a trait
//! with multiple methods) used `fresh_hir_id()` which shares the trait's
//! owner DefId. This caused `TraitMethodResolutionMap` key collisions:
//! `(trait_def_id, type_name)` was the same for ALL methods in the same
//! trait, so the second `insert` overwrote the first. `kv.key()` resolved
//! to `Entry::value` (the last-inserted entry).
//!
//! ## Fix
//!
//! 1. `hir/lower/item.rs`: Bodyless trait methods now use `enter_owner`/
//!    `exit_owner` (same as bodied methods), giving each a unique DefId.
//! 2. `resolve/module_build.rs`: Skip trait method owners in module
//!    registration (same pattern as impl methods) — trait methods are
//!    accessed via trait method resolution, not as free functions.
//! 3. `driver/driver_validations.rs`: `mir_ty_kinds_compatible` adds
//!    Projection arm (Projection ↔ any type = true), mirroring unify.rs
//!    Stage 146.
//!
//! ## Test plan
//!
//! - Multi-assoc-type generic function: 4 tests
//! - Multiple methods with same-name trait: 3 tests
//! - FnMut/FnOnce trait impls: 4 tests (regression)
//! - Three+ assoc types: 2 tests
//! - Edge cases: 2 tests
//! - Regression: 3 tests
//! - Total: 18 tests

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Multi-assoc-type generic function
// ===========================================================================

#[test]
fn stage147_multi_assoc_generic_get_key() {
    let code = r#"
trait KeyValue {
    type Key;
    type Value;
    fn key(&self) -> Self::Key;
    fn value(&self) -> Self::Value;
}

struct Entry { k: i64, v: i64 }

impl KeyValue for Entry {
    type Key = i64;
    type Value = i64;
    fn key(&self) -> i64 { self.k }
    fn value(&self) -> i64 { self.v }
}

fn get_key<K: KeyValue>(kv: &K) -> <K as KeyValue>::Key {
    kv.key()
}

fn main() {
    let e: Entry = Entry { k: 42i64, v: 99i64 };
    println!("{}", get_key(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "multi-assoc generic get_key should compile");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_multi_assoc_generic_get_value() {
    let code = r#"
trait KeyValue {
    type Key;
    type Value;
    fn key(&self) -> Self::Key;
    fn value(&self) -> Self::Value;
}

struct Entry { k: i64, v: i64 }

impl KeyValue for Entry {
    type Key = i64;
    type Value = i64;
    fn key(&self) -> i64 { self.k }
    fn value(&self) -> i64 { self.v }
}

fn get_value<K: KeyValue>(kv: &K) -> <K as KeyValue>::Value {
    kv.value()
}

fn main() {
    let e: Entry = Entry { k: 42i64, v: 99i64 };
    println!("{}", get_value(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "multi-assoc generic get_value should compile");
    assert_eq!(stdout.trim(), "99");
}

#[test]
fn stage147_multi_assoc_generic_both_methods() {
    let code = r#"
trait KeyValue {
    type Key;
    type Value;
    fn key(&self) -> Self::Key;
    fn value(&self) -> Self::Value;
}

struct Entry { k: i64, v: i64 }

impl KeyValue for Entry {
    type Key = i64;
    type Value = i64;
    fn key(&self) -> i64 { self.k }
    fn value(&self) -> i64 { self.v }
}

fn get_key<K: KeyValue>(kv: &K) -> <K as KeyValue>::Key {
    kv.key()
}

fn get_value<K: KeyValue>(kv: &K) -> <K as KeyValue>::Value {
    kv.value()
}

fn main() {
    let e: Entry = Entry { k: 42i64, v: 99i64 };
    println!("{} {}", get_key(&e), get_value(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "both generic methods should compile");
    assert_eq!(stdout.trim(), "42 99");
}

#[test]
fn stage147_multi_assoc_generic_multiple_types() {
    let code = r#"
trait KeyValue {
    type Key;
    type Value;
    fn key(&self) -> Self::Key;
    fn value(&self) -> Self::Value;
}

struct EntryA { k: i64, v: i64 }
struct EntryB { k: i64, v: i64 }

impl KeyValue for EntryA {
    type Key = i64;
    type Value = i64;
    fn key(&self) -> i64 { self.k }
    fn value(&self) -> i64 { self.v }
}

impl KeyValue for EntryB {
    type Key = i64;
    type Value = i64;
    fn key(&self) -> i64 { self.k }
    fn value(&self) -> i64 { self.v }
}

fn get_key<K: KeyValue>(kv: &K) -> <K as KeyValue>::Key {
    kv.key()
}

fn main() {
    let a: EntryA = EntryA { k: 10i64, v: 20i64 };
    let b: EntryB = EntryB { k: 30i64, v: 40i64 };
    println!("{} {}", get_key(&a), get_key(&b));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(
        exit, 0,
        "multiple types with multi-assoc generic should compile"
    );
    assert_eq!(stdout.trim(), "10 30");
}

// ===========================================================================
// Multiple methods with same-name trait
// ===========================================================================

#[test]
fn stage147_three_methods_same_trait() {
    let code = r#"
trait Calculator {
    type Result;
    fn add(&self, a: i64, b: i64) -> Self::Result;
    fn sub(&self, a: i64, b: i64) -> Self::Result;
    fn mul(&self, a: i64, b: i64) -> Self::Result;
}

struct BasicCalc;

impl Calculator for BasicCalc {
    type Result = i64;
    fn add(&self, a: i64, b: i64) -> i64 { a + b }
    fn sub(&self, a: i64, b: i64) -> i64 { a - b }
    fn mul(&self, a: i64, b: i64) -> i64 { a * b }
}

fn main() {
    let c: BasicCalc = BasicCalc;
    println!("{} {} {}", c.add(10i64, 5i64), c.sub(10i64, 5i64), c.mul(10i64, 5i64));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "three methods same trait should compile");
    assert_eq!(stdout.trim(), "15 5 50");
}

#[test]
fn stage147_three_methods_generic() {
    let code = r#"
trait Calculator {
    type Result;
    fn add(&self, a: i64, b: i64) -> Self::Result;
    fn sub(&self, a: i64, b: i64) -> Self::Result;
    fn mul(&self, a: i64, b: i64) -> Self::Result;
}

struct BasicCalc;

impl Calculator for BasicCalc {
    type Result = i64;
    fn add(&self, a: i64, b: i64) -> i64 { a + b }
    fn sub(&self, a: i64, b: i64) -> i64 { a - b }
    fn mul(&self, a: i64, b: i64) -> i64 { a * b }
}

fn calc_add<C: Calculator>(c: &C, a: i64, b: i64) -> <C as Calculator>::Result {
    c.add(a, b)
}

fn main() {
    let c: BasicCalc = BasicCalc;
    println!("{}", calc_add(&c, 20i64, 22i64));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "three methods generic should compile");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_method_order_reversed() {
    // Test that method order in trait doesn't affect resolution.
    // value() declared BEFORE key().
    let code = r#"
trait KeyValue {
    type Key;
    type Value;
    fn value(&self) -> Self::Value;
    fn key(&self) -> Self::Key;
}

struct Entry { k: i64, v: i64 }

impl KeyValue for Entry {
    type Key = i64;
    type Value = i64;
    fn value(&self) -> i64 { self.v }
    fn key(&self) -> i64 { self.k }
}

fn get_key<K: KeyValue>(kv: &K) -> <K as KeyValue>::Key {
    kv.key()
}

fn main() {
    let e: Entry = Entry { k: 42i64, v: 99i64 };
    println!("{}", get_key(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "reversed method order should compile");
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// FnMut/FnOnce trait impls (regression — was broken by Stage 146)
// ===========================================================================

#[test]
fn stage147_fn_mut_trait_impl_compiles() {
    let code = r#"
struct Counter { val: i64 }

impl FnMut<(i64,)> for Counter {
    type Output = i64;
    fn call_mut(&mut self, args: (i64,)) -> i64 {
        self.val = self.val + args.0;
        self.val
    }
}

fn main() {
    let mut c: Counter = Counter { val: 0i64 };
    println!("{}", c.call_mut((21i64,)));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "FnMut impl should compile");
    assert_eq!(stdout.trim(), "21");
}

#[test]
fn stage147_fn_once_trait_impl_compiles() {
    let code = r#"
struct Consumer { val: i64 }

impl FnOnce<(i64,)> for Consumer {
    type Output = i64;
    fn call_once(self, args: (i64,)) -> i64 {
        self.val + args.0
    }
}

fn main() {
    let c: Consumer = Consumer { val: 20i64 };
    println!("{}", c.call_once((22i64,)));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "FnOnce impl should compile");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_fn_trait_impl_compiles() {
    let code = r#"
struct Adder { offset: i64 }

impl Fn<(i64,)> for Adder {
    type Output = i64;
    fn call(&self, args: (i64,)) -> i64 {
        self.offset + args.0
    }
}

fn main() {
    let a: Adder = Adder { offset: 10i64 };
    println!("{}", a.call((32i64,)));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Fn impl should compile");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_fn_mut_call_mut_mutates_state() {
    let code = r#"
struct Counter { val: i64 }

impl FnMut<(i64,)> for Counter {
    type Output = i64;
    fn call_mut(&mut self, args: (i64,)) -> i64 {
        self.val = self.val + args.0;
        self.val
    }
}

fn main() {
    let mut c: Counter = Counter { val: 10i64 };
    let r1: i64 = c.call_mut((5i64,));
    let r2: i64 = c.call_mut((5i64,));
    println!("{} {}", r1, r2);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "FnMut should mutate state");
    assert_eq!(stdout.trim(), "15 20");
}

// ===========================================================================
// Three+ assoc types
// ===========================================================================

#[test]
fn stage147_three_assoc_types() {
    let code = r#"
trait Triple {
    type A;
    type B;
    type C;
    fn get_a(&self) -> Self::A;
    fn get_b(&self) -> Self::B;
    fn get_c(&self) -> Self::C;
}

struct TripleHolder { a: i64, b: i64, c: i64 }

impl Triple for TripleHolder {
    type A = i64;
    type B = i64;
    type C = i64;
    fn get_a(&self) -> i64 { self.a }
    fn get_b(&self) -> i64 { self.b }
    fn get_c(&self) -> i64 { self.c }
}

fn main() {
    let t: TripleHolder = TripleHolder { a: 1i64, b: 2i64, c: 3i64 };
    println!("{} {} {}", t.get_a(), t.get_b(), t.get_c());
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "three assoc types should compile");
    assert_eq!(stdout.trim(), "1 2 3");
}

#[test]
fn stage147_three_assoc_types_generic() {
    let code = r#"
trait Triple {
    type A;
    type B;
    type C;
    fn get_a(&self) -> Self::A;
    fn get_b(&self) -> Self::B;
    fn get_c(&self) -> Self::C;
}

struct TripleHolder { a: i64, b: i64, c: i64 }

impl Triple for TripleHolder {
    type A = i64;
    type B = i64;
    type C = i64;
    fn get_a(&self) -> i64 { self.a }
    fn get_b(&self) -> i64 { self.b }
    fn get_c(&self) -> i64 { self.c }
}

fn get_a<T: Triple>(t: &T) -> <T as Triple>::A {
    t.get_a()
}

fn main() {
    let t: TripleHolder = TripleHolder { a: 42i64, b: 99i64, c: 0i64 };
    println!("{}", get_a(&t));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "three assoc types generic should compile");
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn stage147_display_trait_still_works() {
    // Regression: prelude Display trait (fmt method is bodyless) must still work.
    let code = r#"
fn main() {
    let n: i64 = 42i64;
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Display trait regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_clone_trait_still_works() {
    // Regression: prelude Clone trait (clone method is bodied) must still work.
    let code = r#"
fn main() {
    let n: i32 = 42i32;
    let m: i32 = n.clone();
    println!("{}", m);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Clone trait regression");
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// Regression — existing trait code
// ===========================================================================

#[test]
fn stage147_regression_simple_trait() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i64;
}

struct English;

impl Greeter for English {
    fn greet(&self) -> i64 { 42i64 }
}

fn main() {
    let e: English = English;
    println!("{}", e.greet());
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "simple trait regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_regression_generic_function() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i64;
}

struct English;

impl Greeter for English {
    fn greet(&self) -> i64 { 42i64 }
}

fn greet_all<G: Greeter>(g: &G) -> i64 {
    g.greet()
}

fn main() {
    let e: English = English;
    println!("{}", greet_all(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "generic function regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage147_regression_dyn_trait() {
    // dyn Trait dispatch regression — must still work.
    let code = r#"
trait Greeter {
    fn greet(&self) -> i64;
}

struct English;

impl Greeter for English {
    fn greet(&self) -> i64 { 42i64 }
}

fn use_greeter(g: &dyn Greeter) -> i64 {
    g.greet()
}

fn main() {
    let e: English = English;
    println!("{}", use_greeter(&e));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "dyn trait regression");
    assert_eq!(stdout.trim(), "42");
}
