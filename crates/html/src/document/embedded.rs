//! Вложенный документ (`<iframe>`, SVG с `<foreignObject>`): свой разбор в
//! своё состояние документа, снятие обёрток `html`/`body`, хэш содержимого.

use crate::document::box_style::has_box_style;
use crate::document::state::DocState;
use crate::dom::Node;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(crate) struct EmbeddingScheme {
    pub preferred_dark: bool,
    pub owner_dark: bool,
}

/// Разобранный вложенный документ: дерево, соль буферов проб и СВОЁ
/// состояние (правила, метрики корня, quirks). Сборка его дерева идёт в
/// области `state` (`state.enter()`), элементы — под `Scoped`.
pub struct Embedded {
    pub nodes: Vec<Node>,
    pub salt: u64,
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
    let print = crate::document::state::current().media().print;
    parse_embedded_sized(html, theme_css, viewport, print, false, true, None)
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
    let _scope = state.enter();
    let mut sheets = vec![theme_css.to_string()];
    sheets.extend(crate::dom::style_sources::collect(html, false));
    for css in &sheets {
        crate::text::fonts::load_faces_additive(css);
    }
    crate::style::values::color_space::load_profiles_sheets(&state, &sheets, None);
    let parsed = crate::dom::parse_current(html, theme_css, media, &state);
    let mut nodes =
        super::viewport::prepare_embedded(parsed, responsive, scheme.map(|s| s.owner_dark));
    // HTML's embedding policy also applies when responsive sizing is declined.
    if !scrolling
        && let [Node::Element(root)] = nodes.as_mut_slice()
        && root.tag == "html-viewport"
    {
        root.style.scroll.width = Some(2);
    }
    // Вложенному документу offset-трансформ нужен ровно так же: проход по
    // дереву переехал сюда из разбора стиля (`dom.rs`), и без этой строки
    // `<iframe>` потерял бы всё, что раньше работало.
    crate::animation::motion::settle(&mut nodes);
    drop(_scope);
    Embedded {
        nodes,
        salt: hash_of(html, theme_css),
        state,
    }
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
                        crate::style::cascade::inherit::inherit_unpainted(&e.style, &child.style)
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
