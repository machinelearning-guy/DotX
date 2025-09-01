pub mod compute;
pub mod pipeline;
pub mod preview;

use dotx_core::*;
use wgpu;

pub use compute::*;
pub use pipeline::*;
pub use preview::*;

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuContext {
    pub async fn new() -> anyhow::Result<Option<Self>> {
        let instance = wgpu::Instance::default();
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await;

        let adapter = match adapter {
            Some(adapter) => adapter,
            None => {
                log::warn!("No suitable GPU adapter found");
                return Ok(None);
            }
        };

        let adapter_info = adapter.get_info();
        log::info!("GPU adapter: {} ({:?})", adapter_info.name, adapter_info.device_type);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("DotX GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        Ok(Some(Self {
            device,
            queue,
            adapter_info,
        }))
    }
}

pub trait GpuEngine {
    fn build_tiles(&self, alignments: &[PafRecord], grid: &GridParams) -> anyhow::Result<Vec<Tile>>;
    fn compute_histogram(&self, tiles: &[Tile]) -> anyhow::Result<Vec<u32>>;
    fn preview_align(&self, ref_seq: &[u8], qry_seq: &[u8]) -> anyhow::Result<Vec<PafRecord>>;
}

pub trait CpuEngine {
    fn build_tiles(&self, alignments: &[PafRecord], grid: &GridParams) -> anyhow::Result<Vec<Tile>>;
    fn compute_histogram(&self, tiles: &[Tile]) -> anyhow::Result<Vec<u32>>;
}