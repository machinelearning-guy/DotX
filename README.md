# DOTx — Supercharged Dot-Plot Engine

Fast, precise dot-plotting from plasmids to whole genomes. Headless CLI and a simple desktop GUI, with deterministic rendering and export (SVG/PNG/PDF).

Quick links
- Install & Quickstart: docs/INSTALL.md
- UX Cheatsheet: docs/UX.md
- Project Plan: plan.md

Quickstart
- Build: make build
- Import demo (expects data/demo.paf): make import-demo
- Render demo PDF: make render-demo-pdf
- Launch GUI (dev): make run-ui
- Tests: make test

Notes
- CLI binary is `dotx` (package `dotx-cli`). GUI lives in `dotx-gui/` and is excluded from the Rust workspace to keep core builds lean.
- PDF export is behind the `pdf` feature (enable via `cargo ... --features pdf` or `make build-cli-pdf`).
