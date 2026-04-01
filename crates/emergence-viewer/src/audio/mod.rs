/// V7 — Sound Engine (rodio-backed, synthesized audio, no external files)
///
/// Architecture: SoundEngine runs on its own thread via `std::thread::spawn`.
/// The main tick loop sends commands through a `std::sync::mpsc` channel.
/// Zero tick-loop impact — the audio thread sleeps between commands.
///
/// Audio synthesis:
///   - All sounds are generated as raw f32 PCM buffers (44100 Hz, mono).
///   - Ambient layers: looping low-frequency drones with filtered noise.
///   - Event sounds: parameterized sine envelopes (birth, death, combat, god powers).
///
/// Ambient layers (4):
///   0 — nature   (birds, wind, insects — always present)
///   1 — night    (crickets, owl — time-of-day: night)
///   2 — settlement (campfire crackle, murmur — near settlement)
///   3 — weather  (rain, thunder, wind — during weather events)
///
/// Crossfade: 2-second linear blend when switching ambient layer.
/// Volume: [master, music, sfx, ambient] as [f32; 4], 0.0-1.0.
/// Mute toggle: M key.

use std::sync::mpsc::{self, Sender};
use std::thread;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Biome type at the camera position — drives ambient layer mixing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeAmbience {
    Grassland,
    Forest,
    Mountain,
    Desert,
    Water,
}

impl Default for BiomeAmbience {
    fn default() -> Self { BiomeAmbience::Grassland }
}

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
    /// Biome under the camera center
    pub biome: BiomeAmbience,
    /// Camera zoom level: 0.0 = fully zoomed out (far), 1.0 = fully zoomed in (close).
    /// Drives ambient volume: far = 0.05, close = 0.20.
    pub zoom_normalized: f32,
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
            biome:           BiomeAmbience::Grassland,
            zoom_normalized: 0.5,
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

/// World event sounds (birth, death, combat).
#[derive(Clone, Copy, Debug)]
pub enum WorldEventSound {
    Birth,
    Death,
    Combat,
    KingdomRise,
    KingdomFall,
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
    /// Trigger a world event sound.
    PlayWorldEvent(WorldEventSound),
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
// PCM synthesis helpers
// ---------------------------------------------------------------------------

const SAMPLE_RATE: u32 = 44100;

/// Generate a sine tone with a linear attack/decay envelope.
/// Returns f32 mono samples at 44100 Hz.
fn synth_sine_envelope(
    freq_start: f32,
    freq_end: f32,
    duration_secs: f32,
    amplitude: f32,
    attack_frac: f32,  // 0..1 — fraction of duration used for attack
    decay_frac: f32,   // 0..1 — fraction of duration used for decay
) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut phase: f32 = 0.0;

    for i in 0..n {
        let t = i as f32 / n as f32;
        let freq = freq_start + (freq_end - freq_start) * t;

        // Envelope
        let env = if t < attack_frac {
            t / attack_frac.max(1e-6)
        } else if t > 1.0 - decay_frac {
            (1.0 - t) / decay_frac.max(1e-6)
        } else {
            1.0
        };

        let sample = (phase * std::f32::consts::TAU).sin() * env * amplitude;
        samples.push(sample);

        phase += freq / SAMPLE_RATE as f32;
        if phase >= 1.0 { phase -= 1.0; }
    }

    samples
}

/// White noise burst with exponential decay.
fn synth_noise_burst(duration_secs: f32, amplitude: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut seed: u64 = 0x517cc1b727220a95;

    for i in 0..n {
        // xorshift64
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let noise = (seed as i64 as f32) / i64::MAX as f32;

        let t = i as f32 / n as f32;
        let env = (-t * 6.0_f32).exp(); // exponential decay
        samples.push(noise * env * amplitude);
    }

    samples
}

/// Low-frequency ambient drone: slow sine wave with added harmonic texture.
fn synth_ambient_drone(freq: f32, duration_secs: f32, amplitude: f32) -> Vec<f32> {
    let n = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut phase1: f32 = 0.0;
    let mut phase2: f32 = 0.0;
    let mut phase3: f32 = 0.0;

    for _ in 0..n {
        // Fundamental + 2 overtones at lower amplitude
        let s = (phase1 * std::f32::consts::TAU).sin() * 0.6
            + (phase2 * std::f32::consts::TAU).sin() * 0.25
            + (phase3 * std::f32::consts::TAU).sin() * 0.15;
        samples.push(s * amplitude);

        phase1 += freq / SAMPLE_RATE as f32;
        phase2 += (freq * 2.0) / SAMPLE_RATE as f32;
        phase3 += (freq * 3.0) / SAMPLE_RATE as f32;
        if phase1 >= 1.0 { phase1 -= 1.0; }
        if phase2 >= 1.0 { phase2 -= 1.0; }
        if phase3 >= 1.0 { phase3 -= 1.0; }
    }

    samples
}

/// Convert f32 mono sample buffer to a rodio-compatible source using SamplesBuffer.
fn to_rodio_source(samples: Vec<f32>, volume: f32) -> rodio::buffer::SamplesBuffer<f32> {
    let scaled: Vec<f32> = samples.iter().map(|s| s * volume).collect();
    rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, scaled)
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
}

/// Lerp ambient volume between far and close based on zoom.
/// zoom = 0.0 → 0.05, zoom = 1.0 → 0.20
fn zoom_ambient_volume(zoom: f32) -> f32 {
    0.05 + zoom.clamp(0.0, 1.0) * 0.15
}

impl AmbientLayer {
    fn new(label: &'static str) -> Self {
        AmbientLayer { label, weight: 0.0, prev_weight: 0.0 }
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
    /// Rodio output stream sink for one-shot sounds
    sink:       Option<rodio::Sink>,
    /// Ambient sink (looping)
    ambient_sink: Option<rodio::Sink>,
    /// Ambient loop counter — triggers a new ambient drone every N ms
    ambient_timer: std::time::Instant,
    ambient_interval_secs: f32,
    /// Last known biome for layer mixing
    current_biome: BiomeAmbience,
    /// Last known zoom for ambient volume scaling
    current_zoom: f32,
    /// Simple LCG seed for pitch jitter
    rng_seed: u64,
}

const CROSSFADE_TICKS: u32 = 120; // ~2 seconds at 60fps

impl AudioThreadState {
    fn new(sink: Option<rodio::Sink>, ambient_sink: Option<rodio::Sink>) -> Self {
        AudioThreadState {
            volumes: VolumeSettings::default(),
            layers: [
                AmbientLayer::new("nature"),
                AmbientLayer::new("night"),
                AmbientLayer::new("settlement"),
                AmbientLayer::new("weather"),
            ],
            fade_timer: 0,
            sink,
            ambient_sink,
            ambient_timer: std::time::Instant::now(),
            ambient_interval_secs: 4.0,
            current_biome: BiomeAmbience::Grassland,
            current_zoom: 0.5,
            rng_seed: 0x517cc1b727220a95,
        }
    }

    /// Xorshift64 RNG — returns a value in [0.0, 1.0).
    fn rand_f32(&mut self) -> f32 {
        self.rng_seed ^= self.rng_seed << 13;
        self.rng_seed ^= self.rng_seed >> 7;
        self.rng_seed ^= self.rng_seed << 17;
        (self.rng_seed as u32) as f32 / u32::MAX as f32
    }

    /// Return a pitch multiplier in [0.9, 1.1] for ±10% jitter.
    fn pitch_jitter(&mut self) -> f32 {
        0.9 + self.rand_f32() * 0.2
    }

    fn apply_context(&mut self, ctx: AudioContext) {
        let is_night = ctx.time_of_day < 0.15 || ctx.time_of_day > 0.85;

        // Base nature weight — biome modifies this
        let biome_nature_boost = match ctx.biome {
            BiomeAmbience::Forest    => 1.4,  // strong bird/nature layer
            BiomeAmbience::Grassland => 1.0,
            BiomeAmbience::Water     => 0.6,  // less nature, more ambient rumble below
            BiomeAmbience::Desert    => 0.3,  // sparse nature
            BiomeAmbience::Mountain  => 0.5,  // wind dominates
        };

        let nature_base: f32 = if ctx.weather_active { 0.3 } else { 1.0 };
        let nature_w     = (nature_base * biome_nature_boost).min(1.0);
        let night_w      = if is_night { 0.8 } else { 0.0 };
        let settlement_w = if ctx.near_settlement { 0.6 } else { 0.0 };
        let weather_w    = if ctx.weather_active { 0.9 } else { 0.0 };

        // Track biome and zoom for play_ambient_tick
        self.current_biome = ctx.biome;
        self.current_zoom  = ctx.zoom_normalized;

        for (i, w) in [nature_w, night_w, settlement_w, weather_w].iter().enumerate() {
            if (self.layers[i].weight - w).abs() > 0.01 {
                self.layers[i].prev_weight = self.layers[i].weight;
                self.layers[i].weight = *w;
                self.fade_timer = CROSSFADE_TICKS;
            }
        }

        // Tick ambient loop
        if self.ambient_timer.elapsed().as_secs_f32() >= self.ambient_interval_secs {
            self.ambient_timer = std::time::Instant::now();
            self.play_ambient_tick();
        }
    }

    fn play_ambient_tick(&self) {
        let Some(ref sink) = self.ambient_sink else { return; };
        if sink.len() > 2 { return; } // Don't queue up too many

        let master = self.volumes.effective_master();
        let amb_vol = self.volumes.ambient() * master;
        if amb_vol < 0.01 { return; }

        // Zoom-linked base: quieter when far out, louder when close in
        let zoom_vol = zoom_ambient_volume(self.current_zoom);
        let base_vol = amb_vol * zoom_vol;

        // Determine dominant layer
        let nature_w  = self.layers[0].weight;
        let night_w   = self.layers[1].weight;
        let weather_w = self.layers[3].weight;

        // Persistent low wind underlayer — always present at minimal volume
        // 80–100 Hz filtered noise to fill silence without being annoying
        let wind_vol = base_vol * 0.03;
        let wind = synth_noise_burst(3.0, wind_vol);
        // Low-pass approximation: blend drone at same freq to soften noise
        let mut wind_layer = synth_ambient_drone(90.0, 3.0, wind_vol * 0.5);
        for (a, b) in wind_layer.iter_mut().zip(wind.iter()) { *a += b; }
        sink.append(to_rodio_source(wind_layer, 1.0));

        if weather_w > 0.5 {
            // Rain/storm: broadband noise on top of wind
            let samples = synth_noise_burst(2.0, base_vol * weather_w * 0.6);
            sink.append(to_rodio_source(samples, 1.0));
        } else if night_w > 0.5 {
            // Night: soft high-freq crickets-like tone (quieter than before)
            let samples = synth_sine_envelope(800.0, 820.0, 1.5, base_vol * night_w * 0.4, 0.1, 0.3);
            sink.append(to_rodio_source(samples, 1.0));
        } else {
            // Biome-specific ambient character
            match self.current_biome {
                BiomeAmbience::Forest => {
                    // Rich nature: layered drone + higher-freq bird-like shimmer
                    if nature_w > 0.3 {
                        let mut s = synth_ambient_drone(55.0, 3.0, base_vol * nature_w * 0.5);
                        let shimmer = synth_sine_envelope(1200.0, 1400.0, 0.4, base_vol * 0.08, 0.1, 0.4);
                        for (a, b) in s.iter_mut().zip(shimmer.iter()) { *a += b; }
                        sink.append(to_rodio_source(s, 1.0));
                    }
                }
                BiomeAmbience::Water => {
                    // Low wave-like rumble: slow LFO on drone
                    let mut s = synth_ambient_drone(40.0, 3.5, base_vol * 0.3);
                    let noise = synth_noise_burst(3.5, base_vol * 0.08);
                    for (a, b) in s.iter_mut().zip(noise.iter()) { *a += b; }
                    sink.append(to_rodio_source(s, 1.0));
                }
                BiomeAmbience::Desert => {
                    // Wind: filtered noise, no drone
                    let samples = synth_noise_burst(2.5, base_vol * 0.18);
                    sink.append(to_rodio_source(samples, 1.0));
                }
                BiomeAmbience::Mountain => {
                    // High wind + distant echo-like tone
                    let mut s = synth_noise_burst(2.0, base_vol * 0.2);
                    let echo = synth_sine_envelope(300.0, 250.0, 1.0, base_vol * 0.06, 0.2, 0.6);
                    // Offset echo by 0.5s
                    let gap = (SAMPLE_RATE as f32 * 0.5) as usize;
                    let total = s.len().max(gap + echo.len());
                    s.resize(total, 0.0);
                    for (i, b) in echo.iter().enumerate() {
                        if gap + i < s.len() { s[gap + i] += b; }
                    }
                    sink.append(to_rodio_source(s, 1.0));
                }
                BiomeAmbience::Grassland => {
                    // Soft low drone — always plays (no nature_w gate)
                    let samples = synth_ambient_drone(55.0, 3.0, base_vol * 0.2);
                    sink.append(to_rodio_source(samples, 1.0));
                }
            }
        }
    }

    fn apply_volumes(&mut self, v: [f32; 4]) {
        self.volumes.levels = v;
        self.update_sink_volumes();
    }

    fn set_muted(&mut self, muted: bool) {
        self.volumes.muted = muted;
        self.update_sink_volumes();
    }

    fn update_sink_volumes(&self) {
        let effective = self.volumes.effective_master();
        if let Some(ref sink) = self.sink {
            sink.set_volume(effective * self.volumes.sfx());
        }
        if let Some(ref sink) = self.ambient_sink {
            sink.set_volume(effective * self.volumes.ambient());
        }
    }

    /// Advance crossfade timer.
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

    fn play_god_power(&self, sound: GodPowerSound) {
        let Some(ref sink) = self.sink else { return; };
        let vol = self.volumes.effective_master() * self.volumes.sfx();
        if vol < 0.01 { return; }

        let samples = match sound {
            GodPowerSound::Lightning => {
                // Sharp crack: short noise burst, reduced from 0.9
                synth_noise_burst(0.1, vol * 0.35)
            }
            GodPowerSound::Meteor => {
                // Deep falling rumble: descending tone + noise
                let mut s = synth_sine_envelope(200.0, 60.0, 0.7, vol * 0.30, 0.05, 0.3);
                let noise = synth_noise_burst(0.7, vol * 0.15);
                for (a, b) in s.iter_mut().zip(noise.iter()) { *a += b; }
                s
            }
            GodPowerSound::Earthquake => {
                // Low rumble
                let mut s = synth_ambient_drone(30.0, 0.8, vol * 0.25);
                let noise = synth_noise_burst(0.8, vol * 0.12);
                for (a, b) in s.iter_mut().zip(noise.iter()) { *a += b; }
                s
            }
            GodPowerSound::Tornado => {
                // Rising whoosh noise
                synth_noise_burst(0.4, vol * 0.22)
            }
            GodPowerSound::Volcano => {
                // Deep boom
                synth_sine_envelope(80.0, 40.0, 1.0, vol * 0.32, 0.02, 0.6)
            }
            GodPowerSound::RainStart => {
                synth_noise_burst(0.8, vol * 0.18)
            }
            GodPowerSound::RainStop => {
                // Short descending tone
                synth_sine_envelope(300.0, 200.0, 0.3, vol * 0.15, 0.1, 0.5)
            }
            GodPowerSound::BlessingJoy => {
                // Bright rising chord-like tones — A4 neutral per spec
                let mut s = synth_sine_envelope(440.0, 880.0, 0.4, vol * 0.18, 0.1, 0.4);
                let s2 = synth_sine_envelope(660.0, 1320.0, 0.4, vol * 0.12, 0.1, 0.4);
                for (a, b) in s.iter_mut().zip(s2.iter()) { *a += b; }
                s
            }
            GodPowerSound::CurseAnger => {
                // Dissonant descending tone
                synth_sine_envelope(440.0, 110.0, 0.5, vol * 0.20, 0.05, 0.4)
            }
            GodPowerSound::PlaceBeing => {
                // Soft C5 chime — matches birth sound freq
                synth_sine_envelope(523.0, 1046.0, 0.1, vol * 0.15, 0.05, 0.6)
            }
            GodPowerSound::RemoveBeing => {
                // Short descending A3 note
                synth_sine_envelope(220.0, 110.0, 0.1, vol * 0.15, 0.02, 0.7)
            }
        };

        let source = to_rodio_source(samples, 1.0);
        sink.append(source);
    }

    fn play_world_event(&mut self, sound: WorldEventSound) {
        if self.sink.is_none() { return; }
        let vol = self.volumes.effective_master() * self.volumes.sfx();
        if vol < 0.01 { return; }

        // ±10% pitch jitter (computed before sink borrow to satisfy borrow checker)
        let jitter = self.pitch_jitter();
        let Some(ref sink) = self.sink else { return; };

        let samples = match sound {
            WorldEventSound::Birth => {
                // Gentle C5 chime: 523 Hz, 100ms, soft attack — jitter shifts ±10%
                synth_sine_envelope(523.0 * jitter, 523.0 * jitter, 0.1, vol * 0.15, 0.05, 0.6)
            }
            WorldEventSound::Death => {
                // Low A3 tone, descending slightly, 100ms, quiet
                synth_sine_envelope(220.0 * jitter, 180.0 * jitter, 0.1, vol * 0.15, 0.02, 0.7)
            }
            WorldEventSound::Combat => {
                // Short white noise burst: 80ms, reduced volume
                synth_noise_burst(0.08, vol * 0.15)
            }
            WorldEventSound::KingdomRise => {
                // Neutral A4 sweep + brief ascending arpeggio (C4 E4 G4 C5), low vol
                let mut s = synth_sine_envelope(440.0, 880.0, 0.4, vol * 0.15, 0.1, 0.4);
                // Brief ascending arpeggio: C4, E4, G4, C5
                let note_freqs = [261.63_f32, 329.63, 392.0, 523.25];
                let gap_len = (SAMPLE_RATE as f32 * 0.02) as usize;
                let mut motif: Vec<f32> = Vec::new();
                for &freq in &note_freqs {
                    let note_s = synth_sine_envelope(freq, freq, 0.1, vol * 0.12, 0.05, 0.4);
                    motif.extend(note_s);
                    motif.extend(vec![0.0f32; gap_len]);
                }
                s.extend(motif);
                s
            }
            WorldEventSound::KingdomFall => {
                // Quiet descending tone
                synth_sine_envelope(220.0 * jitter, 110.0 * jitter, 0.5, vol * 0.15, 0.05, 0.5)
            }
        };

        sink.append(to_rodio_source(samples, 1.0));
    }

    fn play_ui(&self, sound: UiSound) {
        let Some(ref sink) = self.sink else { return; };
        let vol = self.volumes.effective_master() * self.volumes.sfx();
        if vol < 0.01 { return; }

        let samples = match sound {
            UiSound::ButtonClick => synth_sine_envelope(600.0, 600.0, 0.05, vol * 0.25, 0.01, 0.5),
            UiSound::PanelOpen   => synth_sine_envelope(400.0, 600.0, 0.1, vol * 0.2, 0.05, 0.5),
            UiSound::PanelClose  => synth_sine_envelope(600.0, 400.0, 0.1, vol * 0.2, 0.05, 0.5),
            UiSound::SpeedChange => synth_sine_envelope(500.0, 500.0, 0.08, vol * 0.2, 0.02, 0.5),
            UiSound::InspectorOpen => synth_sine_envelope(450.0, 650.0, 0.12, vol * 0.2, 0.05, 0.4),
            UiSound::Notification  => synth_sine_envelope(880.0, 660.0, 0.15, vol * 0.3, 0.05, 0.5),
            UiSound::KingdomAlert  => {
                let mut s = synth_sine_envelope(660.0, 880.0, 0.2, vol * 0.35, 0.05, 0.4);
                let s2 = synth_sine_envelope(660.0, 880.0, 0.2, vol * 0.2, 0.05, 0.4);
                // Second pulse after a gap
                let gap = vec![0.0f32; (SAMPLE_RATE as f32 * 0.1) as usize];
                s.extend(gap);
                s.extend(s2);
                s
            }
        };

        let source = to_rodio_source(samples, 1.0);
        sink.append(source);
    }
}

// ---------------------------------------------------------------------------
// SoundEngine — public handle (lives on main thread)
// ---------------------------------------------------------------------------

/// Main-thread handle to the audio subsystem.
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
                // Try to open the default output device.
                let audio_result = rodio::OutputStream::try_default();
                let (sink, ambient_sink) = match audio_result {
                    Ok((_stream, stream_handle)) => {
                        let sfx = rodio::Sink::try_new(&stream_handle).ok();
                        let amb = rodio::Sink::try_new(&stream_handle).ok();
                        // Both _stream and stream_handle must outlive the sinks.
                        // Leak both onto the heap so they live for the process lifetime.
                        // Safe: intentional, we own the audio device for the process duration.
                        let handle_box = Box::new(stream_handle);
                        std::mem::forget(handle_box);
                        std::mem::forget(_stream);
                        (sfx, amb)
                    }
                    Err(e) => {
                        eprintln!("[audio] No output device: {e}");
                        (None, None)
                    }
                };

                let mut state = AudioThreadState::new(sink, ambient_sink);

                // Prime ambient on start — set nature layer and play immediately
                state.layers[0].weight = 1.0;
                state.play_ambient_tick();

                // Startup tone muted — was a harsh beep at 440 Hz on every launch.
                if state.sink.is_some() {
                    eprintln!("[audio] Audio pipeline confirmed (startup tone silenced)");
                } else {
                    eprintln!("[audio] No sink available — audio will be silent");
                }

                loop {
                    match rx.recv() {
                        Ok(cmd) => {
                            if !Self::handle_command(&mut state, cmd) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }

                    // Drain queued commands without blocking
                    while let Ok(cmd) = rx.try_recv() {
                        if !Self::handle_command(&mut state, cmd) {
                            return;
                        }
                    }

                    state.tick_fade();
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
            AudioCommand::UpdateContext(ctx)    => state.apply_context(ctx),
            AudioCommand::PlayGodPower(sound)   => state.play_god_power(sound),
            AudioCommand::PlayUi(sound)         => state.play_ui(sound),
            AudioCommand::PlayWorldEvent(sound) => state.play_world_event(sound),
            AudioCommand::SetVolumes(v)         => state.apply_volumes(v),
            AudioCommand::SetMuted(m)           => state.set_muted(m),
            AudioCommand::Shutdown              => return false,
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

    /// Trigger a world event sound.
    pub fn play_world_event(&self, sound: WorldEventSound) {
        let _ = self.sender.send(AudioCommand::PlayWorldEvent(sound));
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

// ---------------------------------------------------------------------------
// God power ID → sound mapping
// ---------------------------------------------------------------------------

/// Map a god power ID (from power_catalog) to an optional GodPowerSound.
pub fn god_power_id_to_sound(pid: u8) -> Option<GodPowerSound> {
    match pid {
        // Creation
        0..=11 => Some(GodPowerSound::PlaceBeing),
        // Terrain brush (12-21): no sound needed
        12..=21 => None,
        // Weather
        22 => Some(GodPowerSound::RainStart),
        24 => Some(GodPowerSound::RainStart), // Storm
        // Destruction
        30 => Some(GodPowerSound::Lightning),
        31 => Some(GodPowerSound::Meteor),
        32 => Some(GodPowerSound::Earthquake),
        37 => Some(GodPowerSound::Tornado),
        38 | 39 | 40 | 41 => Some(GodPowerSound::RemoveBeing),
        // Blessing
        42..=51 => Some(GodPowerSound::BlessingJoy),
        // Curse
        52..=61 => Some(GodPowerSound::CurseAnger),
        _ => None,
    }
}
