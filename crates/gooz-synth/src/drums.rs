//! One-shot drum voices for the beat builder (R-0009).

/// Which synthesized drum voice to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumKind {
    /// Low 808-style kick.
    Kick,
    /// Noise + body snare.
    Snare,
    /// Short noise hi-hat.
    HiHat,
}

const KICK_SEED: u64 = 0xA5A5_A5A5_A5A5_A5A5;
const SNARE_SEED: u64 = 0x5A5A_5A5A_5A5A_5A5A;
const HAT_SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;

/// Renders a single drum hit into `out` starting at `offset`.
pub(crate) fn mix_hit(
    kind: DrumKind,
    sample_rate: u32,
    level: f32,
    out: &mut [f32],
    offset: usize,
) {
    if sample_rate == 0 || level <= 0.0 {
        return;
    }
    let hit = match kind {
        DrumKind::Kick => synthesize_kick(sample_rate, KICK_SEED),
        DrumKind::Snare => synthesize_snare(sample_rate, SNARE_SEED),
        DrumKind::HiHat => synthesize_hat(sample_rate, HAT_SEED),
    };
    let gain = level.clamp(0.0, 1.0);
    let end = offset.saturating_add(hit.len()).min(out.len());
    for (dst, &sample) in out[offset..end].iter_mut().zip(&hit[..end - offset]) {
        *dst += sample * gain;
    }
}

fn synthesize_kick(sample_rate: u32, seed: u64) -> Vec<f32> {
    let len = (0.20 * f64::from(sample_rate)).round() as usize;
    let mut out = Vec::with_capacity(len);
    let mut phase = 0.0f64;
    for i in 0..len {
        let t = i as f64 / f64::from(sample_rate);
        let freq = 150.0 * (-t * 12.0).exp() + 45.0;
        phase += std::f64::consts::TAU * freq / f64::from(sample_rate);
        let env = (-t * 10.0).exp() as f32;
        out.push(phase.sin() as f32 * env);
    }
    let _ = seed;
    out
}

fn synthesize_snare(sample_rate: u32, seed: u64) -> Vec<f32> {
    let len = (0.15 * f64::from(sample_rate)).round() as usize;
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f64 / f64::from(sample_rate);
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let noise = ((state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0;
        let body = (std::f64::consts::TAU * 180.0 * t).sin() as f32;
        let env = (-t * 18.0).exp() as f32;
        out.push((0.7 * noise + 0.3 * body) * env);
    }
    out
}

fn synthesize_hat(sample_rate: u32, seed: u64) -> Vec<f32> {
    let len = (0.05 * f64::from(sample_rate)).round().max(1.0) as usize;
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f64 / f64::from(sample_rate);
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let noise = ((state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0;
        let env = (-t * 40.0).exp() as f32;
        // High-frequency emphasis via alternating sign.
        let accent = if i % 2 == 0 { 1.0 } else { -1.0 };
        out.push(noise * accent * env);
    }
    out
}
