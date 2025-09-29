use criterion::{criterion_group, criterion_main, Criterion};
use std::process::Command;

fn benchmark_cmon_status(c: &mut Criterion) {
    c.bench_function("cmon status", |b| {
        b.iter(|| {
            Command::new("./target/release/cmon")
                .arg("status")
                .output()
                .expect("Failed to execute cmon status")
        })
    });
}

fn benchmark_cmon_jobs(c: &mut Criterion) {
    c.bench_function("cmon jobs", |b| {
        b.iter(|| {
            Command::new("./target/release/cmon")
                .arg("jobs")
                .output()
                .expect("Failed to execute cmon jobs")
        })
    });
}

fn benchmark_cmon_nodes(c: &mut Criterion) {
    c.bench_function("cmon nodes", |b| {
        b.iter(|| {
            Command::new("./target/release/cmon")
                .arg("nodes")
                .output()
                .expect("Failed to execute cmon nodes")
        })
    });
}

criterion_group!(benches, benchmark_cmon_status, benchmark_cmon_jobs, benchmark_cmon_nodes);
criterion_main!(benches);