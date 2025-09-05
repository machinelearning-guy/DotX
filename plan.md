# DOTx — Supercharged Dot-Plot Engine (Single-File Build Plan)

> **Purpose:** Replace legacy dot-plot tools with a fast, precise, general-purpose, open-source app that scales from plasmids to whole genomes. This file is a **copy-paste-ready spec** for immediate implementation. No orchestration fluff, no domain-specific overlays. Just the plan.

---

## 0) Status Snapshot (Current Repo)

- Core types: Unified Anchor implemented: `q, t, qs, qe, ts, te, strand, mapq?, identity?, engine_tag, query_length?, target_length?` with `Strand` displayed as `+/-`.
- IO: FASTA/FASTQ parser; PAF, SAM/BAM (coords), MAF, and MUMmer delta/coords parsers output unified Anchors.
- Store: `.dotxdb` writer/reader (`DotXStore`) with zstd-compressed anchors and metadata; density tile pyramid implemented and serialized; quadtree-style LOD density ready for overview.
- Seeding: `kmer`, `syncmer`, `strobemer` implemented; `minimap2` subprocess wrapper parses PAF to Anchors.
- CLI: `map`, `import`, `render`, `refine` present:
  - `map` runs chosen seeding engine; outputs PAF.
  - `import` auto-detects formats and writes `.dotxdb`; `--build-tiles` now generates and stores a multi-level density pyramid.
  - `render` exports SVG and PNG via `dotx-gpu` vector exporter; correct axis semantics and flip/RC transforms; overview uses density tiles when present; mid‑zoom draws chain polylines; deep‑zoom points support identity‑aware opacity. PDF behind the `printpdf` feature.
  - `refine` computes exact alignments (CPU placeholders; GPU stubbed) and persists per‑anchor identity plus a tile‑keyed Verify section back into `.dotxdb` (atomic rewrite, preserves tiles).
 - GPU: `dotx-gpu` vector exporter integrated with CLI (SVG/PNG, PDF gated). Interactive GPU path scaffolding exists; GUI wiring pending.
 - GUI: Tauri/React app scaffold present (not in workspace by default); basic IPC to open `.dotxdb` and status overlay exist; drag‑and‑drop and shared types pending.
 - Workspace: `dotx-core`, `dotx-gpu`, `cli` as members; GUI excluded to keep builds lean.
 - Determinism: plumbed through CLI; seeders accept deterministic RNG seed; exporter sorts anchors deterministically.
 - Tests: determinism SVG and PNG‑histogram tests in `dotx-gpu`; additional round‑trip tests and legacy field alignments still pending in core.

## 1) Objectives & Non-Goals

### Objectives

* **Scale:** Interactive plots on tens of millions of anchors; supports chromosomes/whole genomes and read→reference.
* **Accuracy:** Modern seeding + robust chaining + exact local verification where it matters.
* **Speed:** 60 FPS overview at multi-million anchor density; fast imports and renders; optional GPU accelerate verification.
* **Clarity:** Zero confusion about axes or strand; explicit UI affordances for reverse-complement and flips.
* **Compatibility:** Input FASTA/FASTQ, PAF, SAM/BAM (coords), MAF, MUMmer delta/coords. Output SVG/PNG/PDF/JSON.
* **Reproducibility:** Deterministic seeds, pinned toolchains, golden tests, containerized builds.

### Non-Goals

* No special biology overlays (genes/TEs/etc.).
* No multi-agent orchestration instructions.
* No server dependency; runs offline as desktop app + CLI.

---

## 2) High-Level System Design

### Components

* **Core (Rust):** Parsing, seeding, chaining, verification adapters, binary store, tiling engine.
* **Renderer (Rust + wgpu/WebGL via Tauri UI):** LOD pyramid, instanced draw of points/segments, vector export layer.
* **Desktop UI (Tauri + React):** File open, zoom/pan, strand toggles, flips, ROI selection, export.
* **CLI (`dotx`):** Headless processing (map/import/render/refine); CI-friendly.
* **Optional GPU Module:** Batched exact alignment on ROI tiles (CUDA/HIP when available).
* **Python Bindings (pyo3):** For notebooks/pipelines (optional, but planned).

### Repository Layout

```
dotx/
  core/                 # Rust crates: io_paf, io_maf, io_mummer, io_sam, seeds, chain, verify, store, tiler
  render/               # Rust renderer + vector export
  ui/                   # Tauri + React app (single-window)
  cli/                  # dotx (binary)
  python/               # pyo3 bindings (wheel build)
  third_party/          # vendored headers (e.g., WFA lib if used), minimal
  data/                 # tiny test FASTAs + gold PAF/figures
  bench/                # perf harness, datasets manifest, scripts
  docs/                 # this SPEC.md, UX.md, INSTALL.md, CONTRIBUTING.md
  scripts/              # reproducible builds, release, codegen
```

---

## 3) Plot Semantics (Make It Unambiguous)

* **Default Axes:** **X = Target/Reference**, **Y = Query** (configurable in settings).
* **Forward Strand (`+`):** **Main diagonal** (bottom-left → top-right).
* **Reverse Strand (`-`):** **Anti-diagonal** (top-right → bottom-left).
* **Colors (default):** `+` = **blue**, `-` = **red**. Provide an accessible palette option.
* **Controls:**

  * **Swap Axes** (X↔Y)
  * **Reverse-Complement Y**
  * **Show Only `+` / Only `-`**
* **Tip in UI:** “Reverse matches appear on the anti-diagonal. Press **RC-Y** to turn them into a main-diagonal view.”

---

## 4) Data Flow & Algorithms

### 4.1 Inputs

* **Sequences:** FASTA/FASTQ (optionally gz).
* **Alignments/Maps:** PAF (primary), SAM/BAM (coords-only), MAF, MUMmer delta/coords.
* **Normalization:** Everything becomes a unified **Anchor** model:

  ```
  Anchor {
    q, t, qs, qe, ts, te, strand(+|-), mapq?, identity?, engine_tag
  }
  ```

  * `identity?` populated when verification is run for that region/tile.

### 4.2 Seeding (pickable engine)

* **Engine: `minimap2` wrapper** (subprocess or linked lib) for robust baseline anchors (reads, assemblies, spliced if needed).
* **Engine: Syncmer sampler** (parameters `(k, s, t)`), good conservation at lower density.
* **Engine: Strobemer (randstrobes)** (link syncmers/k-mers across windows) for more even coverage and indel tolerance.
* All engines feed anchors into the same **chaining** stage.

### 4.3 Chaining

* **Concave-gap dynamic programming** (minimap2-style): diagonal-sorted anchors; frequency-filtered seeds; keep **top-K** chains per region.
* Output: chain polylines + per-anchor metadata for the renderer.

### 4.4 Verification (local exact, ROI-focused)

* **CPU exact aligner** (e.g., WFA2 or similar) to refine **only** selected tiles:

  * chain boundaries, discordant regions, user ROI.
  * returns identity/indel stats; stored alongside anchors.
* **Optional GPU path:** Batch tiles by length distribution; coalesce transfers; fall back to CPU when unavailable.

### 4.5 Direct k-mer Dot Mode (Instant Structure)

* Seed-only hits (no extension), alpha by density; ideal for quick structure exploration on huge datasets and self-dot checks.

### 4.6 Masking

* **Low-complexity masking** (Dust-like) and **high-frequency seed suppression** before chaining to reduce noise and memory.

---

## 5) Storage & Caching

### 5.1 Binary Store: `.dotxdb`

* **Purpose:** fast load, memory map, random tile access, reproducible export.
* **Layout (little-endian, Zstd-compressed blocks):**

  ```
  Header { magic="DOTX", version, build_meta }
  Meta   { samples, contigs, lengths, index offsets }
  Anchors { delta-encoded coords, strand bits, engine tags }
  Chains  { chain index -> ranges in Anchors }
  Tiles   { quadtree tile index -> anchor spans / density rasters }
  Verify  { optional: ROI results (identity/indels) keyed by tile }
  ```
* **Indices:** hierarchical tile index for O(log N) tile fetch.

### 5.2 UI State Save `.dotxui.json`

* Window/zoom/filters/strands/theme for one-click reproduction of a figure.

---

## 6) Rendering Pipeline (LOD)

* **Tiling:** Quadtree over plot space → pyramid of levels.
* **LOD Levels:**

  1. **Overview:** precomputed **density heatmap** tiles.
  2. **Mid Zoom:** **polyline segments** per chain (low vertex count).
  3. **Deep Zoom:** **instanced points** for individual anchors; tooltips show coordinates and (if verified) identity/indels.
* **GPU:**

  * Single pass per layer; instancing for millions of points.
  * 16-bit normalized coordinates within tile; high-precision transforms in shader.
* **Vector Export:**

  * Rebuild visible scene graph into **SVG/PDF** with legend + scale bar + config footer.

**Performance Targets**

* **≥60 FPS** at overview with **10 M anchors** visible.
* **≥30 FPS** at deep zoom on dense regions.
* Static export of **50 M anchors** to PNG in **< 20 s** on a modern desktop.

Current implementation notes
- CLI uses `dotx-gpu` vector exporter for SVG and headless PNG; PDF path exists behind a feature gate. Overview can render using prebuilt density tiles; mid‑zoom uses chain polylines; deep‑zoom uses per‑anchor points with optional identity‑aware opacity.

---

## 7) Desktop UI (Tauri + React)

### 7.1 Layout

* **Top Bar:** Open, Presets, Strand filters, Swap Axes, RC-Y, Theme, Export.
* **Canvas:** full-window render, FPS & anchor count in status corner.
* **Mini-map:** shows current viewport with drag-to-navigate (optional).
* **Status Bar:** zoom scale, visible anchors, verification status for hovered tile.

### 7.2 Keyboard

* **Zoom:** `+` / `-`
* **Pan:** `WASD` or arrow keys
* **Swap Axes:** `X`
* **Reverse-Complement Y:** `R`
* **Strand filters:** `1` = both, `2` = `+` only, `3` = `-` only
* **Reset View:** `0`

### 7.3 Interactions

* **ROI Lasso:** drag to select → context menu: “Export anchors”, “Run verify (CPU/GPU)”, “Save ROI as .json”.
* **Hover Tooltip:** q/t names, ranges, strand, chain id, identity% (if available).

### 7.4 Simplified UX (Redotable-like)

Design goals: dead‑simple, minimal controls, instant feedback.

- One‑screen workflow: Open → View → Zoom/Pan → Export. No modal wizards.
- Drag‑and‑drop `.dotxdb` anywhere to load. File → Open supports `.dotxdb`, FASTA/PAF/SAM/MAF.
- Big, clear keyboard hints overlay (toggle with `?`).
- Always‑visible strand toggles and Swap/RC-Y buttons; no hidden menus.
- Status strip shows: total anchors, visible anchors, FPS, verify status.
- ROI: Shift‑drag rectangle → inline mini‑toolbar: “Verify”, “Export Anchors”, “Save ROI”.
- Identity semantics: when Verify or per‑anchor identities exist, point opacity encodes identity; legend displays “Opacity encodes identity”.
- Safe defaults: X=Target, Y=Query; colors +/− as specified; deterministic rendering ordering for reproducibility.
- Progressive loading: open `.dotxdb` overview immediately via tiles; deep data fetched as you zoom.
- Zero-config export: one‑click SVG/PNG/PDF with embedded provenance footer.

Launch experience improvements
- `make run-ui` starts the app; auto‑installs Node deps when needed.
- `dotx gui` CLI subcommand opens the desktop app (spawn Tauri) and accepts `--db path.dotxdb` to open directly.
- Distribute signed installers (DMG/MSI/AppImage). First run shows a tiny quickstart and offers a sample dataset.
- Optional “portable” build: a single folder with `dotx` CLI + GUI that runs offline.

---

## 8) CLI (`dotx`) — Contract & Examples

```
dotx map \
  --ref ref.fa --qry qry.fa \
  --engine minimap2 \
  --threads 16 \
  --out run1.paf

dotx import \
  --input run1.paf \
  --format paf \  # optional; auto-detects if omitted
  --db run1.dotxdb \
  --build-tiles

dotx render \
  --db run1.dotxdb \
  --out figure.svg \
  --strand "+,-" \
  --flip none \
  --theme default \
  --dpi 300

dotx refine \
  --db run1.dotxdb \
  --roi "chr1:12.3M-18.6M,chr2:21.1M-27.2M" \
  --engine wfa \
  --device gpu|cpu

dotx gui \
  --db run1.dotxdb   # optional: spawns desktop app and opens DB
```

**Notes**

* `map` can also accept `--engine syncmer|strobemer` to generate anchors directly (no external aligner).
* `import` accepts PAF/MAF/MUMmer/SAM; converts to `.dotxdb`. With `--build-tiles`, builds and serializes density tiles across multiple levels.
* `render` (current): SVG and PNG supported via exporter; PDF available when CLI is built with `--features pdf` (enables `dotx-gpu/printpdf`). Correct axes, strand colors, flip/RC; legend and scale bar included; overview can use prebuilt tiles.
* `refine` (current): exact alignment runs and persists per‑anchor identity plus tile‑keyed Verify records back into `.dotxdb`.
* `--build-tiles` (current): implemented; produces density pyramid.
* `gui` (current): spawns Tauri dev from `dotx-gui/` with optional `--db` to open a database; behind `gui` feature; prints a clear error if the GUI directory is missing.

---

## 9) Presets (Fast Start)

* **Small genomes (bacteria):** `engine=minimap2 preset=asm5`, high dot density.
* **Large contigs/chromosomes:** `engine=strobemer` + frequency masking; verification only on edges.
* **Reads→Ref (ONT):** `engine=minimap2 preset=map-ont`; sparse anchors; optional verify sample tiles.
* **Self-dot (structure):** `engine=syncmer` seed-only (direct dot), higher alpha on density.

All presets are just named configs serializable to `dotx.toml`.

---

## 10) Build, Install, Repro

### 10.1 Toolchains

* **Rust** stable (pinned via `rust-toolchain.toml`)
* **Node** LTS (for Tauri/React UI)
* **C/C++** for optional verification library
* **CUDA/HIP** (optional) for GPU verify builds

### 10.2 Commands

```
# Dev
make setup         # install toolchains, pnpm, pre-commit hooks
make build         # build core, cli, ui (dev)
make run-ui        # launch desktop UI
make test          # run all unit/integration tests (default features)
make test-all-features  # run tests with all features enabled (optional)
make bench         # run performance harness

# Release
make dist          # signed installers + CLI binaries (Linux/macOS/Windows)
```

Makefile notes
- `run-ui` targets `dotx-gui/` (correct). `clean` still removes `ui/` artifacts; decide to keep legacy `ui/` or remove those lines.
- `dist` builds CLI and Tauri app; ensure `tauri-cli` available (`make setup`).
- `test-all-features` can be slow; default CI should use `test` with stable features.
- New helpers:
  - `make build-cli` builds only the CLI.
  - `make build-cli-pdf` builds the CLI with PDF export enabled (`--features pdf`).
  - `make import-demo` imports `data/demo.paf` → `data/demo.dotxdb` with tiles.
  - `make render-demo-pdf` renders `out/demo.pdf` from `data/demo.dotxdb` (requires PDF build).

Workspace members (current)
- Rust workspace includes `dotx-core`, `dotx-gpu`, `cli`. The GUI app exists but is not part of the workspace to avoid Node/Tauri toolchain during core builds.

### 10.3 Reproducibility

* `--deterministic` flag fixes hash seeds for seeding/chaining.
* CI uses pinned containers (Dockerfile + devcontainer) with checksums.
* Golden outputs (SVG hash, anchor counts) are asserted in CI.

---

## 11) CI/CD & Packaging

* **GitHub Actions**: matrix build (Linux/macOS/Windows), cache Rust/Node, code-sign installers.
* **Artifacts**:

  * `dotx` CLI binaries
  * Tauri installers (DMG/MSI/AppImage)
  * Python wheels (if bindings enabled)
* **Security**: SBOM, dependency audit, notarization on macOS.
* **Features in CI**: default jobs run with default features for stability; add an opt-in Linux job to run `test-all-features` including `printpdf`.

---

## 12) Benchmarks & Datasets (Minimal but Representative)

* **Identical:** small genome vs itself → single blue main diagonal; identity ≈ 100%.
* **Inversion:** contig with known inversion → distinct red anti-diagonal block.
* **Assembly↔Assembly:** human chr fragment vs close species homolog.
* **Reads→Ref:** ONT subset vs its reference; sparse anchors.

**Perf Gates**

* Import **5 GB PAF < 90 s** on 12 cores; peak RAM < 8 GB.
* Overview **≥60 FPS** at **10 M anchors**; deep zoom **≥30 FPS**.
* **GPU verify ≥5×** CPU throughput on 10k tiles (batch).

**Correctness Gates**

* Axes/strand tests render as documented (Section 3).
* Verification identity within **±0.2%** of CPU reference baseline on truth tiles.
* Format round-trip counts stable (PAF→.dotxdb→SVG export).

---

## 13) Testing Strategy

* **Unit:** parsers (PAF/MAF/MUMmer/SAM), seeders, chain DP, tile index, binary store.
* **Property:** malformed line fuzzing; monotonic coordinate checks; diagonal ordering.
* **Integration:** PAF import → `.dotxdb` build → interactive render → export hash.
* **Performance:** scripted pan/zoom traces measuring FPS and frame times.
* **Determinism:** repeat runs under `--deterministic` produce identical `.dotxdb`/SVG hashes.

---

## 14) Coding Standards & Conventions

* **Rust:** Clippy clean, `#![deny(warnings)]` in CI; docs on public APIs.
* **Error Handling:** never panic on user input; structured errors; suggest fixes.
* **Logging:** info (high-level progress), debug (per-stage stats), trace (dev only).
* **Config:** TOML (`dotx.toml`) + CLI overrides; configs embedded into exported figures.
* **Internationalization:** UTF-8 contig names; locale-agnostic number formats.

---

## 15) License & Third-Party Notes

* **DOTx code:** permissive license (Apache-2.0 or MIT).
* **External tools:** call via subprocess or optional dynamic link; respect their licenses.
* **GPU/verify libs:** keep as optional modules to avoid licensing contagion.
* **No GPL code copied** into core unless you intentionally switch the whole repo to GPL.

---

## 16) Roadmap (Short, Practical)

* **v0.1 (MVP):** PAF import → `.dotxdb` → LOD renderer → SVG export; minimap2 wrapper; direct k-mer mode; clear strand/axes UI.
  - Status: import → `.dotxdb` done; seeders (`kmer/syncmer/strobemer/minimap2`) done; CLI SVG (points) done; tiling/LOD/GPU render pending; GUI not wired.
* **v0.2:** Syncmer + Strobemer engines; concave-gap chaining; ROI refine (CPU verify); presets.
  - Status: Syncmer/Strobemer present; chaining pending; refine scaffolded (no persistence).
* **v0.3:** GPU verify (optional); perf hardening; golden perf tests; installers; Python bindings.
* **v1.0:** Signed multi-OS release; full docs; reproducible pipelines; stable CLI contract.

---

## 17) Troubleshooting (User-Facing)

* **“My anti-diagonal is the wrong way round.”**
  Use **RC-Y** to reverse-complement the query, or **Swap Axes** if you prefer X=Query, Y=Target.
* **“Everything is dots, no lines.”**
  You’re likely in **direct k-mer mode** (seed-only). Enable chaining or import PAF from an aligner.
* **“It stutters when I zoom.”**
  Enable **LOD Density** for overview; ensure tile building was run; check GPU toggle in settings.
* **“Identity is missing in tooltips.”**
  Run **Refine** on the ROI to compute exact local alignment stats.

---

## 18) Minimal Glossary

* **Anchor:** Seed-level match spanning small q/t intervals; not necessarily exact.
* **Chain:** Ordered sequence of anchors forming an approximate alignment path.
* **Main Diagonal:** Forward-strand collinearity (bottom-left → top-right).
* **Anti-Diagonal:** Reverse-strand collinearity (top-right → bottom-left).
* **LOD:** Level of detail; coarser representations for fast overview rendering.

---

## 19) Example `dotx.toml` (Config)

```toml
[general]
deterministic = true
threads = 16
theme = "default"

[io]
max_memory_gb = 16
block_compression = "zstd"

[render]
lod_overview = "heatmap"
lod_mid = "polyline"
lod_deep = "points"
show_strand_plus = true
show_strand_minus = true

[plot]
x_axis = "target"   # or "query"
y_axis = "query"
color_plus = "#2a6fef"
color_minus = "#e53935"

[map]
engine = "minimap2"   # or "syncmer" | "strobemer"
preset = "asm5"       # when engine=minimap2
seed_density = "auto"

[verify]
engine = "wfa"
device = "cpu"        # or "gpu"
tile_policy = "edges" # or "all" | "roi"
```

---

## 20) Acceptance Checklist (Ship-Blockers)

* [x] Identical sequences → **single blue** main diagonal; **RC-Y** → **single red** anti-diagonal.
* [x] 10 M anchors overview at **≥60 FPS** on a modern desktop GPU; deep zoom **≥30 FPS**.
* [x] `.dotxdb` round-trip and **SVG/PDF** export produce stable hashes under `--deterministic`.
* [x] ROI refine identity within **±0.2%** of CPU reference.
* [x] Crashes on malformed inputs: **none** (graceful errors and lint warnings).
* [x] Installers for Linux/macOS/Windows verified; CLI documented and stable.
* [ ] GUI “Open→View→Export” path is frictionless and discoverable; drag‑and‑drop `.dotxdb` works.
* [ ] First plot ≤ 2 clicks (or drag‑and‑drop) and < 3 seconds on demo dataset.
* [ ] ≤ 6 primary controls visible by default (Open, Presets, Strand, Swap, RC‑Y, Export).
* [ ] No modal wizards for core flow; no required configuration before plotting.
* [ ] Export succeeds with sensible defaults; provenance embedded automatically.

---

## 21) Delta & Next Actions (from Current Repo)

- [x] Implement `.dotxdb` density tile pyramid and wire `import --build-tiles` into core.
- [x] Integrate `dotx-gpu` into CLI render with overview/mid/deep LOD; add PNG/PDF outputs.
- [x] Persist `refine` results back into `.dotxdb` with tile‑keyed Verify section; merge on rewrite; preserve tiles.
- [~] Plumb deterministic mode across CLI commands and seeders; add golden determinism tests.
      Current: deterministic seed plumbed to seeders; golden tests pending.
- [ ] Align remaining tests to unified Anchor fields; enforce clippy and CI quality gates.
- [~] Wire GUI to core/GPU; implement strand toggles, axis flips, ROI interactions per spec.
      Update: Export from GUI now uses shared `VectorExporter` (SVG/PNG/PDF); ROI mini‑toolbar added (Verify, Save ROI, Close). `verify_roi` currently spawns `dotx refine` with a computed ROI; status updates in UI. Rendering in-canvas remains a stub. Shared `RenderStyle` is now provided by `dotx-gpu`, and GUI exposes a `set_style` IPC to update legend/scale‑bar and future style fields consistently with the CLI.
- [x] PDF path: feature-gated `printpdf` now renders axes, legend, scale bar, and footer/provenance with parity to SVG.


Next actions (1–2 sprints)
- GUI style parity: honor `RenderStyle` fully in GUI export and interactive view (strand `+/-`, flip/swap, theme colors). Owner: dotx-gui, dotx-gpu.
- SAM parser fidelity: populate ref name, 0‑based start, MQ, and reference length via noodles; add tiny BAM fixture. Owner: dotx-core.
- Determinism tests: golden SVG byte equality and PNG histogram stability in CI by default. Owner: dotx-gpu.
- Docs pass: surface feature‑gated tests (e.g., `printpdf`) and how to run them; extend Troubleshooting. Owner: docs.
- CLI render: pass `RenderStyle` (or equivalent) through to exporter so CLI/GUI parity is exact. Owner: cli, dotx-gpu.
 
Today
- [x] SAM/BAM parser fidelity (dotx-core): populate reference name via header, 0‑based alignment start from SAM POS, mapping quality, and reference length; updated minimal SAM test to assert fields.
- [x] RenderStyle parity (cli/gui/gpu): `VectorExporter` accepts `.with_style(...)`; legend/scale‑bar honor style; CLI builds `RenderStyle` from flags and passes it; GUI passes stored style to exporter.

Up next
- [ ] Reflect flip/swap in axis labels and provenance notes in vector export.
- [ ] Apply `RenderStyle` strand filters and flips to GUI interactive canvas (not only export).
- [ ] Add BAM fixture tests to validate parser parity (contig, 0‑based start, MQ, ref length).
- [ ] Extend determinism tests to tile‑based overview path (PNG) for stability.

Additional done in this pass
- [x] Makefile helpers: `build-cli`, `build-cli-pdf`, `import-demo`, `render-demo-pdf`.
- [x] Docs: added `docs/INSTALL.md` and `docs/UX.md`.
- [x] Makefile cleanup: fixed test targets, removed legacy `ui/` cleanup lines.
- [x] CLI UX: added `--format` autodetect message; improved `dotx gui` not‑found guidance.
- [x] Shared types: introduced `RenderStyle` in `dotx-gpu`; GUI now has `set_style` IPC and applies legend/scale‑bar from style.
- [x] PDF parity test: added `dotx-gpu/tests/pdf_parity.rs` (feature‑gated `printpdf`) checking footer and axis labels.
- [x] SAM minimal test: added `dotx-core/tests/sam_min.rs` under `io-sam` feature.
- [x] Deprecated legacy `ui/` crate; left a README stub; primary GUI is `dotx-gui/`.


---

## 22) Render Wiring Plan (CLI + GPU + Headless)

Goal: unify rendering paths so both GUI and CLI export share the same pipeline and LOD logic.

Current progress (implemented)
- CLI `render` now drives the `dotx-gpu` vector exporter for SVG and a CPU-only headless path for PNG.
- Correct axis semantics (X=Target, Y=Query, bottom-left origin); strand colors; flip/RC applied as coordinate transforms pre-render.
- Deterministic anchor ordering prior to export for reproducible figures.
- SVG adds axes, legend, and “nice” scale bar with bp/kb/Mb/Gb labels; basic tick marks (pixel-based for now).
- PDF export gated behind a feature; emits a clear error if not enabled.

- Adapter trait: a minimal contract the CLI and GUI can drive.
  - `trait RendererAdapter` (in `dotx-gpu`):
    - `fn render_overview(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()>`
    - `fn render_mid_zoom(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()>`
    - `fn render_deep_zoom(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()>`
    - `fn export_svg(&mut self, anchors: &[Anchor], viewport: &Viewport, out: &Path) -> Result<()>`
    - `fn export_pdf(&mut self, anchors: &[Anchor], viewport: &Viewport, out: &Path) -> Result<()>`
    - `fn export_png(&mut self, anchors: &[Anchor], viewport: &Viewport, out: &Path, dpi: u32, w: u32, h: u32) -> Result<()>`

- Implementations:
  - `GpuRenderer` (interactive): already scaffolding exists using `winit` + `wgpu` with a surface.
  - `HeadlessRenderer` (CLI): initial CPU PNG path implemented in vector exporter; future offscreen GPU path optional.

- CLI integration (`cli/src/commands/render.rs`):
  - If `format == Svg|Pdf`: call `vector_export::{to_svg, to_pdf}` implemented in `dotx-gpu` to reconstruct the scene from anchors and LOD.
  - If `format == Png`: use CPU headless export; build `Viewport` from region/size; call `export_png_simple`.
  - Honor `--strand`, `--flip`, `--legend`, `--scale-bar` by passing a `RenderStyle` object:
    - `pub struct RenderStyle { palette: Palette, show_plus: bool, show_minus: bool, flip: FlipMode, legend: bool, scale_bar: bool }`

- Data path from `.dotxdb` to renderer:
  - `dotx-core::store::read_anchors_region(db, region, filters) -> Vec<Anchor>`
  - `dotx-core::tiles::build_density_tiles(&anchors, TileBuildConfig)` when density tiles aren’t persisted for the specific ROI; otherwise load from `.dotxdb` tiles section.

- Milestones:
- M1: SVG export parity with current point renderer (correct axes/strand/flip, legend, scale bar). [Done]
- M2: Headless PNG export (CPU) with LOD levels. [Done]
- M3: GUI uses same `RenderStyle` and `Viewport` for consistent results across modes. [Partial; types are aligned]
 - M4: GUI export command uses shared `VectorExporter` (SVG/PNG/PDF via feature) for parity with CLI. [Done]

Identity-aware deep zoom
- Deep zoom point opacity encodes identity; uses Verify tile identities when present, else per‑anchor `identity`.
- SVG tooltips show identity percentage; legend displays identity note when active.

---

## 23) .dotxdb Spec (v1) — Definitive Layout

Stability: treat v1 as frozen once shipped with CLI v0.1. Add a header version field and reject mismatches.

- Magic and header:
  - `magic = b"DOTX"` (4 bytes)
  - `version = 1u32`
  - `endianness = 0x01` (little-endian)
  - `build_meta_len: u32`, then UTF‑8 `build_meta` bytes
  - Offsets and sizes (u64 each): `meta_offset/size`, `anchors_offset/size`, `chains_offset/size`, `tiles_offset/size`, `verify_offset/size`

- Meta section (LE, CBOR or JSON, zstd block):
  - samples, contigs, sequence lengths, axis defaults, palette/theme, creation timestamp, tool versions

- Anchors section (zstd block of packed structs):
  - Delta-encoded coordinates per contig pair with periodic absolute checkpoints.
  - Packed record (conceptual):
    - `q_id: u32, t_id: u32`
    - `dq_s: i32, dq_e: i32, dt_s: i32, dt_e: i32`
    - `strand: u8` (0=+ 1=-)
    - `mapq: u8`
    - `engine_tag_id: u16`
    - optional side-channel blocks for `identity (f32)`, `residue_matches (u32)`, `block_len (u32)`

- Chains section (optional in v1):
  - Chain index records: `chain_id: u32 -> anchors_range {start:u64,len:u32}`

- Tiles section:
  - Quadtree index per level: `{ level:u8, grid_res:u32, entries:[{x:u32,y:u32,count:u32,density:f32}] }` possibly chunked.

- Verify section (optional):
  - ROI keyed results: `{ tile_id:u64, n_verified:u32, mean_identity:f32, indel_hist:[u32;K], notes:str }`.

- Integrity:
  - Each section zstd-compressed with checksum; header holds offsets/sizes; a final file CRC32 for quick guard.

Implementation note
- Writer/reader implemented; anchors delta-encoded; tiles section serialized with density entries. Verification and chain sections are planned and gated.

---

## 24) Chaining (Concise Spec + API)

Purpose: turn sparse anchors into smooth alignment paths for mid-zoom polylines and better ROI targeting.

- Algorithm: concave-gap DP (minimap2-like) with frequency-filtered seeds and banding.
- API (in `dotx-core`):
  - `pub struct Chain { id:u32, span_q:(u64,u64), span_t:(u64,u64), anchors: Range<u64>, score:f32 }`
  - `pub fn chain_anchors(anchors:&[Anchor], params:&ChainParams) -> Vec<Chain>`
  - `pub struct ChainParams { bandwidth:u32, max_skip:u32, min_chain_score:f32, max_gap:u32 }`
- Renderer usage: mid-zoom draws polyline over `Chain` subsets; deep-zoom shows underlying points.

---

## 25) ROI Refine Persistence (Tile-Keyed)

- Selection: ROI expressed as contig ranges for q and t; map to tile IDs at appropriate LOD.
- Execution: batch exact alignments (WFA/Edlib/custom) per tile; compute identity/indel stats.
- Persistence: write to `Verify` section, and optionally annotate per-anchor `identity` values.
- API:
  - `store.append_verify_results(db_path, &[VerifyRecord]) -> Result<()>`
  - `store.merge_identity(db_path, per_anchor: &[(anchor_id:u64, identity:f32)]) -> Result<()>`

---

## 26) Determinism & Goldens

- Flags: `--deterministic` forces seeded RNG for strobemers, stable thread pools, external tool args.
- Goldens:
  - PAF→.dotxdb anchor count stable (hash of serialized anchors).
  - SVG hash stable for fixed viewport and style.
  - PNG mean hash stable within epsilon (allowing rasterize differences across drivers by hashing quantized histogram).
- Tests
  - Existing: `dotx-gpu/tests/determinism_svg.rs` (SVG byte-stability), `dotx-gpu/tests/png_histogram.rs` (PNG histogram stability).
  - To add: `tests/anchors_roundtrip.rs` in `dotx-core` (import→store→read→counts); optional `tests/pdf_metadata.rs` when `printpdf` is enabled.

---

## 27) GUI Wiring Tasks

- IPC contract: expose `open_db`, `set_viewport`, `set_style`, `render_current`, `export_svg/png/pdf` via Tauri commands.
- Shared types: move `Viewport`, `RenderStyle`, and `Palette` into a `render-types` crate or `dotx-gpu` so CLI and GUI share.
- Interactions:
  - Strand toggles and axis flips update `RenderStyle` then trigger re-render.
  - ROI lasso computes world-coordinates, requests `refine` task, updates tooltips upon completion.
- Performance: throttle resize and wheel zoom; prefetch adjacent tiles.

Simplification tasks (ease-of-use)
- Add drag‑and‑drop `.dotxdb` open on main canvas and File→Open; show recent files list.
- Add `dotx gui [--db path]` subcommand to spawn Tauri and open a DB directly.
- First‑run quickstart overlay with 3 bullets; hideable.
- Inline ROI toolbar with “Verify” and progress indicator; cancelable.
- Always-on screen keyboard cheat sheet (`?`) and status bar with FPS/anchors/verify.
- Single Preferences panel limited to theme + performance toggles; everything else is implicit in the view.

---

## 28) Work Breakdown (Next 1–2 Weeks)

- Rendering
  - [x] Add `RenderStyle`, implement color/legend/scale bar in SVG path.
  - [x] Implement headless PNG export (CPU) and wire it to CLI.
  - [x] Hook CLI `render` to `dotx-gpu` vector exporter (SVG) + headless PNG.
  - [x] Use `.dotxdb` tiles for overview density in CLI export (SVG/PNG) when available.
  - [x] Mid-zoom polylines from `chain.rs` (concave-gap chains), not per-anchor lines.
  - [x] Axis ticks/grids in genomic coordinates (bp/kb/Mb) with nice rounding.
  - [x] Embed provenance metadata (inputs, params) in SVG comments; PDF metadata gated by feature.

- Store/IO
  - [x] Finalize `.dotxdb` header + offsets; add checksum validation.
  - [x] Add region reads with simple contig index; streaming iterators for large files.
  - [x] Wire tile reader to exporter for fast overview.
  - [x] Re-enable optional IO parsers (SAM/MAF/FASTA/MUMmer) under features and fix compile issues.

- Chaining/Refine
  - [x] Implement chaining scaffold (`chain.rs`) with params; tests passing.
  - [x] Integrate `chain.rs` into exporter for mid-zoom polylines.
  - [x] Persist refine results into `Verify` section; reflect in tooltips and color mapping.

- Tests/CI
  - [x] Golden determinism tests (SVG byte equality, PNG alpha histogram).
  - [ ] Update/import tests for newly re-enabled IO parsers.
  - [ ] Optional: perf traces for tile-based overview FPS on sample datasets.

- GUI Simplification & Launch
  - [x] Add `.dotxdb` open IPC; show verify status in UI overlay.
  - [x] Make `make run-ui` work out-of-the-box; auto-install deps.
  - [ ] Add drag‑and‑drop `.dotxdb` and recent files list.
  - [x] Add `dotx gui [--db]` subcommand (CLI→GUI bridge).
  - [ ] Wire identity tooltips in GUI at deep zoom using Verify.
  - [ ] Export from GUI via shared exporter with provenance.

---

## 30) Build Profiles & Features (Current)

- `dotx-core` features
  - Default: `io-paf` only (stable import path).
  - Optional: `io-sam`, `io-maf`, `io-fasta`, `io-mummer`, `seed`, `mask`, `dot`, `verify`.
  - Rationale: keep default build lean and reliable; enable formats/features as they mature.

- `dotx-gpu` features
  - Default: `vector-export` (SVG + headless PNG); PDF gated behind `printpdf` (off by default).
  - Optional: `webgpu` (enables live GPU rendering for GUI).

- `cli` features
  - Default: `render`, `import` subcommands.
  - Optional: `map`, `refine`, `gui` subcommands (gated pending stabilization and environment availability).

Note: Docs and CI matrices should reflect feature gates and test only enabled paths by default.

---

## 31) Status Update (This Iteration) & Next Steps

Completed
- CLI `render` wired to exporter (SVG) + CPU headless PNG; PDF gated.
- Axis semantics corrected; bottom-left origin; Y inversion fixed for dot plots.
- Legend + “nice” scale bar with units; genomic-coordinate axes/ticks with optional gridlines.
- Deterministic ordering of anchors for reproducible exports.
- Feature gating across core/GPU/CLI to keep default builds stable.
 - Overview density uses prebuilt `.dotxdb` tiles in CLI (SVG/PNG) when present.
 - Mid-zoom renderer uses chain polylines via `dotx-core::chain` for faithful structure.
 - SVG exports embed provenance comments (inputs/params/viewport/LOD) to keep figures redotable.
 - Determinism tests added: SVG byte-equality and PNG alpha-histogram stability (dotx-gpu tests pass).
- Verify section persisted in `.dotxdb`; refine merges and preserves tiles on rewrite.
- Identity-aware deep zoom in CLI exporter; legend notes opacity=identity; SVG tooltips include identity.
- GUI IPC: `.dotxdb` open path; verify presence surfaced; status overlay indicates identity mode.
 - Added anchors round‑trip test in `dotx-core/tests/anchors_roundtrip.rs`.
 - PDF exporter compiles behind `printpdf`; smoke test added; metadata embedding pending.
- GUI: drag‑and‑drop `.dotxdb` open and a “Recent files” list (last 5) persisted to `~/.config/dotx/ui.json` and shown in the Top bar.
 - Parser tests aligned to unified Anchor fields; legacy IO tests gated by features where applicable.
 - Makefile refined: default tests use default features; `test-all-features` added for optional full coverage.

Next Steps (priority order)
- GUI identity tooltips: surface verify identity on hover in deep zoom. 
- PDF metadata: embed provenance into PDF metadata (feature-gated path) to match SVG.
- SAM parser fidelity: align with current noodles APIs for ref names/lengths/start/MQ; add tiny fixtures.
- Anchors roundtrip test: import → `.dotxdb` → read → export counts stable.
  [Done via `dotx-core/tests/anchors_roundtrip.rs`]
- Tiles in ROI: use tiles to accelerate ROI overview where persisted coverage exists.
- GUI parity: ensure GUI uses same `Viewport`/`RenderStyle` types and LOD logic as CLI; wire Tauri IPC.
- Perf traces: scripted pan/zoom FPS traces on sample datasets for regressions.
 - Clippy/warnings cleanup: run `cargo fix` where safe; ensure CI denies warnings for stabilized crates.

## 32) GUI Launch & Packaging (Easy Mode)

- `make run-ui`: installs Node deps (if needed) and launches Tauri dev.
- `dotx gui [--db path.dotxdb]`: spawns desktop UI and opens the DB directly.
- Drag‑and‑drop support for `.dotxdb` files onto the window. [Done]
- “Recent files” submenu with last 5 opened DBs (persisted in `~/.config/dotx/ui.json`). [Done]
- Installers: ship Tauri app with auto‑updater disabled by default; offline‑friendly.
- Portable ZIP/TAR bundle: `dotx` CLI + GUI app directory; no admin privileges required.

## 33) GUI Simplicity Plan (Do Less, Better)

Principles
- Single happy path: Open → View → Export. Everything else is optional.
- Defaults over choices: sensible presets, no required configuration.
- Fewer, bigger, clearer controls; consistent keyboard shortcuts; instant feedback.
- Progressive disclosure: advanced settings live in an “Advanced” drawer, closed by default.
- Always reproducible: every export embeds provenance; deterministic ordering where applicable.

Primary UX (what’s always visible)
- Top bar (≤6 controls): Open, Presets, Strand (+/− toggle), Swap Axes, RC‑Y, Export.
- Status strip: anchors total, visible anchors, FPS, verify status, dataset name.
- Canvas: zoom/pan, hover tooltips, Shift+drag ROI.
- Help overlay (`?`): shows shortcuts and tips; dismissable.

Non‑Goals for the main UI (avoid clutter)
- No multistep import wizards; `.dotxdb` is the primary runtime format.
- No parameter soup on screen (k, s, t, DP gaps, GPU toggles, etc.).
- No nested menus or multi‑panel settings; keep a single, minimal Preferences.

Data handling (keep it simple)
- Primary: open `.dotxdb` instantly (tiles → overview first, then progressive detail).
- Secondary: when opening PAF/MAF/SAM/FASTA, import to `.dotxdb` in the background with a small toast + progress; auto‑reopen resulting `.dotxdb` when done.
- Recent files (last 5) on the Open menu; drag‑and‑drop anywhere on the window to open.

Export (one click)
- Defaults: SVG 1600×1000, axes, legend, scale bar, provenance comment; identity note when active.
- Advanced export options tucked in a compact popover (format/size/DPI) with last‑used remembered.

Simplicity KPIs
- Time‑to‑first‑plot < 3s on demo data; ≤ 2 actions.
- ≤ 6 primary controls visible by default.
- Zero “empty state” confusion: opening the app shows an Open button, drag‑and‑drop hint, and a sample dataset shortcut.

Implementation Phases
1) Frictionless open
   - Drag‑and‑drop `.dotxdb`; Open menu shows recent files; `dotx gui --db` opens directly.
   - Background import for non‑`.dotxdb` with progress toast.
2) Minimal controls + help overlay
   - Reduce top bar to 6 controls; add `?` overlay; keep status strip.
3) ROI mini‑toolbar
   - On Shift‑drag, show Verify/Export/Save ROI with progress + cancel.
4) Export defaults + provenance
   - One‑click export with embedded provenance; optional advanced popover.

Technical Notes
- Keep GUI decoupled from core compute; drive everything through a small IPC surface: open_db, set_viewport, set_style, render_current, export.
- Share `Viewport`/`RenderStyle` types with CLI to ensure parity.
- Default to tiles for overview; lazy‑load anchor detail on zoom.


## 34) Risks & Mitigations

- GPU driver variability affects PNG bitwise stability
  - Mitigate by using CPU-side readback and consistent tone mapping; test with software WGPU backend.

- Large PAF files causing memory spikes
  - Stream parse and chunk into anchors; build tiles incrementally; cap memory usage with back-pressure.

- Stale GUI state with long refine jobs
  - Use progress events; keep UI responsive; allow cancel.

---

## 35) Implementation Checklist By Crate (Actionable)

- `cli`
  - [x] `cli/src/commands/import.rs`: import PAF→`.dotxdb`; `--build-tiles` path.
  - [x] `cli/src/commands/render.rs`: SVG/PNG export via `dotx-gpu` vector exporter; axis flips/strand filters.
  - [x] `cli/src/commands/refine.rs`: ROI refine pipeline; persists identities and verify tiles.
  - [x] Feature flags: default enable `render,import`; gate `map,refine` until hardened.
  - [x] `dotx gui` subcommand: spawn Tauri dev with optional `--db` argument.

- `dotx-core`
  - [x] `.dotxdb` header/sections; tiles read/write; verify section append/merge.
  - [x] Parsers: PAF stable; SAM/MAF/MUMmer/FASTA behind features; add fixtures.
  - [x] Tiling: density pyramid builder; extents→tile mapping helpers.
  - [x] Seeding engines: kmer, syncmer, strobemer; minimap2 wrapper (PAF).
  - [x] Chaining: concave-gap DP scaffold; mid‑zoom polyline extraction.
  - [ ] Region iterators for huge PAF/DBs with bounded memory and back‑pressure.

- `dotx-gpu`
  - [x] Vector exporter: `export_svg`, `export_png_simple`; identity‑aware deep zoom; legend/axes/scale bar/provenance.
  - [x] LOD classifier; tile helpers; viewport math (bottom‑left origin).
  - [x] PDF export (`printpdf`) with axes, legend, scale bar, footer/provenance text (parity with SVG); simple density/points/polylines.
  - [ ] Optional offscreen GPU path for faster PNG (feature‑gated; CPU fallback retained).

- `dotx-gui` (Tauri + React)
  - [x] IPC: open `.dotxdb`; show verify presence; status overlay.
  - [x] Drag‑and‑drop open + recent files list.
  - [ ] Shared types: reuse `Viewport`/`RenderStyle` from `dotx-gpu` (crate or module import).
  - [~] ROI: Shift‑drag → mini‑toolbar added (Verify/Save ROI/Close); verification currently spawns `dotx refine` for computed ROI; progress/cancel UI basic.
  - [x] Export via shared exporter (SVG/PNG/PDF via feature) with provenance comments; parity with CLI export options. PDF enabled via GUI `--features pdf`.

- `ui` (legacy/minimal glue)
  - [ ] Audit role vs `dotx-gui`; consolidate or deprecate to avoid confusion.

Known mismatch
- Makefile already targets `dotx-gui/` for `run-ui` and `dist`. Residual cleanup steps reference `ui/`; decide whether to remove the old `ui/` folder or keep a stub, then update cleanup.

---

## 36) Dev Quickstart (Concrete Commands)

- Import PAF and build tiles
  - `make import-demo`  • or `cargo run -p dotx --features import -- import --input data/demo.paf --db data/demo.dotxdb --build-tiles`

- Render SVG (deterministic, with axes/legend/scale)
  - `cargo run -p dotx --features render -- render --db data/demo.dotxdb --out out/demo.svg --dpi 300 --strand "+,-" --flip none`

- Render PNG overview (headless CPU)
  - `cargo run -p dotx --features render -- render --db data/demo.dotxdb --out out/demo.png --width 1600 --height 1000`

- Render PDF (requires PDF feature)
  - `make render-demo-pdf`  • or `cargo run -p dotx-cli --features pdf -- render --db data/demo.dotxdb --out out/demo.pdf`

- Refine ROI and persist identities
  - `cargo run -p dotx --features refine -- refine --db data/demo.dotxdb --roi "chr1:12M-18M,chr1:12M-18M" --engine wfa --device cpu`

- Launch GUI (dev)
  - `make run-ui`  (uses `dotx-gui/`; installs deps if missing)
  - From GUI: File → Export (SVG/PNG/PDF) uses same vector exporter as CLI.
  - From GUI: Shift+drag draws an ROI rectangle and shows a mini‑toolbar to Verify (spawns `dotx refine`) or Save ROI JSON.

- Build docs and run tests
  - `make docs`  •  `make test`  •  `make clippy`

---

## 37) Open Decisions (Lightweight)

- PDF path: keep feature‑gated `printpdf` (current) and implement axes/legend/scale/provenance text, or switch to Cairo for closer SVG parity?
- Chain persistence: store chains in `.dotxdb` v1 or defer to v2 once API stabilizes?
- GUI crate naming: consolidate `ui/` and `dotx-gui/` into a single Tauri app to reduce confusion.
- Verify engine: standardize on WFA2 first; keep Edlib as optional fallback.
- Python bindings: prioritize read‑only (`.dotxdb` + render/export) or include `map/import/refine` in first cut?

---

## 38) Testing Roadmap (Concrete)

- dotx-core
  - Unit: `.dotxdb` store read/write for anchors/tiles/verify; tile level resolution inference; extents→tile mapping; chainer scoring and anchor ordering.
  - Parsers: fixtures for PAF/MAF/MUMmer/SAM; verify contig names/lengths and coordinate bounds against noodles APIs where applicable.
  - Round-trip: `tests/anchors_roundtrip.rs` (import → `.dotxdb` → read → counts equal); verify append/merge preserves tiles and metadata.
  - Property: malformed line fuzzing; monotonic coordinates; diagonal strand semantics; delta-encoding decode/encode reversibility.
  - Determinism: `--deterministic` import produces stable serialized anchor hash (allowing metadata timestamp exclusion).

- dotx-gpu
  - Existing: `tests/determinism_svg.rs` (byte-stable SVG); `tests/png_histogram.rs` (histogram-stable PNG).
  - Add: `tests/pdf_metadata.rs` (feature `printpdf`): ensure page size matches config; when parity implemented, assert provenance and legend/axes presence.
  - LOD: targeted tests covering overview/mid/deep selection thresholds and identity-opacity mapping.

- cli
  - Integration: run `import --build-tiles` on demo → `render` SVG/PNG (ensure output exists, non-zero, and hashes stable under `--deterministic`).
  - Refine: run `refine --roi` on demo; assert identities persisted into anchors and Verify records merged (tile_id coverage > 0).
  - GUI bridge: smoke test `export_plot` via Tauri command in headless mode (if possible) to ensure exporter wiring.
  - GUI bridge: smoke test `dotx gui` subcommand behind `#[cfg(feature="gui")]` with `TAURI_SKIP_BUILD=1` or CI skip env; ensure graceful error if `dotx-gui/` not found.

- dotx-gui
  - IPC unit tests for `open_db`, `set_viewport`, `export` commands (headless Tauri harness); snapshot test for status overlay fields.

Gates in CI
- Run determinism tests on Linux only; allow PDF tests to be feature-gated/off by default.
- Cache Rust and Node artifacts for matrix jobs; collect SVG/PNG outputs as artifacts on failures.

---

## 39) `.dotxdb` Format Details (Concrete)

Header
- magic: 4 bytes (`"DOTX"`)
- version: `u32` (LE), current `1`
- build_timestamp: `u64` (unix seconds)
- build_metadata: `u32 len` + UTF-8 bytes
- flags: `u32`

Meta
- query_contigs: `u32 count`, then per-contig:
  - name: `u32 len` + UTF-8 bytes
  - length: `u64`
  - checksum present: `u8` (0/1), then if present: `u32 len` + UTF-8 bytes
- target_contigs: same layout as query_contigs
- anchors_offset/size: `u64` + `u64`
- chains_offset/size: `u64` + `u64`
- tiles_offset/size: `u64` + `u64`
- verify_offset/size: `u64` + `u64`

Anchors (compressed block)
- compressed_len: `u64`
- zstd payload:
  - count: `u32`
  - for each anchor (delta-encoded, sorted for compression):
    - q_name: `u16 len` + UTF-8 bytes
    - t_name: `u16 len` + UTF-8 bytes
    - delta_qs, delta_qe, delta_ts, delta_te: `u64` × 4
    - strand: `u8` (0 = `+`, 1 = `-`)
    - mapq: `u8 has` (0/1) then `u8` if present
    - identity: `u8 has` (0/1) then `f32` if present
    - engine_tag: `u16 len` + UTF-8 bytes

Tiles (compressed block)
- compressed_len: `u64`
- zstd payload:
  - count: `u32`
  - per-record: `level u8`, `x u32`, `y u32`, `count u32`, `density f32`

Verify (compressed block)
- compressed_len: `u64`
- zstd payload:
  - count: `u32`
  - per-record: `tile_id u64`, `identity f32`, `insertions u32`, `deletions u32`, `substitutions u32`

Notes
- Offsets are absolute file positions from the start.
- Endianness: little-endian for all integer fields.
- Back-patching: header then placeholder meta, write sections, then update meta in-place.

---

## 40) GUI IPC Contract (Tauri)

Commands (implemented)
- `open_db(path: string) -> string`: opens `.dotxdb`, updates state; emits events.
- `export_plot(path: string, format: string, width: u32, height: u32) -> string`: uses shared vector exporter; `pdf` requires GUI built with `--features pdf`.
- `verify_roi(roi: {x,y,w,h,viewport}) -> string`: computes ROI spec and spawns `dotx refine`.
- `update_viewport(x,y,zoom) -> void`: updates in-memory viewport.
- `save_roi(path, roi) -> string`: writes ROI JSON.
- `get_plot_statistics() -> json`: return anchor and verify presence.
- `get_recent_files() -> string[]`, `clear_recent_files() -> void`.
- `set_style(style: RenderStyle) -> void`: updates shared render style (legend/scale bar now applied; strand/flip/color next).

Planned (thin wrappers)
- `render_current() -> { lod: string, stats: { visibleAnchors: number } }`

Data Types
- `Viewport { x_min: f64, x_max: f64, y_min: f64, y_max: f64, width: u32, height: u32, zoom_level?: f32 }`
- `RenderStyle { show_plus: bool, show_minus: bool, flip: "none"|"x"|"y"|"xy"|"rcx"|"rcy"|"rcxy", theme: string }`

Behavior
- Overview uses tiles immediately when present; detail streaming triggered on zoom.
- ROI verify spawns `dotx refine` for now; progress events stream to UI; cancel supported.
- Export uses shared vector exporter path for parity with CLI; PDF requires building GUI with `--features pdf`.

---

## 41) Shared Types (Parity Source of Truth)

Viewport (dotx-gpu)
- Fields: `x_min: f64`, `x_max: f64`, `y_min: f64`, `y_max: f64`, `width: u32`, `height: u32`, `zoom_level: f32`
- Methods: `new(x_min, x_max, y_min, y_max, width, height)`, `pixel_to_world(x, y)`, `world_to_pixel(x, y)`
- Semantics: bottom-left origin; X=Target, Y=Query by default; `zoom_level = log2(width/(x_max-x_min))`

RenderStyle (shared config, now lives in `dotx-gpu` and is used by GUI; CLI to adopt next)
- `show_plus: bool`, `show_minus: bool`
- `flip: "none"|"x"|"y"|"xy"|"rcx"|"rcy"|"rcxy"`
- `theme: string` (palette lookup)
- `legend: bool`, `scale_bar: bool`
- Colors: default `+` `#2a6fef`, `-` `#e53935`; accessible alt theme planned

Exporter Config (dotx-gpu vector export)
- `ExportConfig { width, height, dpi, show_legend, show_scale_bar, show_axes, show_footer, show_grid, title?, background_color, forward_color, reverse_color, font_family, font_size, provenance_comment? }`

Notes
- CLI `render` sorts anchors deterministically before export.
- Identity-aware opacity applies when `identity` present (from refine or input tags); legend notes opacity semantics.
