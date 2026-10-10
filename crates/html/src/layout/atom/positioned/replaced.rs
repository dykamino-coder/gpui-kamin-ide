//! Абсолютный замещаемый атом: держатель, слой ICB и статическое место.

use crate::dom::Element;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::replaced::image::image;
use crate::layout::replaced::{inline_replaced_position, replaced_content};
use crate::paint::stacking::stacking_context;
use crate::render::{RenderOpts, inline_abs_paint_last, styled_div_with};
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
        crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
    } else if replaced_content::default_iframe(e) {
        // The holder owns the CSS box; empty content paints no second border.
        div().w_0().h_0().flex_shrink_0().into_any_element()
    } else {
        image(&copy)
    };
    let holder = styled_div_with(e, merged).child(built);
    let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
    let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
    if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        spot.set(crate::layout::positioned::containing_block::Spot {
            hole: None,
            next_line: None,
            fixed_axes: (x_set, y_set),
            line_thickness: line_height_px(inherited, opts),
            rtl: inherited.rtl == Some(true),
            vertical: inherited.vertical == Some(true),
            vertical_rl: inherited.vertical_rl == Some(true),
            own_vertical: e.style.vertical == Some(true),
            replaced: matches!(
                e.tag.as_str(),
                "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
            ),
            ..Default::default()
        });
        let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), false);
        return match inline_replaced_position::push(
            spot,
            inline_abs_paint_last(e, holder.into_any_element()),
            &e.style,
            inherited,
        ) {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    let below_icb = e.style.z_index.is_some_and(|z| z < 0) && !stacking_context(inherited);
    if x_set
        && y_set
        && !inline_cb
        && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
        && !(inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited))
    {
        let holder: AnyElement = if below_icb {
            crate::paint::effects::underlay::Underlay::new(holder.into_any_element())
                .into_any_element()
        } else {
            holder.into_any_element()
        };
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        spot.set(crate::layout::positioned::containing_block::Spot {
            fixed_axes: (true, true),
            rtl: inherited.rtl == Some(true),
            vertical: inherited.vertical == Some(true),
            vertical_rl: inherited.vertical_rl == Some(true),
            own_vertical: e.style.vertical == Some(true),
            replaced: matches!(
                e.tag.as_str(),
                "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
            ),
            ..Default::default()
        });
        // Слоя нет — элемент возвращается назад, и рисуем его на
        // месте прежним путём.
        match crate::layout::positioned::containing_block::icb_push(spot, holder.into_any_element())
        {
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
        holder.into_any_element()
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
