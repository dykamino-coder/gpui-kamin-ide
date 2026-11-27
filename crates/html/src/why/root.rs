//! Bounded, document-owned wall-clock phases of a retained root attempt.
//! Inclusive intervals contain nested phases; exclusive intervals partition work.
//! The host gap includes all host work between this root's callbacks.

use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

mod attempt;

const LIMIT: usize = 64;
const NAMES: [&str; 6] = [
    "build",
    "request_layout",
    "layout_as_root",
    "host_layout_between_request_and_prepaint",
    "prepaint",
    "paint",
];

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Build,
    RequestLayout,
    Layout,
    HostLayout,
    Prepaint,
    Paint,
}

/// Lazy storage: an unobserved document allocates no trace and reads no clock.
#[derive(Clone, Default)]
pub(crate) struct Journal(RefCell<Option<Rc<RefCell<Log>>>>);

#[derive(Default)]
struct Log {
    next: u64,
    attempts: Vec<Attempt>,
    dropped: u64,
}

#[derive(Clone, Default)]
pub(crate) struct Attempt(Option<Rc<RefCell<Data>>>);

struct Data {
    identity: Value,
    inclusive: [Duration; 6],
    exclusive: [Duration; 6],
    calls: [u64; 6],
    active: Option<Phase>,
    changed: Option<Instant>,
    gap: Option<Instant>,
    live: bool,
    painted: bool,
}

pub(crate) struct Scope(Option<(Attempt, Phase, Option<Phase>, Instant)>);

impl Journal {
    pub(crate) fn begin(
        &self,
        document: u64,
        tree: u64,
        prepared: bool,
        opts: &crate::RenderOpts,
        root_scroller: bool,
    ) -> Attempt {
        if !super::enabled() {
            return Attempt::default();
        }
        let owner: Rc<RefCell<Log>> = self
            .0
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .clone();
        let mut log = owner.borrow_mut();
        log.next += 1;
        if log.attempts.len() == LIMIT {
            log.dropped += 1;
            return Attempt::default();
        }
        let attempt = Attempt(Some(Rc::new(RefCell::new(Data {
            identity: json!({"document": document, "tree": tree, "attempt": log.next,
                "prepared": prepared, "viewport": opts.viewport,
                "freeze_scrollbars": opts.cx.deferred.freeze_scrollbars,
                "root_scroller": root_scroller}),
            inclusive: [Duration::ZERO; 6],
            exclusive: [Duration::ZERO; 6],
            calls: [0; 6],
            active: None,
            changed: None,
            gap: None,
            live: true,
            painted: false,
        }))));
        log.attempts.push(attempt.clone());
        attempt
    }

    /// A checkpoint invalidates pending gaps and scopes, including abandoned roots.
    pub(crate) fn take(&self) -> Value {
        let Some(stored) = self.0.borrow().clone() else {
            return json!({"clock": "wall", "attempts": [], "dropped": 0});
        };
        let owner: Rc<RefCell<Log>> = stored;
        let mut log = owner.borrow_mut();
        let attempts: Vec<Value> = log
            .attempts
            .drain(..)
            .map(|attempt| {
                let cell: &RefCell<Data> = attempt.0.as_deref().unwrap();
                let mut data = cell.borrow_mut();
                data.live = false;
                let mut out = data.identity.clone();
                out["painted"] = json!(data.painted);
                out["pending_host_layout"] = json!(data.gap.take().is_some());
                for (i, name) in NAMES.iter().enumerate() {
                    out["phases_ms"][name] = json!({
                        "inclusive": data.inclusive[i].as_secs_f64() * 1000.,
                        "exclusive": data.exclusive[i].as_secs_f64() * 1000.,
                        "calls": data.calls[i],
                    });
                }
                out["exclusive_total_ms"] =
                    json!(data.exclusive.iter().sum::<Duration>().as_secs_f64() * 1000.);
                out
            })
            .collect();
        json!({"clock": "wall", "attempts": attempts, "dropped": std::mem::take(&mut log.dropped)})
    }
}

#[cfg(test)]
mod tests;
