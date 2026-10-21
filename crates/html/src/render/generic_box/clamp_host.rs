//! Мост общей коробки к ClampHost: одинаковый каркас Measure и Apply.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::style::computed::Computed;
use crate::style::values::value::px_lossy_zero;
use crate::text::clamp::{ClampHost, ClampInput};
use gpui::{AnyElement, Refineable, Styled};
use std::rc::Rc;

pub(super) fn active(e: &Element, c: &Computed) -> bool {
    (e.style.clamp_lines().is_some()
        || (e.style.clamp_auto == Some(true)
            && e.style.clamp_block_container()
            && auto_clamp_limit(c).is_some()))
        && !crate::layout::multicol::spanner::multicol_container(&e.style)
}

pub(super) fn build(
    e: &Element,
    c: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
    mut d: gpui::Div,
) -> AnyElement {
    let mut style = gpui::Style::default();
    style.refine(d.style());
    // Внешняя коробка участвует в родительском layout. Весь её внутренний
    // каркас, включая border/padding, строится одной функцией ниже.
    style.border_widths = Default::default();
    style.padding = Default::default();
    let side = px_lossy_zero;
    let b = c.borders();
    let bp = side(b.top) + side(b.bottom) + side(c.padding.top) + side(c.padding.bottom);
    let bb = if c.border_box == Some(true) { bp } else { 0.0 };
    let geometry = crate::text::clamp::cut_element::ClampGeometry {
        limit: e.style.clamp_lines(),
        max_h: auto_clamp_limit(&c)
            .filter(|_| e.style.clamp_auto == Some(true))
            .map(|v| (v - bb).max(0.0)),
        trim_end: if c.text_box_trim_end {
            crate::text::text_box::text_box_trim_px(&c, false, opts)
        } else {
            0.0
        },
    };
    let (key, top) = (e.node_id, side(b.top) + side(c.padding.top));
    let paint = opts.cx.paint.clone();
    let e = e.clone();
    let inherited = inherited.clone();
    let positioning = opts.cx.positioning.defer();
    let opts = opts.clone();
    let build = Rc::new(move |input: ClampInput, width, publish| {
        let opts = opts.inner(|cx| {
            cx.clamp_build = cx.clamp_build.container(key);
            cx.clamp_input = Some(input);
            cx.positioning = positioning.fork(publish);
            cx.clamp_frame = Some((key, width));
        });
        super::generic_box(&e, c.clone(), &inherited, &opts, outer_row)
    });
    ClampHost::new(key, style, geometry, build, top, paint).hooked()
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
