//! Контексты наложения и порядок покраски (CSS 2 Приложение E): какие стили образуют контекст,
//! вложен ли ребёнок в некорневой контекст, действует ли `z-index` на коробку (CSS 2.1 §9.9.1,
//! элементы flex/grid по css-flexbox-1 §5.4), какие коробки служат содержащим блоком для
//! `fixed`, и обёртка `layered`: положительный `z-index`, `fixed` и `sticky` выражаются
//! отложенной отрисовкой GPUI с приоритетом, отрицательный — подложкой.
mod ordering;
pub(crate) use ordering::by_layer;

use crate::dom::Node;
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// CSS 2.1 Appendix E step 8: `el` goes to the collector of its stacking
/// context (`gpui::PaintCollect`) under the paint key `key`, and is painted
/// after the inline content of step 7 in the order of the keys.
pub(crate) fn paint_last(el: AnyElement, key: u64) -> AnyElement {
    gpui::PaintLast::new(el).key(key).into_any_element()
}

pub(crate) use crate::style::computed::stacking::{
    fixed_cb_layer_box, nested_context, stacking_context,
};

/// Есть ли в поддереве смешивание (`mix-blend-mode` ≠ normal).
///
/// Спуск НЕ останавливается на вложенных контекстах наложения: лишняя
/// изоляция при обычном сложении картинку не меняет (source-over
/// ассоциативен), а вложенный контекст со смешиванием внутри изолируется тем
/// же правилом сам. Глубина ограничена ради страниц с тысячами вложенных
/// трансформов: обход идёт у каждого контекста наложения.
pub(crate) fn blends_inside(nodes: &[Node], depth: usize) -> bool {
    depth < 32
        && nodes.iter().any(|n| match n {
            Node::Element(c) => {
                c.style.computed().blend.is_some_and(|b| b != 0)
                    || blends_inside(&c.children, depth + 1)
            }
            Node::Text(_) => false,
        })
}

/// Действует ли `z-index` на этой коробке.
///
/// CSS 2.1 §9.9.1 у `z-index` записано «Applies to: positioned elements»: у
/// непозиционированной коробки объявление есть, но силы не имеет. Мы же
/// откладывали ЛЮБУЮ коробку с `z-index > 0`, и она всплывала над всем
/// документом: в `z-index-does-not-apply` красный `#a` (`z-index: 2`,
/// `transform: translateX(0)`, БЕЗ `position`) закрывал зелёного брата — тот
/// же квадрат 125×125 точек в (10,10)-(134,134), у нас красный, у эталона
/// зелёный.
///
/// Исключение — элемент гибкого контейнера или сетки: css-flexbox-1 §5.4
/// («z-index values other than auto create a stacking context even if
/// position is static») и css-grid-2 §6.2 распространяют `z-index` на них
/// БЕЗ `position`. Вид родителя известен из наследуемого стиля.
///
/// Соседний `stacking_context()` этот гейт по `position` держал и раньше —
/// правка убирает расхождение двух мест одного файла.
pub(crate) fn z_index_applies(c: &Computed, parent: &Computed) -> bool {
    matches!(
        c.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
            | Some(crate::style::computed::Position::Sticky)
    ) || matches!(
        parent.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    )
}

/// Будет ли элемент с таким стилем отложен.
pub(crate) fn defers(c: &Computed, parent: &Computed, under_tf: bool) -> bool {
    // `fixed` под трансформированным предком — абсолют в его блоке, а не
    // слой окна (css-transforms-1 §transform-rendering).
    (c.position == Some(crate::style::computed::Position::Fixed) && !under_tf)
        || c.position == Some(crate::style::computed::Position::Sticky)
        || (c.z_index.is_some_and(|z| z > 0) && z_index_applies(c, parent))
}

/// `z-index`: порядок наложения.
///
/// Слоёв в GPUI нет, зато есть отложенная отрисовка с приоритетом — она и
/// задаёт, что рисуется поверх. Отрицательный `z-index` (под потоком) так не
/// выражается, поэтому применяем только положительный.
///
/// `allowed` — снаружи ли мы отложенного поддерева: внутри откладывать нельзя.
pub(crate) fn layered(
    el: AnyElement,
    c: &Computed,
    parent: &Computed,
    allowed: bool,
    under_tf: bool,
) -> AnyElement {
    let fixed_to_window = c.position == Some(crate::style::computed::Position::Fixed) && !under_tf;
    if !allowed {
        // Внутри отложенного поддерева `position: fixed` отсчитывается от
        // ближайшего отложенного предка, а не от окна: своей системы
        // координат ему взять неоткуда.
        if fixed_to_window {
            return div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(el)
                .into_any_element();
        }
        return el;
    }
    // `position: fixed` — отсчёт от ОКНА: отложенная отрисовка выносит
    // элемент из потока родителя, а размер окна задаёт его систему координат.
    if fixed_to_window {
        let priority = c.z_index.unwrap_or(0).max(0) as usize;
        return gpui::deferred(div().absolute().top_0().left_0().size_full().child(el))
            .with_priority(priority)
            .into_any_element();
    }
    match c.z_index {
        // Отложенный слой рисуется вне масок дерева — маску обрезающего
        // предка ему передаёт пара обёрток (`paint::effects::mask::element::MaskKeep/MaskUse`).
        // Гейт `z_index_applies` — CSS 2.1 §9.9.1 «Applies to: positioned
        // elements» (плюс элементы flex/grid по css-flexbox-1 §5.4).
        Some(z) if z > 0 && z_index_applies(c, parent) => {
            let cell: crate::paint::effects::mask::element::MaskCell = Default::default();
            let inner = crate::paint::effects::mask::element::MaskUse {
                cell: cell.clone(),
                child: el,
            };
            let deferred = gpui::deferred(crate::hooks::IntoHooked::into_hooked(inner))
                .with_priority(z as usize)
                .into_any_element();
            let keep = crate::paint::effects::mask::element::MaskKeep {
                cell,
                child: deferred,
            };
            crate::hooks::IntoHooked::hooked(keep)
        }
        // Не откладывать ЛЮБОЙ абсолютный элемент ради покраски поверх соседей
        // (CSS 2.1 §9.9, шаг 8): вложенная отложенная отрисовка в GPUI
        // запрещена, а абсолютные элементы вложены сплошь и рядом — общее
        // правило рушит css-position и css-text целиком. Только с проверкой
        // глубины и по одному месту.
        _ => el,
    }
}
