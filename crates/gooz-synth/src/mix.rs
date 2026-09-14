//! Mixing helpers shared by every renderer in this crate.

/// The quietest peak worth normalizing.
///
/// `1.0 / peak` is the gain, and it stops being representable as an `f32`
/// exactly below `1.0 / f32::MAX` — measured, a peak of `2.938736e-39`
/// normalized to `inf`. A buffer that quiet is silence for any practical
/// purpose, so it is left as it is rather than amplified by 10^45.
const QUIET_FLOOR: f32 = 1.0 / f32::MAX;

/// Scales the buffer so its peak magnitude is 1.0, so whatever FX comes next
/// sees a full-scale `[-1, 1]` signal.
///
/// A silent buffer, one quieter than [`QUIET_FLOOR`], and one that somehow
/// holds a non-finite sample are all left untouched: none of them has a gain
/// that would bring it to full scale, and inventing one produces `inf` or NaN
/// rather than audio.
pub(crate) fn normalize_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if !peak.is_finite() || peak <= QUIET_FLOOR {
        return;
    }
    let gain = 1.0 / peak;
    for x in buf.iter_mut() {
        *x *= gain;
    }
}
