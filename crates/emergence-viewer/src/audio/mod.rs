/// V6 — Sound Engine
///
/// Architecture: SoundEngine runs on its own thread via `std::thread::spawn`.
/// The main tick loop sends commands through a `std::sync::mpsc` channel.
/// Zero tick-loop impact — the audio thread sleeps between commands.
///
/// NOTE: We do NOT ship actual .ogg/.wav files yet. This module builds the
/// complete audio framework with placeholder silent buffers. Real audio assets
/// are a separate task. All "playback" calls are no-ops until assets land.
///
/// Ambient layers (4):
///   0 — nature   (birds, wind, insects — always present)
///   1 — night    (crickets, owl — time-of-day: night)
///   2 — settlement (campfire crackle, murmur — near settlement)
///   3 — weather  (rain, thunder, wind — during weather events)
///
/// Crossfade: 2-second linear blend when switching ambient layer.
/// God power sounds: enum-mapped, placeholder silent buffers.
/// UI click: placeholder.
/// Volume: [master, music, sfx, ambient] as [f32; 4], 0.0-1.0.
/// Mute toggle: M key.

use std::sync::mpsc::{self, Sender};
use std::thread;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Camera and world context for ambient selection.
#[derive(Clone, Copy, Debug)]
pub struct AudioContext {
    /// World-space camera position
    pub camera_pos:   [f32; 2],
    /// Normalized time of day: 0.0 = midnight, 0.5 = noon, 1.0 = midnight again
    pub time_of_day:  f32,
    /// Current season: 0=spring, 1=summer, 2=autumn, 3=winter
    pub season:       u8,
    /// Whether camera is over a settlement (determines settlement layer weight)
    pub near_settlement: bool,
    /// Whether rain/snow is active
    pub weather_active: bool,
    /// Whether any kingdom is at war near camera
    pub war_nearby:   bool,
}

impl Default for AudioContext {
    fn default() -> Self {
        AudioContext {
            camera_pos:      [0.0, 0.0],
            time_of_day:     0.5,
            season:          0,
            near_settlement: false,
            weather_active:  false,
            war_nearby:      false,
        }
    }
}

/// A god power sound event.
#[derive(Clone, Copy, Debug)]
pub enum GodPowerSound {
    Lightning,
    Meteor,
    Earthquake,
    Tornado,
    Volcano,
    RainStart,
    RainStop,
    BlessingJoy,
    CurseAnger,
    PlaceBeing,
    RemoveBeing,
}

/// A UI interaction sound event.
#[derive(Clone, Copy, Debug)]
pub enum UiSound {
    ButtonClick,
    PanelOpen,
    PanelClose,
    SpeedChange,
    InspectorOpen,
    Notification,
    KingdomAlert,
}

/// Commands sent from main thread to the audio thread.
#[derive(Debug)]
enum AudioCommand {
    /// Update ambient layer weights based on new context.
    UpdateContext(AudioContext),
    /// Trigger a one-shot god power sound.
    PlayGodPower(GodPowerSound),
    /// Trigger a one-shot UI sound.
    PlayUi(UiSound),
    /// Set volume levels: [master, music, sfx, ambient].
    SetVolumes([f32; 4]),
    /// Toggle global mute.
    SetMuted(bool),
    /// Shutdown the audio thread.
    Shutdown,
}

// ---------------------------------------------------------------------------
// Volume settings (accessible from UI)
// ---------------------------------------------------------------------------

/// Volume levels for all channels.
/// [0]=master, [1]=music, [2]=sfx, [3]=ambient
#[derive(Clone, Copy, Debug)]
pub struct VolumeSettings {
    pub levels: [f32; 4],
    pub muted:  bool,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        VolumeSettings {
            levels: [0.8, 0.7, 0.8, 0.7],
            muted:  false,
        }
    }
}

impl VolumeSettings {
    pub fn master(&self)  -> f32 { self.levels[0] }
    pub fn music(&self)   -> f32 { self.levels[1] }
    pub fn sfx(&self)     -> f32 { self.levels[2] }
    pub fn ambient(&self) -> f32 { self.levels[3] }
    pub fn effective_master(&self) -> f32 {
        if self.muted { 0.0 } else { self.levels[0] }
    }
}

// ---------------------------------------------------------------------------
// Ambient layer definition
// ---------------------------------------------------------------------------

/// One of the 4 ambient layers. Contains a description and a weight [0,1].
#[derive(Clone, Debug)]
struct AmbientLayer {
    /// Human-readable label for debugging
    label:       &'static str,
    /// Current weight (target)
    weight:      f32,
    /// Weight from last frame (for crossfade)
    prev_weight: f32,
    /// Whether this layer has actual audio loaded (false = silent placeholder)
    loaded:      bool,
}

impl AmbientLayer {
    fn new(label: &'static str) -> Self {
        AmbientLayer { label, weight: 0.0, prev_weight: 0.0, loaded: false }
    }
}

// ---------------------------------------------------------------------------
// Internal audio thread state
// ---------------------------------------------------------------------------

struct AudioThreadState {
    volumes:    VolumeSettings,
    /// 4 ambient layers: [nature, night, settlement, weather]
    layers:     [AmbientLayer; 4],
    /// Crossfade timer: ticks down from CROSSFADE_TICKS
    fade_timer: u32,
}

const CROSSFADE_TICKS: u32 = 120; // ~2 seconds at 60fps

impl AudioThreadState {
    fn new() -> Self {
        AudioThreadState {
            volumes: VolumeSettings::default(),
            layers: [
                AmbientLayer::new("nature"),
                AmbientLayer::new("night"),
                AmbientLayer::new("settlement"),
                AmbientLayer::new("weather"),
            ],
            fade_timer: 0,
        }
    }

    fn apply_context(&mut self, ctx: AudioContext) {
        // Determine target weights based on context
        let is_night = ctx.time_of_day < 0.15 || ctx.time_of_day > 0.85;

        let nature_w     = if ctx.weather_active { 0.3 } else { 1.0 };
        let night_w      = if is_night { 0.8 } else { 0.0 };
        let settlement_w = if ctx.near_settlement { 0.6 } else { 0.0 };
        let weather_w    = if ctx.weather_active { 0.9 } else { 0.0 };

        for (i, w) in [nature_w, night_w, settlement_w, weather_w].iter().enumerate() {
            if (self.layers[i].weight - w).abs() > 0.01 {
                self.layers[i].prev_weight = self.layers[i].weight;
                self.layers[i].weight = *w;
                self.fade_timer = CROSSFADE_TICKS;
            }
        }
    }

    fn apply_volumes(&mut self, v: [f32; 4]) {
        self.volumes.levels = v;
    }

    fn set_muted(&mut self, muted: bool) {
        self.volumes.muted = muted;
    }

    /// Advance crossfade timer. Returns current blended weights.
    fn tick_fade(&mut self) -> [f32; 4] {
        let t = if self.fade_timer > 0 {
            self.fade_timer = self.fade_timer.saturating_sub(1);
            1.0 - (self.fade_timer as f32 / CROSSFADE_TICKS as f32)
        } else {
            1.0
        };

        std::array::from_fn(|i| {
            let prev = self.layers[i].prev_weight;
            let cur  = self.layers[i].weight;
            prev + (cur - prev) * t
        })
    }

    /// Play a god power sound. Placeholder: logs only, no actual audio.
    fn play_god_power(&self, sound: GodPowerSound) {
        // Real implementation: select audio asset, pitch-shift, apply sfx volume, submit to rodio sink.
        // For now: no-op (silent placeholder framework).
        let _ = sound; // suppress unused warning
    }

    /// Play a UI sound. Placeholder.
    fn play_ui(&self, sound: UiSound) {
        let _ = sound;
    }
}

// ---------------------------------------------------------------------------
// SoundEngine — public handle (lives on main thread)
// ---------------------------------------------------------------------------

/// Main-thread handle to the audio subsystem.
/// Cheap to clone or pass around — just wraps an mpsc Sender.
pub struct SoundEngine {
    sender: Sender<AudioCommand>,
    /// Volume settings mirrored on the main thread for UI display
    pub volumes: VolumeSettings,
}

impl SoundEngine {
    /// Spawn the audio thread and return a handle.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCommand>();

        thread::Builder::new()
            .name("emergence-audio".to_string())
            .spawn(move || {
                let mut state = AudioThreadState::new();

                loop {
                    // Block waiting for commands, process all queued
                    match rx.recv() {
                        Ok(cmd) => {
                            if !Self::handle_command(&mut state, cmd) {
                                break; // Shutdown received
                            }
                        }
                        Err(_) => break, // Sender dropped
                    }

                    // Drain any queued commands without blocking
                    while let Ok(cmd) = rx.try_recv() {
                        if !Self::handle_command(&mut state, cmd) {
                            return;
                        }
                    }

                    // Advance crossfade (result used when real audio is wired up)
                    let _blended = state.tick_fade();
                }
            })
            .expect("Failed to spawn audio thread");

        SoundEngine {
            sender: tx,
            volumes: VolumeSettings::default(),
        }
    }

    fn handle_command(state: &mut AudioThreadState, cmd: AudioCommand) -> bool {
        match cmd {
            AudioCommand::UpdateContext(ctx)   => state.apply_context(ctx),
            AudioCommand::PlayGodPower(sound)  => state.play_god_power(sound),
            AudioCommand::PlayUi(sound)        => state.play_ui(sound),
            AudioCommand::SetVolumes(v)        => state.apply_volumes(v),
            AudioCommand::SetMuted(m)          => state.set_muted(m),
            AudioCommand::Shutdown             => return false,
        }
        true
    }

    // ── Public API ──────────────────────────────────────────────────────────

    /// Update ambient selection based on current world/camera context.
    /// Called once per rendered frame. Non-blocking.
    pub fn update_context(&self, ctx: AudioContext) {
        let _ = self.sender.send(AudioCommand::UpdateContext(ctx));
    }

    /// Trigger a god power sound effect.
    pub fn play_god_power(&self, sound: GodPowerSound) {
        let _ = self.sender.send(AudioCommand::PlayGodPower(sound));
    }

    /// Trigger a UI click/interaction sound.
    pub fn play_ui(&self, sound: UiSound) {
        let _ = self.sender.send(AudioCommand::PlayUi(sound));
    }

    /// Set volume levels. Updates both main-thread mirror and audio thread.
    pub fn set_volumes(&mut self, volumes: [f32; 4]) {
        self.volumes.levels = volumes;
        let _ = self.sender.send(AudioCommand::SetVolumes(volumes));
    }

    /// Toggle mute (M key handler). Returns new muted state.
    pub fn toggle_mute(&mut self) -> bool {
        self.volumes.muted = !self.volumes.muted;
        let _ = self.sender.send(AudioCommand::SetMuted(self.volumes.muted));
        self.volumes.muted
    }

    /// Set mute explicitly.
    pub fn set_muted(&mut self, muted: bool) {
        self.volumes.muted = muted;
        let _ = self.sender.send(AudioCommand::SetMuted(muted));
    }

    pub fn is_muted(&self) -> bool {
        self.volumes.muted
    }

    /// Convenience: set master volume only.
    pub fn set_master(&mut self, v: f32) {
        self.volumes.levels[0] = v.clamp(0.0, 1.0);
        let _ = self.sender.send(AudioCommand::SetVolumes(self.volumes.levels));
    }

    /// Convenience: set music volume.
    pub fn set_music_volume(&mut self, v: f32) {
        self.volumes.levels[1] = v.clamp(0.0, 1.0);
        let _ = self.sender.send(AudioCommand::SetVolumes(self.volumes.levels));
    }

    /// Convenience: set sfx volume.
    pub fn set_sfx_volume(&mut self, v: f32) {
        self.volumes.levels[2] = v.clamp(0.0, 1.0);
        let _ = self.sender.send(AudioCommand::SetVolumes(self.volumes.levels));
    }

    /// Convenience: set ambient volume.
    pub fn set_ambient_volume(&mut self, v: f32) {
        self.volumes.levels[3] = v.clamp(0.0, 1.0);
        let _ = self.sender.send(AudioCommand::SetVolumes(self.volumes.levels));
    }
}

impl Drop for SoundEngine {
    fn drop(&mut self) {
        let _ = self.sender.send(AudioCommand::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// egui Volume Sliders UI panel
// ---------------------------------------------------------------------------

impl SoundEngine {
    /// Render volume slider controls in an egui window.
    /// Call from the main game UI loop.
    pub fn show_volume_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Audio")
            .collapsible(true)
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let mute_label = if self.volumes.muted { "Unmute (M)" } else { "Mute (M)" };
                    if ui.button(mute_label).clicked() {
                        self.toggle_mute();
                    }
                });

                ui.add_space(4.0);

                let mut changed = false;

                let mut master = self.volumes.levels[0];
                ui.horizontal(|ui| {
                    ui.label("Master");
                    if ui.add(egui::Slider::new(&mut master, 0.0..=1.0)).changed() {
                        changed = true;
                    }
                });

                let mut music = self.volumes.levels[1];
                ui.horizontal(|ui| {
                    ui.label("Music ");
                    if ui.add(egui::Slider::new(&mut music, 0.0..=1.0)).changed() {
                        changed = true;
                    }
                });

                let mut sfx = self.volumes.levels[2];
                ui.horizontal(|ui| {
                    ui.label("SFX   ");
                    if ui.add(egui::Slider::new(&mut sfx, 0.0..=1.0)).changed() {
                        changed = true;
                    }
                });

                let mut ambient = self.volumes.levels[3];
                ui.horizontal(|ui| {
                    ui.label("Ambient");
                    if ui.add(egui::Slider::new(&mut ambient, 0.0..=1.0)).changed() {
                        changed = true;
                    }
                });

                if changed {
                    self.set_volumes([master, music, sfx, ambient]);
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Key handler helper (integrate into main.rs handle_key)
// ---------------------------------------------------------------------------

/// Returns true if the key was consumed by the audio system.
/// Call from the application key handler.
pub fn handle_key(engine: &mut SoundEngine, key: winit::keyboard::KeyCode) -> bool {
    use winit::keyboard::KeyCode;
    match key {
        KeyCode::KeyM => {
            engine.toggle_mute();
            true
        }
        _ => false,
    }
}
