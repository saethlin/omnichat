#[macro_use]
extern crate criterion;
extern crate serde_json;
extern crate slack_api;
use criterion::Criterion;

use slack_api::channels::HistoryResponse;

fn history_1000(c: &mut Criterion) {
    let the_json = std::fs::read_to_string("general.txt").unwrap();

    c.bench_function("history_1000", move |b| {
        b.iter(|| ::serde_json::from_str::<HistoryResponse>(&the_json).unwrap())
    });
}

criterion_group!(benches, history_1000);
criterion_main!(benches);
