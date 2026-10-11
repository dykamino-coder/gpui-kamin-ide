//! Позиционированный строчный атом: статическая позиция и абсолютная коробка.
// owner: A

use crate::dom::Element;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::replaced::image::image;
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::render::{RenderOpts, blocks, inline_abs_paint_last, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
mod replaced;
use replaced::replaced_abs_atom;

#[allow(clippy::needless_return)]
pub(super) fn static_position_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let mut merged = inherit(inherited, &e.style);
    // Позиционирование с внутренней коробки СНИМАЕТСЯ. Содержащим блоком
    // ей стала бы нулевая пустышка, а ширина у неё «по содержимому» —
    // в нулевом блоке это ноль, и элемент пропадал вовсе. Без
    // позиционирования она меряется своим содержимым и висит от угла
    // пустышки, то есть ровно от статической позиции.
    merged.position = None;
    merged.abs_static = true;
    // Замещаемый элемент строит своя ветка: голая коробка со стилем
    // теряла содержимое (сломанная картинка с alt-подписью пропадала,
    // `abs-pos-vlr-border-001`). Позиция снимается копией — та же
    // причина, что и у merged ниже.
    let replaced: Option<AnyElement> = match e.tag.as_str() {
        "img" | "svg" => {
            let mut copy = e.clone();
            copy.style.position = None;
            // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4), а
            // копия несёт только собственный стиль элемента: без этой
            // строки `image-orientation: none`, заданный на `body`, до
            // картинки не доезжает и EXIF-разворот применяется всё равно.
            copy.style.image_orient_none = merged.image_orient_none;
            Some(match copy.tag.as_str() {
                // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`;
                // позиция снята и со слитого стиля, как с копии выше.
                "svg" => {
                    let mut unpositioned = merged.clone();
                    unpositioned.position = None;
                    svg_replaced(&copy, &copy, &unpositioned).unwrap_or_else(|| image(&copy))
                }
                _ => image(&copy),
            })
        }
        _ => None,
    };
    let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
    // Пустышка прижимается к ВЕРХУ строки: иначе она садится на базовую
    // линию, и содержимое уезжает под неё — абсолютный блок оказывался
    // ниже своей строки, а не на её месте (видно на `static-position`:
    // зелёный блок висел под коробкой, красное проступало).
    // Рисуется элемент ПОВЕРХ соседей по строке, поэтому содержимое
    // уходит в верхний слой блока-контейнера, а в строке остаётся щуп: он
    // и держит место, и сообщает, куда потом вернуть содержимое.
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    let below = e.style.z_index.is_some_and(|z| z < 0);
    // Блочный элемент встал бы на НОВУЮ строку — там его статическая
    // позиция и находится: левый край содержимого родителя, верх — низ
    // текущей строки. Строчный остаётся точкой в самой строке.
    let inline_level = match e.style.display {
        Some(Display::InlineBlock)
        | Some(Display::InlineFlex)
        | Some(Display::InlineGrid)
        | Some(Display::InlineTable) => true,
        Some(Display::GridLanes) => e.style.lanes_inline,
        // Блокифицированный `display: inline` (§9.7) для СТАТИЧЕСКОЙ
        // позиции остаётся строчным: гипотеза §10.3.7 считается без
        // блокификации (htb-rtl-*).
        Some(_) => e.style.inline_display == Some(true),
        None => e.inline,
    };
    // Флаги направления нужны и СТРОЧНОМУ атому: в rtl-строке статическая
    // позиция — правый край, заместитель вешается правым краем на точку
    // распорки. Начало новой строки — только у блочного.
    spot.set(crate::layout::positioned::containing_block::Spot {
        hole: None,
        next_line: (!inline_level).then(|| line_height_px(inherited, opts)),
        rtl: inherited.rtl == Some(true),
        vertical: inherited.vertical == Some(true),
        vertical_rl: inherited.vertical_rl == Some(true),
        ..Default::default()
    });
    let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), false);
    let inner = match replaced {
        Some(el) => el,
        None => inner.into_any_element(),
    };
    let taken = if below {
        Some(inner)
    } else {
        crate::layout::positioned::containing_block::late_push(spot, inner)
    };
    return match taken {
        None => Some(probe),
        Some(kept) => {
            let mut hole = div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            Some(hole.child(kept).into_any_element())
        }
    };
}

#[allow(clippy::needless_return)]
pub(super) fn absolute_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let merged = inherit(inherited, &e.style);
    // Содержащий блок — позиционированный СТРОЧНЫЙ предок в этом абзаце
    // (CSS 2.1 §10.1 п.4): края по обеим осям считает `lines.rs` от
    // прямоугольника его фрагментов, коробка идёт без пустышки и слоёв.
    let inline_cb = crate::text::inline::take_atom_cb()
        && e.style.position == Some(crate::style::computed::Position::Absolute)
        && (edge_set(e.style.inset.left) || edge_set(e.style.inset.right))
        && (edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom))
        && e.style.z_index.unwrap_or(0) >= 0;
    if inline_cb {
        crate::text::inline::note_abs_cb();
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ: отдавать элемент без пустышки, чтобы `inset: 0`
    // считался от позиционированного ПРЕДКА. В раскладке под нами
    // содержащим блоком служит ЛЮБОЙ родитель, поэтому вынос ничего не
    // меняет: css-position 35 → 34, зелёный прямоугольник
    // `position-absolute-in-inline-005` так и не появился.
    // Пустышка нулевого размера сама становится содержащим блоком, и
    // края, заданные с ОБЕИХ сторон оси, схлопываются в ничто. Такому
    // элементу коробка нужна настоящая, поэтому он идёт без пустышки.
    // Остальным она нужна: без неё сдвигаются соседи (замерено на
    // `position-sticky-contained-by-display-table`).
    // ПРОБОВАЛИ И ОТКАТИЛИ: считать «растянутым» и элемент с ДОЛЕЙ
    // размера, чтобы доля не бралась от нулевой пустышки. Замерено по
    // семьям *replaced*, positioning/*, normal-flow/*, *float*: 0 и 0.
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
    let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
    // Незамещаемая коробка с РОВНО ОДНОЙ заданной осью — тем же щупом, что
    // и замещаемая выше: без этого `<span>` с одним краем падал в нулевой
    // держатель, и край считался от него (`abs-pos-non-replaced-vlr-017`:
    // `right: 2em` уводил коробку на свои же 160 влево).
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
            ..Default::default()
        });
        let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), false);
        return match crate::layout::positioned::containing_block::late_push(
            spot,
            inline_abs_paint_last(e, inner.into_any_element()),
        ) {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    if stretched {
        return Some(inner.into_any_element());
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
