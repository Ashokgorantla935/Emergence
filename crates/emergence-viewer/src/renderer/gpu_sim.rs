//! V56: Zero-Copy VRAM Simulation Infrastructure.
//! GPU-native entity physics with CPU-side soul validation.

use dashmap::DashMap;
use once_cell::sync::Lazy;

// ── Maximum entity capacity ─────────────────────────────────────────────────
pub const MAX_ENTITIES: u32 = 1_048_576; // 1M entities
pub const MAX_EVENTS: u32 = 65_536;      // 64K events per tick
pub const MAX_GOD_COMMANDS: u32 = 256;    // God action queue

// ── V56 §1: Fractal Position ────────────────────────────────────────────────
// Sector = macro cell (1 cell = 1 km²). Local = normalized [0.0, 1.0) within sector.

// ── V56 §2: GpuEntity — lives permanently in VRAM ──────────────────────────
/// The fundamental unit of simulation on the GPU. 48 bytes, 16-byte aligned.
/// This struct is shared between the Compute pipeline (read_write) and Vertex pipeline (read).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEntity {
    pub sector_x:      u32,   //  0: macro cell X
    pub sector_y:      u32,   //  4: macro cell Y
    pub local_x:       f32,   //  8: normalized position [0.0, 1.0)
    pub local_y:       f32,   // 12: normalized position [0.0, 1.0)
    pub vel_x:         f32,   // 16: velocity X (sectors/tick)
    pub vel_y:         f32,   // 20: velocity Y
    pub mass_proxy:    f32,   // 24: biological mass (drives sqrt scale)
    pub health:        f32,   // 28: [0.0, 1.0] — death event at 0
    pub uuid_high:     u32,   // 32: upper 32 bits of soul UUID
    pub uuid_low:      u32,   // 36: lower 32 bits
    pub creature_type: u32,   // 40: 0=Human, 1=Wolf, 2=Deer, ...
    pub atlas_index:   u32,   // 44: packed sprite index for rendering
}
// 48 bytes total. 1M entities = 48MB VRAM.

// ── V56 §4: GPU→CPU Event ───────────────────────────────────────────────────
/// Terminal event written by GPU compute via atomics, read by CPU async.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEvent {
    pub event_type: u32,    // EVENT_DEATH=1, EVENT_BIRTH=2, EVENT_BUILD=3, ...
    pub uuid_high:  u32,
    pub uuid_low:   u32,
    pub param:      u32,    // payload (creature type, structure type, etc.)
}
// 16 bytes per event.

pub const EVENT_DEATH: u32 = 1;
pub const EVENT_BIRTH: u32 = 2;
pub const EVENT_BUILD: u32 = 3;
pub const EVENT_DESTROY: u32 = 4;

// ── V56 §6: God Command ─────────────────────────────────────────────────────
/// CPU→GPU command for player actions. Parsed at Phase 1 of compute dispatch.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GodCommand {
    pub command_type: u32,  // 0=spawn, 1=kill, 2=place_structure, 3=weather, ...
    pub target_x:     f32,
    pub target_y:     f32,
    pub param:        u32,  // payload
}
// 16 bytes per command.

// ── V56 §2.3: Simulation Parameters Uniform ─────────────────────────────────
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    pub tick:          u32,
    pub entity_count:  u32,
    pub world_width:   u32,
    pub world_height:  u32,
    pub dt:            f32,
    pub command_count: u32,
    pub _pad0:         f32,
    pub _pad1:         f32,
}
// 32 bytes, 16-byte aligned.

// ── V56 §5: Fixed-Point Thermodynamics (CPU-side) ───────────────────────────
pub const WORLD_ENERGY_CAP_V56: u64 = 1_000_000_000_000;
pub const FIXED_SCALAR: i64 = 1_000_000;

/// V56 §5 (Open Solar Model): Two-tier thermodynamics.
/// - Physical mass (carbon/minerals) is CLOSED — i64 integer-precise, conserved absolutely.
/// - Solar/thermal energy is OPEN — f32 fluid, streams continuously from sunlight.
/// Trees absorb infinite sunlight but require locked_soil_mass to generate physical scaling.
#[derive(Clone, Debug, Default)]
pub struct CellThermodynamics {
    /// CLOSED: Physical carbon/mineral mass locked in this cell. Integer-precise conservation.
    pub locked_soil_mass_i64: i64,
    /// OPEN: Radiant solar energy absorbed. Fluid f32, streams from sun every tick on GPU.
    pub radiant_solar_energy_f32: f32,
}

// ── V56 §3: Soul Database (CPU-side) ────────────────────────────────────────
/// Rich entity history maintained on CPU. GPU only knows UUID; CPU knows the soul.
#[derive(Clone, Debug)]
pub struct SoulMemory {
    pub display_name: String,
    pub creature_type: u8,
    pub genetics: [u8; 16],
    pub kills: u32,
    pub born_tick: u32,
    pub relationships: Vec<(u64, f32)>,  // (other_uuid, trust)
    pub memory_events: Vec<u32>,          // event IDs
}

/// Global lock-free soul database. Zero per-frame cost — only accessed on God-click or terminal events.
pub static SOULS: Lazy<DashMap<u64, SoulMemory>> = Lazy::new(DashMap::new);

/// Pack two u32 halves into a u64 UUID.
pub fn uuid_from_parts(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | (low as u64)
}

/// Split a u64 UUID into two u32 halves for GPU packing.
pub fn uuid_to_parts(uuid: u64) -> (u32, u32) {
    ((uuid >> 32) as u32, uuid as u32)
}

/// Look up a soul by UUID. Used when God clicks on an entity sprite.
pub fn lookup_soul(uuid: u64) -> Option<dashmap::mapref::one::Ref<'static, u64, SoulMemory>> {
    SOULS.get(&uuid)
}

// ── V56 §5: CPU-side ThermodynamicsGrid ─────────────────────────────────────

/// V56 §5: CPU-side integer-precise thermodynamic grid.
/// Used for conservation validation when terminal events arrive from GPU.
pub struct ThermodynamicsGrid {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<CellThermodynamics>,
    pub total_locked_mass: i64,
}

impl ThermodynamicsGrid {
    pub fn new(w: u32, h: u32) -> Self {
        let len = (w * h) as usize;
        Self {
            width: w,
            height: h,
            cells: vec![CellThermodynamics::default(); len],
            total_locked_mass: 0,
        }
    }

    /// Validate conservation: sum of all locked mass must equal initial total.
    pub fn validate_conservation(&self) -> bool {
        let sum: i64 = self.cells.iter().map(|c| c.locked_soil_mass_i64).sum();
        sum == self.total_locked_mass
    }
}

// ── V56 §1.2: LOD Thresholds ────────────────────────────────────────────────
pub const LOD_THRESHOLD_MACRO: f32 = 200.0;  // Above this altitude: heatmap only, no entities
pub const LOD_MACRO_Z_MIN: f32 = 150.0;       // Blend starts
pub const LOD_MACRO_Z_MAX: f32 = 250.0;       // Blend completes (full macro)

// ── V56 §7: Time Dilation ────────────────────────────────────────────────────
/// Execute N compute dispatches per render frame for time acceleration.
/// Do NOT multiply dt — dispatch multiple times for physics accuracy.
pub const MAX_TIME_MULTIPLIER: u32 = 100;
