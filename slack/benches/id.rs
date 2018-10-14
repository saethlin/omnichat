#[macro_use]
extern crate criterion;
use criterion::Criterion;
extern crate slack_api;
use slack_api::GroupId;

#[derive(Debug, Default)]
pub struct DenseId([u8; 8]);

impl<'a> From<&'a str> for DenseId {
    fn from(input: &'a str) -> Self {
        let mut output = Self::default();
        output.0.copy_from_slice(&input.as_bytes()[1..]);
        for i in 0..8 {
            output.0[i] |= ((1u8 << i) & input.as_bytes()[0]) << (7 - i)
        }
        output
    }
}

#[derive(Debug, Default)]
pub struct SimpleId([u8; 9]);

impl<'a> From<&'a str> for SimpleId {
    fn from(input: &'a str) -> Self {
        let mut output = Self::default();
        output.0.copy_from_slice(&input.as_bytes());
        output
    }
}

#[derive(Default)]
pub struct TypedId([u8; 8]);

impl<'a> From<&'a str> for TypedId {
    fn from(input: &'a str) -> Self {
        assert!(input.as_bytes()[0] == b'G');
        let mut output = Self::default();
        output.0.copy_from_slice(&input.as_bytes()[1..]);
        output
    }
}

#[inline(never)]
fn make_simple(c: &mut Criterion) {
    let id_data = std::fs::read_to_string("id.txt").unwrap();
    c.bench_function("simple", move |b| {
        b.iter(|| SimpleId::from(id_data.as_str()));
    });
}

#[inline(never)]
fn make_dense(c: &mut Criterion) {
    let id_data = std::fs::read_to_string("id.txt").unwrap();
    c.bench_function("dense", move |b| {
        b.iter(|| DenseId::from(id_data.as_str()));
    });
}

#[inline(never)]
fn make_typed(c: &mut Criterion) {
    let id_data = std::fs::read_to_string("id.txt").unwrap();
    c.bench_function("typed", move |b| {
        b.iter(|| TypedId::from(id_data.as_str()));
    });
}

#[inline(never)]
fn make_flexible(c: &mut Criterion) {
    let id_data = std::fs::read_to_string("id.txt").unwrap();
    c.bench_function("flexible", move |b| {
        b.iter(|| GroupId::from(id_data.as_str()));
    });
}

criterion_group!(benches, make_simple, make_dense, make_typed, make_flexible);
criterion_main!(benches);
