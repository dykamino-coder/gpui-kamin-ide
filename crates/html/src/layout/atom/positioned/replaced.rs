//! Абсолютный замещаемый атом: держатель, слой ICB и статическое место.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::replaced::image::image;
use crate::layout::replaced::{inline_replaced_position, replaced_content};
use crate::paint::box_tree::styled::styled_div_with;
use crate::paint::stacking::inline_abs::inline_abs_paint_last;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(super) fn replaced_abs_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
    merged: &Computed,
    inline_cb: bool,
    stretched: bool,
) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    let mut copy = e.clone();
    copy.style.image_orient_none = merged.image_orient_none;
    copy.style.position = None;
    copy.style.margin = Default::default();
    if matches!(copy.style.width, Some(Len::Pct(_))) {
        copy.style.width = Some(Len::Pct(1.0));
    }
    if matches!(copy.style.height, Some(Len::Pct(_))) {
        copy.style.height = Some(Len::Pct(1.0));
    }
    let built = if e.tag == "svg" {
        crate::svg::element_with_html(&copy, opts).unwrap_or_else(|| image(&copy, &opts.cx))
    } else if replaced_content::default_iframe(e) {
        // The holder owns the CSS box; empty content paints no second border.
        div().w_0().h_0().flex_shrink_0().into_any_element()
    } else {
        image(&copy, &opts.cx)
    };
    let holder = styled_div_with(e, merged, &opts.cx)
        .child(built)
        .into_any_element();
    // Shape the CSS box before moving it into a deferred positioned layer.
    let holder = if merged.corner_shaped() {
        crate::paint::effects::grouped::grouped_replaced(holder, merged, vcx)
    } else {
        holder
    };
    let holder = crate::layout::positioned::anchor::place::place(holder, merged, inherited, vcx);
    let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
    let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
    if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        spot.set(crate::layout::positioned::containing_block::Spot {
            hole: None,
            next_line: None,
            fixed_axes: (x_set, y_set),
            line_thickness: line_height_px(inherited, opts),
            rtl: inherited.wm().rtl(),
            vertical: inherited.wm().vertical(),
            vertical_rl: inherited.wm().block_rl(),
            own_vertical: e.style.wm().vertical(),
            replaced: matches!(
                e.tag.as_str(),
                "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
            ),
            ..Default::default()
        });
        let probe = crate::layout::positioned::containing_block::spot_probe(
            spot.clone(),
            false,
            opts.cx.line_box.clone(),
            &opts.cx,
        );
        return match inline_replaced_position::push(
            spot,
            inline_abs_paint_last(e, holder, &opts.cx),
            &e.style,
            inherited,
            &opts.cx.positioning,
        ) {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    let below_icb = e.style.z_index.is_some_and(|z| z < 0)
        && !crate::paint::stacking::nested_context(inherited);
    // Позиционированный предок есть — слой ЕГО содержащего блока
    // (CSS 2.1 §10.1 п.4): держатель нулевого размера в строке краёв
    // блока не знает, и `bottom: 0` отсчитывался от верха строки
    // (`baseline-shift-bottom-ref`: картинка 60×60 вылезала над блоком).
    // Отрицательный `z-index` у такого предка остаётся на месте.
    let positioned_cb = inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited);
    if x_set
        && y_set
        && !inline_cb
        && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
        && !(positioned_cb && below_icb)
    {
        let holder: AnyElement = if below_icb {
            crate::paint::effects::underlay::Underlay::new(holder).hooked()
        } else {
            holder
        };
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        spot.set(crate::layout::positioned::containing_block::Spot {
            fixed_axes: (true, true),
            rtl: inherited.wm().rtl(),
            vertical: inherited.wm().vertical(),
            vertical_rl: inherited.wm().block_rl(),
            own_vertical: e.style.wm().vertical(),
            replaced: matches!(
                e.tag.as_str(),
                "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
            ),
            ..Default::default()
        });
        // Слоя нет — элемент возвращается назад, и рисуем его на
        // месте прежним путём.
        let pushed = if positioned_cb {
            opts.cx.positioning.cb_push(
                spot,
                holder.into_any_element(),
                crate::layout::positioned::containing_block::nearest_cb_index(inherited),
            )
        } else {
            opts.cx
                .positioning
                .icb_push(spot, holder.into_any_element())
        };
        match pushed {
            None => {
                return Some(div().w_0().h_0().flex_shrink_0().into_any_element());
            }
            Some(kept) => {
                return Some(
                    div()
                        .w_0()
                        .h_0()
                        .flex_shrink_0()
                        .child(kept)
                        .into_any_element(),
                );
            }
        }
    }
    Some(if stretched {
        holder
    } else {
        div()
            .w_0()
            .h_0()
            .flex_shrink_0()
            .child(holder)
            .into_any_element()
    })
    // Поля несёт ДЕРЖАТЕЛЬ: снимали только позицию, и `margin`
    // прикладывался дважды — раз держателем, раз внутренней коробкой
    // (`absolute-replaced-width-050`: край 48 превращался в 72).
    // Долю размера держатель тоже несёт сам: внутри она считалась ОТ
    // НЕГО и выходила долей от доли — `width: 50%` давало четверть
    // содержащего блока (`absolute-replaced-width-006`). Внутренней
    // коробке остаётся заполнить держателя.
    // Замещаемый атом БЕЗ позиционированного предка считается от
    // начального содержащего блока (§10.1 п.4), а не от строки, где он
    // написан: с обеими заданными осями место в строке ему не нужно
    // вовсе. Тот же приём, что у блочного пути (`to_icb`).
    // CSS 2.1 §§10.1, 10.3.7, 10.6.4: explicit insets use the
    // containing block; the auto axis keeps its inline static spot.
    // Route to that block's layer, rather than the paragraph wrapper.
    // A negative `z-index` joins the ICB layer too, painted in the
    // bottom layer (CSS 2.1 §9.9 step 3) — in place it painted over
    // its later negative-z siblings (`shape-image-009`).
}
