//! The seeded generator (splitmix64): the same seed gives the same session on every machine.

pub(super) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// An item picked with its weight's share of the total, or `None` when there is none to pick.
    pub fn weighted<'a, T>(&mut self, items: &'a [(T, u32)]) -> Option<&'a T> {
        let total: u64 = items.iter().map(|(_, w)| u64::from(*w)).sum();
        if total == 0 {
            return None;
        }
        let mut at = self.next_u64() % total;
        for (item, weight) in items {
            let weight = u64::from(*weight);
            if at < weight {
                return Some(item);
            }
            at -= weight;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_same_numbers() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn known_first_value() {
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn weighted_never_picks_zero() {
        let mut rng = Rng::new(1);
        let items = [("a", 0), ("b", 1)];
        for _ in 0..10_000 {
            assert_eq!(rng.weighted(&items), Some(&"b"));
        }
    }

    #[test]
    fn weighted_empty_is_none() {
        let mut rng = Rng::new(1);
        assert_eq!(rng.weighted::<u8>(&[]), None);
        assert_eq!(rng.weighted(&[(1u8, 0)]), None);
    }

    #[test]
    fn below_stays_below() {
        let mut rng = Rng::new(3);
        assert!((0..1000).all(|_| rng.below(7) < 7));
    }
}
