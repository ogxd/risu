use criterion::{criterion_group, criterion_main, Criterion};
use risu::ArenaLinkedList;

fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("ArenaLinkedList"));

    group.bench_function("Add first", |b| {
        let mut list = ArenaLinkedList::new_with_capacity(4);
        b.iter(|| list.add_first(1));
    });

    group.bench_function("Add last", |b| {
        let mut list = ArenaLinkedList::new_with_capacity(4);
        b.iter(|| list.add_last(1));
    });

    group.bench_function("Remove", |b| {
        let mut list = ArenaLinkedList::new_with_capacity(4);
        let index = list.add_first(1).unwrap();
        b.iter(|| list.remove(index));
    });

    group.finish();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
