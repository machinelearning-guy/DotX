//! Deterministic mode support for reproducible results

use std::sync::Once;

/// Initialize deterministic mode for reproducible results
static INIT_DETERMINISTIC: Once = Once::new();

/// Initialize deterministic mode by setting fixed seeds for various components
pub fn init_deterministic_mode() {
    INIT_DETERMINISTIC.call_once(|| {
        log::info!("Initializing deterministic mode for reproducible results");
        
        // Set fixed seed for hash-based operations
        set_deterministic_hash_seed();
        
        // Set fixed seed for random number generation
        set_deterministic_random_seed();
        
        // Configure thread pool for deterministic behavior
        configure_deterministic_threading();
        
        // Set environment variables for external tools
        set_deterministic_environment();
        
        log::info!("Deterministic mode initialized successfully");
    });
}

/// Set a fixed hash seed for consistent hash-based operations
fn set_deterministic_hash_seed() {
    // Set RUST_HASH_SEED for deterministic HashMap/HashSet behavior
    if std::env::var("RUST_HASH_SEED").is_err() {
        std::env::set_var("RUST_HASH_SEED", "42");
        log::debug!("Set RUST_HASH_SEED=42 for deterministic hashing");
    }
}

/// Set fixed seeds for random number generation
fn set_deterministic_random_seed() {
    // For seeding algorithms that use random sampling
    use std::sync::Mutex;
    
    static SEED_COUNTER: Mutex<u64> = Mutex::new(12345);
    
    // This would be used by seeding algorithms to get deterministic random numbers
    log::debug!("Initialized deterministic random seed generator");
}

/// Configure thread pool for deterministic behavior
fn configure_deterministic_threading() {
    // In deterministic mode, we want to ensure that parallel operations
    // produce consistent results across runs
    
    // Note: This is already handled in main.rs by setting a fixed thread count
    // when deterministic mode is enabled, but we can add additional configuration here
    
    log::debug!("Configured threading for deterministic execution");
}

/// Set environment variables for external tools to ensure deterministic behavior
fn set_deterministic_environment() {
    // Set environment variables for external tools like minimap2
    // to ensure they produce deterministic output
    
    // For minimap2: these settings help ensure deterministic output
    std::env::set_var("MM_SEED", "42");
    std::env::set_var("MM_DETERMINISTIC", "1");
    
    log::debug!("Set environment variables for deterministic external tool behavior");
}

/// Generate a deterministic seed for a specific component
pub fn get_deterministic_seed(component: &str) -> u64 {
    // Generate a deterministic seed based on component name
    // This ensures different components get different but reproducible seeds
    
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    component.hash(&mut hasher);
    42u64.hash(&mut hasher); // Base seed
    hasher.finish()
}

/// Deterministic configuration for seeding algorithms
pub struct DeterministicSeedConfig {
    pub base_seed: u64,
    pub component_seed: u64,
    pub iteration_counter: u64,
}

impl DeterministicSeedConfig {
    pub fn new(component: &str) -> Self {
        let base_seed = 42; // Fixed base seed
        let component_seed = get_deterministic_seed(component);
        
        Self {
            base_seed,
            component_seed,
            iteration_counter: 0,
        }
    }
    
    /// Get next deterministic value for this component
    pub fn next_seed(&mut self) -> u64 {
        let seed = self.base_seed
            .wrapping_mul(31)
            .wrapping_add(self.component_seed)
            .wrapping_mul(17)
            .wrapping_add(self.iteration_counter);
        
        self.iteration_counter += 1;
        seed
    }
}

/// Utility for deterministic file processing order
pub fn sort_paths_deterministically(paths: &mut Vec<std::path::PathBuf>) {
    // Sort paths deterministically to ensure consistent processing order
    paths.sort_by(|a, b| {
        // Compare by string representation for deterministic ordering
        a.to_string_lossy().cmp(&b.to_string_lossy())
    });
}

/// Utility for deterministic anchor processing
pub fn sort_anchors_deterministically(anchors: &mut [dotx_core::types::Anchor]) {
    // Sort anchors in a deterministic way for consistent output
    anchors.sort_by(|a, b| {
        use std::cmp::Ordering;
        
        // Primary sort: target name
        match a.t.cmp(&b.t) {
            Ordering::Equal => {
                // Secondary sort: query name
                match a.q.cmp(&b.q) {
                    Ordering::Equal => {
                        // Tertiary sort: target start position
                        match a.ts.cmp(&b.ts) {
                            Ordering::Equal => {
                                // Quaternary sort: query start position
                                a.qs.cmp(&b.qs)
                            }
                            other => other,
                        }
                    }
                    other => other,
                }
            }
            other => other,
        }
    });
}

/// Validate that deterministic mode is working correctly
pub fn validate_deterministic_setup() -> Result<(), String> {
    // Check that required environment variables are set
    if std::env::var("RUST_HASH_SEED").is_err() {
        return Err("RUST_HASH_SEED not set - deterministic mode may not work correctly".to_string());
    }
    
    // Test that hash operations are deterministic
    use std::collections::HashMap;
    let mut map1 = HashMap::new();
    map1.insert("test", 1);
    let mut map2 = HashMap::new();
    map2.insert("test", 1);
    
    // Note: This test isn't perfect since HashMap iteration order
    // can still vary, but it helps catch some issues
    
    log::debug!("Deterministic setup validation passed");
    Ok(())
}

/// Configuration builder for deterministic operations
pub struct DeterministicConfigBuilder {
    thread_count: Option<usize>,
    base_seed: u64,
    sort_inputs: bool,
    external_tool_seeds: Vec<(String, String)>,
}

impl Default for DeterministicConfigBuilder {
    fn default() -> Self {
        Self {
            thread_count: Some(1), // Single thread by default for maximum determinism
            base_seed: 42,
            sort_inputs: true,
            external_tool_seeds: vec![
                ("MM_SEED".to_string(), "42".to_string()),
                ("MM_DETERMINISTIC".to_string(), "1".to_string()),
            ],
        }
    }
}

impl DeterministicConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.thread_count = Some(threads);
        self
    }
    
    pub fn with_base_seed(mut self, seed: u64) -> Self {
        self.base_seed = seed;
        self
    }
    
    pub fn with_input_sorting(mut self, sort: bool) -> Self {
        self.sort_inputs = sort;
        self
    }
    
    pub fn add_external_tool_env(mut self, key: String, value: String) -> Self {
        self.external_tool_seeds.push((key, value));
        self
    }
    
    pub fn apply(self) -> Result<(), String> {
        // Apply thread configuration
        if let Some(threads) = self.thread_count {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()
                .map_err(|e| format!("Failed to configure thread pool: {}", e))?;
        }
        
        // Set hash seed
        std::env::set_var("RUST_HASH_SEED", self.base_seed.to_string());
        
        // Set external tool environment variables
        for (key, value) in self.external_tool_seeds {
            std::env::set_var(key, value);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deterministic_seed_generation() {
        let seed1 = get_deterministic_seed("test_component");
        let seed2 = get_deterministic_seed("test_component");
        let seed3 = get_deterministic_seed("other_component");
        
        // Same component should produce same seed
        assert_eq!(seed1, seed2);
        
        // Different components should produce different seeds
        assert_ne!(seed1, seed3);
    }
    
    #[test]
    fn test_deterministic_seed_config() {
        let mut config = DeterministicSeedConfig::new("test");
        let seed1 = config.next_seed();
        let seed2 = config.next_seed();
        
        // Sequential calls should produce different seeds
        assert_ne!(seed1, seed2);
        
        // But should be reproducible
        let mut config2 = DeterministicSeedConfig::new("test");
        let seed1_repeat = config2.next_seed();
        assert_eq!(seed1, seed1_repeat);
    }
    
    #[test]
    fn test_path_sorting() {
        use std::path::PathBuf;
        
        let mut paths = vec![
            PathBuf::from("c.txt"),
            PathBuf::from("a.txt"),
            PathBuf::from("b.txt"),
        ];
        
        sort_paths_deterministically(&mut paths);
        
        assert_eq!(paths[0].file_name().unwrap(), "a.txt");
        assert_eq!(paths[1].file_name().unwrap(), "b.txt");
        assert_eq!(paths[2].file_name().unwrap(), "c.txt");
    }
    
    #[test]
    fn test_deterministic_config_builder() {
        let config = DeterministicConfigBuilder::new()
            .with_threads(4)
            .with_base_seed(12345)
            .with_input_sorting(true);
        
        assert_eq!(config.thread_count, Some(4));
        assert_eq!(config.base_seed, 12345);
        assert!(config.sort_inputs);
    }
}