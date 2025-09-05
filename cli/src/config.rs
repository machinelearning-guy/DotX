//! Configuration handling for DOTx CLI
//!
//! Supports loading configuration from dotx.toml files with CLI argument overrides.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub io: IoConfig,
    pub render: RenderConfig,
    pub plot: PlotConfig,
    pub map: MapConfig,
    pub verify: VerifyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Enable deterministic mode for reproducible results
    #[serde(default)]
    pub deterministic: bool,
    
    /// Default number of threads to use
    #[serde(default = "default_threads")]
    pub threads: usize,
    
    /// Color theme name
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoConfig {
    /// Maximum memory usage in GB
    #[serde(default = "default_max_memory")]
    pub max_memory_gb: f64,
    
    /// Block compression algorithm
    #[serde(default = "default_compression")]
    pub block_compression: String,
    
    /// Default compression level
    #[serde(default = "default_compression_level")]
    pub compression_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Level-of-detail mode for overview
    #[serde(default = "default_lod_overview")]
    pub lod_overview: String,
    
    /// Level-of-detail mode for mid zoom
    #[serde(default = "default_lod_mid")]
    pub lod_mid: String,
    
    /// Level-of-detail mode for deep zoom
    #[serde(default = "default_lod_deep")]
    pub lod_deep: String,
    
    /// Show forward strand by default
    #[serde(default = "default_true")]
    pub show_strand_plus: bool,
    
    /// Show reverse strand by default
    #[serde(default = "default_true")]
    pub show_strand_minus: bool,
    
    /// Default DPI for raster outputs
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    
    /// Default width
    #[serde(default = "default_width")]
    pub width: u32,
    
    /// Default height
    #[serde(default = "default_height")]
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotConfig {
    /// X-axis sequence type ("target" or "query")
    #[serde(default = "default_x_axis")]
    pub x_axis: String,
    
    /// Y-axis sequence type ("query" or "target")
    #[serde(default = "default_y_axis")]
    pub y_axis: String,
    
    /// Color for forward strand matches
    #[serde(default = "default_color_plus")]
    pub color_plus: String,
    
    /// Color for reverse strand matches
    #[serde(default = "default_color_minus")]
    pub color_minus: String,
    
    /// Default point size for anchors
    #[serde(default = "default_point_size")]
    pub point_size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    /// Default seeding engine
    #[serde(default = "default_engine")]
    pub engine: String,
    
    /// Default minimap2 preset
    #[serde(default = "default_preset")]
    pub preset: String,
    
    /// Seed density setting
    #[serde(default = "default_seed_density")]
    pub seed_density: String,
    
    /// Default k-mer size
    #[serde(default = "default_k")]
    pub k: u32,
    
    /// Default maximum frequency threshold
    #[serde(default = "default_max_freq")]
    pub max_freq: u32,
    
    /// Default minimum anchor length
    #[serde(default = "default_min_anchor_len")]
    pub min_anchor_len: u32,
    
    /// Enable low-complexity masking by default
    #[serde(default = "default_true")]
    pub mask_low_complexity: bool,
    
    /// Syncmer parameters
    pub syncmer: SyncmerConfig,
    
    /// Strobemer parameters
    pub strobemer: StrobemerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncmerConfig {
    /// Default syncmer size (s parameter)
    #[serde(default = "default_syncmer_s")]
    pub s: u32,
    
    /// Default syncmer threshold (t parameter)
    #[serde(default = "default_syncmer_t")]
    pub t: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrobemerConfig {
    /// Default window size
    #[serde(default = "default_strobemer_window")]
    pub window_size: u32,
    
    /// Default maximum distance
    #[serde(default = "default_strobemer_max_distance")]
    pub max_distance: u32,
    
    /// Default number of strobes
    #[serde(default = "default_strobemer_n_strobes")]
    pub n_strobes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyConfig {
    /// Default verification engine
    #[serde(default = "default_verify_engine")]
    pub engine: String,
    
    /// Default compute device
    #[serde(default = "default_device")]
    pub device: String,
    
    /// Default tile verification policy
    #[serde(default = "default_tile_policy")]
    pub tile_policy: String,
    
    /// Default batch size for GPU processing
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    
    /// Default maximum alignment length
    #[serde(default = "default_max_align_len")]
    pub max_align_len: u32,
}

// Default value functions
fn default_threads() -> usize { num_cpus::get() }
fn default_theme() -> String { "default".to_string() }
fn default_max_memory() -> f64 { 8.0 }
fn default_compression() -> String { "zstd".to_string() }
fn default_compression_level() -> u8 { 6 }
fn default_lod_overview() -> String { "heatmap".to_string() }
fn default_lod_mid() -> String { "polyline".to_string() }
fn default_lod_deep() -> String { "points".to_string() }
fn default_true() -> bool { true }
fn default_dpi() -> u32 { 300 }
fn default_width() -> u32 { 1920 }
fn default_height() -> u32 { 1080 }
fn default_x_axis() -> String { "target".to_string() }
fn default_y_axis() -> String { "query".to_string() }
fn default_color_plus() -> String { "#2a6fef".to_string() }
fn default_color_minus() -> String { "#e53935".to_string() }
fn default_point_size() -> f32 { 1.0 }
fn default_engine() -> String { "minimap2".to_string() }
fn default_preset() -> String { "asm5".to_string() }
fn default_seed_density() -> String { "auto".to_string() }
fn default_k() -> u32 { 15 }
fn default_max_freq() -> u32 { 1000 }
fn default_min_anchor_len() -> u32 { 50 }
fn default_syncmer_s() -> u32 { 5 }
fn default_syncmer_t() -> u32 { 10 }
fn default_strobemer_window() -> u32 { 100 }
fn default_strobemer_max_distance() -> u32 { 200 }
fn default_strobemer_n_strobes() -> u32 { 2 }
fn default_verify_engine() -> String { "wfa".to_string() }
fn default_device() -> String { "cpu".to_string() }
fn default_tile_policy() -> String { "edges".to_string() }
fn default_batch_size() -> u32 { 1000 }
fn default_max_align_len() -> u32 { 10000 }

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                deterministic: false,
                threads: default_threads(),
                theme: default_theme(),
            },
            io: IoConfig {
                max_memory_gb: default_max_memory(),
                block_compression: default_compression(),
                compression_level: default_compression_level(),
            },
            render: RenderConfig {
                lod_overview: default_lod_overview(),
                lod_mid: default_lod_mid(),
                lod_deep: default_lod_deep(),
                show_strand_plus: true,
                show_strand_minus: true,
                dpi: default_dpi(),
                width: default_width(),
                height: default_height(),
            },
            plot: PlotConfig {
                x_axis: default_x_axis(),
                y_axis: default_y_axis(),
                color_plus: default_color_plus(),
                color_minus: default_color_minus(),
                point_size: default_point_size(),
            },
            map: MapConfig {
                engine: default_engine(),
                preset: default_preset(),
                seed_density: default_seed_density(),
                k: default_k(),
                max_freq: default_max_freq(),
                min_anchor_len: default_min_anchor_len(),
                mask_low_complexity: true,
                syncmer: SyncmerConfig {
                    s: default_syncmer_s(),
                    t: default_syncmer_t(),
                },
                strobemer: StrobemerConfig {
                    window_size: default_strobemer_window(),
                    max_distance: default_strobemer_max_distance(),
                    n_strobes: default_strobemer_n_strobes(),
                },
            },
            verify: VerifyConfig {
                engine: default_verify_engine(),
                device: default_device(),
                tile_policy: default_tile_policy(),
                batch_size: default_batch_size(),
                max_align_len: default_max_align_len(),
            },
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let config = match config_path {
            Some(path) => {
                log::info!("Loading configuration from: {}", path.display());
                Self::load_from_file(path)?
            }
            None => {
                // Try to find dotx.toml in current directory
                let default_path = PathBuf::from("dotx.toml");
                if default_path.exists() {
                    log::info!("Loading configuration from: dotx.toml");
                    Self::load_from_file(&default_path)?
                } else {
                    log::info!("Using default configuration");
                    Self::default()
                }
            }
        };
        
        Ok(config)
    }
    
    /// Load configuration from a specific TOML file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;
        
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse configuration file: {}", path.display()))?;
        
        Ok(config)
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;
        
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write configuration file: {}", path.display()))?;
        
        Ok(())
    }
    
    /// Generate example configuration file content
    pub fn example_toml() -> String {
        let config = Self::default();
        toml::to_string_pretty(&config)
            .expect("Failed to serialize default configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.theme, "default");
        assert_eq!(config.map.engine, "minimap2");
        assert_eq!(config.render.dpi, 300);
    }
    
    #[test]
    fn test_config_roundtrip() -> Result<()> {
        let config = Config::default();
        let temp_file = NamedTempFile::new()?;
        
        config.save_to_file(temp_file.path())?;
        let loaded_config = Config::load_from_file(temp_file.path())?;
        
        // Test a few key values
        assert_eq!(config.general.theme, loaded_config.general.theme);
        assert_eq!(config.map.engine, loaded_config.map.engine);
        assert_eq!(config.render.dpi, loaded_config.render.dpi);
        
        Ok(())
    }
    
    #[test]
    fn test_example_toml_generation() {
        let example = Config::example_toml();
        assert!(example.contains("[general]"));
        assert!(example.contains("[map]"));
        assert!(example.contains("[render]"));
    }
}