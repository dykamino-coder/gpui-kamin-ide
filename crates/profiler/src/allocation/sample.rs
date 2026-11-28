//! Byte-interval Poisson sampling. The first interval is random too.

use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering},
};

static SEEDS: AtomicU64 = AtomicU64::new(0x517cc1b727220a95);
thread_local! { static SAMPLE: Cell<Sampler> = const { Cell::new(Sampler { session: 0, state: 0, left: 0.0 }) }; }

#[derive(Clone, Copy)]
struct Sampler {
    session: u64,
    state: u64,
    left: f64,
}

impl Sampler {
    fn interval(&mut self, mean: usize) -> f64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        let uniform = ((value >> 12) + 1) as f64 / ((1u64 << 52) + 1) as f64;
        -(mean as f64) * uniform.ln()
    }

    fn select(&mut self, bytes: usize, mean: usize) -> bool {
        if bytes as f64 >= self.left {
            // Memorylessness permits a fresh interval after the selected block.
            self.left = self.interval(mean);
            true
        } else {
            self.left -= bytes as f64;
            false
        }
    }
}

pub(super) fn selected(session: u64, bytes: usize, mean: usize) -> bool {
    if mean == 0 {
        return true;
    }
    SAMPLE
        .try_with(|slot| {
            let mut sample = slot.get();
            if sample.session != session {
                sample = Sampler {
                    session,
                    state: SEEDS.fetch_add(0x9e3779b97f4a7c15, Ordering::Relaxed),
                    left: 0.0,
                };
                sample.left = sample.interval(mean);
            }
            let selected = sample.select(bytes, mean);
            slot.set(sample);
            selected
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_blocks_and_first_interval_follow_poisson_probability() {
        let mut sample = Sampler {
            session: 1,
            state: 7,
            left: 0.0,
        };
        sample.left = sample.interval(4096);
        let count = (0..100_000).filter(|_| sample.select(64, 4096)).count();
        let expected = 100_000.0 * -(-64.0f64 / 4096.0).exp_m1();
        assert!((count as f64 - expected).abs() < 6.0 * expected.sqrt());
        let first = (1..10_001)
            .filter(|&session| selected(session, 64, 4096))
            .count();
        assert!(
            first > 80 && first < 250,
            "first allocations selected {first}/10000"
        );
    }
}
