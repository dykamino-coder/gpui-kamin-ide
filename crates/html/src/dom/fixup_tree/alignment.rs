//! Правки `align-self` по собранному дереву: применимость к контейнеру.
//! `align_self_from_dom_parent`: у потокового ребёнка блочного контейнера
//! авторское `align-self` гасится — свойство применимо лишь к элементам
//! flex/grid и абсолютным (css-align-3 §6.1), наш блок собран колонкой flex.
//! `align-self: inherit` берёт вычисленное значение родителя в стадии
//! вычисленных значений (`style::cascade::compute::longhands`).
//! `grid_table_items_keep_stretch`: стол-элемент сетки сохраняет растяжку по
//! дорожке (css-align-3 §6.2), а не сжатие по содержимому.

use crate::dom::*;
use crate::style::computed::{Display, Position};

/// Неприменимость `align-self` к ребёнку блока.
///
/// css-align-3 §6.1: `align-self` «Applies to: flex items, grid items, and
/// absolutely-positioned boxes». Наш блок собран колонкой flex, и авторское
/// `align-self: flex-end` у блока в `body` уводило его к правому краю
/// (`flexbox-align-self-vert-001`, `-vert-rtl-001`). Blink читает свойство
/// только в раскладке flex/grid (Blink `flex_layout_algorithm.cc`,
/// `ResolvedAlignSelf`). Гасится только авторское значение и ДО сборки:
/// приёмы сборки пишут в то же поле позже (`render::blocks`: rtl-прижим блока
/// с шириной, `blocks()`, флоаты, столы) и видят пустое поле, как без автора.
/// Гашение — про используемое значение: объявленное (`align_self_decl`)
/// остаётся и наследуется словом `inherit`. Родитель `display: contents` —
/// настоящий контейнер выше, такие дети не трогаются; корневой уровень тоже.
///
/// TRANSITIONAL(cascade-7b-R06): Авторский align-self гасится в cascaded из-за общего поля с командами layout; удалить гашение после переноса команд в used values и проверки применимости в block layout.
pub(crate) fn align_self_from_dom_parent(nodes: &mut [Node], parent: Option<&ParentAlign>) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let s = el.style.cascaded_mut();
        if parent.is_some_and(|p| p.block)
            && s.align_self.is_some()
            && s.align_self_decl == Some(s.align_self)
            && !matches!(s.position, Some(Position::Absolute) | Some(Position::Fixed))
        {
            s.align_self = None;
            s.align_self_normal = false;
        }
        let me = ParentAlign {
            block: matches!(
                s.display,
                None | Some(Display::Block)
                    | Some(Display::ListItem)
                    | Some(Display::InlineBlock)
                    | Some(Display::TableCell)
            ) && s.webkit_box != Some(true),
        };
        align_self_from_dom_parent(&mut el.children, Some(&me));
    }
}

/// Что дети берут у родителя в `align_self_from_dom_parent`.
pub(crate) struct ParentAlign {
    /// Родитель — блочный контейнер (не flex/grid/contents/таблица).
    pub(crate) block: bool,
}

/// Стол — элемент СЕТКИ: растяжка по дорожке остаётся за ним.
///
/// css-align-3 §6.2: начальное `align-self: normal` у элемента сетки
/// «behaves as stretch», и растянутый элемент получает размер ОБЛАСТИ.
/// Сжатие стола по содержимому (CSS 2.1 §17.5.2.2) у нас выражено
/// `align_self = FlexStart` (`layout::table::finish`), а у элемента сетки эта ось —
/// БЛОЧНАЯ: стол переставал расти до дорожки (пустой стол выходил нулевой
/// высоты и ронял базовую линию контейнера — `grid-container-baseline-
/// synthesized-001…004`), а сжатия по строчной оси приём там и не давал:
/// её ведёт `justify-self`. Явная растяжка на самом элементе снимает приём
/// ровно в сетке и нигде больше.
///
/// Гейты: только обычная сетка (у лунок свой проход дорожек), только
/// потоковый ребёнок (внепоточный элементом сетки не является,
/// css-grid-1 §9), только когда контейнер не задал своего `align-items`
/// и автор не задал `align-self` — чужое выравнивание не перебиваем.
pub(crate) fn grid_table_items_keep_stretch(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        grid_table_items_keep_stretch(&mut el.children);
        if !matches!(
            el.style.cascaded().display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            continue;
        }
        if !matches!(
            el.style.cascaded().align_items,
            None | Some(crate::style::computed::Align::Stretch)
        ) {
            continue;
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if matches!(
                child.style.cascaded().position,
                Some(Position::Absolute) | Some(Position::Fixed)
            ) {
                continue;
            }
            let is_table = child.tag == "table"
                || matches!(
                    child.style.cascaded().display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            if is_table && child.style.cascaded().align_self.is_none() {
                child.style.cascaded_mut().align_self =
                    Some(crate::style::computed::Align::Stretch);
            }
        }
    }
}
