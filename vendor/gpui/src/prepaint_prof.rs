//! KaminIDE patch: self-time prepaint, принадлежащий одному приложению.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
struct State {
    stack: Vec<u64>,
    times: HashMap<&'static str, (u64, u32)>,
}

/// Явный профилировщик: стек используется только UI-потоком приложения.
#[derive(Clone, Default)]
pub struct PrepaintProfile {
    state: Arc<Mutex<State>>,
    enabled: bool,
}

impl PrepaintProfile {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    /// Начать замер элемента, не удерживая блокировку во время обхода детей.
    pub fn enter(&self, name: &'static str) -> Option<Guard> {
        if !self.enabled {
            return None;
        }
        self.state.lock().ok()?.stack.push(0);
        Some(Guard {
            started: Instant::now(),
            name,
            state: self.state.clone(),
        })
    }

    /// Топ-N типов: имя, суммарные микросекунды self-time, число вызовов.
    pub fn take_top(&self, n: usize) -> Vec<(String, u64, u32)> {
        let Ok(mut state) = self.state.lock() else {
            return Vec::new();
        };
        let mut values: Vec<_> = state
            .times
            .drain()
            .map(|(name, (us, count))| {
                (name.rsplit("::").next().unwrap_or(name).to_string(), us, count)
            })
            .collect();
        values.sort_by(|a, b| b.1.cmp(&a.1));
        values.truncate(n);
        values
    }
}

/// RAII-замер: при unwind восстанавливает стек того же профилировщика.
pub struct Guard {
    started: Instant,
    name: &'static str,
    state: Arc<Mutex<State>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let total = self.started.elapsed().as_micros() as u64;
        if let Ok(mut state) = self.state.lock() {
            let children = state.stack.pop().unwrap_or(0);
            if let Some(parent) = state.stack.last_mut() {
                *parent += total;
            }
            let entry = state.times.entry(self.name).or_default();
            entry.0 += total.saturating_sub(children);
            entry.1 += 1;
        }
    }
}
