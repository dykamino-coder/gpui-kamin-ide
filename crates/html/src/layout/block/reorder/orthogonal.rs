//! Дети с ортогональным (вертикальным) режимом письма в блочном потоке.

use crate::dom::Node;
use crate::layout::writing_mode::orthogonal_fixed_child;
use crate::render::in_flow;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;

/// Зеркальный ортогональный случай: ВЕРТИКАЛЬНЫЙ блок внутри горизонтального
/// контейнера. Его строчная ось — высота, и авто-размер по ней зажимается
/// высотой контейнера за вычетом вертикальных полей (css-writing-modes-3
/// §7.3). Доля полей здесь обычная — от ширины контейнера, её решает
/// раскладка сама.
pub(crate) fn orthogonal_vertical_children(children: Vec<Node>, container: &Computed) -> Vec<Node> {
    let has_vertical = children.iter().any(|n| match n {
        Node::Element(ch) => !ch.inline && ch.style.vertical == Some(true),
        _ => false,
    });
    if !has_vertical {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(true) {
            continue;
        }
        if !in_flow(&ch.style) {
            continue;
        }
        // Элемент СЕТКИ с невытягивающим выравниванием: по строчной оси
        // (у него вертикальной) он размером в содержимое, а не в область
        // (css-grid-1 §6.6 вместе с css-align-3 §6.1 — `stretch` растягивает,
        // остальное нет). Пока вертикальный абзац брал весь предел
        // ортогонального потока, эталоны `orthogonal-positioned-grid-items-*`
        // (`place-items: start`) вылезали за сетку на всю высоту окна.
        if matches!(
            container.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) && matches!(
            ch.style.align_self.or(container.align_items),
            Some(Align::Start) | Some(Align::Center) | Some(Align::End) | Some(Align::Baseline)
        ) {
            ch.style.hug_inline = true;
        }
        // Корень с vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
        // flow). Прижим самим стилем корня (align-self) — контейнеры-колонки
        // его уважают; для корня с ФОНОМ-КАРТИНКОЙ якорь ранее гасил
        // canvas-слой — тем страницам якорь не ставится (замерено).
        if matches!(ch.tag.as_str(), "html" | "body")
            && ch.style.vertical_rl == Some(true)
            && ch.style.align_self.is_none()
            && ch.style.bg_image.is_none()
        {
            ch.style.align_self = Some(crate::style::computed::Align::End);
        }
        // Ordinary orthogonal blocks now compute their own used inline size.
        if orthogonal_fixed_child::normal_block_flow(&ch.style, container) {
            ch.style.flex_shrink = Some(0.0);
            continue;
        }
        if ch.style.height.is_some() || ch.style.max_height.is_some() {
            continue;
        }
        let margin = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) => match container.width {
                Some(Len::Px(w)) => k * w,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let margins = margin(ch.style.margin.top) + margin(ch.style.margin.bottom);
        ch.style.max_height = Some(match container.height {
            Some(Len::Px(h)) => Len::Px((h - margins).max(0.0)),
            _ => Len::Pct(1.0),
        });
        // Тот же предел — и ПЕРЕНОСУ строк. Авто-строчный размер ортогонального
        // блока — shrink-to-fit к размеру, что «would stretch fit into … the
        // containing block’s size if that is fixed» (css-writing-modes-4
        // §7.3.2, Overview.bs:2175-2183), а stretch-fit вычитает поля ребёнка.
        // Blink: `length_utils.cc:117-146` (`kFitContent` →
        // `ShrinkToFit(available_size - margins.InlineSum())`), авто-длина
        // ортогонального ребёнка — `FitContent` (`length_utils.cc:555-569`).
        // `max_height` выше этого не даёт: вето `element()` «предок уже дал
        // предел» у вертикального блока без своей `height` оставляет предел
        // КОНТЕЙНЕРА, и текст переносился по 200 вместо 200 − 2·50
        // (`sizing-orthogonal-percentage-margin-001/002`: эталон с `height:
        // 100px` после сужения вето переносит по 100, тест — по 200).
        // Свой `ortho_limit` перебивает унаследованный при слиянии
        // (`inline.rs`: `own.ortho_limit.or(parent.ortho_limit)`); рамки и
        // отбивки ребёнка из него вычтет `element()`. Только блочный
        // контейнер (у сетки/гибкого содержащий блок другой) и только при
        // ненулевых полях — без них предел равен унаследованному.
        if let Some(Len::Px(h)) = container.height
            && margins > 0.0
            && matches!(container.display, None | Some(Display::Block))
        {
            let px = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            // Предел — внутренний размер СБ: при `border-box` заданная
            // высота включает его рамки и отбивки по той же оси.
            let inner = if container.border_box == Some(true) {
                let b = container.borders();
                h - px(b.top)
                    - px(b.bottom)
                    - px(container.padding.top)
                    - px(container.padding.bottom)
            } else {
                h
            };
            ch.style.ortho_limit = Some((inner - margins).max(0.0));
        }
    }
    out
}
