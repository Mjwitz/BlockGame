//! Client-side microphone capture for proximity voice chat.

use blockgame_core::voice::{encode_adpcm_frame, frame_rms, VOICE_FRAME_SAMPLES};
use maudio::{
    audio::sample_rate::SampleRate,
    context::{ContextBuilder, ContextOps},
    device::{
        device_builder::{DeviceBuilder, DeviceBuilderOps},
        device_id::DeviceId,
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
    selected_device: String,
    pub available: bool,
}

impl VoiceCaptureManager {
    /// Lists all available microphone / audio capture device names on the system.
    pub fn list_input_devices() -> Vec<String> {
        let Ok(ctx) = ContextBuilder::new().build() else {
            return Vec::new();
        };
        let Ok(devices) = ctx.get_devices() else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for (_, info) in devices.capture() {
            let name = info.device_name().trim();
            if !name.is_empty() && !names.iter().any(|n: &String| n == name) {
                names.push(name.to_string());
            }
        }
        names
    }

    fn find_capture_device_id(device_name: &str) -> Option<DeviceId> {
        if device_name.is_empty() {
            return None;
        }
        let ctx = ContextBuilder::new().build().ok()?;
        let devices = ctx.get_devices().ok()?;
        for (_, info) in devices.capture() {
            if info.device_name().trim() == device_name {
                return Some(info.device_id());
            }
        }
        None
    }

    pub fn new() -> Self {
        Self::new_with_device("")
    }

    /// Initializes microphone capture, optionally targeting a specific device by name.
    /// An empty `device_name` (or an unrecognized one) uses the system default capture device.
    pub fn new_with_device(device_name: &str) -> Self {
        let ring = Arc::new(Mutex::new(VecDeque::with_capacity(VOICE_FRAME_SAMPLES * 8)));
        let ring_cb = ring.clone();
        let mut device = None;
        let mut available = false;

        let target_id = Self::find_capture_device_id(device_name);
        if !device_name.is_empty() && target_id.is_none() {
            eprintln!(
                "[Voice] Requested microphone '{}' not found; falling back to system default.",
                device_name
            );
        }

        let mut builder = DeviceBuilder::capture().f32();
        builder
            .capture_channels(1)
            .sample_rate(SampleRate::Sr16000);
        if let Some(ref dev_id) = target_id {
            builder.capture_device_id(dev_id);
        }

        match builder.with_callback(move |_, input| {
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
                    let label = if target_id.is_some() {
                        device_name
                    } else {
                        "System Default"
                    };
                    println!("[Voice] Microphone capture initialized ({}) at 16kHz.", label);
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
            selected_device: device_name.to_string(),
            available,
        }
    }

    pub fn selected_device(&self) -> &str {
        &self.selected_device
    }

    /// Switches the active capture device, releasing the previous device handle first.
    pub fn set_input_device(&mut self, device_name: &str) {
        self._device = None;
        let next_seq = self.next_seq;
        *self = Self::new_with_device(device_name);
        self.next_seq = next_seq;
    }

    /// Cycles through `""` (System Default) and all enumerated capture devices.
    pub fn cycle_input_device(&self, current: &str) -> String {
        let devices = Self::list_input_devices();
        if devices.is_empty() {
            return String::new();
        }
        if current.is_empty() {
            return devices[0].clone();
        }
        match devices.iter().position(|d| d == current) {
            Some(idx) if idx + 1 < devices.len() => devices[idx + 1].clone(),
            _ => String::new(),
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
