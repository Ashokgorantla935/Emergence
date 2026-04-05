/// Geographic knowledge grid — per-cell bitmask of discovered technologies.
/// Knowledge is spatial: beings can only use techs available at their current location.
/// Techs spread via discovery deposits with a small radius.

// Tech bit constants
pub const TECH_FISHING: u32     = 1 << 0; // Discovered near water + wood
pub const TECH_SMELTING: u32    = 1 << 1; // Discovered near mountain + fire (campfire)
pub const TECH_MASONRY: u32     = 1 << 2; // Discovered near stone + hut
pub const TECH_AGRICULTURE: u32 = 1 << 3; // Discovered near flora_stage >= 2 + grain
pub const TECH_WEAVING: u32     = 1 << 4; // Discovered near forest + settlement
pub const TECH_MEDICINE: u32    = 1 << 5; // Discovered near wetland + elder being
pub const TECH_ENGINEERING: u32 = 1 << 6; // Discovered near castle + forge + stone road

pub struct KnowledgeGrid {
    pub width: u32,
    pub height: u32,
    pub techs: Vec<u32>, // bitmask per cell
}

impl KnowledgeGrid {
    pub fn new(w: u32, h: u32) -> Self {
        let len = (w * h) as usize;
        KnowledgeGrid {
            width: w,
            height: h,
            techs: vec![0u32; len],
        }
    }

    /// Check if a tech is available at a world coordinate.
    pub fn has_tech(&self, x: u32, y: u32, tech: u32) -> bool {
        let idx = (y * self.width + x) as usize;
        if idx >= self.techs.len() {
            return false;
        }
        self.techs[idx] & tech != 0
    }

    /// Deposit a tech discovery at a location with radius spread.
    pub fn deposit_tech(&mut self, cx: u32, cy: u32, tech: u32, radius: u32) {
        let w = self.width;
        let h = self.height;
        let r = radius as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                } // circular radius
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h {
                    let idx = (ny as u32 * w + nx as u32) as usize;
                    self.techs[idx] |= tech;
                }
            }
        }
    }
}
