use crate::world::matter::MatterProperties;
use bitcode::{Decode, Encode};

/// A physical item dropped on the world grid.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct WorldItem {
    pub properties: MatterProperties,
    pub quantity_mass: f32,
}

/// The physical item layer. Each cell holds dropped items.
/// When Heat tensor > threshold on a cell with 2+ items, auto-forge triggers.
#[derive(Clone, Encode, Decode)]
pub struct ObjectGrid {
    pub cells: Vec<Vec<WorldItem>>,
    pub width: u32,
    pub height: u32,
}

impl ObjectGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            cells: vec![Vec::new(); size],
            width,
            height,
        }
    }

    /// Max items per cell — prevents OOM from unbounded accumulation.
    const MAX_ITEMS_PER_CELL: usize = 8;

    /// Drop an item at a world position. Capped at MAX_ITEMS_PER_CELL.
    pub fn drop_item(&mut self, x: u32, y: u32, item: WorldItem) {
        let idx = (y * self.width + x) as usize;
        if idx < self.cells.len() && self.cells[idx].len() < Self::MAX_ITEMS_PER_CELL {
            self.cells[idx].push(item);
        }
    }

    /// Pick up items from a cell (returns all items, empties the cell).
    pub fn pickup_all(&mut self, x: u32, y: u32) -> Vec<WorldItem> {
        let idx = (y * self.width + x) as usize;
        if idx < self.cells.len() {
            std::mem::take(&mut self.cells[idx])
        } else {
            Vec::new()
        }
    }

    /// Auto-forge tick: scan cells, merge items where Heat > threshold.
    /// Called from tick.rs every N ticks.
    pub fn tick_forge(&mut self, tensor: &crate::world::tensor::TensorGrid) {
        let w = self.width;
        for y in 0..self.height {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if self.cells[idx].len() < 2 { continue; }

                let heat = tensor.read(crate::world::tensor::TensorLayer::Heat, x, y);
                let a = self.cells[idx][0];
                let b = self.cells[idx][1];
                let req_energy = (a.properties.combustibility + b.properties.malleability) / 2.0;
                if heat <= req_energy { continue; }

                // Merge first two items via forge()
                if let Some(forged) = MatterProperties::forge(&a.properties, &b.properties, heat) {
                    let combined_mass = a.quantity_mass + b.quantity_mass;
                    self.cells[idx].remove(0);
                    self.cells[idx].remove(0);
                    self.cells[idx].push(WorldItem {
                        properties: forged,
                        quantity_mass: combined_mass,
                    });
                }
            }
        }
    }
}
