//! Alignment for fixup_tree; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Display, Position};

/// `align-self` от ДОМ-родителя: слово `inherit` и неприменимость к блоку.
///
/// 1. `align-self: inherit` (css-cascade-4 §7.3) — вычисленное значение
///    родителя. Прежде слово отбрасывалось разбором, и элемент брал
///    `align-items` контейнера (`flexbox-align-self-vert-001`,
///    `-horiz-001-block`: `inherit` ждёт `flex-end` от `.flexbox`).
///    Берётся АВТОРСКОЕ значение родителя (`align_self_decl`), даже когда
///    у самого родителя оно погашено пунктом 2: гашение — про
///    использованное значение, вычисленное остаётся.
/// 2. css-align-3 §6.1: `align-self` «Applies to: flex items, grid items,
///    and absolutely-positioned boxes». Наш блок собран колонкой flex, и
///    авторское `align-self: flex-end` у блока в `body` уводило его к
///    правому краю (`flexbox-align-self-vert-001`, `-vert-rtl-001`).
///    Blink читает свойство только в раскладке flex/grid
///    (`C:\Users\MSI\Projects\refs\chromium-blink\third_party\blink\renderer\core\layout\flex\flex_layout_algorithm.cc:266`
///    `ResolvedAlignSelf`). Гасится только авторское значение и ДО сборки:
///    приёмы сборки пишут в то же поле позже (`render.rs`: rtl-прижим
///    блока с шириной, `blocks()`, флоаты, столы) и видят пустое поле, как
///    без автора. Запись отката 04.09 (`inline.rs`: гашение в
///    `inline::inherit` без признака авторства, −50 в своде v19) — этот
///    путь гасит только авторское. Родитель `display: contents` — настоящий
///    контейнер выше, такие дети не трогаются; корневой уровень тоже.
pub(crate) fn align_self_from_dom_parent(nodes: &mut [Node], parent: Option<&ParentAlign>) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let s = &mut el.style;
        match parent {
            Some(p) => {
                if s.align_self_inherit {
                    s.align_self = p.value;
                    s.align_self_decl = Some(p.value);
                    s.align_self_inherit = false;
                    (
                        s.align_self_safe,
                        s.align_self_normal,
                        s.align_self_own_axis,
                        s.align_self_flex_kw,
                        s.align_self_last,
                    ) = p.flags;
                }
                if p.block
                    && s.align_self.is_some()
                    && s.align_self_decl == Some(s.align_self)
                    && !matches!(s.position, Some(Position::Absolute) | Some(Position::Fixed))
                {
                    s.align_self = None;
                    s.align_self_normal = false;
                }
            }
            None => s.align_self_inherit = false,
        }
        let me = ParentAlign {
            // Вычисленное значение для `inherit` детей — авторское, если было.
            value: s.align_self_decl.unwrap_or(s.align_self),
            flags: (
                s.align_self_safe,
                s.align_self_normal,
                s.align_self_own_axis,
                s.align_self_flex_kw,
                s.align_self_last,
            ),
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
    pub(crate) value: Option<crate::style::computed::Align>,
    /// safe, normal, own_axis, flex_kw, last.
    pub(crate) flags: (bool, bool, bool, bool, bool),
    /// Родитель — блочный контейнер (не flex/grid/contents/таблица).
    pub(crate) block: bool,
}

/// Стол — элемент СЕТКИ: растяжка по дорожке остаётся за ним.
///
/// css-align-3 §6.2: начальное `align-self: normal` у элемента сетки
/// «behaves as stretch», и растянутый элемент получает размер ОБЛАСТИ.
/// Сжатие стола по содержимому (CSS 2.1 §17.5.2.2) у нас выражено
/// `align_self = FlexStart` (`render.rs: table`), а у элемента сетки эта ось —
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
            el.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            continue;
        }
        if !matches!(
            el.style.align_items,
            None | Some(crate::style::computed::Align::Stretch)
        ) {
            continue;
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if matches!(
                child.style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            ) {
                continue;
            }
            let is_table = child.tag == "table"
                || matches!(
                    child.style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            if is_table && child.style.align_self.is_none() {
                child.style.align_self = Some(crate::style::computed::Align::Stretch);
            }
        }
    }
}
