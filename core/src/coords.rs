//! Coordinate systems and genomic ranges

/// Represents a genomic coordinate range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenomicRange {
    /// Start position (0-based, inclusive)
    pub start: u64,
    /// End position (0-based, exclusive)
    pub end: u64,
}

impl GenomicRange {
    /// Create a new genomic range
    pub fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }
    
    /// Get the length of the range
    pub fn len(&self) -> u64 {
        self.end - self.start
    }
    
    /// Check if the range is empty
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
    
    /// Check if this range overlaps with another
    pub fn overlaps(&self, other: &GenomicRange) -> bool {
        self.start < other.end && other.start < self.end
    }
    
    /// Get the intersection of two ranges
    pub fn intersect(&self, other: &GenomicRange) -> Option<GenomicRange> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if start < end {
            Some(GenomicRange::new(start, end))
        } else {
            None
        }
    }
}

/// Convert from screen coordinates to genomic coordinates
pub fn screen_to_genomic(
    screen_x: f32, 
    screen_y: f32, 
    viewport: &GenomicRange, 
    screen_width: f32, 
    screen_height: f32
) -> (u64, u64) {
    let genomic_x = viewport.start + ((screen_x / screen_width) * viewport.len() as f32) as u64;
    let genomic_y = viewport.start + ((screen_y / screen_height) * viewport.len() as f32) as u64;
    (genomic_x, genomic_y)
}

/// Convert from genomic coordinates to screen coordinates
pub fn genomic_to_screen(
    genomic_x: u64, 
    genomic_y: u64, 
    viewport: &GenomicRange, 
    screen_width: f32, 
    screen_height: f32
) -> (f32, f32) {
    let screen_x = ((genomic_x - viewport.start) as f32 / viewport.len() as f32) * screen_width;
    let screen_y = ((genomic_y - viewport.start) as f32 / viewport.len() as f32) * screen_height;
    (screen_x, screen_y)
}