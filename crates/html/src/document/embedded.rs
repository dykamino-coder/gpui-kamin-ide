//! Вложенный документ (`<iframe>`, SVG с `<foreignObject>`) и снятие обёрток
//! `<html>`/`<body>`.
//!
//! `parse_embedded_sized` гонит разметку рамки тем же конвейером, что
//! `Document::new`, но в её собственном `DocState` (правила, метрики, `@page`),
//! поэтому внешний документ ничего не теряет; `@media` меряется коробкой рамки
//! (mediaqueries-4 §width, HTML §4.8.5). `unwrap_document` снимает обёртки без
//! собственной коробки (`box_style`), чтобы единицей виртуализации был блок
//! верхнего уровня, а их текстовый стиль передаёт детям. `hash_of` — ключ разметки.

use crate::document::box_style::has_box_style;
use crate::document::state::DocState;
use crate::document::state::ValueCx;
use crate::dom::Node;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(crate) struct EmbeddingScheme {
    pub preferred_dark: bool,
    pub owner_dark: bool,
}

/// Разобранный вложенный документ: дерево и СВОЁ
/// состояние (правила, метрики корня, quirks). Сборка его дерева идёт в
/// области `state` (`state.enter()`), элементы — под `Scoped`.
pub struct Embedded {
    pub nodes: Vec<Node>,
    pub state: Rc<DocState>,
}

/// Разбор ВЛОЖЕННОГО документа (`<iframe>`): тот же конвейер, что у
/// `Document::new`, но БЕЗ сброса буферов замеров и проб — они принадлежат
/// внешнему документу, и сброс посреди его отрисовки крал его состояние.
/// Правила и метрики рамки — в её собственном `DocState` (HTML §4.8.5: у
/// рамки свой документ), внешний документ ничего не теряет.
///
/// `viewport` — коробка рамки: `@media (width)` вложенного документа
/// меряется ЕГО областью просмотра (mediaqueries-4 §width — «the width of
/// the targeted display area»; у рамки свой контекст просмотра, HTML
/// §4.8.5), а не внешним окном (`css-page/media-queries-002-print`: рамка
/// 100x100 и `@media (width: 100px) and (height: 100px)`).
#[cfg(test)]
pub(crate) fn parse_embedded(html: &str, theme_css: &str, viewport: (f32, f32)) -> Embedded {
    parse_embedded_sized(html, theme_css, viewport, false, false, true, None)
}

pub(crate) fn parse_embedded_sized(
    html: &str,
    theme_css: &str,
    viewport: (f32, f32),
    print: bool,
    responsive: bool,
    scrolling: bool,
    scheme: Option<EmbeddingScheme>,
) -> Embedded {
    let mut media = crate::style::css::Media {
        width: viewport.0,
        height: viewport.1,
        print,
        ..crate::style::css::Media::default()
    };
    if let Some(scheme) = scheme {
        media.dark = scheme.preferred_dark;
    }
    let state = DocState::new(media);
    let vcx = state.values();
    let mut sheets = vec![theme_css.to_string()];
    sheets.extend(crate::dom::style_sources::collect(html, false));
    for css in &sheets {
        crate::text::fonts::load_faces_additive(css, vcx);
    }
    crate::style::values::color_space::load_profiles_sheets(&sheets, None, vcx);
    let parsed = crate::dom::parse_current(html, theme_css, media, &state);
    let mut nodes =
        super::viewport::prepare_embedded(parsed, responsive, scheme.map(|s| s.owner_dark), vcx);
    // HTML's embedding policy also applies when responsive sizing is declined.
    if !scrolling
        && let [Node::Element(root)] = nodes.as_mut_slice()
        && root.tag == "html-viewport"
    {
        root.style.scroll.width = Some(2);
    }
    // Вложенному документу offset-трансформ нужен ровно так же: проход по
    // дереву живёт здесь, а не в разборе стиля, и без этой строки
    // `<iframe>` потерял бы всё, что раньше работало.
    crate::animation::motion::settle(&mut nodes, vcx);
    state.masks.collect(&nodes, vcx);
    Embedded { nodes, state }
}

/// Снять обёртки `<html>`/`<body>`, которые парсер добавляет всегда.
///
/// Без этого «блоков верхнего уровня» ровно один — весь документ — и
/// виртуализация теряет смысл: список спрашивает единственный элемент и
/// раскладывает вместе с ним всё содержимое.
pub(super) fn unwrap_document(
    nodes: Vec<Node>,
    vcx: ValueCx<'_>,
) -> (Vec<Node>, crate::style::computed::Computed) {
    let mut root = crate::style::computed::Computed::default();
    let mut nodes = nodes;
    loop {
        let single_wrapper = match nodes.as_slice() {
            [Node::Element(e)] if matches!(e.tag.as_str(), "html" | "body") => {
                !e.marks.quirks_fill && !has_box_style(&e.style)
            }
            _ => false,
        };
        if !single_wrapper {
            return (nodes, root);
        }
        let Some(Node::Element(e)) = nodes.pop() else {
            return (nodes, root);
        };
        root = crate::style::cascade::inherit::inherit_unpainted(&root, &e.style, vcx);
        // Обёртка снимается ради виртуализации: единица прокрутки — блок
        // верхнего уровня, а не документ целиком. Но её ТЕКСТОВЫЙ стиль
        // принадлежит содержимому: `body { font: 13px system-ui }` — самый
        // обычный способ задать типографику страницы, и без этого шага он
        // пропадал молча, а текст набирался умолчанием движка.
        nodes = e
            .children
            .into_iter()
            .map(|n| match n {
                Node::Element(mut child) => {
                    child.style = crate::style::cascade::inherit::inherit_unpainted(
                        &e.style,
                        &child.style,
                        vcx,
                    )
                    .into();
                    Node::Element(child)
                }
                other => other,
            })
            .collect();
    }
}

pub(super) fn hash_of(html: &str, theme: &str) -> u64 {
    let mut h = DefaultHasher::new();
    html.hash(&mut h);
    theme.hash(&mut h);
    h.finish()
}
