//! Embedded for document; split out to keep the owning module within 250 lines.

use super::{mark_canvas_background, propagate_writing_mode, resolve_logical, viewport_overflow};
use crate::document::box_style::has_box_style;
use crate::dom::Node;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Разбор ВЛОЖЕННОГО документа (`<iframe>`): тот же конвейер, что у
/// `Document::new`, но БЕЗ сброса буферов замеров и проб — они принадлежат
/// внешнему документу, и сброс посреди его отрисовки крал его состояние.
///
/// `viewport` — коробка рамки: `@media (width)` вложенного документа
/// меряется ЕГО областью просмотра (mediaqueries-4 §width — «the width of
/// the targeted display area»; у рамки свой контекст просмотра, HTML
/// §4.8.5), а не внешним окном (`css-page/media-queries-002-print`: рамка
/// 100x100 и `@media (width: 100px) and (height: 100px)`).
pub fn parse_embedded(html: &str, theme_css: &str, viewport: (f32, f32)) -> (Vec<Node>, u64) {
    crate::text::fonts::load_faces_additive(html);
    crate::style::values::color_space::load_profiles(html);
    // Пулы `@page` ВНЕШНЕГО документа: `parse_media` начинает с их очистки
    // (`take_page_decls`), а рамка разбирается на КАЖДОМ кадре — лист
    // печатной пары со второго кадра терял size/margin/фон. Правила `@page`
    // самой рамки к листам внешнего документа не относятся (css-page-3: page
    // context — только у корневого документа), поэтому пулы возвращаются.
    let outer_rules = crate::style::css::page_rules_snapshot();
    let media = crate::style::css::Media {
        width: viewport.0,
        height: viewport.1,
        ..crate::style::css::Media::default()
    };
    let parsed = crate::dom::parse_media(html, theme_css, media);
    *crate::style::css::PAGE_RULES.lock().unwrap() = outer_rules;
    let (mut nodes, _root) = unwrap_document(mark_canvas_background(resolve_logical(
        propagate_writing_mode(viewport_overflow(parsed)),
    )));
    // Вложенному документу offset-трансформ нужен ровно так же: проход по
    // дереву переехал сюда из разбора стиля (`dom.rs`), и без этой строки
    // `<iframe>` потерял бы всё, что раньше работало.
    crate::animation::motion::settle(&mut nodes);
    (nodes, hash_of(html, theme_css))
}

/// Снять обёртки `<html>`/`<body>`, которые парсер добавляет всегда.
///
/// Без этого «блоков верхнего уровня» ровно один — весь документ — и
/// виртуализация теряет смысл: список спрашивает единственный элемент и
/// раскладывает вместе с ним всё содержимое.
pub(super) fn unwrap_document(nodes: Vec<Node>) -> (Vec<Node>, crate::style::computed::Computed) {
    let mut root = crate::style::computed::Computed::default();
    let mut nodes = nodes;
    loop {
        let single_wrapper = match nodes.as_slice() {
            [Node::Element(e)] if matches!(e.tag.as_str(), "html" | "body") => {
                !has_box_style(&e.style)
            }
            _ => false,
        };
        if !single_wrapper {
            return (nodes, root);
        }
        let Some(Node::Element(e)) = nodes.pop() else {
            return (nodes, root);
        };
        root = crate::style::cascade::inherit::inherit_unpainted(&root, &e.style);
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
                    child.style =
                        crate::style::cascade::inherit::inherit_unpainted(&e.style, &child.style);
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
