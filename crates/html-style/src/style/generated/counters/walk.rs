//! Box addresses and counter-reset cleanup share their immutable prefixes.
use super::*;
impl Counters {
    pub fn enter(&mut self) {
        let level = self.path.len();
        if self.next.len() <= level {
            self.next.push(0);
        }
        let idx = *self.next.get(level).expect("box ordinal");
        self.next.set(level, idx + 1);
        self.path.push(idx);
        self.next.truncate(level + 1);
        self.next.push(0);
    }
    pub fn leave(&mut self) {
        let mut i = 0;
        while i < self.resets.len() {
            if self.resets.get(i).expect("reset").0 == self.path {
                let (_, name) = self.resets.remove(i);
                if self.path.last() != Some(&PSEUDO_MARKER) {
                    self.drop_shadowed_reset(&name);
                }
            } else {
                i += 1;
            }
        }
        self.leave_quote_scope(self.path.len());
        self.path.pop();
        self.next.truncate(self.path.len() + 1);
    }
    pub fn enter_pseudo(&mut self, before: bool) {
        self.path
            .push(if before { PSEUDO_BEFORE } else { PSEUDO_AFTER });
        self.next.push(0);
    }
    pub fn enter_marker(&mut self) {
        self.path.push(PSEUDO_MARKER);
        self.next.push(0);
    }
}
