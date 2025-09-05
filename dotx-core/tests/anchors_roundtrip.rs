use dotx_core::{io::parse_alignment_file, DotXStore, build_density_tiles, TileBuildConfig};
use std::io::Write;
use tempfile::NamedTempFile;
use std::collections::HashMap;
use std::io::Seek;

fn write_paf(lines: &[&str]) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp paf");
    for l in lines { writeln!(f, "{}", l).unwrap(); }
    f
}

#[test]
fn import_roundtrip_no_tiles() {
    // Minimal PAF with 3 anchors across 2 contigs and both strands
    let paf_lines = vec![
        // qname qlen qs qe strand tname tlen ts te matches blen mapq
        "q1\t5000\t100\t900\t+\tt1\t8000\t500\t1300\t700\t800\t60",
        "q1\t5000\t1500\t2000\t-\tt1\t8000\t2000\t2500\t400\t600\t50",
        "q2\t10000\t0\t1000\t+\tt2\t12000\t0\t1000\t1000\t1000\t60",
    ];
    let paf = write_paf(&paf_lines);

    let anchors = parse_alignment_file(paf.path()).expect("parse PAF");
    assert_eq!(anchors.len(), 3);

    let mut store = DotXStore::new();

    // Collect contig lengths from parsed anchors (fallback to max end if missing)
    let mut q_max: HashMap<String, u64> = HashMap::new();
    let mut t_max: HashMap<String, u64> = HashMap::new();
    for a in &anchors {
        let qlen = a.query_length.unwrap_or(a.qe.max(1));
        let tlen = a.target_length.unwrap_or(a.te.max(1));
        q_max.entry(a.q.clone()).and_modify(|m| *m = (*m).max(qlen)).or_insert(qlen);
        t_max.entry(a.t.clone()).and_modify(|m| *m = (*m).max(tlen)).or_insert(tlen);
    }
    for (name, len) in q_max.into_iter() { store.add_query_contig(name, len, None); }
    for (name, len) in t_max.into_iter() { store.add_target_contig(name, len, None); }

    // Write DB without tiles
    let db = NamedTempFile::new().expect("create temp db");
    store.write_to_file(db.path(), &anchors).expect("write .dotxdb");

    // Read back and compare counts
    let loaded = DotXStore::read_from_file(db.path()).expect("read .dotxdb");
    let mut fh = std::fs::File::open(db.path()).unwrap();
    let anchors_back = loaded.read_anchors(&mut fh).expect("read anchors");
    assert_eq!(anchors_back.len(), 3);
}

#[test]
fn import_roundtrip_with_tiles() {
    // Minimal PAF with 4 anchors to exercise tiling
    let paf_lines = vec![
        "qA\t4000\t0\t500\t+\ttA\t6000\t0\t500\t480\t500\t60",
        "qA\t4000\t1000\t1500\t+\ttA\t6000\t1000\t1500\t480\t500\t60",
        "qB\t8000\t200\t1200\t-\ttB\t10000\t300\t1300\t700\t1000\t40",
        "qB\t8000\t5000\t6000\t+\ttB\t10000\t7000\t8000\t900\t1000\t50",
    ];
    let paf = write_paf(&paf_lines);

    let anchors = parse_alignment_file(paf.path()).expect("parse PAF");
    assert_eq!(anchors.len(), 4);

    let mut store = DotXStore::new();
    let mut q_max: HashMap<String, u64> = HashMap::new();
    let mut t_max: HashMap<String, u64> = HashMap::new();
    for a in &anchors {
        let qlen = a.query_length.unwrap_or(a.qe.max(1));
        let tlen = a.target_length.unwrap_or(a.te.max(1));
        q_max.entry(a.q.clone()).and_modify(|m| *m = (*m).max(qlen)).or_insert(qlen);
        t_max.entry(a.t.clone()).and_modify(|m| *m = (*m).max(tlen)).or_insert(tlen);
    }
    for (name, len) in q_max.into_iter() { store.add_query_contig(name, len, None); }
    for (name, len) in t_max.into_iter() { store.add_target_contig(name, len, None); }

    let tiles = build_density_tiles(&anchors, TileBuildConfig::default());
    assert!(!tiles.is_empty());

    let db = NamedTempFile::new().expect("create temp db");
    store.write_to_file_with_tiles(db.path(), &anchors, &tiles).expect("write .dotxdb with tiles");

    let loaded = DotXStore::read_from_file(db.path()).expect("read .dotxdb");
    let mut fh = std::fs::File::open(db.path()).unwrap();
    let anchors_back = loaded.read_anchors(&mut fh).expect("read anchors");
    assert_eq!(anchors_back.len(), 4);
    fh.rewind().unwrap();
    let tiles_back = loaded.read_tiles(&mut fh).expect("read tiles");
    assert!(!tiles_back.is_empty());
}
