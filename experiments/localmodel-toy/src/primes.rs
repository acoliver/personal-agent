//! Deterministic primality testing and safe-prime search over [`u64`].
//!
//! The Miller-Rabin witness set `{2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37}` is
//! deterministic for every `u64`, so no probabilistic fallback is needed. All
//! intermediate products are computed in `u128` so `mul_mod` cannot overflow.

/// Witnesses that make Miller-Rabin deterministic for the whole `u64` range.
///
/// These are also the first twelve primes, so the same list doubles as the trial
/// division set inside [`is_prime`].
pub const WITNESSES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// Computes `(a * b) % m` using `u128` intermediates.
fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(m);
    u64::try_from(reduced).expect("value reduced modulo a u64 always fits in a u64")
}

/// Computes `(base ^ exp) % m` by square-and-multiply.
fn pow_mod(mut base: u64, mut exp: u64, m: u64) -> u64 {
    if m == 1 {
        return 0;
    }
    let mut result = 1u64;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        base = mul_mod(base, base, m);
        exp >>= 1;
    }
    result
}

/// Returns `true` when `n` is prime.
///
/// This is exact for every `u64`: small factors are removed by trial division
/// against [`WITNESSES`], and the remaining candidates go through Miller-Rabin
/// with that same witness set.
#[must_use]
pub fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for witness in WITNESSES {
        if n == witness {
            return true;
        }
        if n.is_multiple_of(witness) {
            return false;
        }
    }

    // `n` is now odd, greater than 37, and coprime to every witness, so no
    // witness is ever congruent to 0 modulo `n`.
    let mut d = n - 1;
    let mut trailing_zeros = 0u32;
    while d.is_multiple_of(2) {
        d /= 2;
        trailing_zeros += 1;
    }

    'witness: for witness in WITNESSES {
        let mut x = pow_mod(witness, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..trailing_zeros {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Returns `true` when `p` is a safe prime, meaning `p` is an odd prime and
/// `(p - 1) / 2` is also prime.
///
/// `(p - 1) / 2` is the matching Sophie Germain prime.
#[must_use]
pub fn is_safe_prime(p: u64) -> bool {
    p % 2 == 1 && is_prime(p) && is_prime((p - 1) / 2)
}

/// Returns the smallest safe prime strictly greater than `after`.
///
/// Returns `None` only when the search runs past [`u64::MAX`] without finding
/// one, which cannot happen for any realistic input.
#[must_use]
pub fn next_secure_prime(after: u64) -> Option<u64> {
    // 5 is the smallest safe prime, so nothing below it is worth testing.
    let mut candidate = after.checked_add(1)?.max(5);
    if candidate % 2 == 0 {
        candidate = candidate.checked_add(1)?;
    }
    loop {
        if is_safe_prime(candidate) {
            return Some(candidate);
        }
        candidate = candidate.checked_add(2)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_below_two() {
        assert!(!is_prime(0));
        assert!(!is_prime(1));
    }

    #[test]
    fn accepts_known_small_primes() {
        for p in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 97, 8191] {
            assert!(is_prime(p), "{p} should be prime");
        }
    }

    #[test]
    fn rejects_known_small_composites() {
        for c in [4u64, 6, 9, 15, 21, 25, 49, 91, 121, 8192] {
            assert!(!is_prime(c), "{c} should be composite");
        }
    }

    #[test]
    fn rejects_carmichael_numbers() {
        // Carmichael numbers pass the Fermat test for every coprime base, so a
        // naive Fermat implementation would call these prime.
        for c in [561u64, 1105, 1729, 2465, 2821, 6601, 8911, 41041, 825265] {
            assert!(!is_prime(c), "Carmichael number {c} should be composite");
        }
    }

    #[test]
    fn rejects_strong_pseudoprimes_to_small_bases() {
        // 2047 = 23 * 89 is a strong pseudoprime to base 2.
        assert!(!is_prime(2047));
        // 1373653 is a strong pseudoprime to bases 2 and 3.
        assert!(!is_prime(1_373_653));
        // 3215031751 is a strong pseudoprime to bases 2, 3, 5 and 7.
        assert!(!is_prime(3_215_031_751));
        // 3474749660383 is a strong pseudoprime to bases 2 through 13.
        assert!(!is_prime(3_474_749_660_383));
    }

    #[test]
    fn accepts_large_u64_primes() {
        // 2^61 - 1 is a Mersenne prime.
        assert!(is_prime(2_305_843_009_213_693_951));
        // The largest prime below 2^64.
        assert!(is_prime(18_446_744_073_709_551_557));
        assert!(is_prime(1_000_000_007));
        assert!(is_prime(1_000_000_009));
    }

    #[test]
    fn rejects_large_u64_composites() {
        assert!(!is_prime(u64::MAX));
        assert!(!is_prime(1_000_000_007 * 3));
    }

    #[test]
    fn identifies_small_safe_primes() {
        for p in [5u64, 7, 11, 23, 47, 59, 83, 107, 167, 179] {
            assert!(is_safe_prime(p), "{p} should be a safe prime");
        }
        for p in [3u64, 13, 17, 19, 29, 31, 37, 41, 43, 53] {
            assert!(!is_safe_prime(p), "{p} should not be a safe prime");
        }
    }

    #[test]
    fn walks_the_known_sequence_of_safe_primes() {
        let expected = [5u64, 7, 11, 23, 47, 59, 83, 107, 167, 179];
        assert_eq!(next_secure_prime(0), Some(5));
        assert_eq!(next_secure_prime(4), Some(5));
        for pair in expected.windows(2) {
            assert_eq!(
                next_secure_prime(pair[0]),
                Some(pair[1]),
                "safe prime after {} should be {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn result_above_one_million_is_a_real_safe_prime() {
        let p = next_secure_prime(1_000_000).expect("a safe prime exists above one million");
        assert!(p > 1_000_000, "{p} must be above the requested bound");
        assert!(is_prime(p), "{p} must be prime");
        assert_eq!(p % 2, 1, "{p} must be odd");
        let sophie_germain = (p - 1) / 2;
        assert!(
            is_prime(sophie_germain),
            "{sophie_germain} must be prime for {p} to be a safe prime"
        );
        // Nothing between the bound and the answer may be a safe prime.
        for candidate in (1_000_001..p).step_by(2) {
            assert!(
                !is_safe_prime(candidate),
                "{candidate} is a safe prime below the reported answer {p}"
            );
        }
    }

    #[test]
    fn the_prime_reported_by_the_real_model_run_is_the_exact_answer() {
        // Pins the value the release binary's live run reported through the
        // tool (see tmp/verify-localmodel/run4.log), so a future change that
        // shifts the search cannot silently invalidate that transcript.
        assert_eq!(next_secure_prime(1_000_000), Some(1_000_667));
        assert!(is_safe_prime(1_000_667));
        assert_eq!((1_000_667 - 1) / 2, 500_333);
        assert!(is_prime(500_333));
    }

    #[test]
    fn returns_none_when_the_search_leaves_the_u64_range() {
        assert_eq!(next_secure_prime(u64::MAX), None);
        assert_eq!(next_secure_prime(u64::MAX - 1), None);
    }

    #[test]
    fn pow_mod_matches_manual_exponentiation() {
        assert_eq!(pow_mod(2, 10, 1_000), 24);
        assert_eq!(pow_mod(3, 0, 7), 1);
        assert_eq!(pow_mod(5, 3, 1), 0);
        assert_eq!(pow_mod(u64::MAX - 2, 2, u64::MAX - 1), 1);
    }
}
