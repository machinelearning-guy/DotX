# DOTx Makefile
# Build and development commands for the genome visualization toolkit

.PHONY: setup build run-ui build-cli build-cli-pdf import-demo render-demo-pdf run-cli test bench dist clean help

# Default target
help:
	@echo "DOTx Development Commands:"
	@echo ""
	@echo "Development:"
	@echo "  make setup     - Install toolchains, dependencies, and pre-commit hooks"
	@echo "  make build     - Build core, cli, and ui (debug mode)"
	@echo "  make build-cli - Build only the dotx CLI (debug)"
	@echo "  make build-cli-pdf - Build the CLI with PDF export enabled (dotx-gpu/printpdf)"
	@echo "  make import-demo - Import data/demo.paf -> data/demo.dotxdb (build tiles)"
	@echo "  make render-demo-pdf - Render out/demo.pdf from data/demo.dotxdb (requires PDF build)"
	@echo "  make run-ui    - Launch desktop UI application"
	@echo "  make test      - Run all unit and integration tests"
	@echo "  make bench     - Run performance benchmarks"
	@echo ""
	@echo "Release:"
	@echo "  make dist      - Build signed installers and CLI binaries (Linux/macOS/Windows)"
	@echo ""
	@echo "Utility:"
	@echo "  make clean     - Clean build artifacts"
	@echo "  make fmt       - Format code"
	@echo "  make clippy    - Run Clippy lints"

# Development targets
setup:
	@echo "Setting up development environment..."
	@# Install Rust if not present
	@command -v rustc >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	@# Update Rust toolchain
	rustup update stable
	@# Install required components
	rustup component add rustfmt clippy
	@# Install Tauri CLI
	cargo install tauri-cli@1.5.0
	@# Install Node.js dependencies (if package.json exists in dotx-gui/)
	@if [ -f dotx-gui/package.json ]; then \
		cd dotx-gui && npm install; \
	fi
	@echo "Development environment setup complete!"

build:
	@echo "Building DOTx workspace (debug mode)..."
	cargo build --workspace

build-cli:
	@echo "Building dotx CLI (debug)..."
	cargo build -p dotx-cli

build-cli-pdf:
	@echo "Building dotx CLI with PDF export enabled..."
	cargo build -p dotx-cli --features pdf

import-demo: build-cli
	@echo "Importing data/demo.paf -> data/demo.dotxdb with tiles..."
	@if [ ! -f data/demo.paf ]; then \
		echo "Missing data/demo.paf. Place a PAF at data/demo.paf or edit the Makefile target."; \
		exit 1; \
	fi
	@mkdir -p data
	cargo run -p dotx-cli -- import --input data/demo.paf --db data/demo.dotxdb --build-tiles

render-demo-pdf: build-cli-pdf
	@echo "Rendering demo PDF from data/demo.dotxdb..."
	@if [ ! -f data/demo.dotxdb ]; then \
		echo "Missing data/demo.dotxdb. Create one first (e.g., via 'dotx import')."; \
		exit 1; \
	fi
	@mkdir -p out
	cargo run -p dotx-cli --features pdf -- render --db data/demo.dotxdb --out out/demo.pdf

build-release:
	@echo "Building DOTx workspace (release mode)..."
	cargo build --workspace --release

run-ui:
	@echo "Launching DOTx desktop UI..."
	cd dotx-gui && cargo tauri dev

run-cli:
	@echo "Running DOTx CLI..."
	cargo run --bin dotx -- --help

test:
	@echo "Running all tests (default features)..."
	cargo test --workspace

test-integration:
	@echo "Running integration tests (default features)..."
	cargo test --workspace --test '*'

test-all-features:
	@echo "Running all tests with all features enabled (may be unstable)..."
	cargo test --workspace --all-features

bench:
	@echo "Running performance benchmarks..."
	cargo bench --workspace

# Release targets
dist: build-release
	@echo "Building distribution packages..."
	@# Build CLI binaries for current platform
	cargo build --release --bin dotx
	@# Build Tauri app for current platform
	cd dotx-gui && cargo tauri build
	@echo "Distribution build complete. Check target/release/ for binaries."

# Utility targets
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@if [ -d dotx-gui/dist ]; then rm -rf dotx-gui/dist; fi
	@if [ -d dotx-gui/target ]; then rm -rf dotx-gui/target; fi
	# Note: legacy `ui/` cleanup removed to avoid confusion (see plan.md)

fmt:
	@echo "Formatting code..."
	cargo fmt --all

clippy:
	@echo "Running Clippy lints..."
	cargo clippy --workspace --all-targets --all-features -- -D warnings

check:
	@echo "Running cargo check..."
	cargo check --workspace --all-targets --all-features

# Development workflow
dev: fmt clippy test

# Full CI pipeline
ci: fmt clippy test bench

# Install locally built binaries
install:
	@echo "Installing DOTx CLI locally..."
	cargo install --path cli --force

# Generate documentation
docs:
	@echo "Generating documentation..."
	cargo doc --workspace --all-features --no-deps --open
