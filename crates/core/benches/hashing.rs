//! Hashing throughput on a large in-memory buffer, standing in for a big
//! ROM file. Run with `cargo bench -p retrotools-core --bench hashing`.
//! There's no fixed baseline checked in (this is a dev machine, not a
//! stable CI runner) — this exists so a future change to `hash.rs` can be
//! compared before/after, not as a pass/fail gate.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use retrotools_core::hash::compute_hashes;
use std::io::Cursor;

fn bench_compute_hashes(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_hashes");
    for size_mb in [1usize, 8, 32] {
        let size = size_mb * 1024 * 1024;
        // Not all-zero: a real ROM isn't a single repeated byte, and some
        // hash implementations special-case runs of zeros.
        let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(format!("{size_mb}MB")), &data, |b, data| {
            b.iter(|| compute_hashes(Cursor::new(black_box(data.as_slice()))).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_compute_hashes);
criterion_main!(benches);
