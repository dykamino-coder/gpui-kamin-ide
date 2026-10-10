//! Раскладка: display/flex/позиционирование/размеры (apply_layout).

use crate::style::apply::*;
use crate::style::computed::{
    Align, Computed, Display, FlexDir, Justify, Overflow, Placement, Position,
};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

mod alignment;
mod display_axes;
mod grid_gap;
mod sizing;
use alignment::layout_alignment;
use display_axes::{layout_display, layout_flex_axes};
use grid_gap::layout_grid_gap;
use sizing::layout_sizing;

pub(super) fn apply_layout(mut d: Div, c: &Computed) -> Div {
    // Вертикальная `-webkit-box` с действующим `continue` (`line-clamp`,
    // `-webkit-line-clamp`) вычисляется в `flow-root` (css-overflow-4
    // §line-clamp, «the computed value becomes flow-root and the box
    // establishes a BFC»): это блочный контейнер, и `align-items` /
    // `justify-content` гибкой коробки к нему не применяются
    // (`line-clamp-017/018`, `webkit-line-clamp-045`: анонимная строка
    // вставала по центру).
    let legacy_block;
    let c = if c.webkit_box == Some(true)
        && c.webkit_box_vertical == Some(true)
        && (c.line_clamp.is_some() || c.clamp_auto == Some(true))
        && (c.align_items.is_some() || c.justify_content.is_some())
    {
        let mut b = c.clone();
        b.align_items = None;
        b.justify_content = None;
        legacy_block = b;
        &legacy_block
    } else {
        c
    };
    d = layout_display(d, c);
    // При вертикальном письме оси меняются местами: `row` — это ось СТРОКИ,
    // а она идёт сверху вниз; `column` — ось потока, справа налево
    // (`vertical-rl`) или слева направо (`vertical-lr`).
    let dir = match (c.flex_dir, c.vertical == Some(true)) {
        // Ось СТРОКИ при вертикальном письме идёт сверху вниз, а
        // `direction: rtl` разворачивает её снизу вверх — как и в обычном
        // письме он разворачивает строку справа налево
        // (`flexbox-writing-mode-005`).
        (Some(d), true) => Some(match (d, c.vertical_rl == Some(true)) {
            (FlexDir::Row, _) if c.rtl == Some(true) => FlexDir::ColReverse,
            (FlexDir::RowReverse, _) if c.rtl == Some(true) => FlexDir::Col,
            (FlexDir::Row, _) => FlexDir::Col,
            (FlexDir::RowReverse, _) => FlexDir::ColReverse,
            (FlexDir::Col, true) => FlexDir::RowReverse,
            (FlexDir::Col, false) => FlexDir::Row,
            (FlexDir::ColReverse, true) => FlexDir::Row,
            (FlexDir::ColReverse, false) => FlexDir::RowReverse,
        }),
        // Умолчание `flex-direction: row` в стиле НЕ записано, а ось менять
        // всё равно надо: в вертикальном письме строка идёт сверху вниз,
        // значит главная ось гибкого ряда — вертикальная. Пока сюда попадало
        // `None`, контейнер оставался горизонтальным (замерено пробой: ряд в
        // `vertical-rl` против колонки с прижимом вправо — 5.42%).
        (None, true) if matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex)) => {
            if c.rtl == Some(true) {
                Some(FlexDir::ColReverse)
            } else {
                Some(FlexDir::Col)
            }
        }
        (d, _) => d,
    };
    // `sideways-lr`: строчная ось идёт СНИЗУ вверх (css-writing-modes-4
    // §block-flow) — вертикальные результаты перевода осей разворачиваются.
    // Горизонтальные (из `column`) не трогаются: ось блока у slr обычная,
    // слева направо.
    let dir = if c.vertical == Some(true) && c.sideways == Some(true) && c.vertical_rl != Some(true)
    {
        dir.map(|d| match d {
            FlexDir::Col => FlexDir::ColReverse,
            FlexDir::ColReverse => FlexDir::Col,
            other => other,
        })
    } else {
        dir
    };
    // Разворот по `direction: rtl` — только для обычного письма: при
    // вертикальном он уже учтён в переводе осей выше, и второй раз
    // переворачивать нельзя (`flexbox-writing-mode-005`: колонки в
    // `vertical-rl` шли слева направо).
    let rtl_row = c.rtl == Some(true) && c.vertical != Some(true);
    d = layout_flex_axes(d, c, dir, rtl_row);
    d = layout_alignment(d, c, dir, rtl_row);
    d = layout_grid_gap(d, c);

    // Движок раскладки всегда трактует размер как `border-box`, а CSS по
    // умолчанию — как `content-box`: заданная ширина не включает отступы и
    // рамку. Без компенсации блок с рамкой 4px выходил на 8 точек уже, чем в
    // браузере, и всё правее него уезжало (поймано сравнением с Chrome).
    let content_box = c.border_box != Some(true);
    // Доля отступа (`padding: 20%`) в точки здесь не переводится — её база,
    // ширина содержащего блока, известна только раскладке. Тогда пересчёт
    // `content-box` отдаётся ей целиком (`taffy::BoxSizing::ContentBox`):
    // прежде такая коробка оставалась border-box, и её содержимое ужималось
    // на ширину отступов (`padding-percentage-inherit-001`: 30 → 6 точек у
    // ребёнка).
    let native_content_box = intrinsic_size::native_content_box(c);
    if native_content_box {
        d.style().content_box = Some(true);
    }
    let extra = |sides: &[Option<Len>]| -> f32 {
        if !content_box || native_content_box {
            return 0.0;
        }
        sides
            .iter()
            .filter_map(|s| match s {
                Some(Len::Px(v)) => Some(*v),
                _ => None,
            })
            .sum()
    };
    let bw = c.borders();
    let pad_x = extra(&[c.padding.left, c.padding.right, bw.left, bw.right]);
    let pad_y = extra(&[c.padding.top, c.padding.bottom, bw.top, bw.bottom]);

    d = layout_sizing(d, c, pad_x, pad_y);
    d
}

/// Идёт ли `aspect-ratio` автоминимумом (css-sizing-4 §5.2): незамещаемая
/// коробка без прокрутки, ровно одна ось задана в точках, а минимум
/// ratio-зависимой оси не задан явно.
fn set_len(l: Option<Len>) -> bool {
    matches!(l, Some(x) if x != Len::Auto)
}

fn ratio_as_auto_min(c: &Computed) -> bool {
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    let px_w = matches!(c.width, Some(Len::Px(_)));
    let px_h = matches!(c.height, Some(Len::Px(_)));
    c.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0)
        // `calc-size()` считает раскладка от размера, выведенного
        // соотношением (`vendor/taffy` — `calc_size_derived`, автоминимум):
        // эмуляция явным минимумом спрятала бы соотношение от неё
        // (`calc-size-aspect-ratio-001…004`).
        && c.calc_size.iter().all(Option::is_none)
        && visible(c.overflow_x)
        && visible(c.overflow_y)
        && !c.scroller
        && !c.flex_item_ratio
        // Абсолют с краями по ОБЕИМ сторонам зависимой оси растягивается
        // краями, и отношение обязано победить растяжку (`abspos-005/006`)
        // — ему отношение остаётся в раскладке; без краёв автоминимум
        // действует как у блока (`abspos-012/013`).
        && !(matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed))
            && (if px_w {
                set_len(c.inset.top) && set_len(c.inset.bottom)
            } else {
                set_len(c.inset.left) && set_len(c.inset.right)
            }))
        && (px_w != px_h)
        && (if px_w {
            !matches!(c.min_height, Some(Len::Px(_)))
        } else {
            !matches!(c.min_width, Some(Len::Px(_)))
        })
        // Процентный предел зависимой оси эмуляцией не выразить: явный
        // минимум из соотношения сильнее любого максимума (CSS 2.1 §10.4), а
        // доля предела во вкладе должна игнорироваться и решаться только в
        // раскладке (css-sizing-3 §5.2.1; `intrinsic-percent-non-replaced-007`:
        // `height:100px; 2/1; max-width:50%` в `max-content` — 100×100, а не
        // 200×100). Такой коробке соотношение остаётся в раскладке.
        && (if px_w {
            !matches!(c.max_height, Some(Len::Pct(_)))
        } else {
            !matches!(c.max_width, Some(Len::Pct(_)))
        })
}
