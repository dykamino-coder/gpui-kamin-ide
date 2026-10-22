//! Позиционированный строчный атом: статическая позиция и абсолютная коробка.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::anchor;
use crate::layout::positioned::predicates::edge_set;

use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
mod paint_last;
mod replaced;
use paint_last::inline_abs_paint_last;
use replaced::replaced_abs_atom;

mod static_box;
pub(super) use static_box::static_position_atom;

pub(super) fn absolute_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if let Some(child) = anchor::trials::inline::defer(e, inherited, opts) {
        return Some(child);
    }
    let vcx = opts.cx.vcx();
    let mut merged = inherit(inherited, &e.style, vcx);
    anchor::prepare_inline_abs(&mut merged, e, inherited, &opts.cx);
    // Содержащий блок — позиционированный СТРОЧНЫЙ предок в этом абзаце
    // (CSS 2.1 §10.1 п.4): края по обеим осям считает `lines.rs` от
    // прямоугольника его фрагментов, коробка идёт без пустышки и слоёв.
    let offer = opts.cx.inline_cb.as_ref();
    let inline_cb = offer.is_some_and(|o| o.take())
        && e.style.position == Some(crate::style::computed::Position::Absolute)
        && (edge_set(e.style.inset.left) || edge_set(e.style.inset.right))
        && (edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom))
        && e.style.z_index.unwrap_or(0) >= 0;
    if inline_cb && let Some(o) = offer {
        o.note_taken();
    }
    // Проверено: отдавать элемент без пустышки, чтобы `inset: 0` считался от
    // позиционированного ПРЕДКА, ничего не меняет — в раскладке под нами
    // содержащим блоком служит ЛЮБОЙ родитель (`position-absolute-in-inline-005`
    // так и не зеленеет).
    // Пустышка нулевого размера сама становится содержащим блоком, и
    // края, заданные с ОБЕИХ сторон оси, схлопываются в ничто. Такому
    // элементу коробка нужна настоящая, поэтому он идёт без пустышки.
    // Остальным она нужна: без неё сдвигаются соседи
    // (`position-sticky-contained-by-display-table`).
    // Проверено: считать «растянутым» и элемент с ДОЛЕЙ размера, чтобы доля
    // не бралась от нулевой пустышки, ничего не меняет (*replaced*,
    // positioning/*, normal-flow/*, *float*).
    let stretched = inline_cb
        || (edge_set(e.style.inset.left) && edge_set(e.style.inset.right))
        || (edge_set(e.style.inset.top) && edge_set(e.style.inset.bottom))
        || matches!(e.style.width, Some(Len::Pct(_)));
    // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
    // путь блоков давал пустую коробку (clip-path-ellipse-2-ref: рисунок
    // absolute с left/top не рисовался вовсе).
    // Тот же список, что и у обычного пути: у замещаемых детей-блоков
    // нет, и блочная ветка давала пустую коробку — картинка с краями
    // не рисовалась вовсе.
    if matches!(
        e.tag.as_str(),
        "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
    ) {
        return replaced_abs_atom(inherited, e, opts, &merged, inline_cb, stretched);
    }
    // Якорный заместитель прозрачен для раскладки (отдаёт `layout_id`
    // ребёнка): вставки с `anchor()` раскладка видит нулями, сдвиг — его.
    let inner = anchor::place::place_in_context(
        opts.cx.build.styled_box(e, &merged, &opts.cx)
            .children(opts.cx.build.blocks(&e.children, &merged, opts))
            .into_any_element(),
        &merged,
        inherited,
        &opts.cx,
    );
    if anchor::trials::inline::is_root(e, opts) {
        return Some(inner);
    }
    // Незамещаемая коробка с РОВНО ОДНОЙ заданной осью — тем же щупом, что
    // и замещаемая выше: без этого `<span>` с одним краем падал в нулевой
    // держатель, и край считался от него (`abs-pos-non-replaced-vlr-017`:
    // `right: 2em` уводил коробку на свои же 160 влево).
    let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
    let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
    if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
        let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
        // A block-level box keeps the block-level static position on its free
        // axis (line start, next line after in-flow content) and aligns in
        // the inline parent's static-position rectangle, as the all-auto
        // branch above (horizontal ltr; `align-self-static-position-003/007`).
        let block_level = !crate::layout::positioned::static_position::hypothetical_inline_level(e)
            && !inherited.wm().vertical()
            && !inherited.wm().turned()
            && !inherited.wm().rtl();
        spot.set(crate::layout::positioned::containing_block::Spot {
            hole: None,
            next_line: block_level.then(|| line_height_px(inherited, opts)),
            self_align: if block_level {
                crate::layout::positioned::static_position::inline_static_self_align(e, inherited)
            } else {
                None
            },
            line_rect: true,
            fixed_axes: (x_set, y_set),
            line_thickness: line_height_px(inherited, opts),
            rtl: inherited.wm().rtl(),
            vertical: inherited.wm().vertical(),
            vertical_rl: inherited.wm().block_rl(),
            own_vertical: e.style.wm().vertical(),
            ..Default::default()
        });
        let probe = crate::layout::positioned::containing_block::spot_probe(
            spot.clone(),
            false,
            opts.cx.line_box.clone(),
            &opts.cx,
        );
        return match opts
            .cx
            .positioning
            .late_push(spot, inline_abs_paint_last(e, inner, &opts.cx))
        {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    if stretched {
        return Some(inner);
    }
    return Some(
        div()
            .w_0()
            .h_0()
            .flex_shrink_0()
            .child(inner)
            .into_any_element(),
    );
}

/// Проба строит сам CSS-бокс, не помещая его повторно в очередь слоя.
pub(crate) fn trial(inherited: &Computed, e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    if crate::layout::positioned::static_position::at_static_position(&e.style) {
        static_position_atom(inherited, e, opts)
    } else {
        absolute_atom(inherited, e, opts)
    }
}
