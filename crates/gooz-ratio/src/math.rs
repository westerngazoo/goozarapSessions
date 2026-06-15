//! Shared integer math used by both the pitch and beat cores.
//!
//! Crate-internal: keeps a single `gcd` (and the `lcm` built on it) so the
//! exact-rational reductions in `ratio.rs` and the grid composition in
//! `beat.rs` never duplicate the algorithm.

/// Greatest common divisor (Euclid). `gcd(0, n) == n` and `gcd(n, 0) == n`.
pub(crate) fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// Least common multiple, or `None` on `u64` overflow.
///
/// Divides before multiplying so the intermediate stays as small as possible;
/// `lcm(a, 0) == Some(0)`.
pub(crate) fn lcm(a: u64, b: u64) -> Option<u64> {
    if a == 0 || b == 0 {
        return Some(0);
    }
    (a / gcd(a, b)).checked_mul(b)
}
