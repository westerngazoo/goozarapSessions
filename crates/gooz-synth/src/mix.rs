//! Mixing helpers shared by every renderer in this crate.

/// Scales the buffer so its peak magnitude is 1.0 (a no-op if it is silent), so
/// whatever FX comes next sees a full-scale `[-1, 1]` signal.
pub(crate) fn normalize_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 0.0 {
        let gain = 1.0 / peak;
        for x in buf.iter_mut() {
            *x *= gain;
        }
    }
}
