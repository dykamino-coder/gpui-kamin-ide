//! Безопасное самовыравнивание абсолюта в flex-контейнере (css-align-3 §auto-safety-position):
//! `align-self: safe …` откладывается до `prepaint`, когда известны и статическая
//! прямоугольная область flex-родителя (щуп-канвас `probe`), и исходный содержащий блок,
//! и коробка сдвигается по их объединению (`SafePlace` поверх `absolute_overflow_math`).

use crate::document::state::ValueCx;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::positioned::absolute_overflow_math::{Span, place};
use crate::layout::positioned::containing_block::Positioning;
use crate::style::computed::{Align, Computed, Display, FlexDir, Position};
use crate::style::values::value::Len;
use gpui::{AnyElement, App, Bounds, Pixels, Window, point, px};
use std::rc::Rc;

/// Probe of a flex container's content box; it records into the build
/// pass's `Positioning`, where the child's `Plan` reads it.
pub(crate) fn probe(
    style: &Computed,
    positioning: &Rc<Positioning>,
    vcx: ValueCx<'_>,
) -> Option<AnyElement> {
    if !matches!(style.display, Some(Display::Flex | Display::InlineFlex)) || style.self_node == 0 {
        return None;
    }
    let id = style.self_node;
    let padding = style.padding;
    let sink = positioning.flex_content.clone();
    let values = vcx.keep();
    Some(sink.record_with(move |g, window| {
        let bounds = g.bounds;
        let vcx = values.vcx();
        let side = |length: Option<Len>| {
            crate::paint::apply::len_to_gpui(length.unwrap_or(Len::Px(0.0)), vcx)
                .to_pixels(bounds.size.width.into(), window.rem_size())
        };
        let left = side(padding.left);
        let top = side(padding.top);
        let content = Bounds::new(
            bounds.origin + point(left, top),
            gpui::size(
                (bounds.size.width - left - side(padding.right)).max(px(0.0)),
                (bounds.size.height - top - side(padding.bottom)).max(px(0.0)),
            ),
        );
        (id, content)
    }))
}

pub(crate) struct Plan {
    flex: u64,
    /// The build pass the flex parent's probe records into.
    positioning: Rc<Positioning>,
    cb: u64,
    horizontal: bool,
    from_end: bool,
    align: Align,
    margin: [Option<Len>; 2],
}
impl Plan {
    pub(crate) fn new(
        own: &Computed,
        parent: &Computed,
        positioning: &Rc<Positioning>,
    ) -> Option<Self> {
        if own.position != Some(Position::Absolute)
            || !matches!(parent.display, Some(Display::Flex | Display::InlineFlex))
            || parent.self_node == 0
            || own.cb_node == parent.self_node
            || own.position_area.is_some()
        {
            return None;
        }
        // A flex container that is itself the original CB already supplies
        // both bounds to native layout; deferred union placement is unnecessary.
        let safe = if own.align_self.is_some() {
            own.align_self_safe
        } else {
            parent.align_items_safe
        };
        let align = own.align_self.or(parent.align_items)?;
        if !safe || !matches!(align, Align::Center | Align::End | Align::Start) {
            return None;
        }
        // Percentage padding requires the parent's actual used box edges;
        // its own width is not necessarily the percentage resolution basis.
        if [
            parent.padding.top,
            parent.padding.right,
            parent.padding.bottom,
            parent.padding.left,
        ]
        .into_iter()
        .any(|v| !matches!(v, None | Some(Len::Px(_))))
        {
            return None;
        }
        let column = matches!(parent.flex_dir, Some(FlexDir::Col | FlexDir::ColReverse));
        let vertical = parent.wm().vertical();
        let horizontal = column != vertical;
        let (inset, margin) = if horizontal {
            (
                [own.inset.left, own.inset.right],
                [own.margin.left, own.margin.right],
            )
        } else {
            (
                [own.inset.top, own.inset.bottom],
                [own.margin.top, own.margin.bottom],
            )
        };
        if inset
            .into_iter()
            .any(|v| !matches!(v, None | Some(Len::Auto)))
            || margin.contains(&Some(Len::Auto))
        {
            return None;
        }
        let from_end = if column {
            parent.wm().rtl()
        } else {
            vertical && parent.wm().block_rl()
        };
        Some(Self {
            flex: parent.self_node,
            positioning: positioning.clone(),
            cb: own.cb_node,
            horizontal,
            from_end,
            align,
            margin,
        })
    }
    pub(crate) fn prepare(&self, style: &mut gpui::StyleRefinement) {
        // Native layout supplies the requested static-position alignment; safety
        // is applied once both geometries are available during prepaint.
        style.align_self = Some(crate::paint::apply::self_align(self.align, false));
        let (items, _, content, justify) = style.safe_alignment.unwrap_or_default();
        style.safe_alignment = Some((items, false, content, justify));
    }
    pub(crate) fn wrap(self, child: AnyElement, vcx: ValueCx<'_>) -> AnyElement {
        SafePlace {
            child,
            plan: self,
            values: vcx.keep(),
        }
        .hooked()
    }
    fn shift(
        &self,
        bounds: Bounds<Pixels>,
        window: &Window,
        vcx: ValueCx<'_>,
    ) -> gpui::Point<Pixels> {
        let Some(flex) = self.positioning.flex_content(self.flex) else {
            return point(px(0.0), px(0.0));
        };
        let cb = if self.cb == 0 {
            Some(Bounds::new(point(px(0.0), px(0.0)), window.viewport_size()))
        } else {
            crate::layout::positioned::anchor::containing_bounds(self.cb, vcx)
        };
        let Some(cb) = cb else {
            return point(px(0.0), px(0.0));
        };
        let axis = |rect: Bounds<Pixels>| {
            if self.horizontal {
                (f32::from(rect.origin.x), f32::from(rect.size.width))
            } else {
                (f32::from(rect.origin.y), f32::from(rect.size.height))
            }
        };
        let (origin, extent) = axis(bounds);
        let (start, size) = axis(flex);
        let (cb_start, cb_size) = axis(cb);
        let margin = self.margin.map(|length| {
            f32::from(
                crate::paint::apply::len_to_gpui(length.unwrap_or(Len::Px(0.0)), vcx)
                    .to_pixels(cb.size.width.into(), window.rem_size()),
            )
        });
        let placed = place(
            (extent + margin[0] + margin[1]).max(0.0),
            origin - margin[0],
            Span {
                start,
                end: start + size,
            },
            Span {
                start: cb_start,
                end: cb_start + cb_size,
            },
            self.from_end,
        );
        let delta = px(placed + margin[0] - origin);
        if self.horizontal {
            point(delta, px(0.0))
        } else {
            point(px(0.0), delta)
        }
    }
}

struct SafePlace {
    child: AnyElement,
    plan: Plan,
    /// Values of the document the plan's lengths come from.
    values: crate::document::state::ValueDoc,
}
impl Wrap for SafePlace {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let shift = self.plan.shift(bounds, window, self.values.vcx());
        window.with_exact_element_offset(shift, |window| self.child.prepaint(window, cx));
    }
}

#[cfg(test)]
mod tests;
