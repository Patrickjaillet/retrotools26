//! DAT parsing throughput on a synthetic DAT sized like a real No-Intro
//! platform DAT (a few thousand entries). Run with
//! `cargo bench -p retrotools-core --bench dat_parsing`. No fixed baseline
//! is checked in — see `hashing.rs` for why.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use retrotools_core::dat::parse_dat_str;
use std::fmt::Write as _;

fn build_dat(game_count: usize) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n<datafile>\n  <header><name>Bench DAT</name><version>1</version></header>\n",
    );
    for i in 0..game_count {
        writeln!(
            xml,
            "  <game name=\"Bench Game {i:05} (Europe)\"><rom name=\"Bench Game {i:05} (Europe).bin\" size=\"1\" crc=\"{:08x}\"/></game>",
            i as u32
        )
        .unwrap();
    }
    xml.push_str("</datafile>\n");
    xml
}

fn bench_parse_dat(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_dat_str");
    for game_count in [500usize, 3_000] {
        let xml = build_dat(game_count);
        group.bench_with_input(BenchmarkId::from_parameter(game_count), &xml, |b, xml| {
            b.iter(|| parse_dat_str(black_box(xml), "Bench").unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse_dat);
criterion_main!(benches);
