use criterion::{black_box, criterion_group, criterion_main, Criterion};
use plurimath_numeric::evaluate_json;
use serde_json::Value;

fn gum_f40_requests() -> Vec<String> {
    let corpus: Value = serde_json::from_str(include_str!("../fixtures/gum-f40.json")).unwrap();
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["request"].to_string())
        .collect()
}

fn evaluate_gum(c: &mut Criterion) {
    let requests = gum_f40_requests();
    c.bench_function("evaluate_json/gum-f40-cases", |b| {
        b.iter(|| {
            for request in &requests {
                black_box(evaluate_json(black_box(request)));
            }
        })
    });
}

criterion_group!(benches, evaluate_gum);
criterion_main!(benches);
