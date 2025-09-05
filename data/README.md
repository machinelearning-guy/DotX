# Test Data

This directory contains test genome data and reference files for development and testing.

## Structure

- `genomes/` - Sample FASTA files for testing
  - `small/` - Small test sequences (< 1MB)
  - `medium/` - Medium test sequences (1-100MB)  
  - `large/` - Large test sequences (> 100MB)
- `expected/` - Reference output files for validation
  - `dotplots/` - Expected dot plot outputs
  - `alignments/` - Expected alignment results
- `benchmarks/` - Performance benchmarking datasets

## Usage

Test data is used by:
- Unit tests (`cargo test`)
- Integration tests
- Benchmarks (`cargo bench`)
- Manual testing and development

## Download

Large test files may need to be downloaded separately:

```bash
# Download test data (future script)
make download-test-data
```