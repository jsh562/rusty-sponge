//! Criterion benches for `rusty-sponge`.
//!
//! Gated behind the `bench` cargo feature so `cargo install` stays minimal.
//! Run with: `cargo bench --features bench`
//!
//! Coverage (per plan §Performance Methodology):
//! - In-memory `Buffer::drain_reader` for sub-threshold inputs
//! - Spill-transition cost (drain past threshold)
//! - Atomic-rename cost via `SpongeBuilder::run` against a regular-file target
//! - Sponge-to-stdout passthrough
//!
//! End-to-end cold-start and throughput-vs-moreutils budgets are measured
//! externally via hyperfine (see plan §Performance Methodology); the criterion
//! benches here cover in-process micro-budgets only. Per plan Regression
//! Gating policy these are advisory-only at v0.1.0 — useful for spotting
//! regressions during local development before they reach CI.

#![cfg(feature = "bench")]

use std::io::Cursor;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use rusty_sponge::{SpongeBuilder, Target, buffer::Buffer};

/// Helper: synthesize a deterministic N-byte input that compresses to ~itself.
fn make_input(n: usize) -> Vec<u8> {
    // Use a 1 KiB rotating pattern so we exercise real read/write paths
    // without depending on /dev/urandom or PRNG state.
    let chunk: Vec<u8> = (0u8..=255u8).cycle().take(1024).collect();
    chunk.iter().cycle().copied().take(n).collect()
}

fn bench_buffer_drain_in_memory(c: &mut Criterion) {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut group = c.benchmark_group("buffer_drain_in_memory");
    group.sample_size(50);
    for &n in &[4 * 1024usize, 64 * 1024, 1024 * 1024] {
        let input = make_input(n);
        group.throughput(criterion::Throughput::Bytes(n as u64));
        group.bench_function(format!("{n}B"), |b| {
            b.iter(|| {
                let mut buf = Buffer::new();
                buf.drain_reader(Cursor::new(&input), n + 1, tmpdir.path())
                    .unwrap();
                std::hint::black_box(buf.len());
            });
        });
    }
    group.finish();
}

fn bench_buffer_drain_spilled(c: &mut Criterion) {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut group = c.benchmark_group("buffer_drain_spilled");
    group.sample_size(20);
    let n = 4 * 1024 * 1024; // 4 MiB, well over a low threshold
    let input = make_input(n);
    let threshold = 64 * 1024; // 64 KiB → forces spill early
    group.throughput(criterion::Throughput::Bytes(n as u64));
    group.bench_function("4MiB_through_64KiB_threshold", |b| {
        b.iter(|| {
            let mut buf = Buffer::new();
            buf.drain_reader(Cursor::new(&input), threshold, tmpdir.path())
                .unwrap();
            std::hint::black_box(buf.len());
        });
    });
    group.finish();
}

fn bench_sponge_atomic_rewrite(c: &mut Criterion) {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut group = c.benchmark_group("sponge_atomic_rewrite");
    group.sample_size(30);
    for &n in &[4 * 1024usize, 256 * 1024, 4 * 1024 * 1024] {
        let input = make_input(n);
        let target: PathBuf = tmpdir.path().join(format!("bench-{n}.bin"));
        group.throughput(criterion::Throughput::Bytes(n as u64));
        group.bench_function(format!("{n}B_to_disk"), |b| {
            b.iter(|| {
                let mut sponge = SpongeBuilder::new()
                    .target(Target::File(target.clone()))
                    .build()
                    .unwrap();
                sponge.run(Cursor::new(&input)).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_sponge_stdout_passthrough(c: &mut Criterion) {
    let mut group = c.benchmark_group("sponge_stdout_passthrough");
    group.sample_size(30);
    let n = 256 * 1024;
    let input = make_input(n);
    group.throughput(criterion::Throughput::Bytes(n as u64));
    group.bench_function(format!("{n}B_to_sink"), |b| {
        b.iter(|| {
            // Use an in-memory Vec as the "stdout" sink so the bench measures
            // the buffer pipeline cost, not terminal output.
            let mut sink = Vec::with_capacity(n);
            let mut buf = Buffer::new();
            buf.drain_reader(
                Cursor::new(&input),
                rusty_sponge::DEFAULT_SPILL_THRESHOLD,
                std::env::temp_dir().as_path(),
            )
            .unwrap();
            buf.write_to(&mut sink).unwrap();
            std::hint::black_box(sink.len());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_drain_in_memory,
    bench_buffer_drain_spilled,
    bench_sponge_atomic_rewrite,
    bench_sponge_stdout_passthrough,
);
criterion_main!(benches);

#[cfg(not(feature = "bench"))]
fn main() {
    eprintln!("rusty-sponge: rebuild with --features bench to run throughput benches");
}
