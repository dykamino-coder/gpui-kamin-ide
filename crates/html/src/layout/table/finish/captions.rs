//! Подписи таблицы (caption): сбор сверху/снизу и обёртка вокруг сетки.

use crate::dom::{Element, Node};
use crate::layout::build_contracts::Effect;
use crate::layout::fragment::paginate::watch::TablePart;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(super) fn collect_captions(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    caps_top: &mut Vec<AnyElement>,
    caps_bot: &mut Vec<AnyElement>,
) {
    let vcx = opts.cx.vcx();
    for c in &e.children {
        if let Node::Element(cap) = c
            && (cap.tag == "caption" || cap.style.computed().is_caption == Some(true))
        {
            let cm = box_style(inherited, &cap.style, vcx);
            // Сторона — с самого заголовка, при пустоте — от таблицы
            // (наследование caption-side).
            let cap_side_bottom = cap
                .style
                .cascaded()
                .caption_bottom
                .or(e.style.cascaded().caption_bottom)
                == Some(true);
            // CSS 2.1 §9.4.1: a table caption is a block container that
            // establishes a block formatting context, so its auto height
            // contains its floats (§10.6.7), like a cell (`LayoutCx::cell_bfc`).
            let inside =
                opts.cx
                    .build
                    .blocks(&cap.children, &cm, &opts.inner(|cx| cx.cell_bfc = true));
            let built = opts
                .cx
                .build
                .styled_box(cap, &cm, &opts.cx)
                .flex()
                .flex_col()
                .children(inside)
                .into_any_element();
            // A caption is a transformable block box (css-transforms-1
            // §transformable-element); its `transform` was dropped
            // (`transform-transformed-caption-contains-fixed-position`).
            let built = opts.cx.build.effects(
                built,
                Effect::transform(&cap.style.computed(), inherited, vcx),
            );
            // Подпись — сосед коробки таблицы в стопке обёртки: её border box
            // читает фрагментация (`watch::wrap`).
            let built = crate::layout::fragment::paginate::watch::wrap(cap.node_id, built, opts);
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
    mark: &dyn Fn(TablePart, AnyElement) -> AnyElement,
) -> AnyElement {
    if caps_top.is_empty() && caps_bot.is_empty() {
        mark(TablePart::Box, outer.into_any_element())
    } else {
        // `caption-side: top/bottom` — стороны block-start/block-end стола
        // (css-writing-modes-4 §6, особое исключение для caption-side): в
        // вертикальном письме заголовок стоит СБОКУ — справа при `vertical-rl`,
        // слева при `vertical-lr` (`caption-side-vrl-002`).
        let vertical = e.style.cascaded().wm().vertical();
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
        // CSS 2.1 §17.4: the table's margins are used on the table WRAPPER
        // box, so captions sit inside them too (`list-style-applies-to-015`:
        // a caption list item of a table with `margin-left: 1in`).
        let margin = std::mem::take(&mut outer.style().margin);
        wrap.style().margin = margin;
        if inherited.flex_item && !vertical {
            let s = outer.style();
            let grow = s.flex_grow.take();
            let shrink = s.flex_shrink.take();
            let own_align = s.align_self.take();
            s.flex_grow = Some(1.0);
            let w = wrap.style();
            w.flex_grow = grow;
            w.flex_shrink = shrink;
            w.align_self = if e.style.computed().align_self.is_some() {
                own_align
            } else {
                None
            };
            // Render sizing has already included content-box borders/padding.
            // Move that resolved inline basis to the wrapper exactly once.
            if !inherited.flex_main_vertical
                && matches!(e.style.computed().flex_basis, Some(Len::Px(_)))
            {
                w.flex_basis = s.flex_basis.take();
            } else if inherited.flex_main_vertical
                && grow == Some(0.0)
                && shrink == Some(0.0)
                && matches!(e.style.computed().flex_basis, Some(Len::Px(_)))
            {
                // A fully inflexible table keeps its definite main size.
                // Include that size when the wrapper measures its captions.
                s.size.height = s.flex_basis;
            }
        } else {
            wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        // CSS 2.1 §17.4: the wrapper uses the table's border-edge inline size.
        // styled_div_with has resolved box sizing and font units on the grid.
        let inline_size = if vertical {
            e.style.computed().height
        } else {
            e.style.computed().width
        };
        if matches!(inline_size, Some(Len::Px(_) | Len::Pct(_))) {
            if vertical {
                wrap.style().size.height = outer.style().size.height;
            } else {
                wrap.style().size.width = outer.style().size.width;
            }
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
            Node::Element(c)
                if c.tag == "caption" || c.style.computed().is_caption == Some(true) =>
            {
                matches!(c.style.computed().width, Some(Len::Px(_)))
            }
            _ => true,
        });
        let mut outer = outer;
        if !vertical
            && caps_px
            && e.style.computed().width.is_none()
            && e.style.computed().align_self.is_none()
        {
            outer.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        cap_wrap.push(own_width(mark(TablePart::Box, outer.into_any_element())));
        for cap_el in caps_bot {
            cap_wrap.push(own_width(cap_el));
        }
        // Заголовок ПЕРЕД коробкой по порядку детей = у начала оси: для vrl
        // начало блочной оси — правый край, а ряд идёт слева направо, значит
        // переворачивается ВЕСЬ порядок обёртки. Прежнее
        // `cap_first = caption_bottom == (vertical && vertical_rl)` — тот же
        // разворот, записанный для ОДНОЙ подписи.
        if vertical && e.style.cascaded().wm().block_rl() {
            cap_wrap.reverse();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        wrap.children(cap_wrap).into_any_element()
    }
}
