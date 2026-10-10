//! apply_layout, этап выравнивания: align-items/self, flex-basis, justify-content, align-content.

use super::*;

pub(super) fn layout_alignment(
    mut d: Div,
    c: &Computed,
    dir: Option<FlexDir>,
    rtl_row: bool,
) -> Div {
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (повторно, теперь узко): зеркало поперечной оси у
    // гибкой КОЛОНКИ при `direction: rtl` (css-flexbox-1 §5.1: cross-start
    // колонки — inline-start письма, то есть правый край) — умолчание
    // `items_end` и зеркало явных `start/end` у `align-items` и `align-self`
    // детей (флаг `cross_mirror` из `inline::inherit`). Срез из 15 пар
    // (`flexbox_rtl-direction`, `flexbox-align-self-vert-rtl-002..005`,
    // `flexbox-align-self-vert-002`, `flexbox-align-self-horiz-002`,
    // `gap-001-rtl` + заложники `text-orientation-*-100`): 6 -> 6, ноль
    // сдвигов. Те пары держит другое (у `flexbox_rtl-direction` расходятся
    // поля и высота коробки, а не сторона прижима).
    // Возвращено 02.10 иначе — флагом раскладки `flex_cross_reverse` выше
    // (разворачивает и растяжение/базовую линию, а не только `start/end`),
    // вместе с гашением авторского `align-self` у блока (`dom.rs`), словом
    // `inherit`, комментариями XHTML и физическим рядом флоатов: срез 10511
    // пар против свода v229 — +11/−1 (`flexbox_rtl-direction`,
    // `-align-self-vert-001/002`, `-vert-rtl-001` и др.; потеря
    // `flexbox-writing-mode-013` — у эталона блок `span` с шириной в rtl
    // стоит слева, сам тест теперь верен).
    match c.align_items {
        Some(Align::Center) => d = d.items_center(),
        Some(Align::Start) => d = d.items_start(),
        Some(Align::End) => d = d.items_end(),
        // `last baseline` — своя группа с прижимом к концу (css-align-3 §9.3).
        // Не у лунок: их дорожки — гибкие ряды движка, и прижим к концу уводил
        // лунки целиком (★ ЗАМЕРЕНО: `row-grid-lanes-item-baseline-001/003`
        // 0.00 → 8.02/7.56, `column-fill-reverse-justify-items-002` 0.00 → 3.87).
        Some(Align::Baseline)
            if c.align_items_last && c.display != Some(Display::GridLanes) && !c.parent_lanes =>
        {
            d.style().align_items = Some(gpui::AlignItems::LastBaseline);
        }
        Some(Align::Baseline) => d = d.items_baseline(),
        // `anchor-center` у `align-items` спекой не предусмотрен — как не задано.
        Some(Align::Stretch) | Some(Align::AnchorCenter) | None => {}
    }
    // Приставка `safe` (css-align-3 §4.4): при переполнении области
    // выравнивание падает к началу, иначе содержимое уезжает за край и
    // становится недоступным. Раскладка читает это из стиля
    // (`flexbox-safe-overflow-position-*`).
    // Лунки решают `safe` сами (`render.rs`: раздача не ставится, когда лунка
    // переполнена) — второй заход в раскладке им мешает (★ ЗАМЕРЕНО:
    // `grid-lanes-justify-content-001` 0.00 -> 1.37).
    if c.display != Some(Display::GridLanes)
        && (c.align_items_safe
            || c.align_self_safe
            || c.align_content_safe
            || c.justify_content_safe
            || c.justify_items_safe
            || c.justify_self_safe)
    {
        d.style().safe_alignment = Some((
            c.align_items_safe,
            c.align_self_safe,
            c.align_content_safe,
            c.justify_content_safe,
        ));
        d.style().safe_justify_alignment = Some((c.justify_items_safe, c.justify_self_safe));
    }
    // `align-self` — про САМ элемент, а не про его детей. Раньше оба свойства
    // писались в одно поле, и элемент выравнивал содержимое вместо себя.
    if let Some(a) = c.align_self {
        d.style().align_self = Some(self_align(a, c.align_self_last));
    }
    // `flex-basis: auto` — это ОТСУТСТВИЕ основы, а не «во всю ширину»:
    // без отсева `flex: none` растягивал кнопку на всю строку.
    if let Some(b) = c.flex_basis.filter(|b| *b != Len::Auto) {
        d = d.flex_basis(len_to_gpui(b));
    }
    if let Some(j) = c.justify_content {
        // `left`/`right` (css-align-3 §5.2): вдоль строчной оси — как
        // `start`/`end` письма (в ряду); «if the property's axis is not
        // parallel with the inline axis, this value behaves as start» —
        // в КОЛОНКЕ это `flex-start` (`flexbox_justifycontent-right-002`).
        // Только горизонтальное письмо: в вертикальном оси уже переставлены
        // поворотом, и `left/right` там держит прежний путь (★ ЗАМЕРЕНО:
        // без этой отсечки `flexbox-justify-content-wmvert-001` 0.00 -> 1.12).
        let main_vertical =
            c.vertical.is_none() && matches!(dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
        let j = match j {
            // Не вдоль строчной оси — `start` ПИСЬМА, а не `flex-start`: у
            // `column-reverse` они смотрят в разные стороны, а спека требует
            // именно начало письма (`flexbox_justifycontent-left-002`:
            // «boxes … in the top left corner … top-to-bottom order»).
            Justify::Left | Justify::Right if main_vertical => Justify::WmStart,
            Justify::Left => Justify::WmStart,
            Justify::Right => Justify::WmEnd,
            other => other,
        };
        // `start`/`end` — начало и конец ОСИ ПИСЬМА (css-align-3 §4), а
        // `AlignContent::Start`/`End` у раскладки физические: смещение всегда
        // считается от `padding_border.main_start`, разворот выражен только
        // обратным обходом. Ось выше уже переведена в физическую (`rtl_row`
        // разворачивает ряд), поэтому при письме справа налево начало и конец
        // надо поменять местами (`flexbox_justifycontent-start-rtl`).
        let j = if rtl_row {
            match j {
                Justify::WmStart => Justify::WmEnd,
                Justify::WmEnd => Justify::WmStart,
                other => other,
            }
        } else {
            j
        };
        d.style().justify_content = Some(to_content(j));
    }
    // `align-content` — распределение СТРОК, когда их несколько: без него
    // перенесённые строки прижимались к началу вместо заданного распределения.
    if let Some(a) = c.align_content {
        d.style().align_content = Some(to_content(a));
    }
    d
}
