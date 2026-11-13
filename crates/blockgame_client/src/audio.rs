//! Comprehensive Sound and Music System for BlockGame.
//!
//! Features:
//! - Material-based Block SFX (breaking and placing) mapped across voxel block types.
//! - 3D spatial attenuation and stereo panning based on listener camera position and yaw.
//! - Random pitch (0.85 to 1.15) and volume (0.85 to 1.05) variation to eliminate repetitive fatigue.
//! - Dynamic footstep triggers synchronized with grounded movement and walk/sprint stride intervals.
//! - Intermittent background music scheduler with non-repeating track selection and 3-8 minute cooldowns.
//! - Self-contained high-fidelity procedural WAV synthesizers with automatic fallback and support for
//!   drop-in WAV/OGG files in `assets/sounds/`.

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
use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::Path;
use std::sync::{Arc, Mutex};

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

/// Represents an active sound voice currently being rendered by the mixer.
struct ActiveVoice {
    samples: Arc<Vec<f32>>,
    cursor: f32,
    pitch: f32,
    volume_l: f32,
    volume_r: f32,
    is_music: bool,
    fade: f32,
    target_fade: f32,
    fade_speed: f32,
}

/// Thread-safe real-time audio mixer.
pub struct AudioMixer {
    voices: Vec<ActiveVoice>,
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
}

impl AudioMixer {
    pub fn new() -> Self {
        Self {
            voices: Vec::with_capacity(32),
            master_volume: 0.8,
            sfx_volume: 0.9,
            music_volume: 0.65,
        }
    }

    /// Add a voice to the active voice list.
    pub fn play_voice(
        &mut self,
        samples: Arc<Vec<f32>>,
        pitch: f32,
        volume_l: f32,
        volume_r: f32,
        is_music: bool,
        fade_in_sec: f32,
    ) {
        let (fade, target_fade, fade_speed) = if fade_in_sec > 0.0 {
            (0.0, 1.0, 1.0 / fade_in_sec)
        } else {
            (1.0, 1.0, 0.0)
        };

        self.voices.push(ActiveVoice {
            samples,
            cursor: 0.0,
            pitch: pitch.clamp(0.2, 4.0),
            volume_l,
            volume_r,
            is_music,
            fade,
            target_fade,
            fade_speed,
        });
    }

    /// Stops any playing music tracks with a fade-out.
    pub fn stop_music(&mut self, fade_out_sec: f32) {
        for voice in &mut self.voices {
            if voice.is_music {
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
            .any(|v| v.is_music && (v.fade > 0.001 || v.target_fade > 0.001))
    }

    /// Mixes active voices into an interleaved stereo output buffer.
    pub fn mix(&mut self, output: &mut [f32], sample_rate: u32) {
        let dt = 1.0 / (sample_rate as f32);
        let frame_count = output.len() / 2;

        for frame_idx in 0..frame_count {
            let mut left_acc = 0.0;
            let mut right_acc = 0.0;

            for voice in &mut self.voices {
                if voice.cursor >= voice.samples.len() as f32 {
                    continue;
                }

                // Update fade multiplier
                if voice.fade < voice.target_fade {
                    voice.fade = (voice.fade + voice.fade_speed * dt).min(voice.target_fade);
                } else if voice.fade > voice.target_fade {
                    voice.fade = (voice.fade - voice.fade_speed * dt).max(voice.target_fade);
                }

                if voice.fade <= 0.0 && voice.target_fade <= 0.0 {
                    voice.cursor = voice.samples.len() as f32; // mark for cleanup
                    continue;
                }

                // Linear interpolation for pitch shifting
                let idx0 = voice.cursor as usize;
                let idx1 = (idx0 + 1).min(voice.samples.len() - 1);
                let frac = voice.cursor - idx0 as f32;
                let sample = voice.samples[idx0] * (1.0 - frac) + voice.samples[idx1] * frac;

                let category_vol = if voice.is_music {
                    self.music_volume
                } else {
                    self.sfx_volume
                };
                let effective_vol = self.master_volume * category_vol * voice.fade;

                left_acc += sample * voice.volume_l * effective_vol;
                right_acc += sample * voice.volume_r * effective_vol;

                voice.cursor += voice.pitch;
            }

            let out_idx = frame_idx * 2;
            output[out_idx] = (output[out_idx] + left_acc).clamp(-1.0, 1.0);
            output[out_idx + 1] = (output[out_idx + 1] + right_acc).clamp(-1.0, 1.0);
        }

        // Retain only voices that still have samples remaining
        self.voices.retain(|v| v.cursor < v.samples.len() as f32);
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

    /// Procedurally generates calm, peaceful C418-style ambient music tracks.
    ///
    /// Built using nostalgic solo piano chords and warm analog synth pads.
    pub fn generate_ambient_music(track_index: usize) -> Vec<f32> {
        let duration = 24.0; // 24 seconds ambient movement
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = vec![0.0_f32; num_samples];

        // Harmonic progressions (frequencies in Hz):
        // Track 0: Peaceful C major / A minor felt piano arpeggios
        // Track 1: Nostalgic D major / F# minor dreamy meditation
        // Track 2: Spacious, minimalist pentatonic stillness
        let notes_schedule: Vec<(f32, f32, f32)> = match track_index % 3 {
            0 => vec![
                (0.0, 261.63, 0.45),  // C4
                (1.5, 329.63, 0.40),  // E4
                (3.0, 392.00, 0.42),  // G4
                (4.5, 523.25, 0.38),  // C5
                (7.0, 220.00, 0.45),  // A3
                (8.5, 261.63, 0.40),  // C4
                (10.0, 329.63, 0.42), // E4
                (12.0, 174.61, 0.45), // F3
                (13.5, 261.63, 0.38), // C4
                (15.0, 349.23, 0.40), // F4
                (17.5, 196.00, 0.45), // G3
                (19.0, 246.94, 0.38), // B3
                (20.5, 293.66, 0.40), // D4
            ],
            1 => vec![
                (0.0, 293.66, 0.45),  // D4
                (1.8, 369.99, 0.40),  // F#4
                (3.6, 440.00, 0.42),  // A4
                (6.5, 185.00, 0.45),  // F#3
                (8.3, 220.00, 0.38),  // A3
                (10.1, 277.18, 0.40), // C#4
                (13.0, 196.00, 0.45), // G3
                (14.8, 246.94, 0.40), // B3
                (16.6, 293.66, 0.42), // D4
                (19.5, 220.00, 0.42), // A3
                (21.0, 277.18, 0.38), // C#4
            ],
            _ => vec![
                (0.0, 196.00, 0.45),  // G3
                (2.2, 293.66, 0.40),  // D4
                (4.4, 329.63, 0.42),  // E4
                (7.5, 261.63, 0.45),  // C4
                (9.7, 392.00, 0.38),  // G4
                (13.0, 220.00, 0.45), // A3
                (15.2, 329.63, 0.40), // E4
                (18.5, 196.00, 0.45), // G3
                (20.7, 261.63, 0.40), // C4
            ],
        };

        // Render piano notes with exponential decay and gentle harmonic overtone
        for &(start_time, freq, amplitude) in &notes_schedule {
            let start_sample = (start_time * SAMPLE_RATE) as usize;
            let note_len = (4.5 * SAMPLE_RATE) as usize;
            for n in 0..note_len {
                let idx = start_sample + n;
                if idx >= num_samples {
                    break;
                }
                let t = n as f32 / SAMPLE_RATE;
                let envelope = (-t * 0.85).exp();
                let fundamental = (2.0 * PI * freq * t).sin();
                let overtone = 0.35 * (2.0 * PI * freq * 2.0 * t).sin() * (-t * 1.8).exp();
                let warmth = 0.15 * (2.0 * PI * freq * 3.0 * t).sin() * (-t * 2.5).exp();
                samples[idx] += amplitude * envelope * (fundamental + overtone + warmth);
            }
        }

        // Layer warm analog synth bass pad underneath
        let pad_root = match track_index % 3 {
            0 => 65.41,  // C2
            1 => 73.42,  // D2
            _ => 98.00,  // G2
        };
        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let pad_envelope = (PI * t / duration).sin().clamp(0.0, 1.0);
            let pad = 0.18
                * pad_envelope
                * ((2.0 * PI * pad_root * t).sin() + 0.3 * (2.0 * PI * (pad_root * 1.5) * t).sin());
            samples[i] = (samples[i] + pad).clamp(-1.0, 1.0);
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

    /// Synthesizes visceral player damage / impact sound ("oof"grunt + flesh impact).
    pub fn generate_hurt(variant: usize) -> Vec<f32> {
        let mut rng = 424242 + (variant as u64 * 99991);
        let duration = 0.18;
        let num_samples = (duration * SAMPLE_RATE) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let progress = t / duration;

            let freq = (180.0 - progress * 80.0).max(60.0);
            let oof = (2.0 * PI * freq * t).sin() * (-progress * 16.0).exp();
            let formant = (2.0 * PI * freq * 2.3 * t).sin() * (-progress * 22.0).exp();
            let impact = lcg_random(&mut rng) * (-progress * 35.0).exp();
            let sub = (2.0 * PI * 85.0 * t).sin() * (-progress * 20.0).exp();

            let sample = 0.45 * oof + 0.25 * formant + 0.20 * impact + 0.10 * sub;
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

        // 10. Ambient Music Tracks (3 calm compositions)
        for track_idx in 0..3 {
            let file_name = format!("ambient_track_{}.wav", track_idx + 1);
            let file_path = assets_dir.join(&file_name);

            let buffer = if has_assets && file_path.exists() {
                Self::load_audio_file(&file_path)
                    .unwrap_or_else(|| sound_generator::generate_ambient_music(track_idx))
            } else {
                sound_generator::generate_ambient_music(track_idx)
            };

            self.music_tracks.push(Arc::new(buffer));
        }

        println!(
            "[Audio] Initialized {} footstep, {} hit, {} break, {} place, {} hurt, {} splash, {} swim, {} bubble, {} tool break, and {} music tracks.",
            self.footsteps.len(),
            self.hits.len(),
            self.breaks.len(),
            self.places.len(),
            self.hurts.len(),
            self.water_splashes.len(),
            self.water_swims.len(),
            self.underwater_bubbles.len(),
            self.tool_breaks.len(),
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
                    mixer.play_voice(samples, pitch, vol_l, vol_r, false, 0.0);
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
                    mixer.play_voice(samples, pitch, vol_l, vol_r, false, 0.0);
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
                mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
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
                    mixer.play_voice(samples, pitch, vol_l, vol_r, false, 0.0);
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
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
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
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
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
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
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
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
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
            mixer.play_voice(samples, pitch, vol * 0.5, vol * 0.5, false, 0.0);
        }
    }

    /// Plays a background music track with smooth fade-in.
    pub fn play_music(&mut self, track_idx: usize, fade_in_sec: f32) {
        if track_idx < self.music_tracks.len() {
            let samples = self.music_tracks[track_idx].clone();
            if let Ok(mut mixer) = self.mixer.lock() {
                mixer.stop_music(1.5);
                mixer.play_voice(samples, 1.0, 0.5, 0.5, true, fade_in_sec);
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

/// Intermittent Background Music Scheduler.
///
/// Features:
/// - Plays ambient music sporadically rather than looping continuously.
/// - Enforces silence cooldowns between 3 and 8 minutes between tracks.
/// - Guarantees non-repeating track selections across sessions.
/// - Handles smooth volume fading.
pub struct MusicScheduler {
    cooldown_timer: f32,
    is_playing: bool,
    last_played_track: Option<usize>,
    total_tracks: usize,
    rng_state: u64,
}

impl MusicScheduler {
    /// Constructs a new music scheduler.
    pub fn new(total_tracks: usize) -> Self {
        Self {
            // Friendly initial cooldown of 15 seconds after launching game
            cooldown_timer: 15.0,
            is_playing: false,
            last_played_track: None,
            total_tracks,
            rng_state: 777123456,
        }
    }

    fn random_f32(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        ((self.rng_state >> 33) as f32) / ((u32::MAX >> 1) as f32)
    }

    /// Computes a randomized silence cooldown between `min_sec` (e.g. 180s) and `max_sec` (e.g. 480s).
    fn next_cooldown(&mut self) -> f32 {
        let min_sec = 180.0_f32; // 3 minutes
        let max_sec = 480.0_f32; // 8 minutes
        min_sec + self.random_f32() * (max_sec - min_sec)
    }

    /// Selects the next track ensuring it doesn't immediately repeat the previous track.
    fn select_next_track(&mut self) -> usize {
        if self.total_tracks <= 1 {
            return 0;
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

    /// Updates the music scheduler each frame.
    pub fn update(&mut self, dt: f32, audio: &mut AudioManager) {
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
                println!(
                    "[MusicScheduler] Track completed. Silence cooldown started: {:.1} seconds ({:.1} min).",
                    self.cooldown_timer,
                    self.cooldown_timer / 60.0
                );
            }
        } else {
            // In silence cooldown period
            self.cooldown_timer -= dt;
            if self.cooldown_timer <= 0.0 {
                let track = self.select_next_track();
                self.last_played_track = Some(track);
                self.is_playing = true;
                println!("[MusicScheduler] Starting ambient track #{} with 2.5s fade-in.", track + 1);
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
        let mut scheduler = MusicScheduler::new(3);
        let mut prev = None;
        for _ in 0..15 {
            let next = scheduler.select_next_track();
            if let Some(p) = prev {
                assert_ne!(next, p, "Next track should not repeat previous track immediately");
            }
            prev = Some(next);
            scheduler.last_played_track = Some(next);
        }
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
    }
}
