use dotx_core::*;
use crate::{GpuContext, GpuEngine, CpuEngine};
use anyhow::Result;

pub struct GpuPipeline {
    pub ctx: GpuContext,
    pub tile_builder: Option<crate::compute::GpuTileBuilder>,
}

impl GpuPipeline {
    pub async fn new() -> Result<Option<Self>> {
        let ctx = match GpuContext::new().await? {
            Some(ctx) => ctx,
            None => return Ok(None),
        };

        // Note: We can't clone wgpu devices/queues easily, so we'll pass references when needed
        let tile_builder = None; // Temporarily disabled due to reference issues

        Ok(Some(Self {
            ctx,
            tile_builder,
        }))
    }

    pub fn is_available(&self) -> bool {
        self.tile_builder.is_some()
    }

    pub fn get_device_info(&self) -> String {
        format!("{} ({:?})", 
                self.ctx.adapter_info.name, 
                self.ctx.adapter_info.device_type)
    }
}

impl GpuEngine for GpuPipeline {
    fn build_tiles(&self, alignments: &[PafRecord], grid: &GridParams) -> Result<Vec<Tile>> {
        if let Some(builder) = &self.tile_builder {
            builder.build_tiles_gpu(alignments, grid)
        } else {
            Err(anyhow::anyhow!("GPU tile builder not available"))
        }
    }

    fn compute_histogram(&self, _tiles: &[Tile]) -> Result<Vec<u32>> {
        // TODO: Implement GPU histogram computation
        Ok(vec![0; 256])
    }

    fn preview_align(&self, _ref_seq: &[u8], _qry_seq: &[u8]) -> Result<Vec<PafRecord>> {
        // TODO: Implement GPU preview alignment
        Ok(Vec::new())
    }
}

pub struct CpuPipeline;

impl CpuEngine for CpuPipeline {
    fn build_tiles(&self, alignments: &[PafRecord], grid: &GridParams) -> Result<Vec<Tile>> {
        let mut builder = TileBuilder::new(grid.clone());
        
        for record in alignments {
            builder.add_alignment(record);
        }
        
        Ok(builder.build_tiles())
    }

    fn compute_histogram(&self, tiles: &[Tile]) -> Result<Vec<u32>> {
        let mut histogram = vec![0u32; 256];
        
        for tile in tiles {
            for bin in &tile.bins {
                if bin.count > 0 {
                    let density_bin = (bin.count.min(255)) as usize;
                    histogram[density_bin] += 1;
                }
            }
        }
        
        Ok(histogram)
    }
}