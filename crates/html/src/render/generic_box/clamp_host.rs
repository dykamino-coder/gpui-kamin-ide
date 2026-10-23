//! Мост общей коробки к ClampHost: одинаковый каркас Measure и Apply.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::style::computed::{BoxStyle, Computed};
use crate::text::clamp::{ClampHost, ClampInput};
use gpui::{AnyElement, Refineable, Styled};
mod frame;
use std::rc::Rc;

pub(super) fn active(e: &Element, c: &Computed) -> bool {
    (e.style.used().clamp_lines().is_some()
        || (e.style.used().clamp_by_height()
            && e.style.used().clamp_block_container()
            && auto_clamp_limit(c).is_some()))
        && !crate::layout::multicol::spanner::multicol_container(&e.style.used())
}

pub(super) fn build(
    e: &Element,
    c: BoxStyle,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
    mut d: gpui::Div,
) -> AnyElement {
    let mut style = gpui::Style::default();
    style.refine(d.style());
    // Native leaf layout разрешает padding от исходного содержащего блока
    // и прибавляет его к измеренному размеру содержимого ровно один раз.
    let geometry = crate::text::clamp::geometry::ClampGeometry {
        limit: e.style.used().clamp_lines(),
        max_h: auto_clamp_limit(&c).filter(|_| e.style.used().clamp_by_height()),
        trim_end: if c.text_box_trim_end {
            crate::text::text_box::text_box_trim_px(&c, false, opts)
        } else {
            0.0
        },
    };
    let key = e.node_id;
    let border_box = c.border_box == Some(true);
    let paint = opts.cx.deferred.paint.clone();
    let children = opts.cx.contracts.build;
    let e = e.clone();
    let inherited = inherited.clone();
    let positioning = opts.cx.position.state.retain();
    let opts = opts.clone();
    let build = Rc::new(move |input: ClampInput, width, used_box, publish| {
        let opts = opts.inner(|cx| {
            cx.clamp.build = cx.clamp.build.container(key);
            cx.clamp.input = Some(input);
            cx.position.state = if publish {
                positioning.resume()
            } else {
                positioning.isolate().0
            };
            cx.clamp.frame = Some((key, width));
        });
        super::generic_box(
            &e,
            frame::resolved_style(&c, used_box, opts.cx.vcx()),
            &inherited,
            &opts,
            outer_row,
        )
    });
    ClampHost::new(key, style, geometry, build, border_box, paint, children).hooked()
}

pub(super) fn frame(d: &mut gpui::Div, width: Option<gpui::Pixels>) {
    let s = d.style();
    s.margin = Default::default();
    s.inset = Default::default();
    s.position = Some(gpui::Position::Relative);
    if let Some(width) = width {
        s.size.width = Some(width.into());
        s.min_size.width = None;
        s.max_size.width = None;
        let mut calc = s.calc_size.unwrap_or_default();
        calc[0] = None;
        calc[2] = None;
        s.calc_size = Some(calc);
        let mut max_calc = s.calc_size_max.unwrap_or_default();
        max_calc[0] = None;
        s.calc_size_max = Some(max_calc);
        for keywords in [
            &mut s.sizing_keywords,
            &mut s.min_sizing_keywords,
            &mut s.max_sizing_keywords,
        ] {
            let mut axes = keywords.unwrap_or_default();
            axes[0] = None;
            *keywords = Some(axes);
        }
        s.content_box = Some(false);
    }
}
