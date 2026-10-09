//! Bounded counters and deterministic top-N; no payloads or environment values.

use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Counts(BTreeMap<String, u64>);
impl Counts {
    pub(super) fn add(&mut self, name: &str) {
        let name = if self.0.contains_key(name) || self.0.len() < 64 {
            name
        } else {
            "other"
        };
        *self.0.entry(name.chars().take(160).collect()).or_default() += 1;
    }
    pub(super) fn take_top(&mut self, n: usize) -> Vec<(String, u64)> {
        let mut rows: Vec<_> = std::mem::take(&mut self.0).into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        rows.truncate(n);
        rows
    }
}

pub(super) fn changed(last: &mut Option<String>, next: String) -> Option<String> {
    if last.as_ref() == Some(&next) {
        return None;
    }
    *last = Some(next.clone());
    Some(next)
}

pub(super) fn line(
    cpu_ms: Option<u64>,
    frames: &[(String, u64)],
    ticks: &[(String, u64)],
) -> Option<String> {
    if cpu_ms.unwrap_or(0) == 0 && frames.is_empty() && ticks.is_empty() {
        return None;
    }
    Some(format!(
        "[why] cpu_ms={cpu_ms:?} cef_top5={frames:?} native_input_wake_pull={ticks:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn top_n_is_sorted_and_resets_counters() {
        let mut counts = Counts::default();
        for id in ["b", "a", "b", "c", "a", "b"] {
            counts.add(id);
        }
        assert_eq!(counts.take_top(2), vec![("b".into(), 3), ("a".into(), 2)]);
        assert!(counts.take_top(5).is_empty());
    }
    #[test]
    fn zero_summary_is_suppressed_and_names_cannot_split_records() {
        assert!(line(Some(0), &[], &[]).is_none());
        let record = line(None, &[("view\nname".into(), 1)], &[]).unwrap();
        assert!(!record.contains('\n'));
        assert!(record.contains("view\\nname"));
    }
    #[test]
    fn unbounded_view_names_do_not_expand_counter_storage() {
        let mut counts = Counts::default();
        for id in 0..1000 {
            counts.add(&id.to_string());
        }
        assert!(counts.0.len() <= 65);
    }
    #[test]
    fn motion_policy_emits_boot_and_changes_without_repeating_unchanged_state() {
        let mut last = None;
        assert!(changed(&mut last, "reduce=true source=rdp".into()).is_some());
        assert!(changed(&mut last, "reduce=true source=rdp".into()).is_none());
        assert!(changed(&mut last, "reduce=false source=rdp".into()).is_some());
        assert!(changed(&mut last, "reduce=false source=env".into()).is_some());
    }
}
