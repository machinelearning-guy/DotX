/*!
# Vector Export System

Provides SVG and PDF export capabilities with legends, scale bars, and configuration footers.
Reconstructs the visible scene from GPU data for high-quality vector output.
*/

use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use crate::{Viewport, LodLevel, RenderStyle};
use dotx_core::types::{Anchor, Strand};
use dotx_core::chain::{Chainer, ChainParams};
use dotx_core::tiles::DensityTile;
use dotx_core::store::VerifyResult;
use std::collections::HashMap;

/// Export configuration
#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub show_legend: bool,
    pub show_scale_bar: bool,
    pub show_axes: bool,
    pub show_footer: bool,
    pub show_grid: bool,
    pub title: Option<String>,
    pub background_color: String,
    pub forward_color: String,
    pub reverse_color: String,
    pub font_family: String,
    pub font_size: u32,
    pub provenance_comment: Option<String>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            dpi: 300,
            show_legend: true,
            show_scale_bar: true,
            show_axes: true,
            show_footer: true,
            show_grid: true,
            title: None,
            background_color: "#ffffff".to_string(),
            forward_color: "#2a6fef".to_string(),
            reverse_color: "#e53935".to_string(),
            font_family: "Arial, sans-serif".to_string(),
            font_size: 12,
            provenance_comment: None,
        }
    }
}

/// Vector export system
pub struct VectorExporter {
    config: ExportConfig,
    style: RenderStyle,
}

impl VectorExporter {
    pub fn new(config: ExportConfig) -> Self {
        Self { config, style: RenderStyle::default() }
    }

    /// Attach a RenderStyle for CLI/GUI parity (strand toggles, flip labels, legend/scale flags)
    pub fn with_style(mut self, style: RenderStyle) -> Self {
        self.style = style;
        self
    }

    /// Export to SVG format
    pub fn export_svg<P: AsRef<Path>>(
        &self,
        path: P,
        anchors: &[Anchor],
        viewport: &Viewport,
        lod_level: LodLevel,
        verify: Option<&[VerifyResult]>,
    ) -> Result<()> {
        let mut svg = SvgBuilder::new(&self.config);
        
        // Add background
        svg.add_background();
        
        // Add provenance comment if provided
        if let Some(comment) = &self.config.provenance_comment {
            svg.add_comment(comment);
        }
        
        // Add title if specified
        if let Some(title) = &self.config.title {
            svg.add_title(title);
        }
        
        // Build optional identity context from verify
        let identity_ctx = verify.and_then(|v| build_identity_ctx(anchors, v));

        // Render anchors based on LOD level
        match lod_level {
            LodLevel::Overview => {
                svg.render_density_heatmap(anchors, viewport)?;
            }
            LodLevel::MidZoom => {
                svg.render_polylines_chains(anchors, viewport)?;
            }
            LodLevel::DeepZoom => {
                svg.render_points(anchors, viewport, identity_ctx.as_ref())?;
            }
        }
        
        // Add legend
        if self.config.show_legend && self.style.legend {
            if identity_ctx.is_some() || anchors.iter().any(|a| a.identity.is_some()) {
                svg.set_identity_note(true);
            }
            svg.add_legend();
        }
        
        // Add axes
        if self.config.show_axes {
            svg.add_axes(viewport, self.config.show_grid);
        }

        // Add scale bar
        if self.config.show_scale_bar && self.style.scale_bar {
            svg.add_scale_bar(viewport);
        }
        
        // Add footer with configuration
        if self.config.show_footer {
            svg.add_footer(viewport, lod_level);
        }
        
        // Write to file
        svg.write_to_file(path)?;
        
        Ok(())
    }

    /// Export to PDF format
    #[cfg(feature = "printpdf")]
    pub fn export_pdf<P: AsRef<Path>>(
        &self,
        path: P,
        anchors: &[Anchor],
        viewport: &Viewport,
        lod_level: LodLevel,
        _verify: Option<&[VerifyResult]>,
    ) -> Result<()> {
        let mut pdf = PdfBuilder::new(&self.config);
        
        // Add page
        pdf.add_page();
        
        // Add background
        pdf.add_background();
        
        // Add title
        if let Some(title) = &self.config.title {
            pdf.add_title(title);
        }
        
        // Render anchors based on LOD level
        match lod_level {
            LodLevel::Overview => {
                pdf.render_density_heatmap(anchors, viewport)?;
            }
            LodLevel::MidZoom => {
                pdf.render_polylines(anchors, viewport)?;
            }
            LodLevel::DeepZoom => {
                pdf.render_points(anchors, viewport)?;
            }
        }
        
        // Add legend
        if self.config.show_legend && self.style.legend {
            pdf.add_legend();
        }
        
        // Add axes
        if self.config.show_axes {
            pdf.add_axes(viewport, self.config.show_grid);
        }

        // Add scale bar
        if self.config.show_scale_bar && self.style.scale_bar {
            pdf.add_scale_bar(viewport);
        }
        
        // Add footer
        if self.config.show_footer {
            pdf.add_footer(viewport, lod_level);
        }
        
        // Write to file
        pdf.write_to_file(path)?;
        
        Ok(())
    }
}

#[cfg(not(feature = "printpdf"))]
impl VectorExporter {
    pub fn export_pdf<P: AsRef<Path>>(
        &self,
        _path: P,
        _anchors: &[Anchor],
        _viewport: &Viewport,
        _lod_level: LodLevel,
        _verify: Option<&[VerifyResult]>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("PDF export not enabled (compile with 'printpdf' feature)"))
    }
}

/// SVG builder for vector graphics
struct SvgBuilder {
    config: ExportConfig,
    elements: Vec<String>,
    width: f32,
    height: f32,
    top_comments: Vec<String>,
    identity_note: bool,
}

impl SvgBuilder {
    fn new(config: &ExportConfig) -> Self {
        Self {
            config: config.clone(),
            elements: Vec::new(),
            width: config.width as f32,
            height: config.height as f32,
            top_comments: Vec::new(),
            identity_note: false,
        }
    }

    fn set_identity_note(&mut self, enabled: bool) { self.identity_note = enabled; }

    fn add_background(&mut self) {
        self.elements.push(format!(
            r#"<rect width="{}" height="{}" fill="{}"/>"#,
            self.width, self.height, self.config.background_color
        ));
    }

    fn add_comment(&mut self, text: &str) {
        self.top_comments.push(text.to_string());
    }

    fn add_title(&mut self, title: &str) {
        let title_y = self.config.font_size as f32 + 10.0;
        self.elements.push(format!(
            r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" text-anchor="middle" font-weight="bold">{}</text>"#,
            self.width / 2.0, title_y, self.config.font_family, self.config.font_size + 4, title
        ));
    }

    fn render_density_heatmap(&mut self, anchors: &[Anchor], _viewport: &Viewport) -> Result<()> {
        // Generate density grid
        let grid_size = 100;
        let mut density_grid = vec![vec![0.0f32; grid_size]; grid_size];
        
        // Calculate bounds
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        
        for anchor in anchors {
            min_x = min_x.min(anchor.ts as f64);
            max_x = max_x.max(anchor.te as f64);
            min_y = min_y.min(anchor.qs as f64);
            max_y = max_y.max(anchor.qe as f64);
        }
        
        if min_x >= max_x || min_y >= max_y {
            return Ok(());
        }
        
        // Populate density grid
        for anchor in anchors {
            let x_norm = ((anchor.ts as f64 - min_x) / (max_x - min_x)).clamp(0.0, 1.0);
            let y_norm = ((anchor.qs as f64 - min_y) / (max_y - min_y)).clamp(0.0, 1.0);
            
            let grid_x = (x_norm * (grid_size - 1) as f64) as usize;
            let grid_y = (y_norm * (grid_size - 1) as f64) as usize;
            
            density_grid[grid_y][grid_x] += 1.0;
        }
        
        // Find max density for normalization
        let max_density = density_grid.iter().flatten().fold(0.0f32, |a, &b| a.max(b));
        
        if max_density > 0.0 {
            // Render density as rectangles
            let cell_width = self.width / grid_size as f32;
            let cell_height = self.height / grid_size as f32;
            
            for (y, row) in density_grid.iter().enumerate() {
                for (x, &density) in row.iter().enumerate() {
                    if density > 0.0 {
                        let normalized_density = density / max_density;
                        let alpha = (normalized_density * 255.0) as u8;
                        let color = format!("rgba(255, 0, 0, {})", alpha as f32 / 255.0);
                        
                        let rect_x = x as f32 * cell_width;
                        let rect_y = y as f32 * cell_height;
                        
                        self.elements.push(format!(
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                            rect_x, rect_y, cell_width, cell_height, color
                        ));
                    }
                }
            }
        }
        
        Ok(())
    }

    fn render_polylines_chains(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()> {
        // Build chains using core chainer
        let chainer = Chainer::new(ChainParams::default());
        let chains = match chainer.chain(anchors) { Ok(c) => c, Err(_) => Vec::new() };

        // Draw each chain as a polyline by connecting anchor midpoints in order
        for chain in chains.iter().take(10_000) { // safety cap
            if chain.anchors.len() < 2 { continue; }
            let color = match chain.strand {
                Strand::Forward => &self.config.forward_color,
                Strand::Reverse => &self.config.reverse_color,
            };
            let mut path = String::new();
            for (k, &idx) in chain.anchors.iter().enumerate() {
                if let Some(a) = anchors.get(idx) {
                    let xm = self.world_to_svg_x(((a.ts as f64 + a.te as f64) * 0.5), viewport);
                    let ym = self.world_to_svg_y(((a.qs as f64 + a.qe as f64) * 0.5), viewport);
                    if k == 0 {
                        path.push_str(&format!("M {:.3} {:.3} ", xm, ym));
                    } else {
                        path.push_str(&format!("L {:.3} {:.3} ", xm, ym));
                    }
                }
            }
            self.elements.push(format!(
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="1.2" stroke-opacity="0.85"/>"#,
                path, color
            ));
        }
        Ok(())
    }

    fn render_points(&mut self, anchors: &[Anchor], viewport: &Viewport, identity_ctx: Option<&IdentityCtx>) -> Result<()> {
        for anchor in anchors.iter().take(10000) { // Limit for performance
            let x = self.world_to_svg_x(anchor.ts as f64, viewport);
            let y = self.world_to_svg_y(anchor.qs as f64, viewport);
            let color = match anchor.strand {
                Strand::Forward => &self.config.forward_color,
                Strand::Reverse => &self.config.reverse_color,
            };
            let mut opacity = 0.6f32;
            let mut id_text: Option<String> = None;
            if let Some(ctx) = identity_ctx {
                if let Some(id) = identity_for_anchor(anchor, ctx) {
                    opacity = (0.2 + (id.max(0.0).min(100.0) / 100.0) * 0.8) as f32;
                    id_text = Some(format!("{:.1}%", id));
                }
            } else if let Some(id_pct) = anchor.identity {
                opacity = (0.2 + (id_pct.max(0.0).min(100.0) / 100.0) * 0.8) as f32;
                id_text = Some(format!("{:.1}%", id_pct));
            }

            self.elements.push(format!(
                r#"<g><circle cx="{}" cy="{}" r="1.5" fill="{}" fill-opacity="{}"/>"#,
                x, y, color, opacity
            ));
            let tooltip = format!(
                "q={} [{}-{}], t={} [{}-{}], strand={}, {}",
                anchor.q, anchor.qs, anchor.qe, anchor.t, anchor.ts, anchor.te,
                match anchor.strand { Strand::Forward => '+', Strand::Reverse => '-' },
                id_text.unwrap_or_else(|| "identity: n/a".to_string())
            );
            self.elements.push(format!(r#"<title>{}</title></g>"#, tooltip));
        }
        
        Ok(())
    }

    fn add_legend(&mut self) {
        let legend_x = self.width - 150.0;
        let legend_y = 50.0;
        
        // Legend background
        self.elements.push(format!(
            r#"<rect x="{}" y="{}" width="130" height="{}" fill="white" stroke="black" stroke-width="1" fill-opacity="0.9"/>"#,
            legend_x, legend_y, if self.identity_note { 100.0 } else { 80.0 }
        ));
        
        // Forward strand
        self.elements.push(format!(
            r#"<circle cx="{}" cy="{}" r="5" fill="{}"/>"#,
            legend_x + 15.0, legend_y + 20.0, self.config.forward_color
        ));
        self.elements.push(format!(
            r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" dominant-baseline="middle">Forward (+)</text>"#,
            legend_x + 30.0, legend_y + 20.0, self.config.font_family, self.config.font_size
        ));
        
        // Reverse strand
        self.elements.push(format!(
            r#"<circle cx="{}" cy="{}" r="5" fill="{}"/>"#,
            legend_x + 15.0, legend_y + 45.0, self.config.reverse_color
        ));
        self.elements.push(format!(
            r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" dominant-baseline="middle">Reverse (-)</text>"#,
            legend_x + 30.0, legend_y + 45.0, self.config.font_family, self.config.font_size
        ));

        if self.identity_note {
            self.elements.push(format!(
                r##"<text x="{}" y="{}" font-family="{}" font-size="{}px" fill="#555">Opacity encodes identity</text>"##,
                legend_x + 15.0, legend_y + 70.0, self.config.font_family, self.config.font_size - 1
            ));
        }
    }

    fn add_axes(&mut self, viewport: &Viewport, show_grid: bool) {
        // Margins for axes
        let left = 50.0; let right = self.width - 20.0; let top = 20.0; let bottom = self.height - 40.0;

        // X axis (Target)
        self.elements.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#, left, bottom, right, bottom));
        self.elements.push(format!(r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" text-anchor="middle">Target (bp)</text>"#, (left+right)/2.0, self.height - 8.0, self.config.font_family, self.config.font_size));

        // Y axis (Query)
        self.elements.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#, left, top, left, bottom));
        self.elements.push(format!(r#"<text x="{}" y="{}" transform="rotate(-90 {} {})" font-family="{}" font-size="{}px" text-anchor="middle">Query (bp)</text>"#, 18.0, (top+bottom)/2.0, 18.0, (top+bottom)/2.0, self.config.font_family, self.config.font_size));

        // Compute nice ticks in WORLD units and map to inner-plot pixel coordinates
        let x_ticks_world = nice_ticks_world(viewport.x_min, viewport.x_max, 6);
        for w in x_ticks_world {
            let px_canvas = self.world_to_svg_x(w, viewport) as f32; // 0..width
            let x = left + (px_canvas / self.width) * (right - left);
            // Tick mark
            self.elements.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#, x, bottom, x, bottom + 5.0));
            // Grid line
            if show_grid {
                self.elements.push(format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#cccccc" stroke-width="1" opacity="0.5"/>"##, x, top, x, bottom));
            }
            // Label
            let label = format_bp(w - viewport.x_min); // relative span label
            self.elements.push(format!(r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" text-anchor="middle">{}</text>"#, x, bottom + 16.0, self.config.font_family, self.config.font_size - 2, label));
        }

        let y_ticks_world = nice_ticks_world(viewport.y_min, viewport.y_max, 6);
        for w in y_ticks_world {
            let py_canvas = self.world_to_svg_y(w, viewport) as f32; // 0..height
            let y = top + (py_canvas / self.height) * (bottom - top);
            // Tick mark
            self.elements.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#, left - 5.0, y, left, y));
            // Grid line
            if show_grid {
                self.elements.push(format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#cccccc" stroke-width="1" opacity="0.5"/>"##, left, y, right, y));
            }
            // Label
            let label = format_bp(w - viewport.y_min);
            self.elements.push(format!(r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" text-anchor="end" dominant-baseline="middle">{}</text>"#, left - 8.0, y, self.config.font_family, self.config.font_size - 2, label));
        }
    }

    fn add_scale_bar(&mut self, viewport: &Viewport) {
        // Aim for a scale bar that spans about 1/5 of the width with a "nice" round number
        let target_px = (self.width as f32 / 5.0).max(60.0);
        let world_per_px = (viewport.x_max - viewport.x_min) / self.width as f64;
        let target_world = (world_per_px * target_px as f64).max(1.0);
        let nice_world = nice_round_length(target_world);
        let scale_bar_length = (nice_world / world_per_px) as f32;
        let scale_bar_x = 50.0;
        let scale_bar_y = self.height - 50.0;
        
        // Scale bar line
        self.elements.push(format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="2"/>"#,
            scale_bar_x, scale_bar_y, scale_bar_x + scale_bar_length, scale_bar_y
        ));
        
        // Scale bar ticks
        self.elements.push(format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#,
            scale_bar_x, scale_bar_y - 5.0, scale_bar_x, scale_bar_y + 5.0
        ));
        self.elements.push(format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#,
            scale_bar_x + scale_bar_length, scale_bar_y - 5.0, scale_bar_x + scale_bar_length, scale_bar_y + 5.0
        ));
        
        // Scale bar label
        let label = format_bp(nice_world);
        self.elements.push(format!(
            r#"<text x="{}" y="{}" font-family="{}" font-size="{}px" text-anchor="middle" dominant-baseline="hanging">{}</text>"#,
            scale_bar_x + scale_bar_length / 2.0, scale_bar_y + 10.0, self.config.font_family, self.config.font_size, label
        ));
    }

    

    fn add_footer(&mut self, viewport: &Viewport, lod_level: LodLevel) {
        let footer_y = self.height - 10.0;
        let footer_text = format!(
            "DotX v1.0 | Viewport: {:.0}-{:.0} x {:.0}-{:.0} | LOD: {:?} | Generated: {}",
            viewport.x_min, viewport.x_max, viewport.y_min, viewport.y_max,
            lod_level,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        
        self.elements.push(format!(
            r#"<text x="10" y="{}" font-family="{}" font-size="{}px" fill="gray">{}</text>"#,
            footer_y, self.config.font_family, self.config.font_size - 2, footer_text
        ));
    }

    fn world_to_svg_x(&self, world_x: f64, viewport: &Viewport) -> f32 {
        ((world_x - viewport.x_min) / (viewport.x_max - viewport.x_min)) as f32 * self.width
    }

    fn world_to_svg_y(&self, world_y: f64, viewport: &Viewport) -> f32 {
        let norm = ((world_y - viewport.y_min) / (viewport.y_max - viewport.y_min)) as f32;
        // Invert Y so that larger values are higher on the canvas (bottom-left origin)
        (1.0 - norm) * self.height
    }

    fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        
        // SVG header
        writeln!(file, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(file, r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">"#,
            self.width, self.height, self.width, self.height)?;
        // Write provenance comments if any
        for c in &self.top_comments {
            for line in c.lines() {
                writeln!(file, "  <!-- {} -->", line)?;
            }
        }
        
        // Write elements
        for element in &self.elements {
            writeln!(file, "  {}", element)?;
        }
        
        // SVG footer
        writeln!(file, "</svg>")?;
        
        Ok(())
    }
}

// Format a basepair length in human-friendly units
fn format_bp(bp: f64) -> String {
    if bp >= 1e9 { format!("{:.2} Gb", bp / 1e9) }
    else if bp >= 1e6 { format!("{:.2} Mb", bp / 1e6) }
    else if bp >= 1e3 { format!("{:.2} kb", bp / 1e3) }
    else { format!("{:.0} bp", bp) }
}

// Round a length to a "nice" number: 1, 2, or 5 × 10^k
fn nice_round_length(x: f64) -> f64 {
    if x <= 0.0 { return 1.0; }
    let exp = x.log10().floor();
    let base = 10f64.powf(exp);
    let mant = x / base;
    let nice = if mant < 2.0 { 2.0 } else if mant < 5.0 { 5.0 } else { 10.0 };
    nice * base
}

// Generate nice tick positions (in pixels here) and labels based on total span
fn nice_ticks_world(min_world: f64, max_world: f64, desired: usize) -> Vec<f64> {
    let span = (max_world - min_world).max(1.0);
    let raw_step = span / desired as f64;
    let step = nice_round_length(raw_step);
    let mut ticks = Vec::new();
    // Start at the first multiple of step >= min_world
    let mut v = (min_world / step).ceil() * step;
    while v <= max_world {
        ticks.push(v);
        v += step;
    }
    ticks
}

/// PDF builder for vector graphics
#[cfg(feature = "printpdf")]
struct PdfBuilder {
    config: ExportConfig,
    doc: printpdf::PdfDocumentReference,
    current_page: Option<printpdf::PdfPageIndex>,
    current_layer: Option<printpdf::PdfLayerIndex>,
    font: Option<printpdf::IndirectFontRef>,
}

// ----- Simple PNG headless export (CPU) -----

impl VectorExporter {
    /// Simple CPU-based PNG export for CLI without creating a GPU surface.
    pub fn export_png_simple<P: AsRef<Path>>(
        &self,
        path: P,
        anchors: &[Anchor],
        viewport: &Viewport,
        lod_level: LodLevel,
        verify: Option<&[VerifyResult]>,
    ) -> Result<()> {
        use image::{Rgba, RgbaImage};
        let mut img = RgbaImage::from_pixel(self.config.width, self.config.height, Rgba([255, 255, 255, 255]));

        let to_px = |wx: f64, wy: f64| -> (i32, i32) {
            let x = ((wx - viewport.x_min) / (viewport.x_max - viewport.x_min) * self.config.width as f64) as i32;
            let y = ((wy - viewport.y_min) / (viewport.y_max - viewport.y_min) * self.config.height as f64) as i32;
            (x.clamp(0, (self.config.width as i32) - 1), y.clamp(0, (self.config.height as i32) - 1))
        };

        // Build identity context
        let identity_ctx = verify.and_then(|v| build_identity_ctx(anchors, v));

        match lod_level {
            LodLevel::Overview => {
                let grid = 128usize;
                let mut counts = vec![0u32; grid * grid];
                let (mut minx, mut maxx) = (f64::INFINITY, f64::NEG_INFINITY);
                let (mut miny, mut maxy) = (f64::INFINITY, f64::NEG_INFINITY);
                for a in anchors { minx = minx.min(a.ts as f64); maxx = maxx.max(a.te as f64); miny = miny.min(a.qs as f64); maxy = maxy.max(a.qe as f64); }
                if !(minx < maxx && miny < maxy) { minx = viewport.x_min; maxx = viewport.x_max; miny = viewport.y_min; maxy = viewport.y_max; }
                for a in anchors {
                    let nx = ((a.ts as f64 - minx) / (maxx - minx)).clamp(0.0, 1.0);
                    let ny = ((a.qs as f64 - miny) / (maxy - miny)).clamp(0.0, 1.0);
                    let ix = (nx * (grid as f64 - 1.0)) as usize;
                    let iy = (ny * (grid as f64 - 1.0)) as usize;
                    counts[iy * grid + ix] += 1;
                }
                let maxc = counts.iter().copied().max().unwrap_or(1);
                for y in 0..self.config.height { for x in 0..self.config.width {
                    let gx = (x as usize * grid) / (self.config.width as usize);
                    let gy = (y as usize * grid) / (self.config.height as usize);
                    let c = counts[gy * grid + gx];
                    if c > 0 {
                        let alpha = ((c as f32 / maxc as f32) * 255.0) as u8;
                        img.put_pixel(x, y, Rgba([255, 0, 0, alpha]));
                    }
                }}
            }
            LodLevel::MidZoom => {
                // Chain-based mid-zoom polylines
                let chainer = Chainer::new(ChainParams::default());
                if let Ok(chains) = chainer.chain(anchors) {
                    for chain in chains.iter() {
                        if chain.anchors.len() < 2 { continue; }
                        let col = match chain.strand { Strand::Forward => Rgba([42, 111, 239, 200]), Strand::Reverse => Rgba([229, 57, 53, 200]) };
                        let mut last: Option<(i32,i32)> = None;
                        for &idx in &chain.anchors {
                            if let Some(a) = anchors.get(idx) {
                                let xm = (a.ts as f64 + a.te as f64) * 0.5;
                                let ym = (a.qs as f64 + a.qe as f64) * 0.5;
                                let (x, y) = to_px(xm, ym);
                                if let Some((lx, ly)) = last {
                                    draw_line(&mut img, lx, ly, x, y, col);
                                }
                                last = Some((x, y));
                            }
                        }
                    }
                }
            }
            LodLevel::DeepZoom => {
                for a in anchors.iter().take(5_000_000) {
                    let mut alpha = 200u8;
                    if let Some(ctx) = &identity_ctx {
                        if let Some(id) = identity_for_anchor(a, ctx) {
                            let a_val = (40.0 + (id.max(0.0).min(100.0) / 100.0) * 215.0) as u8;
                            alpha = a_val;
                        }
                    } else if let Some(id_pct) = a.identity {
                        let a_val = (40.0 + (id_pct.max(0.0).min(100.0) / 100.0) * 215.0) as u8;
                        alpha = a_val;
                    }
                    let (x, y) = to_px(a.ts as f64, a.qs as f64);
                    let col = match a.strand { Strand::Forward => Rgba([42, 111, 239, alpha]), Strand::Reverse => Rgba([229, 57, 53, alpha]) };
                    if x >= 0 && y >= 0 && (x as u32) < self.config.width && (y as u32) < self.config.height {
                        img.put_pixel(x as u32, y as u32, col);
                    }
                }
            }
        }

        img.save(path)?;
        Ok(())
    }
}

impl VectorExporter {
    /// Export overview density using precomputed tiles to SVG.
    /// Note: assumes tiles cover the full dataset extents; best used when rendering full overview (no ROI).
    pub fn export_svg_overview_tiles<P: AsRef<Path>>(
        &self,
        path: P,
        tiles: &[DensityTile],
        _viewport: &Viewport,
    ) -> Result<()> {
        let mut svg = SvgBuilder::new(&self.config);
        svg.add_background();
        if let Some(title) = &self.config.title { svg.add_title(title); }

        // Choose a level close to ~128 cells across
        let target_cells: u32 = 128;
        let mut best_level = 0u8;
        let mut best_diff = u32::MAX;
        let mut levels: Vec<u8> = tiles.iter().map(|t| t.level).collect();
        levels.sort_unstable();
        levels.dedup();
        for lvl in levels {
            let (rx, _ry) = infer_level_resolution(tiles, lvl);
            let diff = if rx > target_cells { rx - target_cells } else { target_cells - rx };
            if diff < best_diff { best_diff = diff; best_level = lvl; }
        }

        // Render rects for that level
        let (res_x, res_y) = infer_level_resolution(tiles, best_level);
        let cell_w = svg.width / res_x as f32;
        let cell_h = svg.height / res_y as f32;
        let max_density = tiles.iter().filter(|t| t.level == best_level).map(|t| t.density).fold(0.0f32, f32::max).max(1e-6);

        for t in tiles.iter().filter(|t| t.level == best_level) {
            let alpha = (t.density / max_density).clamp(0.0, 1.0);
            let x = t.x as f32 * cell_w;
            let y = t.y as f32 * cell_h;
            let color = format!("rgba(255, 0, 0, {:.3})", alpha);
            svg.elements.push(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                x, y, cell_w, cell_h, color
            ));
        }

        if self.config.show_legend && self.style.legend { svg.add_legend(); }
        if self.config.show_axes { svg.add_axes(_viewport, self.config.show_grid); }
        if self.config.show_scale_bar && self.style.scale_bar { svg.add_scale_bar(_viewport); }
        if self.config.show_footer { svg.add_footer(_viewport, LodLevel::Overview); }
        svg.write_to_file(path)?;
        Ok(())
    }

    /// Export overview density using precomputed tiles to PNG.
    pub fn export_png_overview_tiles<P: AsRef<Path>>(
        &self,
        path: P,
        tiles: &[DensityTile],
        _viewport: &Viewport,
    ) -> Result<()> {
        use image::{Rgba, RgbaImage};
        let mut img = RgbaImage::from_pixel(self.config.width, self.config.height, Rgba([255, 255, 255, 255]));

        // Choose level as in SVG path
        let target_cells: u32 = 128;
        let mut best_level = 0u8;
        let mut best_diff = u32::MAX;
        let mut levels: Vec<u8> = tiles.iter().map(|t| t.level).collect();
        levels.sort_unstable();
        levels.dedup();
        for lvl in levels {
            let (rx, _ry) = infer_level_resolution(tiles, lvl);
            let diff = if rx > target_cells { rx - target_cells } else { target_cells - rx };
            if diff < best_diff { best_diff = diff; best_level = lvl; }
        }

        let (res_x, res_y) = infer_level_resolution(tiles, best_level);
        let cell_w = self.config.width as f32 / res_x as f32;
        let cell_h = self.config.height as f32 / res_y as f32;
        let max_density = tiles.iter().filter(|t| t.level == best_level).map(|t| t.density).fold(0.0f32, f32::max).max(1e-6);

        for t in tiles.iter().filter(|t| t.level == best_level) {
            let alpha = (t.density / max_density).clamp(0.0, 1.0);
            let x0 = (t.x as f32 * cell_w) as u32;
            let y0 = (t.y as f32 * cell_h) as u32;
            let x1 = ((t.x as f32 + 1.0) * cell_w).ceil() as u32;
            let y1 = ((t.y as f32 + 1.0) * cell_h).ceil() as u32;
            let color = Rgba([255, 0, 0, (alpha * 255.0) as u8]);
            for y in y0.min(img.height()-1)..y1.min(img.height()) {
                for x in x0.min(img.width()-1)..x1.min(img.width()) {
                    img.put_pixel(x, y, color);
                }
            }
        }

        img.save(path)?;
        Ok(())
    }
}

fn infer_level_resolution(tiles: &[DensityTile], level: u8) -> (u32, u32) {
    let mut max_x = 0u32; let mut max_y = 0u32;
    for t in tiles.iter().filter(|t| t.level == level) {
        if t.x > max_x { max_x = t.x; }
        if t.y > max_y { max_y = t.y; }
    }
    (max_x + 1, max_y + 1)
}

fn draw_line(img: &mut image::RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: image::Rgba<u8>) {
    // Bresenham line drawing
    let (mut x0, mut y0, x1, y1) = (x0, y0, x1, y1);
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as u32) < img.width() && (y0 as u32) < img.height() {
            img.put_pixel(x0 as u32, y0 as u32, color);
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}

// Parse a hex color like "#RRGGBB" into normalized RGB
fn parse_hex_rgb(s: &str) -> Option<(f32, f32, f32)> {
    let hex = s.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 { return None; }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some((r, g, b))
}

// ----- Identity mapping (Verify) helpers -----

struct IdentityCtx {
    level: u8,
    res_x: u32,
    res_y: u32,
    t_min: u64,
    t_max: u64,
    q_min: u64,
    q_max: u64,
    by_tile: HashMap<u64, f32>, // tile_id -> identity percent
}

fn build_identity_ctx(anchors: &[Anchor], verify: &[VerifyResult]) -> Option<IdentityCtx> {
    if verify.is_empty() { return None; }
    let level = verify.iter().map(|v| (v.tile_id >> 56) as u8).max()?;
    // Determine resolution from verify tile ids
    let mut max_x = 0u32; let mut max_y = 0u32;
    for v in verify.iter().filter(|v| ((v.tile_id >> 56) as u8) == level) {
        let x = ((v.tile_id >> 28) & 0x0FFF_FFFF) as u32;
        let y = (v.tile_id & 0x0FFF_FFFF) as u32;
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    }
    let res_x = max_x + 1; let res_y = max_y + 1;
    // Extents from anchors
    let (mut t_min, mut t_max) = (u64::MAX, 0u64);
    let (mut q_min, mut q_max) = (u64::MAX, 0u64);
    for a in anchors {
        t_min = t_min.min(a.ts); t_max = t_max.max(a.te);
        q_min = q_min.min(a.qs); q_max = q_max.max(a.qe);
    }
    if t_min == u64::MAX { t_min = 0; }
    if q_min == u64::MAX { q_min = 0; }

    // Build tile identity map (if multiple per tile, keep max identity)
    let mut by_tile: HashMap<u64, f32> = HashMap::with_capacity(verify.len());
    for v in verify.iter().filter(|v| ((v.tile_id >> 56) as u8) == level) {
        by_tile
            .entry(v.tile_id)
            .and_modify(|e| { if v.identity > *e { *e = v.identity; } })
            .or_insert(v.identity);
    }
    Some(IdentityCtx { level, res_x, res_y, t_min, t_max, q_min, q_max, by_tile })
}

fn identity_for_anchor(a: &Anchor, ctx: &IdentityCtx) -> Option<f32> {
    let t_span = (ctx.t_max - ctx.t_min).max(1) as f64;
    let q_span = (ctx.q_max - ctx.q_min).max(1) as f64;
    let nx = ((a.ts.saturating_sub(ctx.t_min)) as f64 / t_span).clamp(0.0, 1.0);
    let ny = ((a.qs.saturating_sub(ctx.q_min)) as f64 / q_span).clamp(0.0, 1.0);
    let ix = (nx * (ctx.res_x.saturating_sub(1) as f64)).floor().max(0.0) as u32;
    let iy = (ny * (ctx.res_y.saturating_sub(1) as f64)).floor().max(0.0) as u32;
    let tile_id = ((ctx.level as u64) << 56) | ((ix as u64) << 28) | (iy as u64);
    ctx.by_tile.get(&tile_id).copied()
}

#[cfg(feature = "printpdf")]
impl PdfBuilder {
    fn new(config: &ExportConfig) -> Self {
        use printpdf::BuiltinFont;
        let doc = printpdf::PdfDocument::empty("DotX Export");
        // Try to load a simple built-in font; fall back to None if not available
        let font = doc.add_builtin_font(BuiltinFont::Helvetica).ok();
        Self { config: config.clone(), doc, current_page: None, current_layer: None, font }
    }

    fn add_page(&mut self) {
        let (page_index, layer_index) = self
            .doc
            .add_page(
                printpdf::Mm(self.config.width as f32 * 0.264583f32),
                printpdf::Mm(self.config.height as f32 * 0.264583f32),
                "Layer 1",
            );
        
        self.current_page = Some(page_index);
        self.current_layer = Some(layer_index);
    }

    fn add_background(&mut self) {
        // PDF background is typically white by default
    }

    fn add_title(&mut self, title: &str) {
        if self.font.is_none() { return; }
        let px_to_mm = 0.264583f32;
        let y = (self.config.font_size as f32 + 10.0) * px_to_mm;
        let x = (self.config.width as f32 / 2.0) * px_to_mm;
        let layer = self.layer();
        layer.begin_text_section();
        layer.set_font(self.font.as_ref().unwrap(), (self.config.font_size + 4) as f64);
        layer.set_text_cursor(printpdf::Mm(x), printpdf::Mm(y));
        layer.set_line_height(12.0);
        layer.set_text_rendering_mode(printpdf::TextRenderingMode::Fill);
        layer.write_text(title, self.font.as_ref().unwrap());
        layer.end_text_section();
    }

    fn render_density_heatmap(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()> {
        // Render a coarse density grid (no alpha), using light red squares
        let grid = 64usize;
        let mut counts = vec![0u32; grid * grid];
        let (mut minx, mut maxx) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut miny, mut maxy) = (f64::INFINITY, f64::NEG_INFINITY);
        for a in anchors { minx = minx.min(a.ts as f64); maxx = maxx.max(a.te as f64); miny = miny.min(a.qs as f64); maxy = maxy.max(a.qe as f64); }
        if !(minx < maxx && miny < maxy) { minx = viewport.x_min; maxx = viewport.x_max; miny = viewport.y_min; maxy = viewport.y_max; }
        for a in anchors {
            let nx = ((a.ts as f64 - minx) / (maxx - minx)).clamp(0.0, 1.0);
            let ny = ((a.qs as f64 - miny) / (maxy - miny)).clamp(0.0, 1.0);
            let ix = (nx * (grid as f64 - 1.0)) as usize;
            let iy = (ny * (grid as f64 - 1.0)) as usize;
            counts[iy * grid + ix] += 1;
        }
        let maxc = counts.iter().copied().max().unwrap_or(1);
        let px_to_mm = 0.264583f32;
        let cell_w = (self.config.width as f32 / grid as f32) * px_to_mm;
        let cell_h = (self.config.height as f32 / grid as f32) * px_to_mm;
        let layer = self.layer();
        for y in 0..grid {
            for x in 0..grid {
                let c = counts[y * grid + x];
                if c == 0 { continue; }
                let alpha = c as f32 / maxc as f32;
                let MmX = printpdf::Mm(x as f32 * cell_w);
                let MmY = printpdf::Mm((self.config.height as f32 * px_to_mm) - (y as f32 + 1.0) * cell_h);
                let rect = printpdf::Line {
                    points: vec![
                        (printpdf::Point::new(MmX, MmY), false),
                        (printpdf::Point::new(printpdf::Mm(MmX.0 + cell_w as f64), MmY), false),
                        (printpdf::Point::new(printpdf::Mm(MmX.0 + cell_w as f64), printpdf::Mm(MmY.0 + cell_h as f64)), false),
                        (printpdf::Point::new(MmX, printpdf::Mm(MmY.0 + cell_h as f64)), false),
                    ],
                    is_closed: true,
                    has_fill: true,
                    has_stroke: false,
                    is_clipping_path: false,
                };
                // Light red scaled by alpha (approximate by mixing with white)
                let r = 1.0f32;
                let g = (1.0 - 0.6 * alpha) as f32;
                let b = (1.0 - 0.6 * alpha) as f32;
                layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(r.into(), g.into(), b.into(), None)));
                layer.add_shape(rect);
            }
        }
        Ok(())
    }

    fn render_polylines(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()> {
        use dotx_core::chain::{Chainer, ChainParams};
        let chainer = Chainer::new(ChainParams::default());
        let chains = chainer.chain(anchors).unwrap_or_default();
        let px_to_mm = 0.264583f32;
        let layer = self.layer();
        for chain in chains.iter() {
            if chain.anchors.len() < 2 { continue; }
            let color = match chain.strand { Strand::Forward => (42.0/255.0, 111.0/255.0, 239.0/255.0), Strand::Reverse => (229.0/255.0, 57.0/255.0, 53.0/255.0) };
            layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(color.0.into(), color.1.into(), color.2.into(), None)));
            let mut points: Vec<(printpdf::Point, bool)> = Vec::new();
            for &idx in &chain.anchors {
                if let Some(a) = anchors.get(idx) {
                    let xm = (a.ts as f64 + a.te as f64) * 0.5;
                    let ym = (a.qs as f64 + a.qe as f64) * 0.5;
                    let (px, py) = self.world_to_pdf(xm, ym, viewport);
                    points.push((printpdf::Point::new(printpdf::Mm(px as f64 * px_to_mm as f64), printpdf::Mm(py as f64 * px_to_mm as f64)), false));
                }
            }
            let line = printpdf::Line { points, is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false };
            layer.add_shape(line);
        }
        Ok(())
    }

    fn render_points(&mut self, anchors: &[Anchor], viewport: &Viewport) -> Result<()> {
        let px_to_mm = 0.264583f32;
        let (fr, fg, fb) = parse_hex_rgb(&self.config.forward_color).unwrap_or((42.0/255.0, 111.0/255.0, 239.0/255.0));
        let (rr, rg, rb) = parse_hex_rgb(&self.config.reverse_color).unwrap_or((229.0/255.0, 57.0/255.0, 53.0/255.0));
        let layer = self.layer();
        let size_mm = 0.5; // small square
        for a in anchors.iter().take(5_000_000) {
            let (px, py) = self.world_to_pdf(a.ts as f64, a.qs as f64, viewport);
            let x_mm = px as f32 * px_to_mm;
            let y_mm = py as f32 * px_to_mm;
            let (r, g, b) = match a.strand { Strand::Forward => (fr, fg, fb), Strand::Reverse => (rr, rg, rb) };
            layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(r.into(), g.into(), b.into(), None)));
            let rect = printpdf::Line {
                points: vec![
                    (printpdf::Point::new(printpdf::Mm(x_mm as f64), printpdf::Mm(y_mm as f64)), false),
                    (printpdf::Point::new(printpdf::Mm((x_mm + size_mm) as f64), printpdf::Mm(y_mm as f64)), false),
                    (printpdf::Point::new(printpdf::Mm((x_mm + size_mm) as f64), printpdf::Mm((y_mm + size_mm) as f64)), false),
                    (printpdf::Point::new(printpdf::Mm(x_mm as f64), printpdf::Mm((y_mm + size_mm) as f64)), false),
                ],
                is_closed: true,
                has_fill: true,
                has_stroke: false,
                is_clipping_path: false,
            };
            layer.add_shape(rect);
        }
        Ok(())
    }

    fn add_legend(&mut self) {
        if self.font.is_none() { return; }
        let px_to_mm = 0.264583f32;
        let legend_x = 20.0 * px_to_mm;
        let legend_y = 20.0 * px_to_mm;
        let layer = self.layer();
        // Swatch size
        let sw = 8.0 * px_to_mm;
        let sh = 8.0 * px_to_mm;
        let (fr, fg, fb) = parse_hex_rgb(&self.config.forward_color).unwrap_or((42.0/255.0, 111.0/255.0, 239.0/255.0));
        let (rr, rg, rb) = parse_hex_rgb(&self.config.reverse_color).unwrap_or((229.0/255.0, 57.0/255.0, 53.0/255.0));
        // Forward swatch
        layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(fr.into(), fg.into(), fb.into(), None)));
        layer.add_shape(printpdf::Line { points: vec![
            (printpdf::Point::new(printpdf::Mm(legend_x as f64), printpdf::Mm(legend_y as f64)), false),
            (printpdf::Point::new(printpdf::Mm((legend_x + sw) as f64), printpdf::Mm(legend_y as f64)), false),
            (printpdf::Point::new(printpdf::Mm((legend_x + sw) as f64), printpdf::Mm((legend_y + sh) as f64)), false),
            (printpdf::Point::new(printpdf::Mm(legend_x as f64), printpdf::Mm((legend_y + sh) as f64)), false),
        ], is_closed: true, has_fill: true, has_stroke: false, is_clipping_path: false });
        // Reverse swatch
        let y2 = legend_y + 20.0 * px_to_mm;
        layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(rr.into(), rg.into(), rb.into(), None)));
        layer.add_shape(printpdf::Line { points: vec![
            (printpdf::Point::new(printpdf::Mm(legend_x as f64), printpdf::Mm(y2 as f64)), false),
            (printpdf::Point::new(printpdf::Mm((legend_x + sw) as f64), printpdf::Mm(y2 as f64)), false),
            (printpdf::Point::new(printpdf::Mm((legend_x + sw) as f64), printpdf::Mm((y2 + sh) as f64)), false),
            (printpdf::Point::new(printpdf::Mm(legend_x as f64), printpdf::Mm((y2 + sh) as f64)), false),
        ], is_closed: true, has_fill: true, has_stroke: false, is_clipping_path: false });
        // Labels
        let font = self.font.as_ref().unwrap();
        layer.begin_text_section();
        layer.set_font(font, self.config.font_size as f64);
        layer.set_text_cursor(printpdf::Mm((legend_x + sw + 6.0 * px_to_mm) as f64), printpdf::Mm((legend_y + sh * 0.8) as f64));
        layer.write_text("Forward (+)", font);
        layer.set_text_cursor(printpdf::Mm((legend_x + sw + 6.0 * px_to_mm) as f64), printpdf::Mm((y2 + sh * 0.8) as f64));
        layer.write_text("Reverse (-)", font);
        layer.end_text_section();
    }

    fn add_axes(&mut self, viewport: &Viewport, show_grid: bool) {
        if self.font.is_none() { return; }
        let px_to_mm = 0.264583f32;
        let left = 50.0 * px_to_mm; let right = (self.config.width as f32 - 20.0) * px_to_mm;
        let top = 20.0 * px_to_mm; let bottom = (self.config.height as f32 - 40.0) * px_to_mm;
        let layer = self.layer();
        layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
        // X axis
        layer.add_shape(printpdf::Line { points: vec![
            (printpdf::Point::new(printpdf::Mm(left as f64), printpdf::Mm(bottom as f64)), false),
            (printpdf::Point::new(printpdf::Mm(right as f64), printpdf::Mm(bottom as f64)), false),
        ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
        // Y axis
        layer.add_shape(printpdf::Line { points: vec![
            (printpdf::Point::new(printpdf::Mm(left as f64), printpdf::Mm(top as f64)), false),
            (printpdf::Point::new(printpdf::Mm(left as f64), printpdf::Mm(bottom as f64)), false),
        ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });

        // Ticks and grid
        let font = self.font.as_ref().unwrap();
        let x_ticks = nice_ticks_world(viewport.x_min, viewport.x_max, 6);
        for w in x_ticks {
            let px_canvas = ((w - viewport.x_min) / (viewport.x_max - viewport.x_min)) as f32 * (self.config.width as f32);
            let x = left + (px_canvas / self.config.width as f32) * (right - left);
            // Tick mark
            layer.add_shape(printpdf::Line { points: vec![
                (printpdf::Point::new(printpdf::Mm(x as f64), printpdf::Mm(bottom as f64)), false),
                (printpdf::Point::new(printpdf::Mm(x as f64), printpdf::Mm((bottom + 5.0 * px_to_mm) as f64)), false),
            ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
            // Grid
            if show_grid {
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.8, 0.8, 0.8, None)));
                layer.add_shape(printpdf::Line { points: vec![
                    (printpdf::Point::new(printpdf::Mm(x as f64), printpdf::Mm(top as f64)), false),
                    (printpdf::Point::new(printpdf::Mm(x as f64), printpdf::Mm(bottom as f64)), false),
                ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
            }
            // Label (relative span)
            if let Some(font) = self.font.as_ref() {
                let label = format_bp(w - viewport.x_min);
                layer.begin_text_section();
                layer.set_font(font, (self.config.font_size.saturating_sub(2)) as f64);
                layer.set_text_cursor(printpdf::Mm(x as f64), printpdf::Mm((bottom + 16.0 * px_to_mm) as f64));
                layer.write_text(&label, font);
                layer.end_text_section();
            }
        }

        let y_ticks = nice_ticks_world(viewport.y_min, viewport.y_max, 6);
        for w in y_ticks {
            let py_canvas = ((w - viewport.y_min) / (viewport.y_max - viewport.y_min)) as f32 * (self.config.height as f32);
            let y = top + (py_canvas / self.config.height as f32) * (bottom - top);
            layer.add_shape(printpdf::Line { points: vec![
                (printpdf::Point::new(printpdf::Mm((left - 5.0 * px_to_mm) as f64), printpdf::Mm(y as f64)), false),
                (printpdf::Point::new(printpdf::Mm(left as f64), printpdf::Mm(y as f64)), false),
            ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
            if show_grid {
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.8, 0.8, 0.8, None)));
                layer.add_shape(printpdf::Line { points: vec![
                    (printpdf::Point::new(printpdf::Mm(left as f64), printpdf::Mm(y as f64)), false),
                    (printpdf::Point::new(printpdf::Mm(right as f64), printpdf::Mm(y as f64)), false),
                ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
            }
            if let Some(font) = self.font.as_ref() {
                let label = format_bp(w - viewport.y_min);
                layer.begin_text_section();
                layer.set_font(font, (self.config.font_size.saturating_sub(2)) as f64);
                layer.set_text_cursor(printpdf::Mm((left - 8.0 * px_to_mm) as f64), printpdf::Mm(y as f64));
                layer.write_text(&label, font);
                layer.end_text_section();
            }
        }

        // Axis labels
        let font = self.font.as_ref().unwrap();
        layer.begin_text_section();
        layer.set_font(font, self.config.font_size as f64);
        layer.set_text_cursor(printpdf::Mm(((left + right) * 0.5) as f64), printpdf::Mm(((self.config.height as f32 - 8.0) * px_to_mm) as f64));
        layer.write_text("Target (bp)", font);
        layer.set_text_cursor(printpdf::Mm((18.0 * px_to_mm) as f64), printpdf::Mm(((top + bottom) * 0.5) as f64));
        layer.write_text("Query (bp)", font);
        layer.end_text_section();
    }

    fn add_scale_bar(&mut self, viewport: &Viewport) {
        // Reuse SVG logic for a 1/5 width scale bar
        if self.font.is_none() { return; }
        let px_to_mm = 0.264583f32;
        let target_px = (self.config.width as f32 / 5.0).max(60.0);
        let world_per_px = (viewport.x_max - viewport.x_min) / self.config.width as f64;
        let target_world = (world_per_px * target_px as f64).max(1.0);
        let nice_world = nice_round_length(target_world);
        let scale_bar_length_px = (nice_world / world_per_px) as f32;
        let x0 = 50.0 * px_to_mm;
        let y0 = (self.config.height as f32 - 50.0) * px_to_mm;
        let x1 = x0 + scale_bar_length_px * px_to_mm;
        let layer = self.layer();
        layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
        // Main line
        layer.add_shape(printpdf::Line { points: vec![
            (printpdf::Point::new(printpdf::Mm(x0 as f64), printpdf::Mm(y0 as f64)), false),
            (printpdf::Point::new(printpdf::Mm(x1 as f64), printpdf::Mm(y0 as f64)), false),
        ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
        // Ticks
        for &(tx, ty0, ty1) in [
            (x0, y0 - 5.0 * px_to_mm, y0 + 5.0 * px_to_mm),
            (x1, y0 - 5.0 * px_to_mm, y0 + 5.0 * px_to_mm),
        ].iter() {
            layer.add_shape(printpdf::Line { points: vec![
                (printpdf::Point::new(printpdf::Mm(tx as f64), printpdf::Mm(ty0 as f64)), false),
                (printpdf::Point::new(printpdf::Mm(tx as f64), printpdf::Mm(ty1 as f64)), false),
            ], is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false });
        }
        // Label
        let font = self.font.as_ref().unwrap();
        let label = format_bp(nice_world);
        layer.begin_text_section();
        layer.set_font(font, self.config.font_size as f64);
        layer.set_text_cursor(printpdf::Mm(((x0 + x1) * 0.5) as f64), printpdf::Mm((y0 + 10.0 * px_to_mm) as f64));
        layer.write_text(&label, font);
        layer.end_text_section();
    }

    fn add_footer(&mut self, viewport: &Viewport, lod_level: LodLevel) {
        if self.font.is_none() { return; }
        let px_to_mm = 0.264583f32;
        let y = (self.config.height as f32 - 10.0) * px_to_mm;
        let text = format!(
            "DotX | Viewport: {:.0}-{:.0} x {:.0}-{:.0} | LOD: {:?} | Generated: {}",
            viewport.x_min, viewport.x_max, viewport.y_min, viewport.y_max,
            lod_level,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        let font = self.font.as_ref().unwrap();
        let layer = self.layer();
        layer.begin_text_section();
        layer.set_font(font, (self.config.font_size.saturating_sub(2)) as f64);
        layer.set_text_cursor(printpdf::Mm(10.0 * px_to_mm as f64), printpdf::Mm(y as f64));
        layer.set_text_rendering_mode(printpdf::TextRenderingMode::Fill);
        layer.write_text(text, font);
        layer.end_text_section();
    }

    fn write_to_file<P: AsRef<Path>>(self, path: P) -> Result<()> {
        use std::fs::File;
        use std::io::BufWriter;
        let mut out = BufWriter::new(File::create(path)?);
        self.doc.save(&mut out)?;
        Ok(())
    }

    #[inline]
    fn layer(&self) -> printpdf::PdfLayerReference {
        let page = self.current_page.expect("PDF page not initialized");
        let layer = self.current_layer.expect("PDF layer not initialized");
        self.doc.get_page(page).get_layer(layer)
    }

    #[inline]
    fn world_to_pdf(&self, world_x: f64, world_y: f64, viewport: &Viewport) -> (f64, f64) {
        let px = ((world_x - viewport.x_min) / (viewport.x_max - viewport.x_min)) * self.config.width as f64;
        let py_norm = ((world_y - viewport.y_min) / (viewport.y_max - viewport.y_min)).clamp(0.0, 1.0);
        // invert Y for bottom-left origin
        let py = (1.0 - py_norm) * self.config.height as f64;
        (px, py)
    }
}

// PDF helpers omitted for now (font/shape simplicity)
