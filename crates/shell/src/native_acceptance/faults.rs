//! One pending targeted wait fault: consumed before AcquireSync, never after it.

use std::sync::{LazyLock, Mutex};

#[derive(Default)]
struct WaitFault {
    view: String,
    status: i32,
    remaining: u8,
    consumed: u32,
}
impl WaitFault {
    fn take(&mut self, view: &str) -> Option<i32> {
        if self.remaining == 0 || self.view != view {
            return None;
        }
        self.remaining -= 1;
        self.consumed += 1;
        Some(self.status)
    }
}
static WAIT: LazyLock<Mutex<WaitFault>> = LazyLock::new(|| Mutex::new(WaitFault::default()));

pub(crate) fn arm(view: &str, status: i32, count: u8) -> bool {
    if view.is_empty()
        || view.len() > 128
        || !matches!(status, 128 | 258)
        || !(1..=3).contains(&count)
    {
        return false;
    }
    let Ok(mut wait) = WAIT.lock() else {
        return false;
    };
    *wait = WaitFault {
        view: view.into(),
        status,
        remaining: count,
        consumed: 0,
    };
    true
}
pub(crate) fn wait_status(view: &str) -> Option<i32> {
    let status = WAIT.lock().ok()?.take(view);
    if let Some(status) = status {
        crate::web::acceptance_log(format!(
            "[acceptance] mutex view={view:?} injected_status={status} acquired=false"
        ));
    }
    status
}
pub(crate) fn status() -> serde_json::Value {
    let wait = WAIT.lock().unwrap();
    serde_json::json!({"view": wait.view, "remaining": wait.remaining, "consumed": wait.consumed})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targeting_and_budget_do_not_consume_another_views_fault() {
        let mut fault = WaitFault {
            view: "fixture::popup".into(),
            status: 128,
            remaining: 3,
            consumed: 0,
        };
        for _ in 0..10 {
            assert_eq!(fault.take("fixture"), None);
        }
        for _ in 0..3 {
            assert_eq!(fault.take("fixture::popup"), Some(128));
        }
        assert_eq!(fault.take("fixture::popup"), None);
        assert_eq!(fault.consumed, 3);
    }
}
