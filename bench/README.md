# Performance Benchmarking

This directory contains benchmarking harnesses and performance testing tools.

## Structure

- `src/` - Benchmarking source code
  - `genome_loading.rs` - Genome file loading benchmarks
  - `alignment.rs` - Sequence alignment benchmarks
  - `rendering.rs` - GPU rendering performance tests
  - `memory.rs` - Memory usage profiling
- `data/` - Benchmark-specific test data
- `results/` - Benchmark results and reports
- `scripts/` - Analysis and reporting scripts

## Running Benchmarks

```bash
# Run all benchmarks
make bench

# Run specific benchmark category  
cargo bench --package dotx-bench genome_loading

# Generate performance report
cargo bench -- --output-format html
```

## Performance Targets

- Genome loading: > 100MB/s for uncompressed FASTA
- Dot plot generation: < 5s for 10M anchor points
- GPU rendering: > 60 FPS at 1080p
- Memory usage: < 4GB for typical workflows