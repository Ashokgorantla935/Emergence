use bitcode::{Decode, Encode};

pub const CHUNK_SIZE: u32 = 32;

/// Per-chunk state for the Active/Dormant memory management system.
#[derive(Clone, Encode, Decode)]
pub struct ChunkState {
    pub is_active: bool,
    /// Compressed serialized blob of dormant entities (flora, micro-fauna).
    /// When dormant, entities are stripped from the active tick loop entirely.
    pub frozen_blob: Option<Vec<u8>>,
    /// Tick when chunk went dormant — used for catch-up calculation.
    pub dormant_since: u32,
    /// Count of entities stored in frozen_blob (for stats/debugging).
    pub frozen_entity_count: u32,
    /// First being index (in SoA) whose position falls in this chunk.
    /// Only valid after update_being_bounds() is called.
    pub being_index_start: usize,
    /// One-past-last being index in this chunk (exclusive upper bound).
    pub being_index_end: usize,
}

impl ChunkState {
    pub fn new_active() -> Self {
        Self {
            is_active: true,
            frozen_blob: None,
            dormant_since: 0,
            frozen_entity_count: 0,
            being_index_start: 0,
            being_index_end: 0,
        }
    }
}

/// The chunk grid manager. Divides the world into 32×32 chunks.
#[derive(Clone, Encode, Decode)]
pub struct ChunkGrid {
    pub chunks: Vec<ChunkState>,
    pub chunks_wide: u32,
    pub chunks_high: u32,
    pub world_width: u32,
    pub world_height: u32,
}

impl ChunkGrid {
    pub fn new(world_width: u32, world_height: u32) -> Self {
        let chunks_wide = (world_width + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunks_high = (world_height + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let total = (chunks_wide * chunks_high) as usize;
        Self {
            chunks: vec![ChunkState::new_active(); total],
            chunks_wide,
            chunks_high,
            world_width,
            world_height,
        }
    }

    /// Get chunk index from world-space tile coordinates.
    #[inline]
    pub fn chunk_index(&self, tile_x: u32, tile_y: u32) -> usize {
        let cx = tile_x / CHUNK_SIZE;
        let cy = tile_y / CHUNK_SIZE;
        (cy * self.chunks_wide + cx) as usize
    }

    /// Check if a world-space position is in an active chunk.
    #[inline]
    pub fn is_active_at(&self, tile_x: u32, tile_y: u32) -> bool {
        let idx = self.chunk_index(tile_x, tile_y);
        self.chunks.get(idx).map_or(false, |c| c.is_active)
    }

    /// Dormancy check: deactivate chunks outside camera frustum with no urgency.
    /// `camera_min/max` are tile coordinates of the visible region.
    /// `urgency_positions` are tile positions of beings with active cognition (humans, active predators).
    pub fn update_dormancy(
        &mut self,
        camera_min: [u32; 2],
        camera_max: [u32; 2],
        urgency_positions: &[[u32; 2]],
        current_tick: u32,
    ) {
        let cam_cx_min = camera_min[0] / CHUNK_SIZE;
        let cam_cy_min = camera_min[1] / CHUNK_SIZE;
        let cam_cx_max = (camera_max[0] / CHUNK_SIZE).min(self.chunks_wide - 1);
        let cam_cy_max = (camera_max[1] / CHUNK_SIZE).min(self.chunks_high - 1);

        // Build set of chunks with urgency nodes
        let mut has_urgency = vec![false; self.chunks.len()];
        for pos in urgency_positions {
            let idx = self.chunk_index(pos[0], pos[1]);
            if idx < has_urgency.len() {
                has_urgency[idx] = true;
            }
        }

        for cy in 0..self.chunks_high {
            for cx in 0..self.chunks_wide {
                let idx = (cy * self.chunks_wide + cx) as usize;
                let in_frustum = cx >= cam_cx_min && cx <= cam_cx_max
                              && cy >= cam_cy_min && cy <= cam_cy_max;
                let has_urgency_node = has_urgency[idx];

                let should_be_active = in_frustum || has_urgency_node;

                if should_be_active && !self.chunks[idx].is_active {
                    // Reactivate — caller must decompress frozen_blob and apply catch-up
                    self.chunks[idx].is_active = true;
                } else if !should_be_active && self.chunks[idx].is_active {
                    // Deactivate — caller must serialize entities into frozen_blob
                    self.chunks[idx].is_active = false;
                    self.chunks[idx].dormant_since = current_tick;
                }
            }
        }
    }

    /// Calculate missed ticks for a chunk being reactivated.
    pub fn missed_ticks(&self, chunk_idx: usize, current_tick: u32) -> u32 {
        if self.chunks[chunk_idx].dormant_since == 0 { return 0; }
        current_tick.saturating_sub(self.chunks[chunk_idx].dormant_since)
    }

    /// Sort beings by chunk and populate index bounds.
    /// Call after spatial index rebuild in tick.rs.
    /// Tracks the min/max SoA indices of beings belonging to each chunk.
    /// Beings at indices being_index_start..being_index_end include those in this chunk,
    /// though the range may also include beings from adjacent chunks — callers should verify
    /// chunk membership when strict correctness is required.
    pub fn update_being_bounds(
        &mut self,
        positions: &[[f32; 2]],
        states: &[crate::being::data::BeingState],
        count: usize,
    ) {
        let total_chunks = self.chunks.len();
        let mut chunk_min: Vec<usize> = vec![usize::MAX; total_chunks];
        let mut chunk_max: Vec<usize> = vec![0; total_chunks];
        let mut chunk_has: Vec<bool> = vec![false; total_chunks];

        for i in 0..count {
            if states[i] == crate::being::data::BeingState::Dead { continue; }
            let pos = positions[i];
            let tx = (pos[0] as u32).min(self.world_width.saturating_sub(1));
            let ty = (pos[1] as u32).min(self.world_height.saturating_sub(1));
            let cidx = self.chunk_index(tx, ty);
            if cidx < total_chunks {
                if i < chunk_min[cidx] { chunk_min[cidx] = i; }
                if i + 1 > chunk_max[cidx] { chunk_max[cidx] = i + 1; }
                chunk_has[cidx] = true;
            }
        }

        for cidx in 0..total_chunks {
            if chunk_has[cidx] {
                self.chunks[cidx].being_index_start = chunk_min[cidx];
                self.chunks[cidx].being_index_end = chunk_max[cidx];
            } else {
                self.chunks[cidx].being_index_start = 0;
                self.chunks[cidx].being_index_end = 0;
            }
        }
    }
}
