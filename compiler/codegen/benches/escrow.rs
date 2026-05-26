use criterion::{criterion_group, criterion_main, Criterion};
use kaspascript_codegen::compile_file;

fn bench_escrow_pipeline(c: &mut Criterion) {
    let source = include_str!("../../../tests/contracts/escrow.ks");
    c.bench_function("compile escrow", |b| {
        b.iter(|| compile_file(source, "escrow.ks").expect("escrow compiles"))
    });
}

criterion_group!(benches, bench_escrow_pipeline);
criterion_main!(benches);
