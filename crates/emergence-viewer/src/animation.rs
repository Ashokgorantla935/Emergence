//! Animation state machine for beings.
//! Drives atlas UV selection from engine state without touching core.

use emergence_core::being::actions::Action;
use emergence_core::being::data::{BeingState, Beings, CreatureType};

/// Cell size in the entity spritesheet (1 / 4)
const ENTITY_CELL_U: f32 = 1.0 / 4.0;
/// Cell size in the entity spritesheet vertical (1 / 96)
const ENTITY_CELL_V: f32 = 1.0 / 96.0;
/// Cell size in the fauna spritesheet (12 cols × 12 rows).
const FAUNA_CELL_U: f32 = 1.0 / 12.0;
const FAUNA_CELL_V: f32 = 1.0 / 12.0;

/// 10 animation states (matches atlas row layout).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AnimState {
    Idle    = 0,
    Walk    = 1,
    Run     = 2,
    Eat     = 3,
    Sleep   = 4,
    Fight   = 5,
    Share   = 6,
    Mourn   = 7,
    Explore = 8,
    Die     = 9,
}

impl AnimState {
    /// Frames per state.
    pub fn frame_count(self) -> u8 {
        match self {
            AnimState::Idle    => 2,
            AnimState::Walk    => 4,
            AnimState::Run     => 4,
            AnimState::Eat     => 3,
            AnimState::Sleep   => 2,
            AnimState::Fight   => 4,
            AnimState::Share   => 3,
            AnimState::Mourn   => 2,
            AnimState::Explore => 4,
            AnimState::Die     => 4,
        }
    }

    /// Seconds per frame.
    pub fn frame_duration(self) -> f32 {
        match self {
            AnimState::Idle  => 0.5,
            AnimState::Walk  => 0.15,
            AnimState::Run   => 0.10,
            AnimState::Fight => 0.12,
            AnimState::Die   => 0.25,
            _                => 0.20,
        }
    }
}

/// 8 facing directions.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    N  = 0,
    NE = 1,
    E  = 2,
    SE = 3,
    S  = 4,
    SW = 5,
    W  = 6,
    NW = 7,
}

impl Facing {
    pub fn from_velocity(dx: f32, dy: f32) -> Self {
        if dx.abs() < 0.001 && dy.abs() < 0.001 {
            return Facing::S;
        }
        let angle = dy.atan2(dx).to_degrees();
        // Map -180..180 to 8 octants
        let sector = ((angle + 202.5) / 45.0).floor() as i32 % 8;
        match sector.rem_euclid(8) {
            0 => Facing::W,
            1 => Facing::NW,
            2 => Facing::N,
            3 => Facing::NE,
            4 => Facing::E,
            5 => Facing::SE,
            6 => Facing::S,
            7 => Facing::SW,
            _ => Facing::S,
        }
    }
}

pub struct AnimationManager {
    pub frame_timers:     Vec<f32>,
    pub current_frames:   Vec<u8>,
    pub current_states:   Vec<AnimState>,
    pub current_facings:  Vec<Facing>,
    /// Previous positions for velocity calculation.
    prev_positions: Vec<[f32; 2]>,
}

impl AnimationManager {
    pub fn new(capacity: usize) -> Self {
        AnimationManager {
            frame_timers:    vec![0.0; capacity],
            current_frames:  vec![0;   capacity],
            current_states:  vec![AnimState::Idle; capacity],
            current_facings: vec![Facing::S; capacity],
            prev_positions:  vec![[0.0; 2]; capacity],
        }
    }

    /// Advance animations by `dt` seconds.
    pub fn update(&mut self, dt: f32, beings: &Beings) {
        let count = beings.hot.count.min(self.frame_timers.len());

        for i in 0..count {
            if beings.hot.states[i] == BeingState::Dead {
                self.current_states[i] = AnimState::Die;
            }

            // Derive facing from positional delta
            let prev = self.prev_positions[i];
            let curr = beings.hot.positions[i];
            let dx = curr[0] - prev[0];
            let dy = curr[1] - prev[1];
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                self.current_facings[i] = Facing::from_velocity(dx, dy);
            }
            self.prev_positions[i] = curr;

            // Skip dead beings — keep Die state
            // Derive anim state from being state
            let desired_state = self.anim_for_being(beings, i);
            if desired_state != self.current_states[i] {
                // Reset frame when state changes
                self.current_states[i] = desired_state;
                self.current_frames[i] = 0;
                self.frame_timers[i]   = 0.0;
            }

            // Advance frame timer
            let state = self.current_states[i];
            self.frame_timers[i] += dt;
            if self.frame_timers[i] >= state.frame_duration() {
                self.frame_timers[i] = 0.0;
                let frames = state.frame_count();
                self.current_frames[i] = (self.current_frames[i] + 1) % frames;
            }
        }
    }

    fn anim_for_being(&self, beings: &Beings, i: usize) -> AnimState {
        match beings.hot.states[i] {
            BeingState::Dead     => AnimState::Die,
            BeingState::Sleeping => AnimState::Sleep,
            BeingState::Awake    => {
                // Map pending_action (u8) to anim state; 255 = no pending action.
                let action_u8 = beings.hot.pending_action[i];
                if action_u8 != 255 {
                    // Safe: Action is repr(u8) with values 0-21; anything outside falls through.
                    let action = match action_u8 {
                        0  => Some(Action::Wander),
                        1  => Some(Action::SeekFood),
                        2  => Some(Action::SeekShelter),
                        3  => Some(Action::Flee),
                        4  => Some(Action::ApproachBeing),
                        5  => Some(Action::Bond),
                        6  => Some(Action::ShareFood),
                        7  => Some(Action::TakeFood),
                        8  => Some(Action::Explore),
                        9  => Some(Action::Sleep),
                        10 => Some(Action::Cluster),
                        11 => Some(Action::Mourn),
                        12 => Some(Action::AvoidBeing),
                        13 => Some(Action::PickUpFood),
                        14 => Some(Action::Hunt),
                        15 => Some(Action::Teach),
                        16 => Some(Action::Build),
                        17 => Some(Action::Craft),
                        18 => Some(Action::Memorialize),
                        19 => Some(Action::CreateMark),
                        20 => Some(Action::ShareResource),
                        21 => Some(Action::PickUpStone),
                        _  => None,
                    };
                    if let Some(action) = action {
                        let mapped = match action {
                            Action::Hunt | Action::TakeFood               => Some(AnimState::Fight),
                            Action::ShareFood | Action::ShareResource      => Some(AnimState::Share),
                            Action::Teach | Action::Bond                   => Some(AnimState::Share),
                            Action::Mourn | Action::Memorialize             => Some(AnimState::Mourn),
                            Action::Explore                                 => Some(AnimState::Explore),
                            Action::Sleep                                   => Some(AnimState::Sleep),
                            Action::SeekFood | Action::PickUpFood
                            | Action::PickUpStone | Action::Build
                            | Action::Craft | Action::CreateMark            => {
                                // Active tasks: use velocity to pick Walk or Idle
                                None
                            }
                            _ => None,
                        };
                        if let Some(state) = mapped {
                            return state;
                        }
                    }
                }

                // Fallback: derive from velocity magnitude
                let prev = self.prev_positions[i];
                let curr = beings.hot.positions[i];
                let dx = curr[0] - prev[0];
                let dy = curr[1] - prev[1];
                let speed = (dx * dx + dy * dy).sqrt();
                if speed > 0.15 {
                    AnimState::Walk
                } else {
                    AnimState::Idle
                }
            }
        }
    }

    /// Compute atlas UV for being `i`.
    /// Fauna use the shared terrain atlas: rows 12-13, column base per species (4 frames each).
    /// Humans use the entity spritesheet (4x4 grid of 48x48 on 256x6144):
    ///   Row 0: walk down (4 frames)
    ///   Row 1: walk up   (4 frames)
    ///   Row 2: walk right (4 frames)
    ///   Row 3: walk left  (4 frames)
    pub fn atlas_uv(&self, beings: &Beings, i: usize) -> [f32; 2] {
        let ct = CreatureType::from_u8(beings.hot.creature_type[i]);

        if ct != CreatureType::Human {
            return fauna_atlas_uv(ct, self.current_frames[i]);
        }

        let state  = self.current_states[i];
        let frame  = self.current_frames[i] as u32;
        let facing = self.current_facings[i];

        // Map facing to spritesheet row:
        //   Row 0: down (S, SE, SW)
        //   Row 1: up   (N, NE, NW)
        //   Row 2: right (E)
        //   Row 3: left  (W)
        let sheet_row: u32 = match facing {
            Facing::N | Facing::NE | Facing::NW => 1,
            Facing::E                            => 2,
            Facing::W                            => 3,
            _                                    => 0, // S, SE, SW → down
        };

        // For non-walk states that don't have directional animation, use row 0 (down).
        // Walk/Run/Explore cycle all 4 columns; other states hold column 0.
        let (sheet_row, col) = match state {
            AnimState::Walk | AnimState::Run | AnimState::Explore => {
                (sheet_row, frame % 4)
            }
            AnimState::Idle => {
                // Alternate between col 0 and col 1 for idle sway
                (sheet_row, frame % 2)
            }
            // All other states (Sleep, Fight, Share, Mourn, Die, Eat) hold first frame
            _ => (0u32, 0u32),
        };

        // There are 12 NPCs, hashed by their being index `i` or personality to remain stable.
        // Let's mix their bits so it isn't just grouped in chunks.
        let mut x = i as u32;
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846ca68b);
        x ^= x >> 16;
        
        let npc_idx = x % 12;
        let base_row = npc_idx * 8;
        let final_row = base_row + sheet_row;

        [col as f32 * ENTITY_CELL_U, final_row as f32 * ENTITY_CELL_V]
    }
}

fn life_phase_index(beings: &Beings, i: usize) -> u32 {
    match beings.life_phase(i) {
        emergence_core::being::data::LifePhase::Youth => 1,
        emergence_core::being::data::LifePhase::Adult => 0,
        emergence_core::being::data::LifePhase::Elder => 2,
    }
}

fn body_build_index(beings: &Beings, i: usize) -> u32 {
    // Hash personality floats into build 0-3
    let p = &beings.hot.personalities[i];
    if p[0] > 0.3 {
        0 // bold -> stout
    } else if p[2] > 0.3 {
        1 // curious -> lean
    } else if p[1] > 0.3 {
        2 // social -> round
    } else {
        3 // default -> wiry
    }
}

/// Atlas UV for a fauna sprite.
/// fauna_and_races_spritesheet_190.png (8×8 grid) / fauna_spritesheet_190.png. 1 species per row, 3 frames per direction.
///   Row 0: Rabbit
///   Row 1: Deer
///   Row 2: Wolf
///   Row 3: Bear
///   Row 4: Hawk
///   Row 5: Fish
///   Row 6: Snake
///   Rows 7-11: (unused)
fn fauna_atlas_uv(ct: CreatureType, frame: u8) -> [f32; 2] {
    let row: u32 = match ct {
        CreatureType::Rabbit => 0,
        CreatureType::Deer   => 1,
        CreatureType::Wolf   => 2,
        CreatureType::Bear   => 4,
        CreatureType::Hawk   => 5,
        CreatureType::Fish   => 6,
        CreatureType::Snake  => 7,
        CreatureType::Human  => 0,  // fallback
    };
    let col = (frame as u32) % 3; // 3 frames per direction
    [col as f32 * FAUNA_CELL_U, row as f32 * FAUNA_CELL_V]
}
