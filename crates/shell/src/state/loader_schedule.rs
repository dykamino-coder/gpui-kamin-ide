//! Ограниченная retry-политика провайдера; прогресс не требует display frames.

use std::time::Duration;
pub(super) const TICK: Duration = Duration::from_millis(250);

pub(super) fn should_retry(tries: u32, elapsed: Option<Duration>) -> bool {
    tries < 45 && elapsed.is_none_or(|elapsed| {
        elapsed
            >= Duration::from_millis((350.0_f32 * 1.5_f32.powi(tries as i32)).min(3000.0) as u64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_loader_retries_to_exhaustion_with_only_bounded_ticks() {
        let mut tries = 0;
        let mut elapsed = None;
        let mut ticks = 0;
        while tries < 45 {
            if should_retry(tries, elapsed) {
                tries += 1;
                elapsed = Some(Duration::ZERO);
            }
            elapsed = elapsed.map(|e| e + TICK);
            ticks += 1;
            assert!(
                ticks < 600,
                "retry budget must terminate without animated frames"
            );
        }
        assert!(!should_retry(tries, Some(Duration::from_secs(600))));
        assert!(
            should_retry(0, None),
            "Retry resets the budget and sends immediately"
        );
    }
    #[test]
    fn backoff_waits_for_deadline_and_stays_capped() {
        assert!(!should_retry(1, Some(Duration::from_millis(524))));
        assert!(should_retry(1, Some(Duration::from_millis(525))));
        assert!(!should_retry(44, Some(Duration::from_millis(2999))));
        assert!(should_retry(44, Some(Duration::from_millis(3000))));
        assert!(!should_retry(45, None));
    }
}
