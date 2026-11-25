//! Comprehensive Sound and Music System for BlockGame.
//!
//! Features:
//! - Material-based Block SFX (breaking and placing) mapped across voxel block types.
//! - 3D spatial attenuation and stereo panning based on listener camera position and yaw.
//! - Random pitch (0.85 to 1.15) and volume (0.85 to 1.05) variation to eliminate repetitive fatigue.
//! - Dynamic footstep triggers synchronized with grounded movement and walk/sprint stride intervals.
//! - Intermittent background music scheduler with non-repeating track selection and 3-6 minute cooldowns.
//! - Custom stereo MP3 music tracks embedded directly in the binary (with optional disk overrides)
//!   plus self-contained procedural WAV synthesizers for SFX with optional drop-in WAV/OGG files.

use blockgame_core::block::BlockId;
use glam::Vec3;
use maudio::{
    audio::sample_rate::SampleRate,
    data_source::sources::decoder::{DecoderBuilder, DecoderOps},
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        Device,
    },
};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub const MUSIC_TRACK_WHERE_MAPS_END: usize = 0;
pub const MUSIC_TRACK_MEASURE_OF_THE_DARK: usize = 1;
pub const MUSIC_TRACK_WHERE_THE_ROOTS_BREAK_SOIL: usize = 2;
pub const MUSIC_TRACK_THE_VIEW_FROM_THE_RIDGE: usize = 3;
pub const BACKGROUND_MUSIC_TRACKS: [usize; 2] = [
    MUSIC_TRACK_WHERE_THE_ROOTS_BREAK_SOIL,
    MUSIC_TRACK_THE_VIEW_FROM_THE_RIDGE,
];

const MUSIC_TRACK_DEFS: [(&str, &str, &[u8]); 4] = [
    (
        "Where_Maps_End.mp3",
        "Where Maps End (Adventuring)",
        include_bytes!("../../../Music/Where_Maps_End.mp3"),
    ),
    (
        "Measure_of_the_Dark.mp3",
        "Measure of the Dark (Mining)",
        include_bytes!("../../../Music/Measure_of_the_Dark.mp3"),
    ),
    (
        "Where_the_Roots_Break_Soil.mp3",
        "Where the Roots Break Soil (Background)",
        include_bytes!("../../../Music/Where_the_Roots_Break_Soil.mp3"),
    ),
    (
        "The_View_From_The_Ridge.mp3",
        "The View From The Ridge (Background)",
        include_bytes!("../../../Music/The_View_From_The_Ridge.mp3"),
    ),
];

/// Sound profile materials for physics and voxel interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundMaterial {
    Grass,
    Dirt,
    Stone,
    Wood,
    Sand,
    Glass,
}

impl SoundMaterial {
    /// Maps any BlockId into its corresponding sound material profile.
    pub fn from_block(block: BlockId) -> Self {
        match block {
            BlockId::GRASS
            | BlockId::LEAVES
            | BlockId::TALL_GRASS
            | BlockId::BIRCH_LEAVES
            | BlockId::SPRUCE_LEAVES
            | BlockId::ACACIA_LEAVES
            | BlockId::JUNGLE_LEAVES
            | BlockId::DARK_OAK_LEAVES
            | BlockId::LILY_PAD
            | BlockId::SEAGRASS
            | BlockId::WHEAT_0
            | BlockId::WHEAT_1
            | BlockId::WHEAT_2
            | BlockId::WHEAT_3 => SoundMaterial::Grass,

            BlockId::DIRT
            | BlockId::FARMLAND
            | BlockId::FARMLAND_MOIST
            | BlockId::PODZOL
            | BlockId::MYCELIUM
            | BlockId::CLAY => SoundMaterial::Dirt,

            BlockId::STONE
            | BlockId::COBBLESTONE
            | BlockId::BEDROCK
            | BlockId::COAL_ORE
            | BlockId::COPPER_ORE
            | BlockId::IRON_ORE
            | BlockId::GOLD_ORE
            | BlockId::DIAMOND_ORE
            | BlockId::FURNACE
            | BlockId::FURNACE_LIT
            | BlockId::SANDSTONE
            | BlockId::TERRACOTTA
            | BlockId::MOSSY_COBBLESTONE
            | BlockId::PRISMARINE => SoundMaterial::Stone,

            BlockId::WOOD_LOG
            | BlockId::PLANKS
            | BlockId::BIRCH_LOG
            | BlockId::BIRCH_PLANKS
            | BlockId::SPRUCE_LOG
            | BlockId::SPRUCE_PLANKS
            | BlockId::ACACIA_LOG
            | BlockId::ACACIA_PLANKS
            | BlockId::JUNGLE_LOG
            | BlockId::JUNGLE_PLANKS
            | BlockId::DARK_OAK_LOG
            | BlockId::DARK_OAK_PLANKS
            | BlockId::CRAFTING_TABLE
            | BlockId::CHEST
            | BlockId::MUSHROOM_STEM
            | BlockId::TORCH => SoundMaterial::Wood,

            BlockId::SAND | BlockId::RED_SAND | BlockId::GRAVEL => SoundMaterial::Sand,

            BlockId::GLASS | BlockId::ICE | BlockId::SEA_LANTERN => SoundMaterial::Glass,

            _ => SoundMaterial::Dirt,
        }
    }
}

/// Compute 3D distance attenuation and stereo panning relative to camera listener.
///
/// Returns `(attenuation, pan)` where pan is in [-1.0 (left), 1.0 (right)].
pub fn calculate_spatial_audio(
    sound_pos: Vec3,
    listener_pos: Vec3,
    listener_yaw: f32,
    max_distance: f32,
) -> (f32, f32) {
    let delta = sound_pos - listener_pos;
    let dist = delta.length();
    if dist > max_distance {
        return (0.0, 0.0);
    }

    // Realistic inverse-distance curve with smooth rolloff
    let atten = (1.0 / (1.0 + 0.08 * dist * dist)).clamp(0.0, 1.0);
    if dist < 0.001 {
        return (atten, 0.0);
    }

    // Camera right vector in XZ plane
    let right = Vec3::new(-listener_yaw.sin(), 0.0, listener_yaw.cos()).normalize();
    let pan = (delta.dot(right) / dist).clamp(-1.0, 1.0);

    (atten, pan)
}

/// Category of a sound voice for targeted volume control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCategory {
    Master,
    Sfx,
    Music,
    Voice,
    Creature,
}

/// Represents an active sound voice currently being rendered by the mixer.
struct ActiveVoice {
    samples: Arc<Vec<f32>>,
    is_stereo: bool,
    cursor: f32,
    pitch: f32,
    volume_l: f32,
    volume_r: f32,
    category: SoundCategory,
    fade: f32,
    target_fade: f32,
    fade_speed: f32,
}

impl ActiveVoice {
    #[inline]
    fn total_frames(&self) -> usize {
        if self.is_stereo {
            self.samples.len() / 2
        } else {
            self.samples.len()
        }
    }
}

/// Per-player streaming jitter buffer for 16 kHz proximity voice chat.
pub struct VoiceChatStream {
    pub samples: VecDeque<f32>,
    pub cursor: f32,
    pub volume_l: f32,
    pub volume_r: f32,
    pub last_seq: u32,
}

/// Thread-safe real-time audio mixer.
pub struct AudioMixer {
    voices: Vec<ActiveVoice>,
    voice_streams: HashMap<u32, VoiceChatStream>,
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub creature_volume: f32,
    pub voice_volume: f32,
}

impl AudioMixer {
    pub fn new() -> Self {
        Self {
            voices: Vec::with_capacity(32),
            voice_streams: HashMap::new(),
            master_volume: 0.8,
            sfx_volume: 0.9,
            music_volume: 0.65,
            creature_volume: 0.85,
            voice_volume: 1.0,
        }
    }

    /// Queues decoded 16 kHz mono voice samples for a remote player with stereo panning.
    pub fn push_voice_samples(
        &mut self,
        sender_id: u32,
        seq: u32,
        pcm: &[f32],
        volume_l: f32,
        volume_r: f32,
    ) {
        if pcm.is_empty() {
            return;
        }
        let stream = self.voice_streams.entry(sender_id).or_insert_with(|| VoiceChatStream {
            samples: VecDeque::with_capacity(3200),
            cursor: 0.0,
            volume_l,
            volume_r,
            last_seq: seq,
        });
        if seq < stream.last_seq && stream.last_seq - seq < 1000 {
            return; // Ignore out-of-order late packets
        }
        stream.last_seq = seq;
        stream.volume_l = volume_l;
        stream.volume_r = volume_r;
        // Cap jitter buffer at ~0.5s (8000 samples) to keep latency low
        if stream.samples.len() > 8000 {
            let excess = stream.samples.len() - 4000;
            stream.samples.drain(..excess);
            stream.cursor = 0.0;
        }
        stream.samples.extend(pcm.iter().copied());
    }

    /// Add a mono voice to the active voice list.
    pub fn play_voice(
        &mut self,
        samples: Arc<Vec<f32>>,
        pitch: f32,
        volume_l: f32,
        volume_r: f32,
        category: SoundCategory,
        fade_in_sec: f32,
    ) {
        let (fade, target_fade, fade_speed) = if fade_in_sec > 0.0 {
            (0.0, 1.0, 1.0 / fade_in_sec)
        } else {
            (1.0, 1.0, 0.0)
        };

        self.voices.push(ActiveVoice {
            samples,
            is_stereo: false,
            cursor: 0.0,
            pitch: pitch.clamp(0.2, 4.0),
            volume_l,
            volume_r,
            category,
            fade,
            target_fade,
            fade_speed,
        });
    }

    /// Add an interleaved stereo voice (e.g. music tracks) to the active voice list.
    pub fn play_stereo_voice(
        &mut self,
        samples: Arc<Vec<f32>>,
        volume_l: f32,
        volume_r: f32,
        category: SoundCategory,
        fade_in_sec: f32,
    ) {
        let (fade, target_fade, fade_speed) = if fade_in_sec > 0.0 {
            (0.0, 1.0, 1.0 / fade_in_sec)
        } else {
            (1.0, 1.0, 0.0)
        };

        self.voices.push(ActiveVoice {
            samples,
            is_stereo: true,
            cursor: 0.0,
            pitch: 1.0,
            volume_l,
            volume_r,
            category,
            fade,
            target_fade,
            fade_speed,
        });
    }

    /// Stops any playing music tracks with a fade-out.
    pub fn stop_music(&mut self, fade_out_sec: f32) {
        for voice in &mut self.voices {
            if voice.category == SoundCategory::Music {
                voice.target_fade = 0.0;
                voice.fade_speed = if fade_out_sec > 0.0 {
                    1.0 / fade_out_sec
                } else {
                    100.0
                };
            }
        }
    }

    /// Check if any music tracks are actively playing and not faded out.
    pub fn is_music_playing(&self) -> bool {
        self.voices
            .iter()
            .any(|v| v.category == SoundCategory::Music && (v.fade > 0.001 || v.target_fade > 0.001))
    }

    /// Mixes active voices into an interleaved stereo output buffer.
    pub fn mix(&mut self, output: &mut [f32], sample_rate: u32) {
        let dt = 1.0 / (sample_rate as f32);
        let frame_count = output.len() / 2;
        let voice_step = 16_000.0 / (sample_rate as f32);

        for frame_idx in 0..frame_count {
            let mut left_acc = 0.0;
            let mut right_acc = 0.0;

            for voice in &mut self.voices {
                let total_frames = voice.total_frames();
                if total_frames == 0 || voice.cursor >= total_frames as f32 {
                    continue;
                }

                // Update fade multiplier
                if voice.fade < voice.target_fade {
                    voice.fade = (voice.fade + voice.fade_speed * dt).min(voice.target_fade);
                } else if voice.fade > voice.target_fade {
                    voice.fade = (voice.fade - voice.fade_speed * dt).max(voice.target_fade);
                }

                if voice.fade <= 0.0 && voice.target_fade <= 0.0 {
                    voice.cursor = total_frames as f32; // mark for cleanup
                    continue;
                }

                let idx0 = voice.cursor as usize;
                let idx1 = (idx0 + 1).min(total_frames - 1);
                let frac = voice.cursor - idx0 as f32;

                let (sample_l, sample_r) = if voice.is_stereo {
                    let s0 = idx0 * 2;
                    let s1 = idx1 * 2;
                    let l = voice.samples[s0] * (1.0 - frac) + voice.samples[s1] * frac;
                    let r = voice.samples[s0 + 1] * (1.0 - frac) + voice.samples[s1 + 1] * frac;
                    (l, r)
                } else {
                    let s = voice.samples[idx0] * (1.0 - frac) + voice.samples[idx1] * frac;
                    (s, s)
                };

                let category_vol = match voice.category {
                    SoundCategory::Music => self.music_volume,
                    SoundCategory::Creature => self.creature_volume,
                    _ => self.sfx_volume,
                };
                let effective_vol = self.master_volume * category_vol * voice.fade;

                left_acc += sample_l * voice.volume_l * effective_vol;
                right_acc += sample_r * voice.volume_r * effective_vol;

                voice.cursor += voice.pitch;
            }

            // Mix active proximity voice chat streams (resampled 16 kHz -> output sample_rate)
            let voice_gain = self.master_volume * self.voice_volume;
            if voice_gain > 0.001 {
                for stream in self.voice_streams.values_mut() {
                    if stream.samples.len() < 2 {
                        continue;
                    }
                    let idx0 = stream.cursor as usize;
                    if idx0 + 1 >= stream.samples.len() {
                        continue;
                    }
                    let frac = stream.cursor - idx0 as f32;
                    let sample = stream.samples[idx0] * (1.0 - frac) + stream.samples[idx0 + 1] * frac;
                    left_acc += sample * stream.volume_l * voice_gain;
                    right_acc += sample * stream.volume_r * voice_gain;
                    stream.cursor += voice_step;
                }
            }

            let out_idx = frame_idx * 2;
            output[out_idx] = (output[out_idx] + left_acc).clamp(-1.0, 1.0);
            output[out_idx + 1] = (output[out_idx + 1] + right_acc).clamp(-1.0, 1.0);
        }

        // Drain consumed voice samples
        for stream in self.voice_streams.values_mut() {
            let consumed = stream.cursor as usize;
            if consumed > 0 {
                let drain_n = consumed.min(stream.samples.len());
                stream.samples.drain(..drain_n);
                stream.cursor -= drain_n as f32;
            }
        }
        self.voice_streams.retain(|_, s| !s.samples.is_empty());

        // Retain only voices that still have samples remaining
        self.voices.retain(|v| v.cursor < v.total_frames() as f32);
    }
}

/// Procedural audio synthesizers producing 44.1kHz mono PCM buffers.
pub mod sound_generator {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    fn lcg_random(state: &mut u64) -> f32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((*state >> 33) as f32) / ((u32::MAX >> 1) as f32) - 1.0
    }

    /// Synthesizes crisp, one-shot footstep clips for different terrain materials.
    pub fn generate_footstep(material: SoundMaterial, variant: usize) -> Vec<f32> {
        let mut rng = (material as u64 + 1) * 1013904223 + (variant as u64 * 31337);
        let duration = match material {
            SoundMaterial::Grass => 0.12,
            SoundMaterial::Dirt => 0.11,
            SoundMaterial::Stone => 0.09,
            SoundMaterial::Wood => 0.13,
            SoundMaterial::Sand => 0.14,
            SoundMaterial::Glass => 0.10,
        };

        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let sample = match material {
                SoundMaterial::Grass => {
                    // Soft low thud + quick burst of filtered dry rustle
                    let thud = (2.0 * PI * 80.0 * t).sin() * (-progress * 25.0).exp();
                    let rustle = lcg_random(&mut rng) * (-progress * 18.0).exp();
                    0.6 * thud + 0.4 * rustle
                }
                SoundMaterial::Dirt => {
                    // Damped low-frequency thump with packed soil crunch
                    let thud = (2.0 * PI * 65.0 * t).sin() * (-progress * 30.0).exp();
                    let grit = lcg_random(&mut rng) * (-progress * 22.0).exp();
                    0.7 * thud + 0.3 * grit
                }
                SoundMaterial::Stone => {
                    // Crisp solid boot tap on cobblestone with short ring
                    let tap = (2.0 * PI * 1800.0 * t).sin() * (-progress * 50.0).exp();
                    let body = (2.0 * PI * 320.0 * t).sin() * (-progress * 35.0).exp();
                    let click = lcg_random(&mut rng) * (-progress * 60.0).exp();
                    0.5 * tap + 0.3 * body + 0.2 * click
                }
                SoundMaterial::Wood => {
                    // Hollow wooden plank knock with warm resonance
                    let knock = (2.0 * PI * 220.0 * t).sin() * (-progress * 28.0).exp();
                    let sub = (2.0 * PI * 110.0 * t).sin() * (-progress * 20.0).exp();
                    let creak = (2.0 * PI * 650.0 * t).sin() * (-progress * 45.0).exp();
                    0.5 * knock + 0.35 * sub + 0.15 * creak
                }
                SoundMaterial::Sand => {
                    // Soft muffled granular brush and friction
                    let noise = lcg_random(&mut rng) * (-progress * 16.0).exp();
                    let thud = (2.0 * PI * 55.0 * t).sin() * (-progress * 20.0).exp();
                    0.65 * noise + 0.35 * thud
                }
                SoundMaterial::Glass => {
                    // Delicate high-pitched chime tap
                    let ping1 = (2.0 * PI * 2800.0 * t).sin() * (-progress * 35.0).exp();
                    let ping2 = (2.0 * PI * 4200.0 * t).sin() * (-progress * 45.0).exp();
                    0.6 * ping1 + 0.4 * ping2
                }
            };

            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes crunchy block breaking audio effects.
    pub fn generate_break(material: SoundMaterial, variant: usize) -> Vec<f32> {
        let mut rng = (material as u64 + 7) * 7919 + (variant as u64 * 65537);
        let duration = match material {
            SoundMaterial::Grass => 0.26,
            SoundMaterial::Dirt => 0.28,
            SoundMaterial::Stone => 0.34,
            SoundMaterial::Wood => 0.32,
            SoundMaterial::Sand => 0.28,
            SoundMaterial::Glass => 0.38,
        };

        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let sample = match material {
                SoundMaterial::Grass => {
                    // Roots tearing and crisp vegetal tearing crunch
                    let rip = lcg_random(&mut rng) * (1.0 - progress).powi(2);
                    let pop = (2.0 * PI * 140.0 * t).sin() * (-progress * 15.0).exp();
                    0.65 * rip + 0.35 * pop
                }
                SoundMaterial::Dirt => {
                    // Quick digging crunch into packed earth
                    let earth = lcg_random(&mut rng) * (1.0 - progress).powi(3);
                    let thud = (2.0 * PI * 75.0 * t).sin() * (-progress * 12.0).exp();
                    0.6 * earth + 0.4 * thud
                }
                SoundMaterial::Stone => {
                    // Pickaxe strike and crunchy rock crumbling
                    let crack = lcg_random(&mut rng) * (1.0 - progress).powi(2);
                    let strike = (2.0 * PI * 1400.0 * t).sin() * (-progress * 40.0).exp();
                    let crumble = (2.0 * PI * 240.0 * t).sin() * (-progress * 10.0).exp();
                    0.5 * crack + 0.3 * strike + 0.2 * crumble
                }
                SoundMaterial::Wood => {
                    // Snapping timber with splintering fracture
                    let snap = (2.0 * PI * 340.0 * t).sin() * (-progress * 25.0).exp();
                    let splinter = lcg_random(&mut rng) * (1.0 - progress).powi(2);
                    0.5 * snap + 0.5 * splinter
                }
                SoundMaterial::Sand => {
                    // Granular collapse and dry sand shift
                    let grains = lcg_random(&mut rng) * (1.0 - progress).powi(2);
                    let low = (2.0 * PI * 60.0 * t).sin() * (-progress * 10.0).exp();
                    0.7 * grains + 0.3 * low
                }
                SoundMaterial::Glass => {
                    // Sharp crystalline shattering with bright dispersion
                    let shatter = lcg_random(&mut rng) * (1.0 - progress).powi(2);
                    let chime1 = (2.0 * PI * 3100.0 * t).sin() * (-progress * 18.0).exp();
                    let chime2 = (2.0 * PI * 4800.0 * t).sin() * (-progress * 22.0).exp();
                    0.4 * shatter + 0.3 * chime1 + 0.3 * chime2
                }
            };

            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes satisfying block placement snap/thud.
    pub fn generate_place(material: SoundMaterial, variant: usize) -> Vec<f32> {
        let mut rng = (material as u64 + 19) * 2909 + (variant as u64 * 12345);
        let duration = 0.16;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let sample = match material {
                SoundMaterial::Grass => {
                    let thud = (2.0 * PI * 90.0 * t).sin() * (-progress * 24.0).exp();
                    let rustle = lcg_random(&mut rng) * (-progress * 20.0).exp();
                    0.7 * thud + 0.3 * rustle
                }
                SoundMaterial::Dirt => {
                    let thud = (2.0 * PI * 80.0 * t).sin() * (-progress * 28.0).exp();
                    let grit = lcg_random(&mut rng) * (-progress * 25.0).exp();
                    0.75 * thud + 0.25 * grit
                }
                SoundMaterial::Stone => {
                    let click = (2.0 * PI * 1600.0 * t).sin() * (-progress * 55.0).exp();
                    let thud = (2.0 * PI * 180.0 * t).sin() * (-progress * 30.0).exp();
                    0.6 * click + 0.4 * thud
                }
                SoundMaterial::Wood => {
                    let snap = (2.0 * PI * 280.0 * t).sin() * (-progress * 30.0).exp();
                    let click = (2.0 * PI * 820.0 * t).sin() * (-progress * 45.0).exp();
                    0.65 * snap + 0.35 * click
                }
                SoundMaterial::Sand => {
                    let shh = lcg_random(&mut rng) * (-progress * 18.0).exp();
                    let thud = (2.0 * PI * 70.0 * t).sin() * (-progress * 22.0).exp();
                    0.6 * shh + 0.4 * thud
                }
                SoundMaterial::Glass => {
                    let ping = (2.0 * PI * 2400.0 * t).sin() * (-progress * 35.0).exp();
                    let click = (2.0 * PI * 4000.0 * t).sin() * (-progress * 50.0).exp();
                    0.6 * ping + 0.4 * click
                }
            };

            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }


    /// Synthesizes short, crisp block hitting/chipping audio effects for mining progress.
    pub fn generate_hit(material: SoundMaterial, variant: usize) -> Vec<f32> {
        let mut rng = (material as u64 + 43) * 31337 + (variant as u64 * 17921);
        let duration = 0.08;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let sample = match material {
                SoundMaterial::Grass => {
                    let rustle = lcg_random(&mut rng) * (-progress * 30.0).exp();
                    let tap = (2.0 * PI * 110.0 * t).sin() * (-progress * 35.0).exp();
                    0.6 * rustle + 0.4 * tap
                }
                SoundMaterial::Dirt => {
                    let crunch = lcg_random(&mut rng) * (-progress * 32.0).exp();
                    let thud = (2.0 * PI * 95.0 * t).sin() * (-progress * 38.0).exp();
                    0.55 * crunch + 0.45 * thud
                }
                SoundMaterial::Stone => {
                    let clink = (2.0 * PI * 2200.0 * t).sin() * (-progress * 60.0).exp();
                    let spark = lcg_random(&mut rng) * (-progress * 70.0).exp();
                    let body = (2.0 * PI * 440.0 * t).sin() * (-progress * 45.0).exp();
                    0.5 * clink + 0.3 * spark + 0.2 * body
                }
                SoundMaterial::Wood => {
                    let chop = (2.0 * PI * 360.0 * t).sin() * (-progress * 40.0).exp();
                    let pop = lcg_random(&mut rng) * (-progress * 45.0).exp();
                    0.6 * chop + 0.4 * pop
                }
                SoundMaterial::Sand => {
                    let grit = lcg_random(&mut rng) * (-progress * 25.0).exp();
                    let thud = (2.0 * PI * 80.0 * t).sin() * (-progress * 30.0).exp();
                    0.7 * grit + 0.3 * thud
                }
                SoundMaterial::Glass => {
                    let tap = (2.0 * PI * 3400.0 * t).sin() * (-progress * 50.0).exp();
                    0.8 * tap + 0.2 * lcg_random(&mut rng) * (-progress * 55.0).exp()
                }
            };

            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes realistic, visceral human damage audio (sudden flesh impact + vocal "OOF / UGH" grunt).
    pub fn generate_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 424242 + (variant as u64 * 99991);
        let duration = 0.32;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        // Vocal fundamental and formant frequencies based on variant
        let (f0_start, f0_end, f1, f2) = if variant == 0 {
            (155.0, 80.0, 480.0, 920.0) // Deep guttural "OOF" grunt
        } else {
            (175.0, 90.0, 560.0, 1150.0) // Sharp breath-expelled "UGH" grunt
        };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            // 1. Initial blunt force flesh & bone impact transient (0 - 25ms)
            let impact_noise = lcg_random(&mut rng) * (-progress * 42.0).exp();
            let impact_sub = (2.0 * PI * (90.0 - progress * 40.0) * t).sin() * (-progress * 26.0).exp();
            let impact_snap = (2.0 * PI * 850.0 * t).sin() * (-progress * 50.0).exp();
            let impact = 0.45 * impact_noise + 0.35 * impact_sub + 0.20 * impact_snap;

            // 2. Vocal reaction: Vocal fold glottal excitation compressed by chest trauma
            let pitch = (f0_start + (f0_end - f0_start) * progress.powf(0.8)).max(50.0);
            let phase = 2.0 * PI * pitch * t;
            let glottal = phase.sin() + 0.5 * (phase * 2.0).sin() + 0.25 * (phase * 3.0).sin();

            // Throat / vocal tract formant resonance filters
            let formant1 = (2.0 * PI * f1 * t).sin();
            let formant2 = (2.0 * PI * f2 * t).sin();
            let vocal_body = glottal * (0.55 * formant1 + 0.45 * formant2);

            // Breath expulsion / vocal fry roughness
            let breath = lcg_random(&mut rng) * 0.3 * (1.0 + (progress * PI).sin());

            // Vocal envelope: immediate attack (5ms), peaking at 20ms, trailing off naturally
            let vocal_env = if t < 0.02 {
                t / 0.02
            } else {
                (-(progress - 0.0625).max(0.0) * 11.0).exp()
            };

            let grunt = (0.75 * vocal_body + 0.25 * breath) * vocal_env;
            let sample = 0.42 * impact + 0.70 * grunt;
            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes bovine moo sound (warm, deep vocal harmonics with slow swell).
    pub fn generate_cow_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 10101 + variant as u64 * 31337;
        let duration = 0.90;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 135.0 } else { 148.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = base_f + 18.0 * (PI * progress).sin() - 15.0 * progress;
            let phase = 2.0 * PI * freq * t;
            let h1 = phase.sin();
            let h2 = 0.45 * (phase * 2.0).sin();
            let h3 = 0.25 * (phase * 3.0).sin();
            let h4 = 0.12 * (phase * 4.0).sin();
            let breath = 0.06 * lcg_random(&mut rng);
            let env = (PI * progress).sin().powf(0.85);
            let sample = (h1 + h2 + h3 + h4 + breath) * env * 0.75;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes bovine hurt grunt / strained moo.
    pub fn generate_cow_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 20202 + variant as u64 * 41414;
        let duration = 0.35;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 210.0 } else { 235.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = (base_f - progress * 90.0).max(80.0);
            let phase = 2.0 * PI * freq * t;
            let vocal = phase.sin() + 0.4 * (phase * 2.0).sin() + 0.2 * (phase * 3.0).sin();
            let rasp = 0.25 * lcg_random(&mut rng);
            let env = (-progress * 7.5).exp();
            let sample = (0.75 * vocal + 0.25 * rasp) * env * 0.85;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes pig oink / snort with throat flutter.
    pub fn generate_pig_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 30303 + variant as u64 * 51515;
        let duration = 0.28;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 230.0 } else { 260.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = (base_f - progress * 70.0).max(100.0);
            let flutter = 0.5 + 0.5 * (2.0 * PI * 26.0 * t).sin();
            let tone = (2.0 * PI * freq * t).sin() * flutter;
            let snort = lcg_random(&mut rng) * (-progress * 12.0).exp();
            let env = (PI * progress).sin().powf(0.7);
            let sample = (0.65 * tone + 0.35 * snort) * env * 0.8;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes high-pitched pig squeal of distress.
    pub fn generate_pig_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 40404 + variant as u64 * 61616;
        let duration = 0.38;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let start_f = if variant == 0 { 880.0 } else { 960.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = (start_f - progress * 280.0).max(350.0);
            let vibrato = 1.0 + 0.12 * (2.0 * PI * 34.0 * t).sin();
            let phase = 2.0 * PI * freq * vibrato * t;
            let tone = phase.sin() + 0.3 * (phase * 2.0).sin();
            let grit = 0.18 * lcg_random(&mut rng);
            let env = (-progress * 6.0).exp();
            let sample = (0.82 * tone + 0.18 * grit) * env * 0.9;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes chicken staccato clucks with beak clicks.
    pub fn generate_chicken_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 50505 + variant as u64 * 71717;
        let duration = 0.22;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let t1 = t;
            let t2 = t - 0.10;
            let pulse1 = if (0.0..0.08).contains(&t1) {
                let p = t1 / 0.08;
                (2.0 * PI * 440.0 * t1).sin() * (PI * p).sin()
            } else { 0.0 };
            let pulse2 = if (0.0..0.08).contains(&t2) {
                let p = t2 / 0.08;
                (2.0 * PI * 380.0 * t2).sin() * (PI * p).sin()
            } else { 0.0 };
            let click = lcg_random(&mut rng) * 0.15 * ((-t1 * 60.0).exp() + (-t2.max(0.0) * 60.0).exp());
            let sample = (pulse1 + pulse2 + click) * 0.75;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes fluttering chicken distress squawk.
    pub fn generate_chicken_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 60606 + variant as u64 * 81818;
        let duration = 0.26;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 640.0 } else { 720.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = base_f + 120.0 * (2.0 * PI * 42.0 * t).sin();
            let flutter = (2.0 * PI * freq * t).sin();
            let flap = lcg_random(&mut rng) * (-progress * 15.0).exp();
            let env = (-progress * 8.0).exp();
            let sample = (0.65 * flutter + 0.35 * flap) * env * 0.85;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes sheep "Baaaa" with vocal vibrato flutter.
    pub fn generate_sheep_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 70707 + variant as u64 * 91919;
        let duration = 0.75;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 215.0 } else { 240.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let tremolo = 0.75 + 0.25 * (2.0 * PI * 12.0 * t).sin();
            let freq = base_f + 8.0 * (2.0 * PI * 12.0 * t).sin();
            let phase = 2.0 * PI * freq * t;
            let vocal = phase.sin() + 0.45 * (phase * 2.0).sin() + 0.35 * (phase * 3.0).sin();
            let breath = 0.1 * lcg_random(&mut rng);
            let env = (PI * progress).sin().powf(0.8);
            let sample = (vocal * tremolo + breath) * env * 0.75;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes short strained sheep hurt grunt.
    pub fn generate_sheep_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 80808 + variant as u64 * 13131;
        let duration = 0.30;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 290.0 } else { 330.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = (base_f - progress * 100.0).max(120.0);
            let phase = 2.0 * PI * freq * t;
            let vocal = phase.sin() + 0.4 * (phase * 2.0).sin();
            let rasp = 0.25 * lcg_random(&mut rng);
            let env = (-progress * 9.0).exp();
            let sample = (0.75 * vocal + 0.25 * rasp) * env * 0.85;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes low guttural zombie moan / groan.
    pub fn generate_zombie_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 90909 + variant as u64 * 24242;
        let duration = 0.95;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 68.0 } else { 75.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let gargle = 0.6 + 0.4 * (2.0 * PI * 16.0 * t).sin();
            let freq = base_f - 10.0 * progress + 4.0 * (2.0 * PI * 5.0 * t).sin();
            let phase = 2.0 * PI * freq * t;
            let groan = (phase.sin() + 0.5 * (phase * 2.0).sin() + 0.3 * (phase * 3.0).sin()) * gargle;
            let rasp = 0.20 * lcg_random(&mut rng);
            let env = (PI * progress).sin().powf(0.85);
            let sample = (0.80 * groan + 0.20 * rasp) * env * 0.80;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes aggressive zombie hurt snarl.
    pub fn generate_zombie_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 112233 + variant as u64 * 35353;
        let duration = 0.36;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let base_f = if variant == 0 { 95.0 } else { 110.0 };

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let freq = (base_f - progress * 35.0).max(50.0);
            let growl = (2.0 * PI * freq * t).sin() * (0.5 + 0.5 * (2.0 * PI * 28.0 * t).sin());
            let snarl_noise = lcg_random(&mut rng) * (-progress * 9.0).exp();
            let env = (-progress * 7.5).exp();
            let sample = (0.55 * growl + 0.45 * snarl_noise) * env * 0.90;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes skeleton dry bone rattle / clattering clicks.
    pub fn generate_skeleton_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 334455 + variant as u64 * 46464;
        let duration = 0.36;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let click1 = if (0.0..0.04).contains(&t) {
                let dt = t;
                (2.0 * PI * 1800.0 * dt).sin() * (-dt * 120.0).exp()
            } else { 0.0 };
            let click2 = if (0.08..0.13).contains(&t) {
                let dt = t - 0.08;
                (2.0 * PI * 1450.0 * dt).sin() * (-dt * 110.0).exp()
            } else { 0.0 };
            let click3 = if (0.18..0.24).contains(&t) {
                let dt = t - 0.18;
                (2.0 * PI * 2200.0 * dt).sin() * (-dt * 130.0).exp()
            } else { 0.0 };
            let clatter = lcg_random(&mut rng) * 0.12 * (-t * 18.0).exp();
            let sample = (0.75 * (click1 + click2 + click3) + 0.25 * clatter) * 0.85;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes sharp skeleton bone fracture snap.
    pub fn generate_skeleton_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 556677 + variant as u64 * 57575;
        let duration = 0.28;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let crack = (2.0 * PI * 2600.0 * t).sin() * (-progress * 55.0).exp();
            let hollow_wood = (2.0 * PI * 580.0 * t).sin() * (-progress * 35.0).exp();
            let snap = lcg_random(&mut rng) * (-progress * 45.0).exp();
            let sample = (0.45 * crack + 0.35 * hollow_wood + 0.20 * snap) * 0.90;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes spider chitinous skitter / soft hiss with mandible clicks.
    pub fn generate_spider_ambient(variant: usize) -> Vec<f32> {
        let mut rng = 778899 + variant as u64 * 68686;
        let duration = 0.44;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let click_mod = 0.6 + 0.4 * (2.0 * PI * 32.0 * t).sin();
            let hiss = lcg_random(&mut rng) * (-progress * 8.0).exp() * click_mod;
            let mandible = (2.0 * PI * 1100.0 * t).sin() * (0.5 + 0.5 * (2.0 * PI * 32.0 * t).sin()) * (-progress * 12.0).exp();
            let sample = (0.65 * hiss + 0.35 * mandible) * 0.70;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes sharp spider screech / aggressive attack hiss.
    pub fn generate_spider_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 990011 + variant as u64 * 79797;
        let duration = 0.32;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;
            let sweep_f = 1750.0 - progress * 700.0;
            let screech = (2.0 * PI * sweep_f * t).sin() * (-progress * 10.0).exp();
            let harsh_noise = lcg_random(&mut rng) * (-progress * 14.0).exp();
            let sample = (0.55 * screech + 0.45 * harsh_noise) * 0.85;
            samples.push(sample.clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes water splash sound effect for entering or leaping out of water.
    pub fn generate_water_splash(variant: usize) -> Vec<f32> {
        let mut rng = 13371337 + (variant as u64 * 54321);
        let duration = 0.35;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let spray = lcg_random(&mut rng) * (-progress * 12.0).exp();
            let plunge_freq = (100.0 - progress * 50.0).max(40.0);
            let plunge = (2.0 * PI * plunge_freq * t).sin() * (-progress * 8.0).exp();
            let slap = (2.0 * PI * 240.0 * t).sin() * (-progress * 28.0).exp();

            let sample = 0.50 * spray + 0.35 * plunge + 0.15 * slap;
            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes water wading / swimming paddle sounds.
    pub fn generate_water_swim(variant: usize) -> Vec<f32> {
        let mut rng = 887766 + (variant as u64 * 33221);
        let duration = 0.22;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let paddle = (2.0 * PI * 75.0 * t).sin() * (-progress * 14.0).exp();
            let swish = lcg_random(&mut rng) * (progress * (1.0 - progress) * 4.0) * (-progress * 8.0).exp();

            let sample = 0.45 * paddle + 0.55 * swish;
            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes muffled underwater bubble murmurs.
    pub fn generate_underwater_bubble(variant: usize) -> Vec<f32> {
        let mut rng = 991122 + (variant as u64 * 44332);
        let duration = 0.30;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let bubble_freq = 320.0 + 550.0 * progress.powi(2);
            let bubble = (2.0 * PI * bubble_freq * t).sin() * (-progress * 14.0).exp();
            let murmur = (2.0 * PI * 55.0 * t).sin() * 0.4;
            let noise = lcg_random(&mut rng) * 0.10 * (-progress * 10.0).exp();

            let sample = 0.65 * bubble + 0.25 * murmur + 0.10 * noise;
            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes a tool breaking sound effect (crisp snap followed by crumbling shatter).
    pub fn generate_tool_break(variant: usize) -> Vec<f32> {
        let mut rng = 7711223344 + variant as u64 * 49287;
        let duration = 0.35;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let snap = (2.0 * PI * 580.0 * t).sin() * (-progress * 28.0).exp();
            let metal_ping = (2.0 * PI * 2400.0 * t).sin() * (-progress * 18.0).exp();
            let shatter = lcg_random(&mut rng) * (-progress * 14.0).exp();

            let sample = 0.55 * snap + 0.30 * metal_ping + 0.35 * shatter;
            samples.push(sample.clamp(-1.0, 1.0));
        }

        samples
    }

    /// Synthesizes a brisk fishing rod cast whoosh.
    pub fn generate_fishing_cast() -> Vec<f32> {
        let mut rng = 44556677u64;
        let duration = 0.24;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let p = t / duration;
            let env = (p * std::f32::consts::PI).sin().powf(0.8);
            let swish = lcg_random(&mut rng) * 0.45;
            let whip = (2.0 * PI * (280.0 + 750.0 * p) * t).sin() * 0.35;
            samples.push(((swish + whip) * env).clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes a crisp water plop / splash when the bobber lands or fish escapes.
    pub fn generate_fishing_splash() -> Vec<f32> {
        let mut rng = 88990011u64;
        let duration = 0.36;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let p = t / duration;
            let plop = (2.0 * PI * (220.0 + 480.0 * p) * t).sin() * (-p * 16.0).exp() * 0.55;
            let splash = lcg_random(&mut rng) * (-p * 9.0).exp() * 0.50;
            samples.push((plop + splash).clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes an urgent double-chime fish bite "!" alert.
    pub fn generate_fishing_bite() -> Vec<f32> {
        let duration = 0.28;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let note1 = if t < 0.14 {
                (2.0 * PI * 987.77 * t).sin() * (-t * 20.0).exp()
            } else {
                0.0
            };
            let note2 = if t >= 0.10 {
                let dt = t - 0.10;
                (2.0 * PI * 1318.51 * dt).sin() * (-dt * 18.0).exp()
            } else {
                0.0
            };
            samples.push(((note1 + note2) * 0.65).clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes a joyful rising 3-note fanfare when catching a fish.
    pub fn generate_fishing_catch() -> Vec<f32> {
        let duration = 0.55;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let notes = [(0.0, 523.25), (0.11, 659.25), (0.22, 783.99), (0.33, 1046.50)];

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let mut sum = 0.0;
            for &(start, freq) in &notes {
                if t >= start {
                    let dt = t - start;
                    let env = (1.0 - (-dt * 60.0).exp()) * (-dt * 8.0).exp();
                    sum += ((2.0 * PI * freq * dt).sin() * 0.7 + (2.0 * PI * freq * 2.0 * dt).sin() * 0.2) * env;
                }
            }
            samples.push((sum * 0.55).clamp(-1.0, 1.0));
        }
        samples
    }

    /// Synthesizes a bright celebratory chime when an Achievement is unlocked.
    pub fn generate_achievement_unlock() -> Vec<f32> {
        let duration = 0.75;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        // Rising golden triad + high shimmer: G5 -> B5 -> D6 -> G6
        let notes = [
            (0.00, 783.99_f32),
            (0.09, 987.77),
            (0.18, 1174.66),
            (0.28, 1567.98),
        ];

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let mut sum = 0.0;
            for &(start, freq) in &notes {
                if t >= start {
                    let dt = t - start;
                    let env = (1.0 - (-dt * 80.0).exp()) * (-dt * 6.5).exp();
                    let tone = (2.0 * PI * freq * dt).sin() * 0.65
                        + (2.0 * PI * freq * 2.0 * dt).sin() * 0.22
                        + (2.0 * PI * freq * 3.0 * dt).sin() * 0.08;
                    sum += tone * env;
                }
            }
            samples.push((sum * 0.50).clamp(-1.0, 1.0));
        }
        samples
    }
}

/// Global audio manager orchestrating 3D spatial playback, materials, and mixer device.
pub struct AudioManager {
    mixer: Arc<Mutex<AudioMixer>>,
    _device: Option<Device<f32>>,
    footsteps: HashMap<(SoundMaterial, usize), Arc<Vec<f32>>>,
    hits: HashMap<(SoundMaterial, usize), Arc<Vec<f32>>>,
    breaks: HashMap<(SoundMaterial, usize), Arc<Vec<f32>>>,
    places: HashMap<(SoundMaterial, usize), Arc<Vec<f32>>>,
    hurts: Vec<Arc<Vec<f32>>>,
    water_splashes: Vec<Arc<Vec<f32>>>,
    water_swims: Vec<Arc<Vec<f32>>>,
    underwater_bubbles: Vec<Arc<Vec<f32>>>,
    tool_breaks: Vec<Arc<Vec<f32>>>,
    fishing_cast: Arc<Vec<f32>>,
    fishing_splash: Arc<Vec<f32>>,
    fishing_bite: Arc<Vec<f32>>,
    fishing_catch: Arc<Vec<f32>>,
    achievement_unlock: Arc<Vec<f32>>,
    animal_ambient: HashMap<(blockgame_core::animal::AnimalKind, usize), Arc<Vec<f32>>>,
    animal_hurt: HashMap<(blockgame_core::animal::AnimalKind, usize), Arc<Vec<f32>>>,
    monster_ambient: HashMap<(blockgame_core::monster::MonsterKind, usize), Arc<Vec<f32>>>,
    monster_hurt: HashMap<(blockgame_core::monster::MonsterKind, usize), Arc<Vec<f32>>>,
    music_tracks: Vec<Arc<Vec<f32>>>,
    rng_state: u64,
}

impl AudioManager {
    /// Updates master, SFX, and music volume levels (`0.0..=1.0`).
    pub fn set_volumes(&self, master: f32, sfx: f32, music: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.master_volume = master.clamp(0.0, 1.0);
            m.sfx_volume = sfx.clamp(0.0, 1.0);
            m.music_volume = music.clamp(0.0, 1.0);
        }
    }

    /// Updates creature / mob sound volume (`0.0..=1.0`).
    pub fn set_creature_volume(&self, creature: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.creature_volume = creature.clamp(0.0, 1.0);
        }
    }

    /// Updates proximity voice chat playback volume (`0.0..=1.0`).
    pub fn set_voice_volume(&self, voice: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.voice_volume = voice.clamp(0.0, 1.0);
        }
    }

    /// Queues a decoded 16 kHz mono voice frame with 3D spatial attenuation and stereo panning.
    pub fn push_spatial_voice(
        &self,
        sender_id: u32,
        seq: u32,
        pcm: &[f32],
        sender_pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let (atten, pan) = calculate_spatial_audio(
            sender_pos,
            listener_pos,
            listener_yaw,
            blockgame_core::net::MAX_VOICE_RANGE,
        );
        if atten <= 0.001 {
            return;
        }
        let vol_l = atten * ((1.0 - pan) * 0.5).sqrt();
        let vol_r = atten * ((1.0 + pan) * 0.5).sqrt();
        if let Ok(mut m) = self.mixer.lock() {
            m.push_voice_samples(sender_id, seq, pcm, vol_l, vol_r);
        }
    }

    /// Initializes the audio manager and playback device.
    ///
    /// Gracefully falls back to silent operation if no audio hardware is found.
    pub fn new() -> Self {
        let mixer = Arc::new(Mutex::new(AudioMixer::new()));
        let mut device = None;

        let mixer_cb = mixer.clone();
        match DeviceBuilder::playback()
            .f32()
            .playback_channels(2)
            .sample_rate(SampleRate::Sr44100)
            .with_callback(move |_, out| {
                out.fill(0.0);
                if let Ok(mut m) = mixer_cb.lock() {
                    m.mix(out, 44100);
                }
            })
        {
            Ok(mut dev) => {
                if let Err(e) = dev.device_start() {
                    eprintln!("[Audio] Warning: Failed to start audio playback device: {:?}", e);
                } else {
                    println!("[Audio] Playback device successfully initialized at 44.1kHz.");
                    device = Some(dev);
                }
            }
            Err(e) => {
                eprintln!(
                    "[Audio] Notice: No audio playback device available ({:?}). Running in silent mode.",
                    e
                );
            }
        }

        let mut manager = Self {
            mixer,
            _device: device,
            footsteps: HashMap::new(),
            hits: HashMap::new(),
            breaks: HashMap::new(),
            places: HashMap::new(),
            hurts: Vec::new(),
            water_splashes: Vec::new(),
            water_swims: Vec::new(),
            underwater_bubbles: Vec::new(),
            tool_breaks: Vec::new(),
            fishing_cast: Arc::new(sound_generator::generate_fishing_cast()),
            fishing_splash: Arc::new(sound_generator::generate_fishing_splash()),
            fishing_bite: Arc::new(sound_generator::generate_fishing_bite()),
            fishing_catch: Arc::new(sound_generator::generate_fishing_catch()),
            achievement_unlock: Arc::new(sound_generator::generate_achievement_unlock()),
            animal_ambient: HashMap::new(),
            animal_hurt: HashMap::new(),
            monster_ambient: HashMap::new(),
            monster_hurt: HashMap::new(),
            music_tracks: Vec::new(),
            rng_state: 888888888,
        };

        manager.load_or_generate_assets();
        manager
    }

    fn random_f32(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng_state >> 33) as f32) / ((u32::MAX >> 1) as f32)
    }

    /// Preloads procedural sounds and checks optional `assets/sounds/` directory.
    fn load_or_generate_assets(&mut self) {
        let materials = [
            SoundMaterial::Grass,
            SoundMaterial::Dirt,
            SoundMaterial::Stone,
            SoundMaterial::Wood,
            SoundMaterial::Sand,
            SoundMaterial::Glass,
        ];

        let assets_dir = Path::new("assets/sounds");
        let has_assets = assets_dir.exists();

        // 1. Footsteps (3 variations per material)
        for &mat in &materials {
            for variant in 0..3 {
                let file_name = format!("footstep_{:?}_{}.wav", mat, variant).to_lowercase();
                let file_path = assets_dir.join(&file_name);

                let buffer = if has_assets && file_path.exists() {
                    Self::load_audio_file(&file_path)
                        .unwrap_or_else(|| sound_generator::generate_footstep(mat, variant))
                } else {
                    sound_generator::generate_footstep(mat, variant)
                };

                self.footsteps.insert((mat, variant), Arc::new(buffer));
            }
        }

        // 2. Block Break (2 variations per material)
        for &mat in &materials {
            for variant in 0..2 {
                let file_name = format!("break_{:?}_{}.wav", mat, variant).to_lowercase();
                let file_path = assets_dir.join(&file_name);

                let buffer = if has_assets && file_path.exists() {
                    Self::load_audio_file(&file_path)
                        .unwrap_or_else(|| sound_generator::generate_break(mat, variant))
                } else {
                    sound_generator::generate_break(mat, variant)
                };

                self.breaks.insert((mat, variant), Arc::new(buffer));
            }
        }

        // 3. Block Place (2 variations per material)
        for &mat in &materials {
            for variant in 0..2 {
                let file_name = format!("place_{:?}_{}.wav", mat, variant).to_lowercase();
                let file_path = assets_dir.join(&file_name);

                let buffer = if has_assets && file_path.exists() {
                    Self::load_audio_file(&file_path)
                        .unwrap_or_else(|| sound_generator::generate_place(mat, variant))
                } else {
                    sound_generator::generate_place(mat, variant)
                };

                self.places.insert((mat, variant), Arc::new(buffer));
            }
        }

        // 4. Block Hit / Mining Progress (2 variations per material)
        for &mat in &materials {
            for variant in 0..2 {
                let file_name = format!("hit_{:?}_{}.wav", mat, variant).to_lowercase();
                let file_path = assets_dir.join(&file_name);

                let buffer = if has_assets && file_path.exists() {
                    Self::load_audio_file(&file_path)
                        .unwrap_or_else(|| sound_generator::generate_hit(mat, variant))
                } else {
                    sound_generator::generate_hit(mat, variant)
                };

                self.hits.insert((mat, variant), Arc::new(buffer));
            }
        }

        // 5. Player Hurt (2 variations)
        for variant in 0..2 {
            let file_name = format!("player_hurt_{}.wav", variant);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_hurt(variant))
            } else {
                sound_generator::generate_hurt(variant)
            };

            self.hurts.push(Arc::new(buffer));
        }

        // 6. Water Splash (2 variations)
        for variant in 0..2 {
            let file_name = format!("water_splash_{}.wav", variant);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_water_splash(variant))
            } else {
                sound_generator::generate_water_splash(variant)
            };

            self.water_splashes.push(Arc::new(buffer));
        }

        // 7. Water Swim / Paddle (2 variations)
        for variant in 0..2 {
            let file_name = format!("water_swim_{}.wav", variant);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_water_swim(variant))
            } else {
                sound_generator::generate_water_swim(variant)
            };

            self.water_swims.push(Arc::new(buffer));
        }

        // 8. Underwater Ambience / Bubbles (2 variations)
        for variant in 0..2 {
            let file_name = format!("underwater_bubble_{}.wav", variant);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_underwater_bubble(variant))
            } else {
                sound_generator::generate_underwater_bubble(variant)
            };

            self.underwater_bubbles.push(Arc::new(buffer));
        }

        // 9. Tool Break (2 variations)
        for variant in 0..2 {
            let file_name = format!("tool_break_{}.wav", variant);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_tool_break(variant))
            } else {
                sound_generator::generate_tool_break(variant)
            };

            self.tool_breaks.push(Arc::new(buffer));
        }

        // 10. Custom Stereo Music Tracks (4 MP3 tracks: Adventuring, Mining, and 2 Anytime Background)
        let custom_music_dir = Path::new("assets/music");
        let root_music_dir = Path::new("Music");
        for (file_name, _label, embedded_bytes) in MUSIC_TRACK_DEFS {
            let assets_path = custom_music_dir.join(file_name);
            let root_path = root_music_dir.join(file_name);

            let buffer = if assets_path.exists() {
                Self::load_audio_file_stereo(&assets_path)
            } else if root_path.exists() {
                Self::load_audio_file_stereo(&root_path)
            } else {
                None
            }
            .or_else(|| Self::load_audio_bytes_stereo(embedded_bytes))
            .unwrap_or_else(|| {
                eprintln!("[Audio] Warning: Failed to decode music track {}", file_name);
                Vec::new()
            });

            self.music_tracks.push(Arc::new(buffer));
        }

        // 11. Animal Sounds (Cow, Pig, Chicken, Sheep - ambient & hurt)
        let animal_kinds = [
            blockgame_core::animal::AnimalKind::Cow,
            blockgame_core::animal::AnimalKind::Pig,
            blockgame_core::animal::AnimalKind::Chicken,
            blockgame_core::animal::AnimalKind::Sheep,
        ];
        for &kind in &animal_kinds {
            for variant in 0..2 {
                let amb_name = format!("animal_{:?}_ambient_{}.wav", kind, variant).to_lowercase();
                let amb_path = assets_dir.join(&amb_name);
                let amb_buffer = if has_assets && amb_path.exists() {
                    Self::load_audio_file(&amb_path).unwrap_or_else(|| match kind {
                        blockgame_core::animal::AnimalKind::Cow => sound_generator::generate_cow_ambient(variant),
                        blockgame_core::animal::AnimalKind::Pig => sound_generator::generate_pig_ambient(variant),
                        blockgame_core::animal::AnimalKind::Chicken => sound_generator::generate_chicken_ambient(variant),
                        blockgame_core::animal::AnimalKind::Sheep => sound_generator::generate_sheep_ambient(variant),
                    })
                } else {
                    match kind {
                        blockgame_core::animal::AnimalKind::Cow => sound_generator::generate_cow_ambient(variant),
                        blockgame_core::animal::AnimalKind::Pig => sound_generator::generate_pig_ambient(variant),
                        blockgame_core::animal::AnimalKind::Chicken => sound_generator::generate_chicken_ambient(variant),
                        blockgame_core::animal::AnimalKind::Sheep => sound_generator::generate_sheep_ambient(variant),
                    }
                };
                self.animal_ambient.insert((kind, variant), Arc::new(amb_buffer));

                let hurt_name = format!("animal_{:?}_hurt_{}.wav", kind, variant).to_lowercase();
                let hurt_path = assets_dir.join(&hurt_name);
                let hurt_buffer = if has_assets && hurt_path.exists() {
                    Self::load_audio_file(&hurt_path).unwrap_or_else(|| match kind {
                        blockgame_core::animal::AnimalKind::Cow => sound_generator::generate_cow_hurt(variant),
                        blockgame_core::animal::AnimalKind::Pig => sound_generator::generate_pig_hurt(variant),
                        blockgame_core::animal::AnimalKind::Chicken => sound_generator::generate_chicken_hurt(variant),
                        blockgame_core::animal::AnimalKind::Sheep => sound_generator::generate_sheep_hurt(variant),
                    })
                } else {
                    match kind {
                        blockgame_core::animal::AnimalKind::Cow => sound_generator::generate_cow_hurt(variant),
                        blockgame_core::animal::AnimalKind::Pig => sound_generator::generate_pig_hurt(variant),
                        blockgame_core::animal::AnimalKind::Chicken => sound_generator::generate_chicken_hurt(variant),
                        blockgame_core::animal::AnimalKind::Sheep => sound_generator::generate_sheep_hurt(variant),
                    }
                };
                self.animal_hurt.insert((kind, variant), Arc::new(hurt_buffer));
            }
        }

        // 12. Monster Sounds (Zombie, Skeleton, Spider - ambient & hurt)
        let monster_kinds = [
            blockgame_core::monster::MonsterKind::Zombie,
            blockgame_core::monster::MonsterKind::Skeleton,
            blockgame_core::monster::MonsterKind::Spider,
        ];
        for &kind in &monster_kinds {
            for variant in 0..2 {
                let amb_name = format!("monster_{:?}_ambient_{}.wav", kind, variant).to_lowercase();
                let amb_path = assets_dir.join(&amb_name);
                let amb_buffer = if has_assets && amb_path.exists() {
                    Self::load_audio_file(&amb_path).unwrap_or_else(|| match kind {
                        blockgame_core::monster::MonsterKind::Skeleton => sound_generator::generate_skeleton_ambient(variant),
                        blockgame_core::monster::MonsterKind::Spider => sound_generator::generate_spider_ambient(variant),
                        _ => sound_generator::generate_zombie_ambient(variant),
                    })
                } else {
                    match kind {
                        blockgame_core::monster::MonsterKind::Skeleton => sound_generator::generate_skeleton_ambient(variant),
                        blockgame_core::monster::MonsterKind::Spider => sound_generator::generate_spider_ambient(variant),
                        _ => sound_generator::generate_zombie_ambient(variant),
                    }
                };
                self.monster_ambient.insert((kind, variant), Arc::new(amb_buffer));

                let hurt_name = format!("monster_{:?}_hurt_{}.wav", kind, variant).to_lowercase();
                let hurt_path = assets_dir.join(&hurt_name);
                let hurt_buffer = if has_assets && hurt_path.exists() {
                    Self::load_audio_file(&hurt_path).unwrap_or_else(|| match kind {
                        blockgame_core::monster::MonsterKind::Skeleton => sound_generator::generate_skeleton_hurt(variant),
                        blockgame_core::monster::MonsterKind::Spider => sound_generator::generate_spider_hurt(variant),
                        _ => sound_generator::generate_zombie_hurt(variant),
                    })
                } else {
                    match kind {
                        blockgame_core::monster::MonsterKind::Skeleton => sound_generator::generate_skeleton_hurt(variant),
                        blockgame_core::monster::MonsterKind::Spider => sound_generator::generate_spider_hurt(variant),
                        _ => sound_generator::generate_zombie_hurt(variant),
                    }
                };
                self.monster_hurt.insert((kind, variant), Arc::new(hurt_buffer));
            }
        }

        println!(
            "[Audio] Initialized {} footstep, {} hit, {} break, {} place, {} hurt, {} splash, {} swim, {} bubble, {} tool break, {} animal, {} monster, and {} music tracks.",
            self.footsteps.len(),
            self.hits.len(),
            self.breaks.len(),
            self.places.len(),
            self.hurts.len(),
            self.water_splashes.len(),
            self.water_swims.len(),
            self.underwater_bubbles.len(),
            self.tool_breaks.len(),
            self.animal_ambient.len() + self.animal_hurt.len(),
            self.monster_ambient.len() + self.monster_hurt.len(),
            self.music_tracks.len()
        );
    }

    /// Attempts to decode an external audio file into 44.1kHz mono PCM f32 samples.
    fn load_audio_file(path: &Path) -> Option<Vec<f32>> {
        let mut decoder = DecoderBuilder::new_f32()
            .channels(1)
            .sample_rate(SampleRate::Sr44100)
            .from_file(path)
            .ok()?;

        let mut samples = Vec::new();
        let mut frame_buf = [0.0_f32; 1024];

        loop {
            let frames_read = decoder.read_pcm_frames_into(&mut frame_buf).unwrap_or(0);
            if frames_read == 0 {
                break;
            }
            samples.extend_from_slice(&frame_buf[0..frames_read]);
        }

        if !samples.is_empty() {
            Some(samples)
        } else {
            None
        }
    }

    /// Decodes an external audio file into 44.1kHz interleaved stereo PCM f32 samples.
    fn load_audio_file_stereo(path: &Path) -> Option<Vec<f32>> {
        let mut decoder = DecoderBuilder::new_f32()
            .channels(2)
            .sample_rate(SampleRate::Sr44100)
            .from_file(path)
            .ok()?;

        let mut samples = Vec::new();
        let mut frame_buf = [0.0_f32; 2048];

        loop {
            let frames_read = decoder.read_pcm_frames_into(&mut frame_buf).unwrap_or(0);
            if frames_read == 0 {
                break;
            }
            samples.extend_from_slice(&frame_buf[0..(frames_read * 2)]);
        }

        if !samples.is_empty() {
            Some(samples)
        } else {
            None
        }
    }

    /// Decodes embedded audio bytes (e.g. MP3) into 44.1kHz interleaved stereo PCM f32 samples.
    fn load_audio_bytes_stereo(bytes: &[u8]) -> Option<Vec<f32>> {
        let mut decoder = DecoderBuilder::new_f32()
            .channels(2)
            .sample_rate(SampleRate::Sr44100)
            .from_memory(bytes)
            .ok()?;

        let mut samples = Vec::new();
        let mut frame_buf = [0.0_f32; 2048];

        loop {
            let frames_read = decoder.read_pcm_frames_into(&mut frame_buf).unwrap_or(0);
            if frames_read == 0 {
                break;
            }
            samples.extend_from_slice(&frame_buf[0..(frames_read * 2)]);
        }

        if !samples.is_empty() {
            Some(samples)
        } else {
            None
        }
    }

    /// Triggers a 3D spatial sound effect when a block is broken.
    pub fn play_block_break(
        &mut self,
        block: BlockId,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let mat = SoundMaterial::from_block(block);
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;

        if let Some(samples) = self.breaks.get(&(mat, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 32.0);
            if atten > 0.001 {
                // Subtle pitch variation (0.85 to 1.15) & volume variation (0.85 to 1.05)
                let pitch = 0.85 + 0.30 * self.random_f32();
                let vol_var = 0.85 + 0.20 * self.random_f32();
                let base_vol = atten * vol_var * 0.9;

                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Sfx, 0.0);
                }
            }
        }
    }

    /// Triggers a 3D spatial sound effect when a block is placed.
    pub fn play_block_place(
        &mut self,
        block: BlockId,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let mat = SoundMaterial::from_block(block);
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;

        if let Some(samples) = self.places.get(&(mat, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 32.0);
            if atten > 0.001 {
                let pitch = 0.85 + 0.30 * self.random_f32();
                let vol_var = 0.85 + 0.20 * self.random_f32();
                let base_vol = atten * vol_var * 0.85;

                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Sfx, 0.0);
                }
            }
        }
    }

    /// Triggers dynamic footstep audio based on the surface material beneath the player.
    pub fn play_footstep(&mut self, material: SoundMaterial, volume_scale: f32) {
        let variant = (self.random_f32() * 3.0).floor() as usize % 3;

        if let Some(samples) = self.footsteps.get(&(material, variant)).cloned() {
            let pitch = 0.88 + 0.24 * self.random_f32();
            let vol_var = 0.90 + 0.15 * self.random_f32();
            let vol = volume_scale * vol_var * 0.7;

            // Footsteps are centered on the listener (pan = 0.0)
            if let Ok(mut mixer) = self.mixer.lock() {
                mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
            }
        }
    }

    /// Triggers a 3D spatial hit/tapping sound during block breaking progress.
    pub fn play_block_hit(
        &mut self,
        block: BlockId,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let mat = SoundMaterial::from_block(block);
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;

        if let Some(samples) = self.hits.get(&(mat, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 28.0);
            if atten > 0.001 {
                let pitch = 0.90 + 0.20 * self.random_f32();
                let vol_var = 0.85 + 0.15 * self.random_f32();
                let base_vol = atten * vol_var * 0.65;

                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Sfx, 0.0);
                }
            }
        }
    }

    /// Triggers player damage sound effect ("oof"grunt).
    pub fn play_hurt(&mut self) {
        if self.hurts.is_empty() {
            return;
        }
        let variant = (self.random_f32() * self.hurts.len() as f32).floor() as usize
            % self.hurts.len();
        let samples = self.hurts[variant].clone();
        let pitch = 0.90 + 0.20 * self.random_f32();
        let vol = 0.85 + 0.15 * self.random_f32();

        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
        }
    }

    /// Triggers water splash sound when entering or jumping out of water.
    pub fn play_water_splash(&mut self, volume_scale: f32) {
        if self.water_splashes.is_empty() {
            return;
        }
        let variant = (self.random_f32() * self.water_splashes.len() as f32).floor() as usize
            % self.water_splashes.len();
        let samples = self.water_splashes[variant].clone();
        let pitch = 0.88 + 0.24 * self.random_f32();
        let vol = volume_scale * (0.85 + 0.15 * self.random_f32());

        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
        }
    }

    /// Triggers water wading / swimming paddle sound while moving through water.
    pub fn play_water_swim(&mut self, volume_scale: f32) {
        if self.water_swims.is_empty() {
            return;
        }
        let variant = (self.random_f32() * self.water_swims.len() as f32).floor() as usize
            % self.water_swims.len();
        let samples = self.water_swims[variant].clone();
        let pitch = 0.90 + 0.20 * self.random_f32();
        let vol = volume_scale * (0.85 + 0.15 * self.random_f32());

        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
        }
    }

    /// Triggers subtle underwater bubbling / murmur sound while submerged.
    pub fn play_underwater_ambience(&mut self) {
        if self.underwater_bubbles.is_empty() {
            return;
        }
        let variant = (self.random_f32() * self.underwater_bubbles.len() as f32).floor() as usize
            % self.underwater_bubbles.len();
        let samples = self.underwater_bubbles[variant].clone();
        let pitch = 0.95 + 0.10 * self.random_f32();
        let vol = 0.40 * (0.85 + 0.15 * self.random_f32());

        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
        }
    }

    /// Triggers a tool break sound effect when a tool's durability reaches zero.
    pub fn play_tool_break(&mut self) {
        if self.tool_breaks.is_empty() {
            return;
        }
        let variant = (self.random_f32() * self.tool_breaks.len() as f32).floor() as usize
            % self.tool_breaks.len();
        let samples = self.tool_breaks[variant].clone();
        let pitch = 0.90 + 0.20 * self.random_f32();
        let vol = 0.90 + 0.15 * self.random_f32();

        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays a short crisp crunch sound while eating food or sipping Herbal Tea.
    pub fn play_eat_crunch(&mut self) {
        const SAMPLE_RATE: f32 = 44100.0;
        const LEN: usize = 3600; // ~82ms crisp crunch
        let mut buf = Vec::with_capacity(LEN);
        let mut rng: u32 = (self.random_f32().to_bits() ^ 0xA3C5_9E11).max(1);
        let mut lp = 0.0_f32;
        for i in 0..LEN {
            let t = i as f32 / LEN as f32;
            let env = (1.0 - t).powf(1.8) * (t * 18.0).min(1.0);
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = ((rng as f32) / (u32::MAX as f32)) * 2.0 - 1.0;
            // Band-shaped crunch noise + subtle bite thump
            lp += (noise - lp) * 0.35;
            let thump = (i as f32 / SAMPLE_RATE * std::f32::consts::TAU * 210.0).sin() * 0.25;
            buf.push((lp * 0.75 + thump) * env * 0.65);
        }
        let samples = std::sync::Arc::new(buf);
        let pitch = 0.88 + 0.24 * self.random_f32();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, 0.42, 0.42, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays the fishing rod cast whoosh sound.
    pub fn play_fishing_cast(&mut self) {
        let samples = self.fishing_cast.clone();
        let pitch = 0.94 + 0.12 * self.random_f32();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, 0.45, 0.45, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays a water splash sound when the bobber lands or fish escapes.
    pub fn play_fishing_splash(&mut self) {
        let samples = self.fishing_splash.clone();
        let pitch = 0.92 + 0.16 * self.random_f32();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, pitch, 0.48, 0.48, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays the urgent "!" alert chime when a fish bites.
    pub fn play_fishing_bite(&mut self) {
        let samples = self.fishing_bite.clone();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, 1.0, 0.55, 0.55, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays the celebratory fanfare when catching a fish.
    pub fn play_fishing_catch(&mut self) {
        let samples = self.fishing_catch.clone();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, 1.0, 0.55, 0.55, SoundCategory::Sfx, 0.0);
        }
    }

    /// Plays the celebratory chime when unlocking an Achievement.
    pub fn play_achievement_unlock(&mut self) {
        let samples = self.achievement_unlock.clone();
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play_voice(samples, 1.0, 0.60, 0.60, SoundCategory::Sfx, 0.0);
        }
    }

    /// Triggers a 3D spatial ambient animal sound (moo, oink, cluck, baa).
    pub fn play_animal_ambient(
        &mut self,
        kind: blockgame_core::animal::AnimalKind,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;
        if let Some(samples) = self.animal_ambient.get(&(kind, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 24.0);
            if atten > 0.001 {
                let pitch = 0.92 + 0.16 * self.random_f32();
                let vol_var = 0.90 + 0.15 * self.random_f32();
                let base_vol = atten * vol_var * 0.95;
                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Creature, 0.0);
                }
            }
        }
    }

    /// Triggers a 3D spatial animal hurt sound effect.
    pub fn play_animal_hurt(
        &mut self,
        kind: blockgame_core::animal::AnimalKind,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;
        if let Some(samples) = self.animal_hurt.get(&(kind, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 28.0);
            if atten > 0.001 {
                let pitch = 0.95 + 0.15 * self.random_f32();
                let base_vol = atten * 1.0;
                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Creature, 0.0);
                }
            }
        }
    }

    /// Triggers a 3D spatial ambient monster sound (zombie groan, skeleton rattle, spider hiss).
    pub fn play_monster_ambient(
        &mut self,
        kind: blockgame_core::monster::MonsterKind,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;
        if let Some(samples) = self.monster_ambient.get(&(kind, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 26.0);
            if atten > 0.001 {
                let pitch = 0.92 + 0.16 * self.random_f32();
                let vol_var = 0.90 + 0.15 * self.random_f32();
                let base_vol = atten * vol_var * 0.95;
                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Creature, 0.0);
                }
            }
        }
    }

    /// Triggers a 3D spatial monster hurt sound effect.
    pub fn play_monster_hurt(
        &mut self,
        kind: blockgame_core::monster::MonsterKind,
        pos: Vec3,
        listener_pos: Vec3,
        listener_yaw: f32,
    ) {
        use blockgame_core::monster::MonsterKind;
        let variant = (self.random_f32() * 2.0).floor() as usize % 2;
        let sound_kind = match kind {
            MonsterKind::Zombie
            | MonsterKind::SandMummy
            | MonsterKind::OvergrownGolem
            | MonsterKind::FrostTitan => MonsterKind::Zombie,
            MonsterKind::Skeleton
            | MonsterKind::SunArcher
            | MonsterKind::FrostWraith
            | MonsterKind::ShadowKnight
            | MonsterKind::SunPharaoh
            | MonsterKind::ShadowLich => MonsterKind::Skeleton,
            MonsterKind::Spider | MonsterKind::JungleStalker => MonsterKind::Spider,
        };
        if let Some(samples) = self.monster_hurt.get(&(sound_kind, variant)).cloned() {
            let (atten, pan) = calculate_spatial_audio(pos, listener_pos, listener_yaw, 36.0);
            if atten > 0.001 {
                let pitch = if kind.is_boss() {
                    0.62 + 0.10 * self.random_f32()
                } else {
                    0.95 + 0.15 * self.random_f32()
                };
                let base_vol = atten * if kind.is_boss() { 1.2 } else { 1.0 };
                let vol_l = ((1.0 - pan) * 0.5) * base_vol;
                let vol_r = ((1.0 + pan) * 0.5) * base_vol;

                if let Ok(mut mixer) = self.mixer.lock() {
                    mixer.play_voice(samples, pitch, vol_l, vol_r, SoundCategory::Creature, 0.0);
                }
            }
        }
    }

    /// Plays a stereo background music track with smooth fade-in.
    pub fn play_music(&mut self, track_idx: usize, fade_in_sec: f32) {
        if track_idx < self.music_tracks.len() {
            let samples = self.music_tracks[track_idx].clone();
            if samples.is_empty() {
                return;
            }
            if let Ok(mut mixer) = self.mixer.lock() {
                mixer.stop_music(1.5);
                mixer.play_stereo_voice(samples, 0.85, 0.85, SoundCategory::Music, fade_in_sec);
            }
        }
    }

    /// Fades out active background music.
    pub fn stop_music(&mut self, fade_out_sec: f32) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.stop_music(fade_out_sec);
        }
    }

    /// Returns whether background music is actively playing.
    pub fn is_music_playing(&self) -> bool {
        if let Ok(mixer) = self.mixer.lock() {
            mixer.is_music_playing()
        } else {
            false
        }
    }

    /// Returns the number of loaded ambient music tracks.
    pub fn music_track_count(&self) -> usize {
        self.music_tracks.len()
    }
}

/// Contextual environmental state for dynamic music selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct MusicContext {
    pub is_underground: bool,
    pub is_adventuring: bool,
    pub is_mining: bool,
}

/// Intermittent Background Music Scheduler.
///
/// Track layout:
/// - `0` (`Where_Maps_End`): triggered when running through the world adventuring.
/// - `1` (`Measure_of_the_Dark`): triggered while mining.
/// - `2` (`Where_the_Roots_Break_Soil`) & `3` (`The_View_From_The_Ridge`): background tracks that can play anytime.
pub struct MusicScheduler {
    cooldown_timer: f32,
    context_rest_timer: f32,
    adventure_hold_sec: f32,
    mining_hold_sec: f32,
    is_playing: bool,
    last_played_track: Option<usize>,
    total_tracks: usize,
    rng_state: u64,
}

impl MusicScheduler {
    /// Constructs a new music scheduler.
    pub fn new(total_tracks: usize) -> Self {
        Self {
            // Friendly initial cooldown of 12 seconds after launching game
            cooldown_timer: 12.0,
            context_rest_timer: 3.0,
            adventure_hold_sec: 0.0,
            mining_hold_sec: 0.0,
            is_playing: false,
            last_played_track: None,
            total_tracks,
            rng_state: 777123456,
        }
    }

    pub fn random_f32(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        ((self.rng_state >> 33) as f32) / ((u32::MAX >> 1) as f32)
    }

    /// Computes a randomized silence cooldown between `min_sec` (180s) and `max_sec` (360s).
    fn next_cooldown(&mut self) -> f32 {
        let min_sec = 180.0_f32; // 3 minutes
        let max_sec = 360.0_f32; // 6 minutes
        min_sec + self.random_f32() * (max_sec - min_sec)
    }

    /// Selects an anytime background track (`Where_the_Roots_Break_Soil` or `The_View_From_The_Ridge`)
    /// ensuring it doesn't immediately repeat the previous track.
    pub fn select_next_track(&mut self) -> usize {
        if self.total_tracks <= 1 {
            return 0;
        }

        if self.total_tracks >= 4 {
            let mut pool_idx = (self.random_f32() * BACKGROUND_MUSIC_TRACKS.len() as f32).floor()
                as usize
                % BACKGROUND_MUSIC_TRACKS.len();
            let mut candidate = BACKGROUND_MUSIC_TRACKS[pool_idx];

            if let Some(prev) = self.last_played_track {
                if candidate == prev {
                    pool_idx = (pool_idx + 1) % BACKGROUND_MUSIC_TRACKS.len();
                    candidate = BACKGROUND_MUSIC_TRACKS[pool_idx];
                }
            }
            return candidate;
        }

        let mut candidate = (self.random_f32() * self.total_tracks as f32).floor() as usize
            % self.total_tracks;

        if let Some(prev) = self.last_played_track {
            if candidate == prev {
                candidate = (candidate + 1) % self.total_tracks;
            }
        }

        candidate
    }

    /// Selects the next track taking player activity into account:
    /// - Mining -> `Measure_of_the_Dark`
    /// - Running/Adventuring -> `Where_Maps_End`
    /// - Otherwise -> anytime background pool (`Where_the_Roots_Break_Soil` / `The_View_From_The_Ridge`)
    pub fn select_next_track_contextual(&mut self, ctx: MusicContext) -> usize {
        if self.total_tracks <= 1 {
            return 0;
        }

        if ctx.is_mining && self.total_tracks > MUSIC_TRACK_MEASURE_OF_THE_DARK {
            if self.last_played_track != Some(MUSIC_TRACK_MEASURE_OF_THE_DARK) {
                return MUSIC_TRACK_MEASURE_OF_THE_DARK;
            }
        } else if ctx.is_adventuring && self.total_tracks > MUSIC_TRACK_WHERE_MAPS_END {
            if self.last_played_track != Some(MUSIC_TRACK_WHERE_MAPS_END) {
                return MUSIC_TRACK_WHERE_MAPS_END;
            }
        }

        self.select_next_track()
    }

    /// Updates the music scheduler each frame with environmental context.
    pub fn update(&mut self, dt: f32, audio: &mut AudioManager, ctx: MusicContext) {
        if self.total_tracks == 0 {
            self.total_tracks = audio.music_track_count();
            if self.total_tracks == 0 {
                return;
            }
        }

        if self.is_playing {
            // Track is playing: check if it finished
            if !audio.is_music_playing() {
                self.is_playing = false;
                self.cooldown_timer = self.next_cooldown();
                self.context_rest_timer = 45.0;
                self.adventure_hold_sec = 0.0;
                self.mining_hold_sec = 0.0;
                println!(
                    "[MusicScheduler] Track completed. Silence cooldown started: {:.1} seconds ({:.1} min).",
                    self.cooldown_timer,
                    self.cooldown_timer / 60.0
                );
            }
        } else {
            // In silence cooldown period
            self.cooldown_timer -= dt;
            self.context_rest_timer = (self.context_rest_timer - dt).max(0.0);

            if ctx.is_mining {
                self.mining_hold_sec += dt;
            } else {
                self.mining_hold_sec = (self.mining_hold_sec - dt * 2.0).max(0.0);
            }

            if ctx.is_adventuring {
                self.adventure_hold_sec += dt;
            } else {
                self.adventure_hold_sec = (self.adventure_hold_sec - dt * 2.0).max(0.0);
            }

            let can_trigger_mining = ctx.is_mining
                && self.mining_hold_sec >= 2.5
                && self.context_rest_timer <= 0.0
                && self.last_played_track != Some(MUSIC_TRACK_MEASURE_OF_THE_DARK);

            let can_trigger_adventuring = ctx.is_adventuring
                && self.adventure_hold_sec >= 3.5
                && self.context_rest_timer <= 0.0
                && self.last_played_track != Some(MUSIC_TRACK_WHERE_MAPS_END);

            if self.cooldown_timer <= 0.0 || can_trigger_mining || can_trigger_adventuring {
                let track = self.select_next_track_contextual(ctx);
                self.last_played_track = Some(track);
                self.is_playing = true;
                self.adventure_hold_sec = 0.0;
                self.mining_hold_sec = 0.0;
                let track_desc = MUSIC_TRACK_DEFS
                    .get(track)
                    .map(|(_, label, _)| *label)
                    .unwrap_or("Background");
                println!(
                    "[MusicScheduler] Starting track #{} [{}] with 2.5s fade-in.",
                    track + 1,
                    track_desc
                );
                audio.play_music(track, 2.5);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_material_mapping() {
        assert_eq!(SoundMaterial::from_block(BlockId::GRASS), SoundMaterial::Grass);
        assert_eq!(SoundMaterial::from_block(BlockId::DIRT), SoundMaterial::Dirt);
        assert_eq!(SoundMaterial::from_block(BlockId::STONE), SoundMaterial::Stone);
        assert_eq!(SoundMaterial::from_block(BlockId::WOOD_LOG), SoundMaterial::Wood);
        assert_eq!(SoundMaterial::from_block(BlockId::SAND), SoundMaterial::Sand);
        assert_eq!(SoundMaterial::from_block(BlockId::GLASS), SoundMaterial::Glass);
    }

    #[test]
    fn test_spatial_audio_calculation() {
        let listener = Vec3::new(0.0, 0.0, 0.0);
        let yaw = 0.0; // Facing +X, right is (0, 0, 1)

        // Sound to the right
        let sound_right = Vec3::new(0.0, 0.0, 5.0);
        let (atten, pan) = calculate_spatial_audio(sound_right, listener, yaw, 32.0);
        assert!(atten > 0.0 && atten <= 1.0);
        assert!(pan > 0.8, "Sound to the right should have positive pan: pan={}", pan);

        // Sound to the left
        let sound_left = Vec3::new(0.0, 0.0, -5.0);
        let (_atten, pan_l) = calculate_spatial_audio(sound_left, listener, yaw, 32.0);
        assert!(pan_l < -0.8, "Sound to the left should have negative pan: pan={}", pan_l);

        // Sound out of range
        let sound_far = Vec3::new(100.0, 0.0, 0.0);
        let (atten_far, _) = calculate_spatial_audio(sound_far, listener, yaw, 32.0);
        assert_eq!(atten_far, 0.0);
    }

    #[test]
    fn test_music_scheduler_non_repeating() {
        let mut scheduler = MusicScheduler::new(4);
        let mut prev = None;
        for _ in 0..25 {
            let next = scheduler.select_next_track();
            assert!(
                BACKGROUND_MUSIC_TRACKS.contains(&next),
                "Anytime background pool should pick Roots or Ridge, got {}",
                next
            );
            if let Some(p) = prev {
                assert_ne!(next, p, "Next track should not repeat previous track immediately");
            }
            prev = Some(next);
            scheduler.last_played_track = Some(next);
        }
    }

    #[test]
    fn test_music_scheduler_contextual() {
        let mut scheduler = MusicScheduler::new(4);
        let adventure_ctx = MusicContext {
            is_underground: false,
            is_adventuring: true,
            is_mining: false,
        };
        let track = scheduler.select_next_track_contextual(adventure_ctx);
        assert_eq!(
            track, MUSIC_TRACK_WHERE_MAPS_END,
            "Adventuring should select Where_Maps_End"
        );

        let mining_ctx = MusicContext {
            is_underground: true,
            is_adventuring: false,
            is_mining: true,
        };
        let track2 = scheduler.select_next_track_contextual(mining_ctx);
        assert_eq!(
            track2, MUSIC_TRACK_MEASURE_OF_THE_DARK,
            "Mining should select Measure_of_the_Dark"
        );

        // Surface mining should also trigger Measure_of_the_Dark
        let surface_mining_ctx = MusicContext {
            is_underground: false,
            is_adventuring: false,
            is_mining: true,
        };
        assert_eq!(
            scheduler.select_next_track_contextual(surface_mining_ctx),
            MUSIC_TRACK_MEASURE_OF_THE_DARK
        );

        // Idle background context should pick from anytime background tracks
        let idle_ctx = MusicContext::default();
        let bg = scheduler.select_next_track_contextual(idle_ctx);
        assert!(BACKGROUND_MUSIC_TRACKS.contains(&bg));
    }

    #[test]
    fn test_custom_music_tracks_decode() {
        for (name, _label, bytes) in MUSIC_TRACK_DEFS {
            let samples = AudioManager::load_audio_bytes_stereo(bytes)
                .unwrap_or_else(|| panic!("Failed to decode embedded MP3 track: {}", name));
            assert_eq!(
                samples.len() % 2,
                0,
                "Decoded stereo buffer for {} must have even interleaved length",
                name
            );
            let frames = samples.len() / 2;
            let duration_sec = frames as f32 / 44_100.0;
            assert!(
                duration_sec > 30.0,
                "Expected full-length music track for {}, got {:.1}s",
                name,
                duration_sec
            );
            let peak = samples.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
            assert!(
                peak > 0.01 && peak.is_finite(),
                "Expected audible signal for {}, peak={}",
                name,
                peak
            );
        }
    }

    #[test]
    fn test_stereo_mixer_playback() {
        let mut mixer = AudioMixer::new();
        let samples = Arc::new(vec![0.5, -0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5]);
        mixer.play_stereo_voice(samples, 1.0, 1.0, SoundCategory::Music, 0.0);
        assert!(mixer.is_music_playing());
        let mut out = [0.0_f32; 4];
        mixer.mix(&mut out, 44100);
        assert!(out[0] > 0.0 && out[1] < 0.0, "Stereo L/R channels should remain distinct");
    }

    #[test]
    fn test_music_scheduler_contextual_triggers() {
        let mut audio = AudioManager::new();
        assert_eq!(audio.music_track_count(), 4);

        // 1. Sustained mining triggers Measure_of_the_Dark
        let mut sched = MusicScheduler::new(audio.music_track_count());
        let mining_ctx = MusicContext {
            is_underground: true,
            is_adventuring: false,
            is_mining: true,
        };
        for _ in 0..4 {
            sched.update(1.0, &mut audio, mining_ctx);
        }
        assert!(sched.is_playing);
        assert_eq!(sched.last_played_track, Some(MUSIC_TRACK_MEASURE_OF_THE_DARK));
        audio.stop_music(0.0);

        // 2. Sustained surface adventuring triggers Where_Maps_End
        let mut sched2 = MusicScheduler::new(audio.music_track_count());
        let adv_ctx = MusicContext {
            is_underground: false,
            is_adventuring: true,
            is_mining: false,
        };
        for _ in 0..4 {
            sched2.update(1.0, &mut audio, adv_ctx);
        }
        assert!(sched2.is_playing);
        assert_eq!(sched2.last_played_track, Some(MUSIC_TRACK_WHERE_MAPS_END));
        audio.stop_music(0.0);

        // 3. Idle background cooldown picks from Where_the_Roots_Break_Soil / The_View_From_The_Ridge
        let mut sched3 = MusicScheduler::new(audio.music_track_count());
        for _ in 0..13 {
            sched3.update(1.0, &mut audio, MusicContext::default());
        }
        assert!(sched3.is_playing);
        assert!(BACKGROUND_MUSIC_TRACKS.contains(&sched3.last_played_track.unwrap()));
    }

    #[test]
    fn test_new_sound_generators() {
        let hit = sound_generator::generate_hit(SoundMaterial::Stone, 0);
        assert!(!hit.is_empty());
        let hurt = sound_generator::generate_hurt(0);
        assert!(!hurt.is_empty());
        let splash = sound_generator::generate_water_splash(0);
        assert!(!splash.is_empty());
        let swim = sound_generator::generate_water_swim(0);
        assert!(!swim.is_empty());
        let bubble = sound_generator::generate_underwater_bubble(0);
        assert!(!bubble.is_empty());

        // Animal sounds
        assert!(!sound_generator::generate_cow_ambient(0).is_empty());
        assert!(!sound_generator::generate_cow_hurt(0).is_empty());
        assert!(!sound_generator::generate_pig_ambient(0).is_empty());
        assert!(!sound_generator::generate_pig_hurt(0).is_empty());
        assert!(!sound_generator::generate_chicken_ambient(0).is_empty());
        assert!(!sound_generator::generate_chicken_hurt(0).is_empty());
        assert!(!sound_generator::generate_sheep_ambient(0).is_empty());
        assert!(!sound_generator::generate_sheep_hurt(0).is_empty());

        // Monster sounds
        assert!(!sound_generator::generate_zombie_ambient(0).is_empty());
        assert!(!sound_generator::generate_zombie_hurt(0).is_empty());
        assert!(!sound_generator::generate_skeleton_ambient(0).is_empty());
        assert!(!sound_generator::generate_skeleton_hurt(0).is_empty());
        assert!(!sound_generator::generate_spider_ambient(0).is_empty());
        assert!(!sound_generator::generate_spider_hurt(0).is_empty());
    }
}
