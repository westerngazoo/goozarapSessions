//! Shared integer math used by both the pitch and beat cores.
//!
//! Crate-internal: keeps a single `gcd` so the exact-rational reductions in
//! `ratio.rs` and the grid composition in `beat.rs` never duplicate the
//! algorithm. (`lcm` is not a shared helper: its only use,
//! `Polyrhythm::grid_steps`, composes two `u32`s where `(a / gcd) * b` is
//! provably overflow-free in `u64`, so it is computed inline without any
//! fallible arithmetic.)

/// Greatest common divisor (Euclid). `gcd(0, n) == n` and `gcd(n, 0) == n`.
pub(crate) fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}
