//! Пункт списка с заданным стилем: маркер, его позиция и содержимое.

use super::{shrink0, tabular_marker};
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::render::{blocks, styled_div_with};
use crate::style::computed::Computed;
use crate::text::inline;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

pub(crate) fn render_with_style(
    li: &Element,
    inherited: &Computed,
    merged: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let resolved = super::marker::resolve(li, inherited, merged, vcx);
    let no_marker = resolved.is_none();
    let (marker, marker_style) = resolved.map(|m| (m.text, m.style)).unwrap_or_default();
    // `inside`: маркер — ПЕРВЫЙ инлайновый кусок содержимого пункта
    // (css-lists-3 §4), поэтому он просто дописывается текстом в начало.
    // Своей колонки при этом нет, и текст пункта начинается там же, где
    // у обычного абзаца.
    let inside = merged.list_style_inside == Some(true);
    if inside {
        let mut kids: Vec<Node> = Vec::with_capacity(li.children.len() + 1);
        if !no_marker {
            kids.push(super::marker::node(
                li,
                merged,
                super::marker::Marker {
                    text: marker,
                    style: marker_style,
                },
            ));
        }
        kids.extend(li.children.iter().cloned());
        return shrink0(styled_div_with(li, merged, &opts.cx), li, inherited)
            .flex()
            .flex_col()
            .children(blocks(&kids, merged, opts))
            .into_any_element();
    }
    // Знаки маркера без своего семейства — шрифтом ДОКУМЕНТА: голый
    // `apply_text` без семейства брал шрифт интерфейса (Segoe UI), и
    // строковый маркер выходил чужой гарнитурой рядом с Times пункта
    // (`list-style-type-string-*`).
    let mut mark_style = marker_style.unwrap_or_else(|| merged.clone());
    if mark_style.font_family.as_deref().is_none_or(str::is_empty)
        && mark_style.monospace != Some(true)
    {
        mark_style.font_family = Some(opts.text.font_family.to_string());
    }
    tabular_marker(&mut mark_style, merged, li.style.marker_layer.as_deref());
    let marker = inline::transform_case(&marker, &mark_style);
    // Without line boxes the outside marker is top-aligned to the item, and
    // the item's content height is at least the marker's (csswg-drafts#2417,
    // #2418; Blink `UnpositionedListMarker::AddToBoxWithoutLineBoxes`). An
    // item with no content gets a hidden zero-width copy of the marker line.
    let empty = li.children.iter().all(crate::dom::whitespace::is_blank);
    let strut = (!no_marker && empty).then(|| {
        let mut d = crate::paint::apply::apply_text(div(), &mark_style, vcx)
            .w_0()
            .whitespace_nowrap()
            .child(SharedString::from(marker.clone()));
        d.style().visibility = Some(gpui::Visibility::Hidden);
        d
    });
    // Внешний маркер (css-lists-3 §list-style-position `outside`) висит
    // СНАРУЖИ коробки пункта, концом к началу содержимого, и текст пункта
    // не двигает. Text transforms apply to its own text (§3.1.1).
    shrink0(styled_div_with(li, merged, &opts.cx), li, inherited)
        .relative()
        .flex()
        .flex_col()
        .children((!no_marker).then(|| {
            // Знаки маркера набираются шрифтом и цветом ПУНКТА:
            // отдельной коробке текстовые свойства не достаются сами,
            // и маркер выходил чужой гарнитурой и кеглем. Выключка
            // текста на него НЕ переносится: маркер стоит у своего
            // края колонки, куда бы ни равнялся текст пункта
            // (`list-style-position-018`). Со слоем `::marker` —
            // стилем слоя: разрядка, межсловный пробел, шрифт и цвет
            // маркера объявлены на нём (css-lists-3
            // §marker-properties).
            // Сторона начала строки пункта: при `direction: rtl`
            // маркер висит СПРАВА от коробки и равняется к ней своим
            // левым краем (`list-style-type-string-003`: строка
            // маркера уходила за левый край окна).
            {
                let m = crate::paint::apply::apply_text(div(), &mark_style, vcx)
                    .absolute()
                    .top_0();
                // An outside marker sits outside the item's principal BORDER
                // box (CSS 2.1 §12.5.1; Blink places it before the content
                // start minus border and padding). The absolute insets here
                // resolve against the padding box, so the start border
                // becomes the marker's end margin (`padding-left-applies-to-
                // 010`: the bullet stays left of a 10px start border).
                let side = li.style.borders();
                let px_of = |l: Option<crate::style::values::value::Len>| match l {
                    Some(crate::style::values::value::Len::Px(v)) if v > 0.0 => v,
                    _ => 0.0,
                };
                // CSS 2.1 §12.5.1: the outside marker follows the first
                // line's available area, including a preceding float.
                // The hidden strut supplies that line for an empty item.
                let intrusion = merged.flow_shapes.as_ref().map_or(0.0, |shapes| {
                    let side = if merged.wm().rtl() {
                        &shapes.1
                    } else {
                        &shapes.0
                    };
                    side.iter().map(|s| s.cut(0.0, 1.0)).fold(0.0, f32::max)
                });
                if merged.wm().rtl() {
                    m.left(gpui::relative(1.))
                        .ml(gpui::px(px_of(side.right) - intrusion))
                        .text_left()
                } else {
                    let m = m.mr(gpui::px(px_of(side.left) - intrusion));
                    // Строки многострочного маркера равняются по
                    // КОНЦУ, к началу содержимого пункта
                    // (`marker-text-align-001`: `"[m] longtext"` при
                    // `white-space: pre`).
                    m.right(gpui::relative(1.)).text_right()
                }
            }
            .whitespace_nowrap()
            // Хвост срезается только у СОБСТВЕННЫХ отбивок движка
            // (обычный пробел после номера пункта). Авторская
            // строка `list-style-type: "..."` идёт дословно:
            // `trim_end` в Rust считает пробелом и U+00A0, а
            // неразрывный пробел в такой строке — ЗНАЧАЩИЙ.
            // Замер нейтрален (срез 2268 пар, +0/-0: у
            // `list-style-type-string-005a/b/-006` остаток не
            // здесь), но срезать значащий знак всё равно нельзя.
            // Отбивка после номера (`1. `) — часть маркера: она и
            // даёт зазор до содержимого. Пробел в конце строки
            // свернулся бы, поэтому он неразрывный.
            .child(SharedString::from(match marker.strip_suffix(' ') {
                Some(head) => format!("{head}\u{a0}"),
                None => marker.clone(),
            }))
        }))
        // Содержимое — своей колонкой, как прежде: выключка пункта
        // (`text-align: end` у `<li>`) иначе становится выравниванием
        // его детей, и блок с `text-align: initial` уезжал к концу
        // (`marker-text-align-001-ref`).
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .children(blocks(&li.children, merged, opts))
                .children(strut),
        )
        .into_any_element()
}

/// String list-style-type values inherit through ordinary block wrappers;
/// generated marker content belongs only to the originating item.
pub(crate) fn inherited_style(e: &Element, parent: &Computed, merged: &mut Computed) {
    if e.style.marker_text.is_none()
        && e.style.list_style_type.is_none()
        && e.style.no_marker.is_none()
        && parent
            .marker_layer
            .as_deref()
            .is_none_or(|m| m.content.is_none())
    {
        merged.marker_text = parent.marker_text.clone();
    }
}
