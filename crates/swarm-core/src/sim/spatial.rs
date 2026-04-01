use crate::being::data::BeingState;

pub struct SpatialIndex {
    cell_size: f32,
    grid_width: u32,
    grid_height: u32,
    cells: Vec<Vec<usize>>,
}

impl SpatialIndex {
    pub fn new(world_width: u32, world_height: u32, cell_size: f32) -> Self {
        let gw = ((world_width as f32) / cell_size).ceil() as u32;
        let gh = ((world_height as f32) / cell_size).ceil() as u32;
        let cell_count = (gw * gh) as usize;
        SpatialIndex {
            cell_size,
            grid_width: gw,
            grid_height: gh,
            cells: vec![Vec::new(); cell_count],
        }
    }

    pub fn rebuild(&mut self, positions: &[[f32; 2]], states: &[BeingState]) {
        for cell in self.cells.iter_mut() {
            cell.clear();
        }
        for (i, pos) in positions.iter().enumerate() {
            if states[i] == BeingState::Dead {
                continue;
            }
            let cx = ((pos[0] / self.cell_size) as u32).min(self.grid_width - 1);
            let cy = ((pos[1] / self.cell_size) as u32).min(self.grid_height - 1);
            let cell_idx = (cy * self.grid_width + cx) as usize;
            self.cells[cell_idx].push(i);
        }
    }

    pub fn query_radius(&self, x: f32, y: f32, radius: f32) -> Vec<usize> {
        self.query_radius_with_positions(x, y, radius, &[])
    }

    pub fn query_radius_with_positions(&self, x: f32, y: f32, radius: f32, positions: &[[f32; 2]]) -> Vec<usize> {
        let mut result = Vec::new();
        let r_sq = radius * radius;

        let min_cx = ((x - radius).max(0.0) / self.cell_size) as u32;
        let max_cx = (((x + radius) / self.cell_size) as u32).min(self.grid_width - 1);
        let min_cy = ((y - radius).max(0.0) / self.cell_size) as u32;
        let max_cy = (((y + radius) / self.cell_size) as u32).min(self.grid_height - 1);

        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let cell_idx = (cy * self.grid_width + cx) as usize;
                for &being_idx in &self.cells[cell_idx] {
                    if !positions.is_empty() && being_idx < positions.len() {
                        let dx = positions[being_idx][0] - x;
                        let dy = positions[being_idx][1] - y;
                        if dx * dx + dy * dy > r_sq {
                            continue;
                        }
                    }
                    result.push(being_idx);
                }
            }
        }
        result
    }

    pub fn count_in_radius(&self, x: f32, y: f32, radius: f32) -> usize {
        self.query_radius(x, y, radius).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_index_query() {
        let mut spatial = SpatialIndex::new(256, 256, 4.0);

        let positions: Vec<[f32; 2]> = vec![
            [10.0, 10.0],
            [11.0, 10.0],
            [50.0, 50.0],
            [10.5, 10.5],
            [200.0, 200.0],
        ];
        let states = vec![
            BeingState::Awake,
            BeingState::Awake,
            BeingState::Awake,
            BeingState::Awake,
            BeingState::Dead, // should be excluded
        ];

        spatial.rebuild(&positions, &states);

        let nearby = spatial.query_radius(10.0, 10.0, 5.0);
        assert!(nearby.contains(&0), "should contain being 0");
        assert!(nearby.contains(&1), "should contain being 1");
        assert!(nearby.contains(&3), "should contain being 3");
        assert!(!nearby.contains(&2), "should not contain distant being 2");
        assert!(
            !nearby.contains(&4),
            "should not contain dead being 4"
        );
    }
}
