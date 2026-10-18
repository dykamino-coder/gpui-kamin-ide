//! CSS Overflow 3 §2.2: descendant overflow is intersected with ancestor clips.
use crate::style::computed::{Computed, Overflow, Sides};
use crate::style::values::value::Len;

pub(super) type Rect = [f32; 4];

pub(super) fn union(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

pub(super) fn sides(s: Sides, base: f32) -> [f32; 4] {
    let value = |v| match v {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(v)) => v * base,
        _ => 0.0,
    };
    [value(s.left), value(s.top), value(s.right), value(s.bottom)]
}

#[derive(Clone, Copy)]
pub(super) struct Clip {
    border: Sides,
    padding: Sides,
    axes: [u8; 2],
    margin: f32,
    reference: u8,
}
impl Clip {
    pub(super) fn new(c: &Computed) -> Self {
        let axis = |overflow| {
            if c.contain_layout == Some(true)
                || c.contain_paint == Some(true)
                || overflow == Some(Overflow::Clip)
            {
                1
            } else if matches!(overflow, Some(Overflow::Hidden | Overflow::Scroll)) {
                2
            } else {
                0
            }
        };
        Self {
            border: c.borders(),
            padding: c.padding,
            axes: [axis(c.overflow_x), axis(c.overflow_y)],
            margin: c.clip_margin.unwrap_or(0.0),
            reference: c.clip_margin_box.unwrap_or(1),
        }
    }
    pub(super) fn border(&self) -> [f32; 4] {
        sides(self.border, 0.0)
    }
    pub(super) fn intersect(&self, own: Rect, mut child: Rect) -> Option<Rect> {
        let border = self.border();
        let padding = sides(self.padding, own[2] - own[0]);
        for axis in 0..2 {
            if self.axes[axis] == 0 {
                continue;
            }
            let reference = if self.axes[axis] == 1 {
                self.reference
            } else {
                1
            };
            let margin = if self.axes[axis] == 1 {
                self.margin
            } else {
                0.0
            };
            let inset = |side| {
                if reference == 2 {
                    0.0
                } else {
                    border[side] + if reference == 0 { padding[side] } else { 0.0 }
                }
            };
            child[axis] = child[axis].max(own[axis] + inset(axis) - margin);
            child[axis + 2] = child[axis + 2].min(own[axis + 2] - inset(axis + 2) + margin);
            if child[axis] > child[axis + 2] {
                return None;
            }
        }
        Some(child)
    }
}

#[cfg(test)]
mod tests {
    //! Overflow bounds regression checks, independent of painting and DOM selectors.
    use super::*;

    #[test]
    fn clip_margin_exposes_only_the_intersection_of_descendant_overflow() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("overflow", "clip", vcx);
        c.apply_one("overflow-clip-margin", "10px", vcx);
        let own = [0.0, 0.0, 100.0, 100.0];
        let child = [-50.0, -50.0, 150.0, 150.0];
        assert_eq!(
            Clip::new(&c).intersect(own, child),
            Some([-10.0, -10.0, 110.0, 110.0])
        );
        c.apply_one("overflow-clip-margin", "100px", vcx);
        assert_eq!(Clip::new(&c).intersect(own, child), Some(child));
    }

    #[test]
    fn paint_containment_clips_overflow_but_hidden_ignores_clip_margin() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed {
            contain_paint: Some(true),
            clip_margin: Some(10.0),
            ..Default::default()
        };
        let own = [0.0, 0.0, 100.0, 100.0];
        let child = [-50.0, -50.0, 150.0, 150.0];
        assert_eq!(
            Clip::new(&c).intersect(own, child),
            Some([-10.0, -10.0, 110.0, 110.0])
        );
        c.contain_paint = None;
        c.apply_one("overflow", "hidden", vcx);
        assert_eq!(Clip::new(&c).intersect(own, child), Some(own));
    }
}
