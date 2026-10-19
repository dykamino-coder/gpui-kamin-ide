//! The shape-estimate record of a stack child, restored when pagination fails.

/// The stack record of a child before pagination (`paginate_one`).
pub(super) struct Saved {
    el: gpui::AnyElement,
    frags: Vec<gpui::AnyElement>,
    h: f32,
    over: f32,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    monolith: bool,
    avoid_only: bool,
    overflow_top: bool,
}

impl Saved {
    pub(super) fn take(c: &mut crate::layout::fragment::types::StackChild) -> Self {
        use gpui::IntoElement;
        Saved {
            el: std::mem::replace(&mut c.el, gpui::Empty.into_any_element()),
            frags: std::mem::take(&mut c.frags),
            h: c.h,
            over: c.over,
            cuts: c.cuts.clone(),
            forced: c.forced.clone(),
            solid: c.solid.clone(),
            monolith: c.monolith,
            avoid_only: c.par.avoid_only,
            overflow_top: c.overflow_top,
        }
    }

    pub(super) fn restore(self, c: &mut crate::layout::fragment::types::StackChild) {
        c.el = self.el;
        c.frags = self.frags;
        c.h = self.h;
        c.over = self.over;
        c.cuts = self.cuts;
        c.forced = self.forced;
        c.solid = self.solid;
        c.monolith = self.monolith;
        c.par.avoid_only = self.avoid_only;
        c.overflow_top = self.overflow_top;
    }
}
