//! [`Distortion`] — the waveshaping FX (soft- or hard-clip).

/// A distortion curve applied to a sample with a drive amount.
///
/// For input in `[-1, 1]` both keep the output in `[-1, 1]`. The "clean" setting
/// differs by mode: `SoftClip` approaches identity as drive → 0, while
/// `HardClip` is exactly identity at drive `1.0` (and near-silent at very low
/// drive, by construction).
///
/// ```
/// use gooz_synth::Distortion;
///
/// assert_eq!(Distortion::HardClip.apply(0.5, 1.0), 0.5); // identity at drive 1.0
/// assert_eq!(Distortion::HardClip.apply(0.5, 4.0), 1.0); // clamped
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Distortion {
    /// Smooth `tanh` saturation — warm overdrive.
    SoftClip,
    /// Boost then clamp to `[-1, 1]` — aggressive fuzz.
    HardClip,
}

impl Distortion {
    /// Applies the curve to `x` at the given `drive` (`drive` is floored at a
    /// tiny positive value so soft-clip never divides by zero).
    pub fn apply(self, x: f32, drive: f32) -> f32 {
        let d = drive.max(1e-3);
        match self {
            Distortion::SoftClip => (d * x).tanh() / d.tanh(),
            Distortion::HardClip => (d * x).clamp(-1.0, 1.0),
        }
    }
}
