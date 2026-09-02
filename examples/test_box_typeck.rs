use landin_compiler::compile;

fn main() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
}
