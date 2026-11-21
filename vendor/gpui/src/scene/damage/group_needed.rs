//! Select groups intersecting frame damage and preserve their dependencies.
use super::*;

impl Scene {
    /// KaminIDE patch: groups whose frame surface intersects `x, y, width, height`,
    /// and groups nested inside a needed group.
    #[doc(hidden)]
    pub fn groups_needed(&self, rect: [u32; 4]) -> Vec<bool> {
        let r = [
            rect[0] as f32,
            rect[1] as f32,
            (rect[0] + rect[2]) as f32,
            (rect[1] + rect[3]) as f32,
        ];
        self.groups_needed_where(|a| a[0] < r[2] && a[2] > r[0] && a[1] < r[3] && a[3] > r[1])
    }

    /// KaminIDE patch: one surface/dependency traversal with a backend damage query.
    #[doc(hidden)]
    pub fn groups_needed_where(&self, intersects: impl Fn([f32; 4]) -> bool) -> Vec<bool> {
        let mut needed = vec![false; self.groups.len()];
        for (i, s) in self.surfaces.iter().enumerate() {
            if s.group == 0 {
                continue;
            }
            let entry = (7 << KIND_SHIFT) | i as u32;
            let a = entry_area(self, entry).unwrap_or(FULL);
            let hit = a.iter().any(|v| v.is_nan()) || intersects(a);
            if hit && let Some(n) = needed.get_mut(s.group as usize - 1) {
                *n = true;
            }
        }
        // An enclosing group follows its nested groups in the list.
        for g in (0..self.groups.len()).rev() {
            if !needed[g] {
                continue;
            }
            for s in &self.groups[g].scene.surfaces {
                if s.group > 0
                    && let Some(n) = needed.get_mut(s.group as usize - 1)
                {
                    *n = true;
                }
            }
        }
        needed
    }
}
