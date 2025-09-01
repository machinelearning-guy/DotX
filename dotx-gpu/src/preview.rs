use anyhow::Result;
use dotx_core::*;

pub struct PreviewAligner {
    // GPU-based minimizer extraction and coarse chaining
    #[allow(dead_code)]
    device: wgpu::Device,
    #[allow(dead_code)]
    queue: wgpu::Queue,
}

impl PreviewAligner {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self { device, queue }
    }

    pub fn preview_align(
        &self,
        ref_seq: &[u8],
        qry_seq: &[u8],
        k: u16,
        w: u16,
    ) -> Result<Vec<PafRecord>> {
        // Extract minimizers from both sequences
        let ref_minimizers = self.extract_minimizers_gpu(ref_seq, k, w)?;
        let qry_minimizers = self.extract_minimizers_gpu(qry_seq, k, w)?;

        // Find common minimizers (seeds)
        let seeds = self.find_common_minimizers(&ref_minimizers, &qry_minimizers);

        // Perform coarse chaining
        let chains = self.coarse_chain_gpu(&seeds)?;

        // Convert chains to PAF records
        let mut paf_records = Vec::new();
        for chain in chains {
            if let Some(record) =
                self.chain_to_paf(&chain, ref_seq.len() as u64, qry_seq.len() as u64)
            {
                paf_records.push(record);
            }
        }

        Ok(paf_records)
    }

    fn extract_minimizers_gpu(&self, seq: &[u8], k: u16, w: u16) -> Result<Vec<Minimizer>> {
        // For now, implement CPU version - GPU version would use compute shaders
        self.extract_minimizers_cpu(seq, k, w)
    }

    fn extract_minimizers_cpu(&self, seq: &[u8], k: u16, w: u16) -> Result<Vec<Minimizer>> {
        let mut minimizers = Vec::new();
        let k = k as usize;
        let w = w as usize;

        if seq.len() < k {
            return Ok(minimizers);
        }

        // Rolling hash for k-mers
        let mut window = Vec::new();

        for i in 0..=seq.len() - k {
            let kmer = &seq[i..i + k];
            let hash = self.hash_kmer(kmer);

            window.push((hash, i));

            // Keep window size to w
            if window.len() > w {
                window.remove(0);
            }

            // Find minimum in current window
            if let Some((min_hash, min_pos)) = window.iter().min_by_key(|(hash, _)| *hash) {
                minimizers.push(Minimizer {
                    hash: *min_hash,
                    position: *min_pos as u64,
                    strand: true, // Forward strand for now
                });
            }
        }

        // Remove duplicates
        minimizers.sort_by_key(|m| (m.hash, m.position));
        minimizers.dedup_by_key(|m| (m.hash, m.position));

        Ok(minimizers)
    }

    fn hash_kmer(&self, kmer: &[u8]) -> u64 {
        // Simple hash function - in practice would use a better hash
        let mut hash = 0u64;
        for &base in kmer {
            hash = hash
                .wrapping_mul(4)
                .wrapping_add(match base.to_ascii_uppercase() {
                    b'A' => 0,
                    b'C' => 1,
                    b'G' => 2,
                    b'T' => 3,
                    _ => 0,
                });
        }
        hash
    }

    fn find_common_minimizers(&self, ref_mins: &[Minimizer], qry_mins: &[Minimizer]) -> Vec<Seed> {
        let mut seeds = Vec::new();

        // Create hash map of reference minimizers
        use std::collections::HashMap;
        let mut ref_map: HashMap<u64, Vec<&Minimizer>> = HashMap::new();
        for min in ref_mins {
            ref_map.entry(min.hash).or_default().push(min);
        }

        // Find matches in query
        for qry_min in qry_mins {
            if let Some(ref_matches) = ref_map.get(&qry_min.hash) {
                for ref_min in ref_matches {
                    seeds.push(Seed {
                        ref_pos: ref_min.position,
                        qry_pos: qry_min.position,
                        hash: ref_min.hash,
                    });
                }
            }
        }

        seeds
    }

    fn coarse_chain_gpu(&self, seeds: &[Seed]) -> Result<Vec<Chain>> {
        // For now, implement simple CPU chaining
        self.coarse_chain_cpu(seeds)
    }

    fn coarse_chain_cpu(&self, seeds: &[Seed]) -> Result<Vec<Chain>> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }

        let mut chains = Vec::new();
        let mut used = vec![false; seeds.len()];

        for i in 0..seeds.len() {
            if used[i] {
                continue;
            }

            let mut chain = Chain {
                seeds: vec![seeds[i]],
                score: 1.0,
            };
            used[i] = true;

            // Extend chain with compatible seeds
            for j in i + 1..seeds.len() {
                if used[j] {
                    continue;
                }

                if let Some(last_seed) = chain.seeds.last() {
                    if self.seeds_compatible(last_seed, &seeds[j]) {
                        chain.seeds.push(seeds[j]);
                        chain.score += 1.0;
                        used[j] = true;
                    }
                }
            }

            // Only keep chains with multiple seeds
            if chain.seeds.len() > 1 {
                chains.push(chain);
            }
        }

        // Sort chains by score
        chains.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(chains)
    }

    fn seeds_compatible(&self, seed1: &Seed, seed2: &Seed) -> bool {
        let ref_diff = seed2.ref_pos as i64 - seed1.ref_pos as i64;
        let qry_diff = seed2.qry_pos as i64 - seed1.qry_pos as i64;

        // Seeds should be in same relative order and reasonable distance
        ref_diff > 0 && qry_diff > 0 && (ref_diff - qry_diff).abs() < 1000 // Allow some divergence
    }

    fn chain_to_paf(&self, chain: &Chain, ref_len: u64, qry_len: u64) -> Option<PafRecord> {
        if chain.seeds.is_empty() {
            return None;
        }

        let first_seed = &chain.seeds[0];
        let last_seed = &chain.seeds[chain.seeds.len() - 1];

        Some(PafRecord {
            query_name: "query".to_string(),
            query_len: qry_len,
            query_start: first_seed.qry_pos,
            query_end: last_seed.qry_pos,
            strand: Strand::Forward,
            target_name: "reference".to_string(),
            target_len: ref_len,
            target_start: first_seed.ref_pos,
            target_end: last_seed.ref_pos,
            residue_matches: (chain.score * 0.9) as u64, // Estimate
            alignment_len: chain.score as u64,
            mapping_quality: (chain.score.min(60.0)) as u8,
            tags: vec![("tp".to_string(), "A".to_string(), "P".to_string())],
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Minimizer {
    pub hash: u64,
    pub position: u64,
    pub strand: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Seed {
    pub ref_pos: u64,
    pub qry_pos: u64,
    pub hash: u64,
}

#[derive(Debug, Clone)]
pub struct Chain {
    pub seeds: Vec<Seed>,
    pub score: f64,
}
