//! Независимость раскладки блока корневого потока от ширины окна.
//!
//! Смена ширины окна меняет инлайн-размер содержащего блока детей потока, а
//! раскладка ребёнка зависит от него только через его используемую ширину,
//! процентные поля и отступы, `auto`-поля (CSS 2.1 §10.3.3) и через единицы
//! окна в поддереве (css-values-4 §6.1.2). Проценты потомков считаются от
//! коробки самого блока (§10.1), а позиционированные и флоаты в записи не
//! бывают (`plain`). Своя `width` в точках при точечных полях и отступах от
//! содержащего блока не зависит (§10.3.3; у таблицы она — нижняя граница
//! ширины сетки, §17.5.2), и начало блока в потоке слева направо — тоже.
//!
//! Блок с `width: auto` зависит от места, пока его ширина — это место:
//! растянутый блок (§10.3.3) или таблица по содержимому, `max(min-content,
//! min(доступное, max-content))` (§17.5.2.2; ширина блока — ширина таблицы
//! в её обёртке, `table::StartWrapper`). Если же его ширина больше места, её держит пол
//! (`min-width`, `min-content` таблицы), а он от места не зависит: при любом
//! месте не шире этой ширины блок тот же (`Width::Floor`, решается по
//! раскладке записи).
//!
//! Растянутый блок, весь текст которого — один абзац в одну строку от
//! начального края (`text-align: start`/`left` слева направо), меняет при
//! другой ширине только ширину своей коробки: строка не рвётся, пока место
//! не уже её, высота и положение знаков те же (`Width::Stretch`; запас
//! строки снимает `lines`). Его лист берёт ширину от потока, а краска
//! прошлого кадра при другой ширине не годна (фон и рамка шире).
//!
//! Blink решает так же по уже вычисленной ширине нового пространства
//! (`CalculateSizeBasedLayoutCacheStatus`: тот же `border_box_size` —
//! попадание в кэш результата).

use crate::dom::{Element, Node};
use crate::style::values::value::Len;
use crate::values::ValueCx;

/// Как раскладка блока зависит от ширины потока.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Width {
    /// Зависит: запись годна только при той же ширине.
    Dependent,
    /// Не зависит: своя ширина в точках.
    Fixed,
    /// Ширина `auto`: не зависит, пока поток не шире блока, если в записи
    /// блок был шире потока (его держал пол, а не место).
    Floor,
    /// Ширина `auto`, а содержимое — только текст от начального края: при
    /// записи в одну строку не зависит от ширины не меньше её.
    Stretch,
}

fn px_or_none(l: Option<Len>) -> bool {
    matches!(l, None | Some(Len::Px(_)))
}

/// Зависимость раскладки блока `e` от ширины содержащего блока.
pub(super) fn independent(e: &Element, vcx: ValueCx<'_>) -> Width {
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let (m, p) = (&b.margin, &b.padding);
    let edges = !style.wm().vertical()
        && !style.wm().rtl()
        && [m.top, m.right, m.bottom, m.left]
            .into_iter()
            .all(px_or_none)
        && [p.top, p.right, p.bottom, p.left]
            .into_iter()
            .all(px_or_none)
        && px_or_none(b.min_width)
        && px_or_none(b.max_width);
    if !edges || viewport_dependent(e, vcx) {
        return Width::Dependent;
    }
    match b.width {
        Some(Len::Px(_)) => Width::Fixed,
        None if text_only(e) => Width::Stretch,
        None => Width::Floor,
        Some(_) => Width::Dependent,
    }
}

/// The border box of block `e` of `Width::Stretch` is the flow width less its
/// fixed margins at every flow width (CSS 2.1 §10.3.3): no `min-width` or
/// `max-width` holds it.
pub(super) fn tracks_flow(e: &Element) -> bool {
    let b = &e.style.used().reset_box_layout;
    b.width.is_none() && b.min_width.is_none() && b.max_width.is_none()
}

/// Блок без своих псевдослоёв, дети которого — только текст, набранный от
/// начального края без процентного отступа.
fn text_only(e: &Element) -> bool {
    use crate::style::computed::{Display, Overflow, TextAlign};
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let flow = &style.inh.text_flow;
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    matches!(b.display, None | Some(Display::Block))
        && e.list_item.is_none()
        && e.first_line.is_none()
        && e.first_letter.is_none()
        && visible(b.overflow_x)
        && visible(b.overflow_y)
        && matches!(
            flow.text_align,
            None | Some(TextAlign::Start | TextAlign::Left)
        )
        && px_or_none(flow.text_indent)
        && !e.children.is_empty()
        && e.children.iter().all(|n| matches!(n, Node::Text(_)))
}

/// Ширины потока `lo..=hi`, при которых годна запись блока ширины `border`
/// (рамка), записанного в потоке ширины `flow`, с запасом `slack` строки
/// (`lines`): `None` — только при той же.
pub(super) fn reusable_range(
    width: Width,
    border: f32,
    flow: Option<f32>,
    slack: Option<f32>,
) -> Option<(f32, f32)> {
    // Полточки запаса: рамка округлена до пикселей устройства.
    let floor = |flow: f32| (border > flow + 0.5).then_some((0.0, border));
    match width {
        Width::Fixed => Some((0.0, f32::INFINITY)),
        Width::Dependent => None,
        Width::Floor => flow.and_then(floor),
        // Точка запаса: строка, вставшая впритык, при узком месте рвётся.
        Width::Stretch => flow.and_then(|flow| {
            floor(flow).or_else(|| {
                slack
                    .filter(|s| *s >= 1.0)
                    .map(|s| (flow - s + 1.0, f32::INFINITY))
            })
        }),
    }
}

/// Есть ли в поддереве длины в единицах окна или контейнера.
fn viewport_dependent(e: &Element, vcx: ValueCx<'_>) -> bool {
    !super::plain::all_elements(e, |e| !e.style.used().has_viewport_lengths(vcx))
}

#[cfg(test)]
mod tests;
