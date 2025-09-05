DOTx — Install & Quickstart

Requirements
- Rust (stable) with `rustup`, `rustfmt`, `clippy`
- Node.js (LTS) for GUI
- Tauri CLI (`cargo install tauri-cli`)

Setup
- make setup

Build
- make build

Run CLI
- cargo run --bin dotx -- --help

Run GUI (dev)
- make run-ui

Release builds
- make dist

Notes
- `make test` runs default-feature tests; `make test-all-features` is slower and optional.
- If `dotx-gui/` is missing, the `dotx gui` command and `make run-ui` will error; clone with the repo.

