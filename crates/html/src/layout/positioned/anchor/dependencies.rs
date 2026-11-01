//! Зависимости от ещё не опубликованных якорей, без чтения другого кадра.

use super::{
    AnchorPlan,
    parse::{VIS_VALID, VIS_VISIBLE},
};
use crate::style::computed::BoxStyle;
use crate::style::values::value::{Len, anchor_get};
use crate::values::ValueCx;

pub(crate) struct Dependencies(Vec<Inputs>);

struct Inputs {
    plan: Option<AnchorPlan>,
    names: Vec<Option<String>>,
}

impl Dependencies {
    pub(crate) fn of(
        c: &BoxStyle,
        inherited: &BoxStyle,
        vcx: ValueCx<'_>,
        layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
    ) -> Self {
        let styles = super::trials::Options::new(c, inherited, vcx, layout).styles;
        Self(
            styles
                .iter()
                .map(|(_, c)| Inputs::of(c, inherited, vcx, layout))
                .collect(),
        )
    }

    pub(crate) fn ready(&self) -> bool {
        self.0.iter().all(Inputs::ready)
    }

    pub(crate) fn sequence(&self) -> u32 {
        self.0
            .iter()
            .find_map(|i| i.plan.as_ref().map(|p| p.seq))
            .unwrap_or(0)
    }

    /// Layout state of the document the dependent plans read.
    pub(crate) fn layout(&self) -> Option<std::rc::Rc<crate::layout::layout_state::LayoutState>> {
        self.0
            .iter()
            .find_map(|i| i.plan.as_ref().map(|p| p.layout.clone()))
    }
}

impl Inputs {
    fn of(
        c: &BoxStyle,
        inherited: &BoxStyle,
        vcx: ValueCx<'_>,
        layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
    ) -> Self {
        let plan = AnchorPlan::of(c, inherited, vcx, layout);
        let mut names = Vec::new();
        if plan
            .as_ref()
            .is_some_and(|p| p.area.is_some() || p.refs_default)
            || c.reset_positioned.position_visibility & (VIS_VALID | VIS_VISIBLE) != 0
        {
            names.push(None);
        }
        fn collect(l: Option<Len>, names: &mut Vec<Option<String>>, vcx: ValueCx<'_>, depth: u8) {
            if depth == 0 {
                return;
            }
            if let Some(Len::Anchor(i)) = l
                && let Some(f) = anchor_get(i, vcx)
            {
                names.push(f.name.clone());
                collect(f.fallback, names, vcx, depth - 1);
                for alt in &f.alts {
                    names.push(alt.name.clone());
                }
            }
        }
        for l in [
            c.reset_box_layout.width,
            c.reset_box_layout.height,
            c.reset_box_layout.min_width,
            c.reset_box_layout.min_height,
            c.reset_box_layout.max_width,
            c.reset_box_layout.max_height,
            c.reset_positioned.inset.top,
            c.reset_positioned.inset.right,
            c.reset_positioned.inset.bottom,
            c.reset_positioned.inset.left,
        ] {
            collect(l, &mut names, vcx, 4);
        }
        Self { plan, names }
    }

    fn ready(&self) -> bool {
        self.plan
            .as_ref()
            .is_none_or(|p| self.names.iter().all(|n| p.rec(n.as_deref()).is_some()))
    }
}

#[cfg(test)]
mod tests;
