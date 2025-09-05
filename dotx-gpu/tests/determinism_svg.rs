use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_gpu::{Viewport, LodLevel};
use dotx_core::types::{Anchor, Strand};

fn demo_anchors() -> Vec<Anchor> {
    vec![
        Anchor::new("chrT".into(), "chrQ".into(), 100, 200, 100, 200, Strand::Forward, "demo".into()),
        Anchor::new("chrT".into(), "chrQ".into(), 300, 400, 300, 400, Strand::Forward, "demo".into()),
        Anchor::new("chrT".into(), "chrQ".into(), 500, 700, 520, 720, Strand::Reverse, "demo".into()),
        Anchor::new("chrT".into(), "chrQ".into(), 800, 900, 810, 910, Strand::Reverse, "demo".into()),
    ]
}

#[test]
fn svg_export_is_deterministic() {
    let anchors = demo_anchors();
    let vp = Viewport::new(0.0, 1000.0, 0.0, 1000.0, 800, 600);

    let cfg = ExportConfig {
        width: 800,
        height: 600,
        dpi: 96,
        show_legend: true,
        show_scale_bar: true,
        show_axes: true,
        show_footer: false, // disable dynamic timestamp
        show_grid: true,
        title: Some("Determinism Test".into()),
        background_color: "#ffffff".into(),
        forward_color: "#2a6fef".into(),
        reverse_color: "#e53935".into(),
        font_family: "Arial, sans-serif".into(),
        font_size: 12,
        provenance_comment: None,
    };
    let exporter = VectorExporter::new(cfg);

    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("a.svg");
    let f2 = dir.path().join("b.svg");

    exporter.export_svg(&f1, &anchors, &vp, LodLevel::DeepZoom, None).unwrap();
    exporter.export_svg(&f2, &anchors, &vp, LodLevel::DeepZoom, None).unwrap();

    let b1 = std::fs::read(&f1).unwrap();
    let b2 = std::fs::read(&f2).unwrap();
    assert_eq!(b1, b2, "SVG bytes differ between identical renders");
}
