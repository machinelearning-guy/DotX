// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dotx_core::{Sequence};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{Manager, State, WindowEvent};
use std::sync::{Arc, Mutex};
use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_gpu::{Viewport, LodLevel, RenderStyle};
use dotx_core::types::Anchor;

#[derive(Debug, Serialize, Deserialize)]
struct AppState {
    db_path: Option<PathBuf>,
    anchor_count: usize,
    verify_present: bool,
    sequences: Vec<Sequence>,
    viewport: ViewPort,
    render_style: RenderStyle,
    recent_files: Vec<PathBuf>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            db_path: None,
            anchor_count: 0,
            verify_present: false,
            sequences: Vec::new(),
            viewport: ViewPort::default(),
            render_style: RenderStyle::default(),
            recent_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ViewPort {
    x: f64,
    y: f64,
    zoom: f64,
    width: u32,
    height: u32,
}

impl Default for ViewPort {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0, width: 1600, height: 1000 }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PlotConfig {
    show_forward_strand: bool,
    show_reverse_strand: bool,
    swap_axes: bool,
    reverse_complement_y: bool,
    color_theme: String,
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            show_forward_strand: true,
            show_reverse_strand: true,
            swap_axes: false,
            reverse_complement_y: false,
            color_theme: "default".to_string(),
        }
    }
}

// Tauri commands
#[tauri::command]
async fn open_fasta_files(paths: Vec<String>) -> Result<Vec<String>, String> {
    log::info!("Opening FASTA files: {:?}", paths);
    // TODO: Actually load FASTA files using dotx-core
    Ok(paths)
}

#[tauri::command]
async fn load_alignment_file(path: String) -> Result<String, String> {
    log::info!("Loading alignment file: {}", path);
    // TODO: Load PAF/MAF/SAM files using dotx-core
    Ok(format!("Loaded alignment from {}", path))
}

#[tauri::command]
async fn start_alignment(
    ref_path: String,
    query_path: String,
    preset: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    log::info!("Starting alignment: {} vs {} with preset {}", ref_path, query_path, preset);
    
    // TODO: Implement alignment using dotx-core
    let _app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    
    Ok("Alignment started".to_string())
}

#[tauri::command]
async fn update_viewport(
    x: f64,
    y: f64,
    zoom: f64,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    
    app_state.viewport.x = x;
    app_state.viewport.y = y;
    app_state.viewport.zoom = zoom;
    
    Ok(())
}

#[tauri::command]
async fn open_db(path: String, state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    use std::io::Seek;
    let p = PathBuf::from(&path);
    let store = dotx_core::store::DotXStore::read_from_file(&p)
        .map_err(|e| format!("Failed to read .dotxdb: {}", e))?;
    let mut f = std::fs::File::open(&p).map_err(|e| format!("Failed to open file: {}", e))?;
    let anchors = store.read_anchors(&mut f).map_err(|e| format!("Read anchors: {}", e))?;
    // seek back and try verify
    f.rewind().ok();
    let verify = store.read_verify(&mut f).unwrap_or_default();
    let mut app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    app_state.db_path = Some(p.clone());
    app_state.anchor_count = anchors.len();
    app_state.verify_present = !verify.is_empty();
    add_recent_and_persist(&mut app_state, &p)?;
    Ok(format!("Opened DB with {} anchors{}", anchors.len(), if verify.is_empty() { "" } else { ", verify present" }))
}

#[tauri::command]
async fn export_plot(
    path: String,
    format: String,
    width: u32,
    height: u32,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    log::info!("Exporting plot to {} in format {} ({}x{})", path, format, width, height);

    // Load current DB from state (we re-open on disk for consistency)
    let (db_path_opt, _viewport_copy, style_copy) = {
        let guard = state.lock().map_err(|e| format!("State lock error: {}", e))?;
        (guard.db_path.clone(), guard.viewport.clone(), guard.render_style.clone())
    };

    let db_path = db_path_opt.ok_or_else(|| "No .dotxdb is open".to_string())?;

    // Read anchors (and verify/tiles if present)
    use std::io::Seek;
    let store = dotx_core::store::DotXStore::read_from_file(&db_path)
        .map_err(|e| format!("Failed to read .dotxdb: {}", e))?;
    let mut f = std::fs::File::open(&db_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut anchors = store.read_anchors(&mut f).map_err(|e| format!("Read anchors: {}", e))?;

    // Deterministic ordering for reproducibility
    anchors.sort_by(|a, b| a.t.cmp(&b.t)
        .then(a.q.cmp(&b.q))
        .then(a.ts.cmp(&b.ts))
        .then(a.qs.cmp(&b.qs))
    );

    // Compute extents and viewport
    let ext = compute_extents(&anchors);
    let vp = make_viewport_from_extents(ext, width, height);
    let lod = determine_lod(&anchors, &vp);

    // Build exporter config (simple defaults; provenance handled in SVG via comments)
    let export_cfg = ExportConfig {
        width,
        height,
        dpi: 300,
        show_legend: style_copy.legend,
        show_scale_bar: style_copy.scale_bar,
        show_axes: true,
        show_footer: true,
        show_grid: true,
        title: Some("DotX Export".to_string()),
        background_color: "#ffffff".to_string(),
        forward_color: "#2a6fef".to_string(),
        reverse_color: "#e53935".to_string(),
        font_family: "Arial, sans-serif".to_string(),
        font_size: 12,
        provenance_comment: None,
    };
    let exporter = VectorExporter::new(export_cfg).with_style(style_copy);

    // Choose format
    let fmt = format.to_lowercase();
    match fmt.as_str() {
        "svg" => {
            exporter
                .export_svg(&path, &anchors, &vp, lod, None)
                .map_err(|e| format!("SVG export failed: {}", e))?;
        }
        "png" => {
            exporter
                .export_png_simple(&path, &anchors, &vp, lod, None)
                .map_err(|e| format!("PNG export failed: {}", e))?;
        }
        "pdf" => {
            exporter
                .export_pdf(&path, &anchors, &vp, lod, None)
                .map_err(|e| format!("PDF export failed: {}", e))?;
        }
        _ => return Err("Unsupported export format (use svg|png|pdf)".to_string()),
    }

    Ok(format!("Exported to {}", path))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoiPayload {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    viewport: ViewPort,
}

#[tauri::command]
async fn save_roi(path: String, roi: RoiPayload) -> Result<String, String> {
    let payload = serde_json::json!({
        "roi": { "x": roi.x, "y": roi.y, "w": roi.w, "h": roi.h },
        "viewport": { "x": roi.viewport.x, "y": roi.viewport.y, "zoom": roi.viewport.zoom, "width": roi.viewport.width, "height": roi.viewport.height },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&payload).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write ROI file: {}", e))?;
    Ok(path)
}

#[tauri::command]
async fn verify_roi(roi: RoiPayload, state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    use std::io::Seek;
    log::info!("Verify ROI requested: x={}, y={}, w={}, h={} (zoom={})", roi.x, roi.y, roi.w, roi.h, roi.viewport.zoom);

    // Read DB path
    let db_path = {
        let guard = state.lock().map_err(|e| format!("State lock error: {}", e))?;
        guard.db_path.clone().ok_or_else(|| "No .dotxdb is open".to_string())?
    };

    // Load anchors to infer mapping and dominant contigs
    let store = dotx_core::store::DotXStore::read_from_file(&db_path)
        .map_err(|e| format!("Failed to read .dotxdb: {}", e))?;
    let mut f = std::fs::File::open(&db_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let anchors = store.read_anchors(&mut f).map_err(|e| format!("Read anchors: {}", e))?;

    if anchors.is_empty() {
        return Err("Database has no anchors".to_string());
    }

    // Choose dominant contigs for target and query
    use std::collections::HashMap;
    let mut t_counts: HashMap<&str, usize> = HashMap::new();
    let mut q_counts: HashMap<&str, usize> = HashMap::new();
    for a in &anchors { *t_counts.entry(&a.t).or_default() += 1; *q_counts.entry(&a.q).or_default() += 1; }
    let t_contig = t_counts.into_iter().max_by_key(|(_,c)| *c).map(|(k,_)| k.to_string()).unwrap_or_else(|| anchors[0].t.clone());
    let q_contig = q_counts.into_iter().max_by_key(|(_,c)| *c).map(|(k,_)| k.to_string()).unwrap_or_else(|| anchors[0].q.clone());

    // Extents for chosen contigs
    let (mut t_min, mut t_max) = (u64::MAX, 0u64);
    let (mut q_min, mut q_max) = (u64::MAX, 0u64);
    for a in &anchors {
        if a.t == t_contig { t_min = t_min.min(a.ts); t_max = t_max.max(a.te); }
        if a.q == q_contig { q_min = q_min.min(a.qs); q_max = q_max.max(a.qe); }
    }
    if t_min == u64::MAX || q_min == u64::MAX { return Err("Unable to infer contig extents for ROI".to_string()); }

    // Map ROI pixels to plot area then to world coordinates
    let plot_left = 40.0f64; let plot_top = 20.0f64;
    let plot_right = roi.viewport.width as f64 - 20.0f64;
    let plot_bottom = roi.viewport.height as f64 - 40.0f64;
    let plot_w = (plot_right - plot_left).max(1.0);
    let plot_h = (plot_bottom - plot_top).max(1.0);

    let rx = roi.x.max(0) as f64; let ry = roi.y.max(0) as f64;
    let rw = roi.w.max(1) as f64; let rh = roi.h.max(1) as f64;
    let rx0 = (rx - plot_left).clamp(0.0, plot_w);
    let rx1 = (rx + rw - plot_left).clamp(0.0, plot_w);
    let ry0 = (ry - plot_top).clamp(0.0, plot_h);
    let ry1 = (ry + rh - plot_top).clamp(0.0, plot_h);

    let tx0 = t_min as f64 + (rx0 / plot_w) * (t_max.saturating_sub(t_min) as f64);
    let tx1 = t_min as f64 + (rx1 / plot_w) * (t_max.saturating_sub(t_min) as f64);

    // Y is inverted in plot
    let q_span = (q_max.saturating_sub(q_min)) as f64;
    let qy_top = q_min as f64 + ((1.0 - (ry0 / plot_h)) * q_span);
    let qy_bottom = q_min as f64 + ((1.0 - (ry1 / plot_h)) * q_span);
    let q0 = qy_bottom.min(qy_top); let q1 = qy_bottom.max(qy_top);

    let t_start = tx0.min(tx1).max(0.0) as u64;
    let t_end = tx0.max(tx1) as u64;
    let q_start = q0.max(0.0) as u64;
    let q_end = q1 as u64;

    let roi_spec = format!("{}:{}-{},{}:{}-{}", t_contig, t_start, t_end, q_contig, q_start, q_end);
    log::info!("Computed ROI spec: {}", roi_spec);

    // Spawn CLI refine: dotx refine --db <db> --roi <roi_spec> --engine wfa --device cpu
    let output = std::process::Command::new("dotx")
        .arg("refine")
        .arg("--db").arg(&db_path)
        .arg("--roi").arg(&roi_spec)
        .arg("--engine").arg("wfa")
        .arg("--device").arg("cpu")
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                // Mark verify present in UI state
                if let Ok(mut guard) = state.lock() { guard.verify_present = true; }
                Ok("Verify job completed".to_string())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(format!("Refine failed: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to spawn 'dotx refine': {}", e)),
    }
}

// Update render style from the GUI
#[tauri::command]
async fn set_style(style: RenderStyle, state: State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let mut app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    app_state.render_style = style;
    Ok(())
}

#[tauri::command]
async fn get_plot_statistics(state: State<'_, Arc<Mutex<AppState>>>) -> Result<serde_json::Value, String> {
    let app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    
    let stats = serde_json::json!({
        "anchor_count": app_state.anchor_count,
        "sequence_count": app_state.sequences.len(),
        "viewport": {
            "x": app_state.viewport.x,
            "y": app_state.viewport.y,
            "zoom": app_state.viewport.zoom
        },
        "verificationStatus": if app_state.verify_present { "partial" } else { "none" }
    });
    
    Ok(stats)
}

#[tauri::command]
async fn get_recent_files(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Vec<String>, String> {
    let app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    Ok(app_state.recent_files.iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
async fn clear_recent_files(state: State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let mut app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    app_state.recent_files.clear();
    persist_recent_files(&app_state).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    env_logger::init();

    let mut initial = AppState::default();
    if let Ok(list) = load_recent_files() {
        initial.recent_files = list;
    }
    let app_state = Arc::new(Mutex::new(initial));
    let app_state_clone = app_state.clone();

    // Pre-open DB if --db <path> is provided
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(idx) = args.iter().position(|a| a == "--db") {
            if let Some(path) = args.get(idx + 1) {
                let p = PathBuf::from(path);
                if p.exists() {
                    if let Err(e) = preopen_db(&app_state_clone, &p) {
                        log::warn!("Failed to open DB from --db '{}': {}", p.display(), e);
                    }
                } else {
                    log::warn!("--db path does not exist: {}", p.display());
                }
            }
        }
    }

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            open_fasta_files,
            load_alignment_file,
            start_alignment,
            open_db,
            update_viewport,
            export_plot,
            save_roi,
            verify_roi,
            set_style,
            get_plot_statistics,
            get_recent_files,
            clear_recent_files
        ])
        .on_window_event(|app, event| {
            // Basic file drop support: open dropped .dotxdb files
            if let WindowEvent::FileDrop(paths) = event {
                for p in paths {
                    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("dotxdb") {
                            let state: State<'_, Arc<Mutex<AppState>>> = app.state::<Arc<Mutex<AppState>>>();
                            match preopen_db(&state, &p) {
                                Ok(()) => {
                                    let _ = app.emit_all("db-opened", p.to_string_lossy().to_string());
                                }
                                Err(e) => {
                                    log::warn!("File drop open failed: {}", e);
                                    let _ = app.emit_all("db-open-error", format!("{}: {}", p.display(), e));
                                }
                            }
                            break;
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn preopen_db(state: &Arc<Mutex<AppState>>, path: &PathBuf) -> Result<(), String> {
    use std::io::Seek;
    let store = dotx_core::store::DotXStore::read_from_file(path)
        .map_err(|e| format!("Failed to read .dotxdb: {}", e))?;
    let mut f = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let anchors = store.read_anchors(&mut f).map_err(|e| format!("Read anchors: {}", e))?;
    f.rewind().ok();
    let verify = store.read_verify(&mut f).unwrap_or_default();
    let mut app_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
    app_state.db_path = Some(path.clone());
    app_state.anchor_count = anchors.len();
    app_state.verify_present = !verify.is_empty();
    add_recent_and_persist(&mut app_state, path)?;
    Ok(())
}

fn config_path() -> Result<PathBuf, String> {
    let base = tauri::api::path::config_dir().ok_or_else(|| "No config dir".to_string())?;
    let dir = base.join("dotx");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Create config dir: {}", e))?;
    Ok(dir.join("ui.json"))
}

// ----- Minimal render helpers (mirroring CLI) -----

fn compute_extents(anchors: &[Anchor]) -> (u64, u64, u64, u64) {
    let (mut t_min, mut t_max) = (u64::MAX, 0u64);
    let (mut q_min, mut q_max) = (u64::MAX, 0u64);
    for a in anchors {
        if a.ts < t_min { t_min = a.ts; }
        if a.te > t_max { t_max = a.te; }
        if a.qs < q_min { q_min = a.qs; }
        if a.qe > q_max { q_max = a.qe; }
    }
    if t_min == u64::MAX { t_min = 0; }
    if q_min == u64::MAX { q_min = 0; }
    (t_min, t_max, q_min, q_max)
}

fn make_viewport_from_extents(ext: (u64, u64, u64, u64), width: u32, height: u32) -> Viewport {
    let (t_min, t_max, q_min, q_max) = ext;
    Viewport::new(t_min as f64, t_max as f64, q_min as f64, q_max as f64, width, height)
}

fn determine_lod(anchors: &[Anchor], vp: &Viewport) -> LodLevel {
    let n = anchors.len();
    if n > 5_000_000 { return LodLevel::Overview; }
    if n > (vp.width as usize * 3) { return LodLevel::MidZoom; }
    LodLevel::DeepZoom
}

fn load_recent_files() -> Result<Vec<PathBuf>, String> {
    let path = config_path()?;
    if !path.exists() { return Ok(Vec::new()); }
    let data = std::fs::read_to_string(&path).map_err(|e| format!("Read recent: {}", e))?;
    let v: serde_json::Value = serde_json::from_str(&data).map_err(|e| format!("Parse recent: {}", e))?;
    let mut out = Vec::new();
    if let Some(arr) = v.get("recent").and_then(|x| x.as_array()) {
        for s in arr.iter().filter_map(|x| x.as_str()) { out.push(PathBuf::from(s)); }
    }
    Ok(out)
}

fn persist_recent_files(state: &AppState) -> Result<(), String> {
    let path = config_path()?;
    let list: Vec<String> = state.recent_files.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let json = serde_json::json!({ "recent": list });
    std::fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).map_err(|e| format!("Write recent: {}", e))
}

fn add_recent_and_persist(state: &mut AppState, path: &PathBuf) -> Result<(), String> {
    // dedupe and cap at 5
    state.recent_files.retain(|p| p != path);
    state.recent_files.insert(0, path.clone());
    if state.recent_files.len() > 5 { state.recent_files.truncate(5); }
    persist_recent_files(state)
}
