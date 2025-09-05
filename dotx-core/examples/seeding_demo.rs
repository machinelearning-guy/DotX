//! DOTx Seeding Engines Demo
//!
//! This example demonstrates the usage of different seeding engines
//! provided by the DOTx core library.

use dotx_core::{SeedParams, SeederFactory, AlgorithmParams};
use dotx_core::seed::{
    kmer::KmerPresets,
    syncmer::SyncmerPresets,
    strobemer::StrobemerPresets,
    minimap2::Minimap2Presets,
};

fn main() {
    println!("DOTx Seeding Engines Demo");
    println!("========================\n");

    // Example sequences - similar with some variations
    let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
    let target = b"ATCGATCGATCGATCGAAATCGATCGATCGATCGATCGATCGATCGATCGATC"; // Small insertion

    println!("Query:  {}", std::str::from_utf8(query).unwrap());
    println!("Target: {}", std::str::from_utf8(target).unwrap());
    println!("        ^                  ^^ insertion here\n");

    // Demo each seeding engine
    demo_kmer_seeding(query, target);
    demo_syncmer_seeding(query, target);
    demo_strobemer_seeding(query, target);
    demo_minimap2_seeding(query, target);
}

fn demo_kmer_seeding(query: &[u8], target: &[u8]) {
    println!("🧬 K-mer Seeding Engine");
    println!("======================");

    let params = SeedParams {
        k: 15,
        algorithm_params: AlgorithmParams::Kmer,
        max_freq: Some(10),
        min_anchor_len: 15,
        ..Default::default()
    };

    let seeder = SeederFactory::create(&params);
    
    match seeder.seed(query, "query", target, "target", &params) {
        Ok(anchors) => {
            println!("Found {} anchors", anchors.len());
            for (i, anchor) in anchors.iter().enumerate().take(5) {
                println!("  Anchor {}: Q{}..{} -> T{}..{} ({})", 
                    i + 1,
                    anchor.query_start, anchor.query_end,
                    anchor.target_start, anchor.target_end,
                    anchor.strand
                );
            }
            if anchors.len() > 5 {
                println!("  ... and {} more anchors", anchors.len() - 5);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();
}

fn demo_syncmer_seeding(query: &[u8], target: &[u8]) {
    println!("🔗 Syncmer Seeding Engine"); 
    println!("=========================");

    let params = SeedParams {
        k: 15,
        algorithm_params: SyncmerPresets::default(),
        min_anchor_len: 15,
        ..Default::default()
    };

    let seeder = SeederFactory::create(&params);
    
    match seeder.seed(query, "query", target, "target", &params) {
        Ok(anchors) => {
            println!("Found {} anchors", anchors.len());
            for (i, anchor) in anchors.iter().enumerate().take(3) {
                println!("  Anchor {}: Q{}..{} -> T{}..{} ({})", 
                    i + 1,
                    anchor.query_start, anchor.query_end,
                    anchor.target_start, anchor.target_end,
                    anchor.strand
                );
            }
            if anchors.len() > 3 {
                println!("  ... and {} more anchors", anchors.len() - 3);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();
}

fn demo_strobemer_seeding(query: &[u8], target: &[u8]) {
    println!("🌐 Strobemer Seeding Engine");
    println!("===========================");

    let params = SeedParams {
        k: 15,
        algorithm_params: StrobemerPresets::default(),
        min_anchor_len: 20,
        ..Default::default()
    };

    let seeder = SeederFactory::create(&params);
    
    match seeder.seed(query, "query", target, "target", &params) {
        Ok(anchors) => {
            println!("Found {} anchors", anchors.len());
            for (i, anchor) in anchors.iter().enumerate().take(3) {
                println!("  Anchor {}: Q{}..{} -> T{}..{} ({}) [len: {}]", 
                    i + 1,
                    anchor.query_start, anchor.query_end,
                    anchor.target_start, anchor.target_end,
                    anchor.strand,
                    anchor.query_len()
                );
            }
            if anchors.len() > 3 {
                println!("  ... and {} more anchors", anchors.len() - 3);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();
}

fn demo_minimap2_seeding(query: &[u8], target: &[u8]) {
    println!("⚡ Minimap2 Seeding Engine");
    println!("=========================");

    let seeder = SeederFactory::create(&SeedParams {
        algorithm_params: Minimap2Presets::asm5(),
        ..Default::default()
    });

    if !seeder.is_available() {
        println!("Minimap2 binary not available - skipping demo");
        println!("To try this engine, install minimap2 and ensure it's in PATH");
        println!();
        return;
    }

    let params = SeedParams {
        k: 15,
        algorithm_params: Minimap2Presets::asm5(),
        min_anchor_len: 20,
        ..Default::default()
    };
    
    match seeder.seed(query, "query", target, "target", &params) {
        Ok(anchors) => {
            println!("Found {} anchors", anchors.len());
            for (i, anchor) in anchors.iter().enumerate().take(3) {
                let identity_str = anchor.identity
                    .map(|id| format!("{:.1}%", id))
                    .unwrap_or_else(|| "N/A".to_string());
                    
                println!("  Anchor {}: Q{}..{} -> T{}..{} ({}) [identity: {}]", 
                    i + 1,
                    anchor.query_start, anchor.query_end,
                    anchor.target_start, anchor.target_end,
                    anchor.strand,
                    identity_str
                );
            }
            if anchors.len() > 3 {
                println!("  ... and {} more anchors", anchors.len() - 3);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_sequences() {
        let query = b"ATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCGATCG";
        let target = b"ATCGATCGATCGATCGAAATCGATCGATCGATCGATCGATCGATCGATCGATC";
        
        assert!(query.len() > 30);
        assert!(target.len() > 30);
        
        // Should be able to create seeders without errors
        let kmer_seeder = SeederFactory::create(&SeedParams {
            algorithm_params: AlgorithmParams::Kmer,
            ..Default::default()
        });
        
        assert_eq!(kmer_seeder.name(), "kmer");
    }
}