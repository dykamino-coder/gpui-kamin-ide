//! CSS counter operations preserve prior snapshots while changing one named stack.
use super::*;
impl Counters {
    pub fn reset(&mut self, name: &str, value: i64) {
        self.reset_flagged(name, value, false);
    }
    pub fn reset_flagged(&mut self, name: &str, value: i64, reversed: bool) {
        let mut st = self.stack.pruned(name, &self.path).unwrap_or_default();
        if st
            .last()
            .is_some_and(|top| same_parent(&top.owner, &self.path))
        {
            st.pop();
        }
        st.push(Entry {
            owner: self.path.clone(),
            value,
            reversed,
        });
        self.stack.put(name, st);
        if !self
            .resets
            .iter()
            .any(|(o, n)| *o == self.path && n.as_ref() == name)
        {
            self.resets.push((self.path.clone(), Rc::from(name)));
        }
    }
    pub(super) fn drop_shadowed_reset(&mut self, name: &str) {
        let Some(mut st) = self.stack.pruned(name, &self.path) else {
            return;
        };
        if st.len() < 2 || st.last().is_none_or(|e| e.owner != self.path) {
            return;
        }
        let prev = &st.get(st.len() - 2).expect("outer counter").owner;
        if self
            .boundaries
            .last()
            .is_some_and(|root| !(prev.len() > root.len() && covers(root, prev)))
        {
            return;
        }
        let pp_len = prev.len().saturating_sub(1);
        if covers(prev, &self.path)
            || (prefix(prev, pp_len, &self.path) && !same_parent(prev, &self.path))
        {
            st.pop();
            self.stack.put(name, st);
        }
    }
    pub fn is_reversed(&mut self, name: &str) -> bool {
        let Some(st) = self.stack.pruned(name, &self.path) else {
            return false;
        };
        st.last().is_some_and(|e| {
            self.boundaries
                .last()
                .is_none_or(|root| e.owner.len() > root.len() && covers(root, &e.owner))
                && e.reversed
        })
    }
    pub fn update(&mut self, name: &str, delta: i64, is_set: bool) {
        let mut st = self.stack.pruned(name, &self.path).unwrap_or_default();
        let local = self.boundaries.last().is_none_or(|root| {
            st.last()
                .is_some_and(|e| e.owner.len() > root.len() && covers(root, &e.owner))
        });
        if let Some(mut top) = st.last().filter(|_| local).cloned() {
            top.value = if is_set {
                delta
            } else {
                top.value.saturating_add(delta)
            };
            st.set(st.len() - 1, top);
        } else {
            st.push(Entry {
                owner: self.path.clone(),
                value: delta,
                reversed: false,
            });
        }
        self.stack.put(name, st);
    }
    pub fn value_of(&mut self, name: &str) -> i64 {
        self.stack
            .pruned(name, &self.path)
            .and_then(|st| st.last().map(|e| e.value))
            .unwrap_or(0)
    }
    pub fn chain_of(&mut self, name: &str) -> Vec<i64> {
        match self
            .stack
            .pruned(name, &self.path)
            .filter(|st| !st.is_empty())
        {
            Some(st) => st.iter().map(|e| e.value).collect(),
            None => vec![0],
        }
    }
}
