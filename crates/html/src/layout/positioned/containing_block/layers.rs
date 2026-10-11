//! Стеки слоёв содержащих блоков: ICB и позднее размещение (открыть, закрыть, положить).

use super::SpotCell;
use super::late::LatePlace;
use super::late::spot_place;
use gpui::{AnyElement, IntoElement};

thread_local! {
    /// Слои верхней отрисовки: по одному на каждый блок-контейнер в работе.
    /// Позиционированный элемент кладёт себя в верхний слой, а контейнер
    /// забирает слой целиком и дописывает его последними детьми.
    pub(crate) static LATE: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

// Открыть слой на время сборки детей контейнера.
thread_local! {
    /// Слои НАЧАЛЬНОГО содержащего блока: внепоточные элементы, которым не
    /// нашлось позиционированного предка. По §10.1 их содержащий блок —
    /// область просмотра, а не ближайший родитель, поэтому они дописываются
    /// последними детьми документа.
    /// Пара `(SpotCell, AnyElement)`: по ПУСТОЙ оси элемент стоит на
    /// статической позиции, и её сообщает щуп с его места в потоке.
    pub(crate) static ICB: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Открыть слой ICB: документ, блок ленты или вложенный документ.
pub fn icb_open() {
    ICB.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем ICB и закрыть его.
pub fn icb_close() -> Vec<AnyElement> {
    ICB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Заместитель слоя ICB — БЕЗ обёртки, в отличие от `spot_place`.
///
/// Любая коробка вокруг стала бы для раскладки содержащим блоком абсолютного
/// ребёнка (понятия «позиционированный предок» у раскладки нет), и края
/// считались бы от неё, а не от области просмотра — то есть ровно то, ради
/// чего затеян вынос. `LatePlace` своей коробки не заводит: он отдаёт
/// `layout_id` ребёнка.
pub(super) fn icb_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    LatePlace {
        child: Some(child),
        spot,
    }
    .into_any_element()
}

/// Открыт ли слой начального содержащего блока.
///
/// Поддерево ленты прокрутки строится в замыкании, а зовёт его
/// `ScrollArea::request_layout` — уже ПОСЛЕ `icb_close()`. Вынимать оттуда
/// кандидатов можно только пока слой ещё есть.
pub fn icb_active() -> bool {
    ICB.with(|s| !s.borrow().is_empty())
}

/// Отдать элемент слою ICB. Слоя нет — элемент возвращается, рисовать на
/// месте.
pub fn icb_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    ICB.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

pub fn late_open() {
    LATE.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем и закрыть его.
pub fn late_close() -> Vec<AnyElement> {
    LATE.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| spot_place(spot, el))
        .collect()
}

/// Есть ли в открытом верхнем слое накопленное содержимое. Спрашивает цикл
/// детей контейнера перед позиционированным соседом: слой выпускается
/// раньше него, чтобы абсолют на статической позиции красился в порядке
/// дерева (CSS 2.1 прил. E, шаг 8), а не поверх всех позиционированных
/// соседей после него.
pub fn late_pending() -> bool {
    LATE.with(|s| s.borrow().last().is_some_and(|layer| !layer.is_empty()))
}

/// Отдать содержимое верхнему слою. Если слоя нет (элемент собирают вне
/// блока-контейнера), содержимое возвращается — рисовать его на месте.
pub fn late_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    LATE.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}
