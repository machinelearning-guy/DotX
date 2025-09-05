# DOTx Python Bindings

This directory will contain Python bindings for the DOTx core library using pyo3.

## Future Structure

- `pyproject.toml` - Python package configuration
- `src/lib.rs` - PyO3 bindings implementation  
- `python/` - Python wrapper code and examples
- `tests/` - Python-specific tests

## Installation (Future)

```bash
pip install dotx
```

## Usage (Future)

```python
import dotx

# Load genome sequences
genome1 = dotx.load_fasta("genome1.fa")
genome2 = dotx.load_fasta("genome2.fa") 

# Generate dot plot
plot = dotx.generate_dotplot(genome1, genome2)
plot.save("output.svg")
```