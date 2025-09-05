#![cfg(feature = "printpdf")]

use dotx_gpu::{Viewport, LodLevel};
use dotx_gpu::vector_export::{VectorExporter, ExportConfig};
use dotx_core::types::{Anchor, Strand};
use tempfile::NamedTempFile;

#[test]
fn export_pdf_smoke() {
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
    let cfg = ExportConfig { title: Some("DOTx PDF Test".into()), ..Default::default() };
    let exporter = VectorExporter::new(cfg);
    let out = NamedTempFile::new().expect("tmp pdf");
    // Just check it doesn't error when feature is enabled
    let res = exporter.export_pdf(out.path(), &anchors, &vp, LodLevel::DeepZoom, None);
    assert!(res.is_ok());
}

