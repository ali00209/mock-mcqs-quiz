/// Returns a deterministic Fisher-Yates permutation of `0..len`.
///
/// The same seed always produces the same order, which lets an attempt be
/// resumed and reviewed exactly as it was presented.
pub fn shuffled_order(len: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    if len < 2 {
        return order;
    }
    let mut state = seed;
    for i in (1..len).rev() {
        let j = (next_u64(&mut state) % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    order
}

fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn seed_for(seed: u64, label: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in label.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    seed ^ hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_is_stable_and_complete() {
        let a = shuffled_order(10, 42);
        let b = shuffled_order(10, 42);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..10).collect::<Vec<_>>());
        assert_ne!(a, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn small_inputs_are_identity() {
        assert_eq!(shuffled_order(0, 1), Vec::<usize>::new());
        assert_eq!(shuffled_order(1, 1), vec![0]);
    }
}
