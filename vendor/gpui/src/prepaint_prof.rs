//! KaminIDE patch: self-time of one element phase by element type, owned by
//! one application. The host may attach monotonic cost counters; each element
//! then also gets the part of their growth that happened outside its children.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

/// Number of host cost counters read by a [`CostProbe`].
pub const COSTS: usize = 4;
/// Current values of the host's monotonic cost counters.
pub type CostProbe = fn() -> [u64; COSTS];

/// Exclusive cost of one element type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ElementCost {
    /// Nanoseconds outside child elements of the same phase.
    pub self_ns: u64,
    pub calls: u32,
    /// Growth of the host counters outside child elements.
    pub self_costs: [u64; COSTS],
}

#[derive(Default)]
struct Open {
    children_ns: u64,
    children_costs: [u64; COSTS],
    start_costs: [u64; COSTS],
}

#[derive(Default)]
struct State {
    stack: Vec<Open>,
    times: HashMap<&'static str, ElementCost>,
    probe: Option<CostProbe>,
}

/// Explicit profiler; its stack is used only by the application's UI thread.
#[derive(Clone, Default)]
pub struct ElementProfile {
    state: Arc<Mutex<State>>,
    enabled: bool,
}

impl ElementProfile {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    /// Attach the host's cost counters to every following element.
    pub fn set_costs(&self, probe: CostProbe) {
        if let Ok(mut state) = self.state.lock() {
            state.probe = Some(probe);
        }
    }

    /// Start measuring an element without holding the lock over its children.
    pub fn enter(&self, name: &'static str) -> Option<Guard> {
        if !self.enabled {
            return None;
        }
        let mut state = self.state.lock().ok()?;
        let start_costs = state.probe.map(|probe| probe()).unwrap_or_default();
        state.stack.push(Open {
            start_costs,
            ..Open::default()
        });
        Some(Guard {
            started: Instant::now(),
            name,
            state: self.state.clone(),
        })
    }

    /// Top-N types: short name, total self-time in microseconds, calls.
    pub fn take_top(&self, n: usize) -> Vec<(String, u64, u32)> {
        let mut values: Vec<_> = self
            .take()
            .into_iter()
            .map(|(name, cost)| {
                let short = name.rsplit("::").next().unwrap_or(name).to_string();
                (short, cost.self_ns / 1_000, cost.calls)
            })
            .collect();
        values.sort_by(|a, b| b.1.cmp(&a.1));
        values.truncate(n);
        values
    }

    /// Every measured type by its full name since the last take.
    pub fn take(&self) -> Vec<(&'static str, ElementCost)> {
        let Ok(mut state) = self.state.lock() else {
            return Vec::new();
        };
        state.times.drain().collect()
    }
}

/// RAII measurement: on unwind it restores the stack of the same profiler.
pub struct Guard {
    started: Instant,
    name: &'static str,
    state: Arc<Mutex<State>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let total = u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if let Ok(mut state) = self.state.lock() {
            let end_costs = state.probe.map(|probe| probe()).unwrap_or_default();
            let open = state.stack.pop().unwrap_or_default();
            let grown: [u64; COSTS] =
                std::array::from_fn(|i| end_costs[i].saturating_sub(open.start_costs[i]));
            if let Some(parent) = state.stack.last_mut() {
                parent.children_ns += total;
                for (child, grew) in parent.children_costs.iter_mut().zip(grown) {
                    *child += grew;
                }
            }
            let entry = state.times.entry(self.name).or_default();
            entry.self_ns += total.saturating_sub(open.children_ns);
            entry.calls += 1;
            for ((own, grew), children) in entry
                .self_costs
                .iter_mut()
                .zip(grown)
                .zip(open.children_costs)
            {
                *own += grew.saturating_sub(children);
            }
        }
    }
}
