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
//! Таблица с `width: auto` формально тоже независима, пока шире места
//! (`max(min-content, min(доступное, max-content))`, CSS 2.1 §17.5.2.2), но
//! наша коробка такой таблицы растягивается по потоку, а сетка переливается
//! из неё: её запись при другой ширине не годна.
//!
//! Blink решает так же по уже вычисленной ширине нового пространства
//! (`CalculateSizeBasedLayoutCacheStatus`: тот же `border_box_size` —
//! попадание в кэш результата).

use crate::dom::{Element, Node};
use crate::style::values::value::Len;
use crate::values::ValueCx;

fn px_or_none(l: Option<Len>) -> bool {
    matches!(l, None | Some(Len::Px(_)))
}

/// Раскладка блока `e` не зависит от ширины содержащего блока.
pub(super) fn independent(e: &Element, vcx: ValueCx<'_>) -> bool {
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let (m, p) = (&b.margin, &b.padding);
    !style.wm().vertical()
        && !style.wm().rtl()
        && matches!(b.width, Some(Len::Px(_)))
        && [m.top, m.right, m.bottom, m.left]
            .into_iter()
            .all(px_or_none)
        && [p.top, p.right, p.bottom, p.left]
            .into_iter()
            .all(px_or_none)
        && px_or_none(b.min_width)
        && px_or_none(b.max_width)
        && !viewport_dependent(e, vcx)
}

/// Есть ли в поддереве длины в единицах окна или контейнера.
fn viewport_dependent(e: &Element, vcx: ValueCx<'_>) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if e.style.used().has_viewport_lengths(vcx) {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(child) => Some(child),
            Node::Text(_) => None,
        }));
    }
    false
}

#[cfg(test)]
#[path = "width_tests.rs"]
mod tests;
