//! Client-side microphone capture for proximity voice chat.

use blockgame_core::voice::{encode_adpcm_frame, frame_rms, VOICE_FRAME_SAMPLES};
use maudio::{
    audio::sample_rate::SampleRate,
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        Device,
    },
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Manages 16 kHz mono microphone capture for proximity voice chat.
pub struct VoiceCaptureManager {
    _device: Option<Device<f32>>,
    ring: Arc<Mutex<VecDeque<f32>>>,
    next_seq: u32,
    pub available: bool,
}

impl VoiceCaptureManager {
    pub fn new() -> Self {
        let ring = Arc::new(Mutex::new(VecDeque::with_capacity(VOICE_FRAME_SAMPLES * 8)));
        let ring_cb = ring.clone();
        let mut device = None;
        let mut available = false;

        match DeviceBuilder::capture()
            .f32()
            .capture_channels(1)
            .sample_rate(SampleRate::Sr16000)
            .with_callback(move |_, input| {
                if let Ok(mut buf) = ring_cb.lock() {
                    // Cap buffer at ~0.5s to avoid latency buildup
                    if buf.len() > VOICE_FRAME_SAMPLES * 25 {
                        let drop_n = buf.len() - VOICE_FRAME_SAMPLES * 10;
                        buf.drain(..drop_n);
                    }
                    buf.extend(input.iter().copied());
                }
            }) {
            Ok(mut dev) => {
                if let Err(e) = dev.device_start() {
                    eprintln!("[Voice] Warning: Failed to start microphone capture: {:?}", e);
                } else {
                    println!("[Voice] Microphone capture initialized at 16kHz.");
                    device = Some(dev);
                    available = true;
                }
            }
            Err(e) => {
                eprintln!(
                    "[Voice] Notice: No microphone capture device available ({:?}). Voice transmit disabled.",
                    e
                );
            }
        }

        Self {
            _device: device,
            ring,
            next_seq: 1,
            available,
        }
    }

    /// Drains ready 20ms frames from the capture buffer.
    /// Returns encoded `(seq, adpcm_bytes)` packets when transmitting, or clears the buffer when muted/idle.
    pub fn poll_encoded_frames(
        &mut self,
        ptt_held: bool,
        voice_mode: u8, // 0 = Off, 1 = Push-to-Talk, 2 = Voice Activity
    ) -> Vec<(u32, Vec<u8>)> {
        let mut out = Vec::new();
        let Ok(mut buf) = self.ring.lock() else {
            return out;
        };

        if voice_mode == 0 || (voice_mode == 1 && !ptt_held) {
            buf.clear();
            return out;
        }

        while buf.len() >= VOICE_FRAME_SAMPLES {
            let mut chunk = Vec::with_capacity(VOICE_FRAME_SAMPLES);
            for _ in 0..VOICE_FRAME_SAMPLES {
                if let Some(s) = buf.pop_front() {
                    chunk.push(s);
                }
            }
            let should_send = if voice_mode == 1 {
                ptt_held
            } else {
                frame_rms(&chunk) >= 0.015
            };
            if should_send {
                let encoded = encode_adpcm_frame(&chunk);
                if !encoded.is_empty() {
                    let seq = self.next_seq;
                    self.next_seq = self.next_seq.wrapping_add(1);
                    out.push((seq, encoded));
                }
            }
        }

        out
    }
}
