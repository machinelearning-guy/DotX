use dotx_core::*;

#[test]
fn test_anchor_creation() {
    let anchor = Anchor::new(
        "chr1".to_string(),
        "chr2".to_string(),
        1000,
        2000,
        3000,
        4000,
        Strand::Forward,
        "minimap2".to_string(),
    ).with_mapq(60).with_identity(0.95);

    assert_eq!(anchor.q, "chr1");
    assert_eq!(anchor.t, "chr2");
    assert_eq!(anchor.qs, 1000);
    assert_eq!(anchor.qe, 2000);
    assert_eq!(anchor.ts, 3000);
    assert_eq!(anchor.te, 4000);
    assert_eq!(anchor.strand, Strand::Forward);
    assert_eq!(anchor.mapq, Some(60));
    assert_eq!(anchor.identity, Some(0.95));
    assert_eq!(anchor.engine_tag, "minimap2");
    assert_eq!(anchor.query_length(), 1000);
    assert_eq!(anchor.target_length(), 1000);
    assert_eq!(anchor.alignment_length(), 1000);
}

#[test]
fn test_chain_creation() {
    let mut chain = Chain::new(42);
    assert_eq!(chain.chain_id, 42);
    assert!(chain.is_empty());
    assert_eq!(chain.len(), 0);

    let anchor1 = Anchor::new(
        "chr1".to_string(),
        "chr2".to_string(),
        100,
        200,
        300,
        400,
        Strand::Forward,
        "minimap2".to_string(),
    );

    let anchor2 = Anchor::new(
        "chr1".to_string(),
        "chr2".to_string(),
        250,
        350,
        450,
        550,
        Strand::Forward,
        "minimap2".to_string(),
    );

    chain.add_anchor(anchor1);
    chain.add_anchor(anchor2);

    assert_eq!(chain.len(), 2);
    assert!(!chain.is_empty());
    assert_eq!(chain.total_query_span(), 250); // 350 - 100
    assert_eq!(chain.total_target_span(), 250); // 550 - 300
    assert_eq!(chain.score, 200.0); // 100 + 100 alignment lengths
}

#[test]
fn test_dotxdb_file() {
    let mut db = DotxdbFile::new(6);
    
    // Add sample and contig
    db.add_sample(Sample {
        name: "test_sample".to_string(),
        path: "/test.fa".to_string(),
        description: Some("Test".to_string()),
        total_length: 10000,
        num_contigs: 2,
        checksum: None,
    });

    db.add_contig(ContigInfo {
        name: "chr1".to_string(),
        length: 5000,
        sample_id: "test_sample".to_string(),
        index: 0,
    });

    assert_eq!(db.meta.samples.len(), 1);
    assert_eq!(db.meta.contigs.len(), 1);

    // Test serialization
    let bytes = db.to_bytes().unwrap();
    let restored = DotxdbFile::from_bytes(&bytes).unwrap();
    
    assert_eq!(db.header.magic, restored.header.magic);
    assert_eq!(db.meta.samples.len(), restored.meta.samples.len());
    assert_eq!(db.meta.contigs.len(), restored.meta.contigs.len());
}

#[test]
fn test_serializer() {
    let config = SerializationConfig::default();
    let serializer = Serializer::new(config);

    let anchor = Anchor::new(
        "chrX".to_string(),
        "chrY".to_string(),
        500,
        1500,
        2000,
        3000,
        Strand::Reverse,
        "syncmer".to_string(),
    );

    let bytes = serializer.serialize(&anchor).unwrap();
    let restored: Anchor = serializer.deserialize(&bytes).unwrap();

    assert_eq!(anchor.q, restored.q);
    assert_eq!(anchor.t, restored.t);
    assert_eq!(anchor.qs, restored.qs);
    assert_eq!(anchor.qe, restored.qe);
    assert_eq!(anchor.ts, restored.ts);
    assert_eq!(anchor.te, restored.te);
    assert_eq!(anchor.strand, restored.strand);
    assert_eq!(anchor.engine_tag, restored.engine_tag);
}

#[test]
fn test_dotxdb_builder() {
    let mut builder = DotxdbBuilder::new(6);
    
    // Add sample and contigs
    builder.add_sample(Sample {
        name: "test".to_string(),
        path: "/test.fa".to_string(),
        description: None,
        total_length: 10000,
        num_contigs: 2,
        checksum: None,
    });

    builder.add_contig(ContigInfo {
        name: "chr1".to_string(),
        length: 5000,
        sample_id: "test".to_string(),
        index: 0,
    });

    builder.add_contig(ContigInfo {
        name: "chr2".to_string(),
        length: 5000,
        sample_id: "test".to_string(),
        index: 1,
    });

    // Add some anchors
    let anchors = vec![
        Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            100,
            200,
            300,
            400,
            Strand::Forward,
            "minimap2".to_string(),
        ),
        Anchor::new(
            "chr1".to_string(),
            "chr2".to_string(),
            250,
            350,
            450,
            550,
            Strand::Forward,
            "minimap2".to_string(),
        ),
    ];

    builder.add_anchors(anchors).unwrap();
    let db = builder.build();

    assert_eq!(db.meta.samples.len(), 1);
    assert_eq!(db.meta.contigs.len(), 2);
    assert_eq!(db.anchors.len(), 2);
    assert_eq!(db.meta.total_anchors, 2);
}