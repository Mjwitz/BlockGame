//! Pure-Rust IMA-ADPCM voice codec for low-latency proximity voice chat.
//!
//! Uses 16 kHz mono 20 ms frames (320 samples -> 164 bytes encoded, ~65.6 kbps when speaking).

pub const VOICE_SAMPLE_RATE: u32 = 16_000;
pub const VOICE_FRAME_SAMPLES: usize = 320; // 20 ms at 16 kHz

const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

const INDEX_TABLE: [i32; 16] = [
    -1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8,
];

/// Computes the RMS amplitude (`0.0..=1.0`) of a mono PCM frame.
pub fn frame_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Encodes a mono `f32` (`-1.0..=1.0`) PCM slice into an IMA-ADPCM packet.
/// Format: `[predictor_i16_le (2B), step_index_u8 (1B), sample_count_lo_hi (2B), nibbles...]`
pub fn encode_adpcm_frame(samples: &[f32]) -> Vec<u8> {
    if samples.is_empty() {
        return Vec::new();
    }
    let count = samples.len().min(u16::MAX as usize);
    let mut out = Vec::with_capacity(5 + (count + 1) / 2);

    let first_pcm = (samples[0].clamp(-1.0, 1.0) * 32767.0) as i16;
    let mut predictor: i32 = first_pcm as i32;
    let mut step_index: i32 = if count >= 2 {
        let second_pcm = (samples[1].clamp(-1.0, 1.0) * 32767.0) as i32;
        let init_diff = (second_pcm - predictor).abs();
        STEP_TABLE
            .iter()
            .position(|&s| s * 2 >= init_diff)
            .unwrap_or(88) as i32
    } else {
        0
    };

    out.extend_from_slice(&first_pcm.to_le_bytes());
    out.push(step_index as u8);
    out.extend_from_slice(&(count as u16).to_le_bytes());

    let mut packed_byte: u8 = 0;
    let mut high_nibble = false;

    for &s in samples.iter().take(count).skip(1) {
        let pcm = (s.clamp(-1.0, 1.0) * 32767.0) as i32;
        let step = STEP_TABLE[step_index as usize];
        let mut diff = pcm - predictor;
        let mut nibble: u8 = 0;

        if diff < 0 {
            nibble = 8;
            diff = -diff;
        }

        let mut delta = step >> 3;
        if diff >= step {
            nibble |= 4;
            diff -= step;
            delta += step;
        }
        let step2 = step >> 1;
        if diff >= step2 {
            nibble |= 2;
            diff -= step2;
            delta += step2;
        }
        let step4 = step >> 2;
        if diff >= step4 {
            nibble |= 1;
            delta += step4;
        }

        if (nibble & 8) != 0 {
            predictor -= delta;
        } else {
            predictor += delta;
        }
        predictor = predictor.clamp(-32768, 32767);
        step_index = (step_index + INDEX_TABLE[nibble as usize]).clamp(0, 88);

        if !high_nibble {
            packed_byte = nibble & 0x0F;
            high_nibble = true;
        } else {
            packed_byte |= (nibble & 0x0F) << 4;
            out.push(packed_byte);
            packed_byte = 0;
            high_nibble = false;
        }
    }

    if high_nibble {
        out.push(packed_byte);
    }

    out
}

/// Decodes an IMA-ADPCM packet produced by [`encode_adpcm_frame`] back into mono `f32` PCM.
pub fn decode_adpcm_frame(data: &[u8]) -> Vec<f32> {
    if data.len() < 5 {
        return Vec::new();
    }
    let mut predictor = i16::from_le_bytes([data[0], data[1]]) as i32;
    let mut step_index = (data[2] as i32).clamp(0, 88);
    let sample_count = u16::from_le_bytes([data[3], data[4]]) as usize;
    if sample_count == 0 {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(sample_count);
    out.push((predictor as f32) / 32768.0);

    for &byte in &data[5..] {
        for shift in [0u8, 4u8] {
            if out.len() >= sample_count {
                break;
            }
            let nibble = (byte >> shift) & 0x0F;
            let step = STEP_TABLE[step_index as usize];

            let mut delta = step >> 3;
            if (nibble & 4) != 0 {
                delta += step;
            }
            if (nibble & 2) != 0 {
                delta += step >> 1;
            }
            if (nibble & 1) != 0 {
                delta += step >> 2;
            }

            if (nibble & 8) != 0 {
                predictor -= delta;
            } else {
                predictor += delta;
            }
            predictor = predictor.clamp(-32768, 32767);
            step_index = (step_index + INDEX_TABLE[nibble as usize]).clamp(0, 88);

            out.push((predictor as f32) / 32768.0);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adpcm_roundtrip_preserves_length_and_waveform() {
        let mut input = Vec::with_capacity(VOICE_FRAME_SAMPLES);
        for i in 0..VOICE_FRAME_SAMPLES {
            let t = i as f32 / VOICE_SAMPLE_RATE as f32;
            input.push((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.6);
        }

        let encoded = encode_adpcm_frame(&input);
        assert!(encoded.len() <= 165, "Expected ~165 bytes, got {}", encoded.len());

        let decoded = decode_adpcm_frame(&encoded);
        assert_eq!(decoded.len(), input.len());

        let max_err = input
            .iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(max_err < 0.08, "ADPCM max error too large: {}", max_err);
    }
}
