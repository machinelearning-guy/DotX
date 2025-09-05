use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_gpu::{Viewport, LodLevel};
use dotx_core::types::{Anchor, Strand};

fn demo_anchors_dense() -> Vec<Anchor> {
    // Make a small dense cluster to exercise PNG density drawing
    let mut v = Vec::new();
    for i in 0..1000u64 {
        v.push(Anchor::new(
            "t".into(),
            "q".into(),
            100 + i, 100 + i + 1,
            100 + i, 100 + i + 1,
            if i % 2 == 0 { Strand::Forward } else { Strand::Reverse },
            "demo".into(),
        ));
    }
    v
}

fn histogram(png_bytes: &[u8]) -> [u32; 256] {
    let img = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        // bucket by alpha for stability across tiny color diffs
        hist[p[3] as usize] += 1;
    }
    hist
}

#[test]
fn png_histogram_is_stable() {
    let anchors = demo_anchors_dense();
    let vp = Viewport::new(0.0, 1000.0, 0.0, 1000.0, 640, 480);

    let cfg = ExportConfig {
        width: 640,
        height: 480,
        dpi: 96,
        show_legend: false,
        show_scale_bar: false,
        show_axes: false,
        show_footer: false,
        show_grid: false,
        title: None,
        background_color: "#ffffff".into(),
        forward_color: "#2a6fef".into(),
        reverse_color: "#e53935".into(),
        font_family: "Arial, sans-serif".into(),
        font_size: 12,
        provenance_comment: None,
    };
    let exporter = VectorExporter::new(cfg);

    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("d1.png");
    let f2 = dir.path().join("d2.png");

    exporter.export_png_simple(&f1, &anchors, &vp, LodLevel::Overview, None).unwrap();
    exporter.export_png_simple(&f2, &anchors, &vp, LodLevel::Overview, None).unwrap();
    let b1 = std::fs::read(&f1).unwrap();
    let b2 = std::fs::read(&f2).unwrap();

    let h1 = histogram(&b1);
    let h2 = histogram(&b2);
    assert_eq!(h1, h2, "Alpha histogram differs between identical renders");
}
