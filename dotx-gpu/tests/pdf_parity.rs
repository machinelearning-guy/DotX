#![cfg(feature = "printpdf")]

use dotx_gpu::{Viewport, LodLevel};
use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_core::types::{Anchor, Strand};
use tempfile::NamedTempFile;

#[test]
fn export_pdf_contains_footer_and_axes_labels() {
    let anchors = vec![
        Anchor::new(
            "q".to_string(),
            "t".to_string(),
            10, 20,
            30, 40,
            Strand::Forward,
            "test".to_string(),
        ),
    ];

    let vp = Viewport::new(0.0, 100.0, 0.0, 100.0, 800, 600);
    let cfg = ExportConfig { title: Some("DOTx PDF Parity".into()), ..Default::default() };
    let exporter = VectorExporter::new(cfg);
    let out = NamedTempFile::new().expect("tmp pdf");
    exporter.export_pdf(out.path(), &anchors, &vp, LodLevel::DeepZoom, None).expect("pdf export");

    let bytes = std::fs::read(out.path()).expect("read pdf bytes");
    let haystack = String::from_utf8_lossy(&bytes);
    // Footer text includes this label
    assert!(haystack.contains("DotX | Viewport:"));
    // Axis labels added in PDF path
    assert!(haystack.contains("Target (bp)"));
    assert!(haystack.contains("Query (bp)"));
}

