use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dotx_core::{SeedParams, SeederFactory, AlgorithmParams};
use dotx_core::seed::{
    kmer::KmerPresets,
    syncmer::SyncmerPresets,
    strobemer::StrobemerPresets,
};

fn generate_test_sequence(length: usize) -> Vec<u8> {
    let pattern = b"ATCGATCG";
    let mut sequence = Vec::with_capacity(length);
    
    while sequence.len() < length {
        let remaining = length - sequence.len();
        let chunk_size = std::cmp::min(pattern.len(), remaining);
        sequence.extend_from_slice(&pattern[..chunk_size]);
    }
    
    sequence
}

fn bench_kmer_seeding(c: &mut Criterion) {
    let query = generate_test_sequence(10000);
    let target = generate_test_sequence(10000);
    
    let seeder = SeederFactory::create(&SeedParams {
        k: 15,
        algorithm_params: AlgorithmParams::Kmer,
        ..Default::default()
    });
    
    c.bench_function("kmer_10kb", |b| {
        b.iter(|| {
            let result = seeder.seed(
                black_box(&query),
                "query",
                black_box(&target), 
                "target",
                &SeedParams {
                    k: 15,
                    algorithm_params: AlgorithmParams::Kmer,
                    ..Default::default()
                }
            );
            black_box(result)
        })
    });
}

fn bench_syncmer_seeding(c: &mut Criterion) {
    let query = generate_test_sequence(10000);
    let target = generate_test_sequence(10000);
    
    let seeder = SeederFactory::create(&SeedParams {
        k: 15,
        algorithm_params: SyncmerPresets::default(),
        ..Default::default()
    });
    
    c.bench_function("syncmer_10kb", |b| {
        b.iter(|| {
            let result = seeder.seed(
                black_box(&query),
                "query",
                black_box(&target),
                "target", 
                &SeedParams {
                    k: 15,
                    algorithm_params: SyncmerPresets::default(),
                    ..Default::default()
                }
            );
            black_box(result)
        })
    });
}

fn bench_strobemer_seeding(c: &mut Criterion) {
    let query = generate_test_sequence(10000);
    let target = generate_test_sequence(10000);
    
    let seeder = SeederFactory::create(&SeedParams {
        k: 15,
        algorithm_params: StrobemerPresets::default(),
        ..Default::default()
    });
    
    c.bench_function("strobemer_10kb", |b| {
        b.iter(|| {
            let result = seeder.seed(
                black_box(&query),
                "query",
                black_box(&target),
                "target",
                &SeedParams {
                    k: 15,
                    algorithm_params: StrobemerPresets::default(),
                    ..Default::default()
                }
            );
            black_box(result)
        })
    });
}

fn bench_different_k_sizes(c: &mut Criterion) {
    let query = generate_test_sequence(5000);
    let target = generate_test_sequence(5000);
    
    let mut group = c.benchmark_group("kmer_k_sizes");
    
    for k in [10, 15, 20, 25].iter() {
        let seeder = SeederFactory::create(&SeedParams {
            k: *k,
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        });
        
        group.bench_with_input(format!("k_{}", k), k, |b, &k| {
            b.iter(|| {
                let result = seeder.seed(
                    black_box(&query),
                    "query",
                    black_box(&target),
                    "target",
                    &SeedParams {
                        k,
                        algorithm_params: AlgorithmParams::Kmer,
                        ..Default::default()
                    }
                );
                black_box(result)
            })
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_kmer_seeding,
    bench_syncmer_seeding,
    bench_strobemer_seeding,
    bench_different_k_sizes
);
criterion_main!(benches);