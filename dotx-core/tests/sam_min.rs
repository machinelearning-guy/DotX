#![cfg(feature = "io-sam")]

use dotx_core::io::SamParser;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn parse_minimal_sam_record() {
    // Minimal SAM with header and one aligned record
    let mut f = NamedTempFile::new().expect("tmp sam");
    writeln!(f, "@HD\tVN:1.6").unwrap();
    writeln!(f, "@SQ\tSN:ref\tLN:1000").unwrap();
    // QNAME FLAG RNAME POS MAPQ CIGAR RNEXT PNEXT TLEN SEQ QUAL
    writeln!(f, "r1\t0\tref\t101\t60\t10M\t*\t0\t0\tACGTACGTAC\t*").unwrap();
    f.as_file().sync_all().unwrap();

    let anchors = SamParser::parse_file(f.path()).expect("parse sam");
    assert_eq!(anchors.len(), 1);
    let a = &anchors[0];
    // We only assert basic coordinate sanity here (parser fidelity evolves)
    assert!(a.qe > a.qs);
    assert!(a.te > a.ts);
    // Reference name, 0-based start, and MAPQ/length should be populated
    assert_eq!(a.t, "ref");
    assert_eq!(a.ts, 100); // POS=101 in SAM -> 0-based 100
    assert_eq!(a.mapq, Some(60));
    assert_eq!(a.target_length, Some(1000));
}
