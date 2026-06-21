//! [`KarplusString`] — the Karplus-Strong plucked-string voice (crate-private).

/// Amplitude the natural decay must fall below to be considered silent (sets the
/// let-ring tail length).
const EPS: f64 = 1e-3;

/// A plucked string: a delay line of noise that rings down via a one-zero
/// low-pass feedback. The fundamental is `sample_rate / n` where `n` is the
/// delay length.
pub(crate) struct KarplusString {
    buf: Vec<f32>,
    pos: usize,
    decay: f32,
    tail: usize,
}

impl KarplusString {
    /// Plucks a string at `freq_hz`: a delay line of `round(sample_rate/freq)`
    /// samples filled with deterministic noise (a fixed-seed LCG keyed by
    /// `seed`). `decay` (in `(0, 1)`) sets how long it rings.
    pub(crate) fn pluck(freq_hz: f64, sample_rate: u32, decay: f32, seed: u64) -> KarplusString {
        let n = ((f64::from(sample_rate) / freq_hz).round() as usize).max(2);
        let mut state = seed;
        let buf: Vec<f32> = (0..n)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                ((state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
            })
            .collect();
        // Envelope ≈ decay^(k/n); reaches EPS at k = n·ln(EPS)/ln(decay).
        let max_tail = 5 * sample_rate as usize;
        let tail =
            ((n as f64 * EPS.ln() / f64::from(decay).ln()).ceil() as usize).clamp(n, max_tail);
        KarplusString {
            buf,
            pos: 0,
            decay,
            tail,
        }
    }

    /// How many samples to render for this pluck (its natural decay tail).
    pub(crate) fn tail_len(&self) -> usize {
        self.tail
    }

    /// Produces the next sample and advances the string.
    pub(crate) fn next_sample(&mut self) -> f32 {
        let n = self.buf.len();
        let out = self.buf[self.pos];
        let next = self.buf[(self.pos + 1) % n];
        self.buf[self.pos] = self.decay * 0.5 * (out + next);
        self.pos = (self.pos + 1) % n;
        out
    }
}
