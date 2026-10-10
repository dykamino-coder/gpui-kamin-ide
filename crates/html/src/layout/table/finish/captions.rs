//! Подписи таблицы (caption): сбор сверху/снизу и обёртка вокруг сетки.

use crate::dom::{Element, Node};
use crate::layout::block::margins::CELL_BFC;
use crate::paint::effects::transform::transformed;
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, FlexDir};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

pub(super) fn collect_captions(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    caps_top: &mut Vec<AnyElement>,
    caps_bot: &mut Vec<AnyElement>,
) {
    for c in &e.children {
        if let Node::Element(cap) = c
            && (cap.tag == "caption" || cap.style.is_caption == Some(true))
        {
            let cm = inherit(inherited, &cap.style);
            // Сторона — с самого заголовка, при пустоте — от таблицы
            // (наследование caption-side).
            let cap_side_bottom = cap.style.caption_bottom.or(e.style.caption_bottom) == Some(true);
            // CSS 2.1 §9.4.1: a table caption is a block container that
            // establishes a block formatting context, so its auto height
            // contains its floats (§10.6.7), like a cell (`CELL_BFC`).
            CELL_BFC.with(|c| c.set(true));
            let inside = blocks(&cap.children, &cm, opts);
            CELL_BFC.with(|c| c.set(false));
            let built = styled_div_with(cap, &cm)
                .flex()
                .flex_col()
                .children(inside)
                .into_any_element();
            // A caption is a transformable block box (css-transforms-1
            // §transformable-element); its `transform` was dropped
            // (`transform-transformed-caption-contains-fixed-position`).
            let built = transformed(built, &cap.style, inherited);
            // ВСЕ подписи, а не первая. Прежний `break` ронял вторую целиком:
            // у таблицы с верхней И нижней подписью рисовалась только верхняя
            // (`table-border-002/003/004`, снимок `table-border-004`: зелёное
            // теста обрывается на y = 253, то есть на 110 + 20 + 20 = 150
            // css-пикселях, а нижняя подпись в 250 пикселей не нарисована
            // ВООБЩЕ). CSS 2.1 §17.4 и css-tables-3 §terminology кладут в
            // обёртку ВСЕ подписи; Blink — двумя петлями по всем подписям
            // своей стороны (`table_layout_algorithm.cc:988` «Add all the top
            // captions», `:1584` «Add all the bottom captions»).
            if cap_side_bottom {
                caps_bot.push(built);
            } else {
                caps_top.push(built);
            }
        }
    }
}

pub(super) fn wrap_with_captions(
    e: &Element,
    caps_top: Vec<AnyElement>,
    caps_bot: Vec<AnyElement>,
    inherited: &Computed,
    outer: gpui::Div,
) -> AnyElement {
    if caps_top.is_empty() && caps_bot.is_empty() {
        outer.into_any_element()
    } else {
        // `caption-side: top/bottom` — стороны block-start/block-end стола
        // (css-writing-modes-4 §6, особое исключение для caption-side): в
        // вертикальном письме заголовок стоит СБОКУ — справа при `vertical-rl`,
        // слева при `vertical-lr` (`caption-side-vrl-002`).
        let vertical = e.style.vertical == Some(true);
        let mut wrap = if vertical {
            div().flex().flex_row()
        } else {
            div().flex().flex_col()
        };
        // Элемент гибкого контейнера у стола с подписями — ОБЁРТКА
        // (css-flexbox-1 §4: «the table wrapper box becomes the flex item, and
        // the order and align-self properties apply to it … the flex item's
        // final size is calculated … as if the distance between the table
        // wrapper box's edges and the table box's content edges were all part
        // of the table box's border+padding area»). Рост и сжатие уходят на
        // обёртку, стол внутри неё забирает остаток и растягивается по её
        // ширине — подписи и стол одной ширины. Основа остаётся на столе: в
        // колонке `wrap` его главная ось та же, что у контейнера-колонки
        // (`table-as-item-inflexible-in-column-2`). Прижим `FlexStart` —
        // только вне гибкого контейнера: там он даёт сжатие по содержимому
        // (§17.5.2), а в гибком контейнере отбирал растяжение
        // (`table-as-item-stretch-cross-size*`, `-flex-cross-size`).
        let mut outer = outer;
        if inherited.flex_item && !vertical {
            let s = outer.style();
            let grow = s.flex_grow.take();
            let shrink = s.flex_shrink.take();
            let own_align = s.align_self.take();
            s.flex_grow = Some(1.0);
            let w = wrap.style();
            w.flex_grow = grow;
            w.flex_shrink = shrink;
            w.align_self = if e.style.align_self.is_some() {
                own_align
            } else {
                None
            };
            // Основа в РЯДУ: главная ось контейнера — строчная ось обёртки,
            // и основа, оставленная на столе внутри колонки `wrap`, там не
            // действует (у колонки это поперечная ось). Переносится на
            // обёртку вместе с рамкой и отбивкой стола content-box —
            // css-flexbox-1 §4: «as if the distance between the table wrapper
            // box's edges and the table box's content edges were all part of
            // the table box's border+padding area»
            // (`table-as-item-inflexible-in-row-2`: `flex: 0 0 80px; border:
            // 10px solid` — стол выходил 20 точек вместо 100).
            if !matches!(
                inherited.flex_dir,
                Some(FlexDir::Col) | Some(FlexDir::ColReverse)
            ) && let Some(Len::Px(b)) = e.style.flex_basis
            {
                s.flex_basis = None;
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let bd = e.style.borders();
                let edges = if e.style.border_box == Some(true) {
                    0.0
                } else {
                    side(bd.left)
                        + side(bd.right)
                        + side(e.style.padding.left)
                        + side(e.style.padding.right)
                };
                w.flex_basis = Some(gpui::Length::Definite(px(b + edges).into()));
            }
        } else {
            wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        // Инлайн-размер ОБЁРТКИ — инлайн-размер САМОГО стола, а не наоборот.
        // CSS 2.1 §17.4: «The width of the table wrapper box is the border-edge
        // width of the table grid box inside it … Percentages on 'width' and
        // 'height' on the table are relative to the table wrapper box's
        // containing block, NOT the table wrapper box itself» (то же
        // css-tables-3 §fixup-algorithm). Доля коробки рядов опиралась на
        // обёртку, а обёртка — гибкий КОРЕНЬ копии фрагмента
        // (`flow.rs` `layout_as_root`), и taffy тянет по доступному месту
        // только БЛОЧНЫЙ корень (`vendor/taffy/src/compute/mod.rs:68`
        // `if style.is_block()`): стол с `inline-size: 100%` и подписью
        // схлопывался до ширины содержимого — 20 css у
        // `table-grid-paint-htb-ltr` (две рамки) и 34.4 у `table-row-paint-htb-ltr`
        // (один `border-spacing`). Blink держит один размер на стол и обёртку:
        // `table_layout_algorithm.cc:139` `available_size = {table_inline_size,
        // kIndefiniteSize}` и `:71` `builder.SetAvailableSize(available_size)`.
        // Берём только точки и долю: `em`/`ch` у обёртки считались бы по ЧУЖОМУ
        // шрифту (`apply::len_to_gpui` ветка запасных величин), а стол с
        // `width: auto` обязан остаться сжатым по содержимому (§17.5.2).
        match if vertical {
            e.style.height
        } else {
            e.style.width
        } {
            Some(Len::Px(v)) if vertical => wrap = wrap.h(px(v)),
            Some(Len::Px(v)) => wrap = wrap.w(px(v)),
            Some(Len::Pct(v)) if vertical => wrap = wrap.h(gpui::relative(v)),
            Some(Len::Pct(v)) => wrap = wrap.w(gpui::relative(v)),
            _ => {}
        }
        // В ряду второй ребёнок сжимался в ноль — каждому своя ширина.
        let own_width = |x: AnyElement| -> AnyElement {
            if vertical {
                div().flex_shrink_0().child(x).into_any_element()
            } else {
                x
            }
        };
        // Порядок обёртки — верхние подписи, коробка рядов, нижние подписи
        // (css-tables-3 §terminology; Blink `table_layout_algorithm.cc:988`
        // и `:1584`). Тот же порядок мерит `table_shape`.
        let mut cap_wrap: Vec<AnyElement> = Vec::new();
        for cap_el in caps_top {
            cap_wrap.push(own_width(cap_el));
        }
        // Стол не уже подписи (css-tables-3 §computing-the-table-width:
        // «the used min-width of a table is the greater of the resolved
        // min-width, CAPMIN, and GRIDMIN»). Обёртка сжата по содержимому,
        // значит она шириной с самую широкую из подписи и стола, и растяжка
        // стола по ней даёт ровно max(стол, CAPMIN) — но только когда CAPMIN
        // известен, то есть у каждой подписи ширина в точках. Подпись
        // текстом мерилась бы max-content, а не min-content, и распирала бы
        // колонки (`anonymous-table-box-width-001`: пустая подпись в 100
        // точек, рамка `border-bottom: 100px` у стола нулевой ширины).
        let caps_px = e.children.iter().all(|n| match n {
            Node::Element(c) if c.tag == "caption" || c.style.is_caption == Some(true) => {
                matches!(c.style.width, Some(Len::Px(_)))
            }
            _ => true,
        });
        let mut outer = outer;
        if !vertical && caps_px && e.style.width.is_none() && e.style.align_self.is_none() {
            outer.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        cap_wrap.push(own_width(outer.into_any_element()));
        for cap_el in caps_bot {
            cap_wrap.push(own_width(cap_el));
        }
        // Заголовок ПЕРЕД коробкой по порядку детей = у начала оси: для vrl
        // начало блочной оси — правый край, а ряд идёт слева направо, значит
        // переворачивается ВЕСЬ порядок обёртки. Прежнее
        // `cap_first = caption_bottom == (vertical && vertical_rl)` — тот же
        // разворот, записанный для ОДНОЙ подписи.
        if vertical && e.style.vertical_rl == Some(true) {
            cap_wrap.reverse();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        wrap.children(cap_wrap).into_any_element()
    }
}
