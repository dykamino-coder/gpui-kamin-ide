//! Позиционированный строчный атом: статическая позиция и абсолютная коробка.
// owner: A

use crate::dom::Element;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::replaced::image::image;
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::layout::replaced::{inline_replaced_position, replaced_content};
use crate::paint::stacking::stacking_context;
use crate::render::{RenderOpts, blocks, inline_abs_paint_last, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

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
        let mut copy = e.clone();
        copy.style.image_orient_none = merged.image_orient_none;
        copy.style.position = None;
        // Поля несёт ДЕРЖАТЕЛЬ: снимали только позицию, и `margin`
        // прикладывался дважды — раз держателем, раз внутренней коробкой
        // (`absolute-replaced-width-050`: край 48 превращался в 72).
        copy.style.margin = Default::default();
        // Долю размера держатель тоже несёт сам: внутри она считалась ОТ
        // НЕГО и выходила долей от доли — `width: 50%` давало четверть
        // содержащего блока (`absolute-replaced-width-006`). Внутренней
        // коробке остаётся заполнить держателя.
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
        let holder = styled_div_with(e, &merged).child(built);
        // Замещаемый атом БЕЗ позиционированного предка считается от
        // начального содержащего блока (§10.1 п.4), а не от строки, где он
        // написан: с обеими заданными осями место в строке ему не нужно
        // вовсе. Тот же приём, что у блочного пути (`to_icb`).
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        // CSS 2.1 §§10.1, 10.3.7, 10.6.4: explicit insets use the
        // containing block; the auto axis keeps its inline static spot.
        // Route to that block's layer, rather than the paragraph wrapper.
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
        // A negative `z-index` joins the ICB layer too, painted in the
        // bottom layer (CSS 2.1 §9.9 step 3) — in place it painted over
        // its later negative-z siblings (`shape-image-009`).
        let below_icb = e.style.z_index.is_some_and(|z| z < 0) && !stacking_context(inherited);
        if x_set
            && y_set
            && !inline_cb
            && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
            && !(inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited))
        {
            let holder: AnyElement = if below_icb {
                crate::paint::effects::underlay::Underlay::new(holder.into_any_element()).into_any_element()
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
            match crate::layout::positioned::containing_block::icb_push(spot, holder.into_any_element()) {
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
        return Some(if stretched {
            holder.into_any_element()
        } else {
            div()
                .w_0()
                .h_0()
                .flex_shrink_0()
                .child(holder)
                .into_any_element()
        });
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
        return match crate::layout::positioned::containing_block::late_push(spot, inline_abs_paint_last(e, inner.into_any_element())) {
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
