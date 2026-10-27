//! Распространение свойств `<html>`/`<body>` на область просмотра и канвас.
//!
//! Проходы по собранному дереву перед снятием обёрток: `viewport_overflow`
//! переносит `overflow` корня или тела на область просмотра (css-overflow-3
//! §Overflow Viewport Propagation, с учётом обособления по css-contain),
//! `mark_canvas_background` помечает узел, чей фон красит канвас (CSS 2.2
//! §14.2: корень, а без его фона — тело. Фон донора передаётся отдельным входом холста).
//! Здесь же предикаты `any_side` и `any_containment` для соседних проходов.

use crate::dom::Node;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Есть ли у обёртки НЕНУЛЕВОЙ отступ хоть с одной стороны.
///
/// Нулевой не считается: `body { margin: 0 }` пишут именно затем, чтобы
/// обёртка ничего не делала, — держать её ради этого значит терять
/// виртуализацию на ровном месте.
pub(super) fn any_side(s: &crate::style::computed::Sides) -> bool {
    [&s.top, &s.right, &s.bottom, &s.left]
        .into_iter()
        .any(|one| !matches!(one, None | Some(Len::Px(0.0))))
}

/// `overflow` у `<html>`/`<body>` принадлежит ОБЛАСТИ ПРОСМОТРА, а не элементу.
///
/// Так велит CSS: прокрутку страницы задают на этих тегах, но прокручивается
/// при этом окно, а сам элемент считается `visible`. Пока мы понимали запись
/// буквально, `body { overflow: hidden }` заводил на теле отдельный контекст
/// форматирования, отступ первого абзаца переставал схлопываться с полем тела,
/// и вся страница уезжала вниз на этот отступ.
pub(super) fn viewport_overflow(mut nodes: Vec<Node>) -> Vec<Node> {
    fn strip(nodes: &mut [Node], root_contained: bool) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            if matches!(e.tag.as_str(), "html" | "body") {
                // Любое ограничение РВЁТ цепочку распространения: своё — у
                // элемента, корневое — у тела тоже (значение тела уезжает
                // во вьюпорт ЧЕРЕЗ корень; css-contain-1 §3.1,
                // contain-{body,html}-overflow-001..004).
                let own = any_containment(e);
                // Значение тела уезжает во вьюпорт лишь когда у корня своё
                // `overflow` — visible по обеим осям (css-overflow-3 §Overflow
                // Viewport Propagation): иначе во вьюпорт идёт корневое, а
                // тело держит своё (`overflow-body-propagation-012`).
                let root_own_overflow = e.tag == "html"
                    && (matches!(e.style.cascaded().overflow_x, Some(o) if o != crate::style::computed::Overflow::Visible)
                        || matches!(e.style.cascaded().overflow_y, Some(o) if o != crate::style::computed::Overflow::Visible));
                if !own && !root_contained {
                    if e.tag == "html" {
                        e.style.cascaded_mut().scroll.viewport_root = true;
                    }
                    e.style.cascaded_mut().overflow_x = None;
                    e.style.cascaded_mut().overflow_y = None;
                }
                strip(
                    &mut e.children,
                    root_contained || (e.tag == "html" && (own || root_own_overflow)),
                );
            }
        }
    }
    strip(&mut nodes, false);
    nodes
}

/// Есть ли на элементе хоть одно ограничение (`contain`, включая
/// `content-visibility: hidden`): оно выключает распространение свойств
/// элемента в область просмотра.
pub(super) fn any_containment(e: &crate::dom::Element) -> bool {
    e.style.cascaded().contain_paint == Some(true)
        || e.style.cascaded().contain_size == Some(true)
        || e.style.cascaded().contain_layout == Some(true)
        || e.style.cascaded().contain_style == Some(true)
        || e.style.cascaded().skip_content == Some(true)
}

/// Отметить узел, чей фон красит канвас (CSS 2.2 §14.2): корневой `html`,
/// а без его фона — `body`. `contain: paint` глушит распространение.
pub(super) fn mark_canvas_background(
    mut nodes: Vec<Node>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Vec<Node> {
    fn has_bg(style: &Computed) -> bool {
        // Прозрачный цвет — НЕ краска: `html { background: transparent }`
        // отдаёт канвас телу, как и отсутствие объявления.
        style.background.is_some_and(|c| c.a > 0.0)
            || style.gradient.is_some()
            // Фон-картинка — та же краска канваса (CSS 2.2 §14.2): без
            // пометки она рисовалась коробкой корня и начиналась с его
            // сдвинутого схлопкой верха (background-size-document-root-vrl).
            || style.bg_image.is_some()
    }
    for n in nodes.iter_mut() {
        let Node::Element(html) = n else { continue };
        // §10.5: «A percentage height on the root element is relative to the
        // initial containing block» — высота начального блока определена
        // всегда. Пометка живёт на самом узле: снимаемая обёртка уносит её с
        // собой, и дети `body` верхнего уровня её не получают.
        if html.tag == "html" {
            html.style.cascaded_mut().root_box = true;
            // css-display-3 §2.7: «a display of contents computes to block on
            // the root element». Без коробки корня слой канваса (`canvas_bg`
            // ниже) не заводился, и фон-картинка корня пропадала целиком
            // (`display-contents-root-background` 99.93).
            if html.style.cascaded().display == Some(crate::style::computed::Display::Contents) {
                html.style.cascaded_mut().display = Some(crate::style::computed::Display::Block);
            }
        }
        // Фон переносится только от элемента С КОРОБКОЙ: `display: none` и
        // `display: contents` коробки не дают, и канвас остаётся чистым
        // (`background-color-body-propagation-007`, `-root-propagation-001`).
        let boxed = |e: &crate::dom::Element| {
            !matches!(
                e.style.cascaded().display,
                Some(crate::style::computed::Display::None)
                    | Some(crate::style::computed::Display::Contents)
            )
        };
        let root_style = crate::style::cascade::preparation::computed(
            &Computed::default(),
            html.style.cascaded(),
            vcx,
        );
        if html.tag != "html" {
            // css-contain-2 §2.1: распространение свойств тела в область
            // просмотра и на канвас глушит ЛЮБОЕ обособление на `html` или
            // `body`, а не только `paint`. Узкая проверка оставляла зелёной
            // одну `contain-{body,html}-bg-002`, где написан именно
            // `contain: paint`; `layout`, `size` и `style` пролезали.
            if html.tag == "body" && has_bg(&root_style) && boxed(html) && !any_containment(html) {
                html.style.cascaded_mut().canvas_bg = true;
            }
            continue;
        }
        // Обособление на КОРНЕ рвёт ту же цепочку: значение тела уезжает во
        // вьюпорт ЧЕРЕЗ корень (`contain-html-bg-001/003/004`).
        if any_containment(html) {
            continue;
        }
        if has_bg(&root_style) && boxed(html) {
            html.style.cascaded_mut().canvas_bg = true;
            continue;
        }
        // css-backgrounds-3 §2.11.2: донор задаёт фон холста, но не CSS
        // корня. Область позиционирования по-прежнему принадлежит корню.
        let mut moved = None;
        for c in html.children.iter_mut() {
            let Node::Element(body) = c else { continue };
            // Обособление body отменяет распространение его фона.
            if body.tag != "body" || !boxed(body) || any_containment(body) {
                continue;
            }
            let donor = crate::style::cascade::preparation::computed(
                &root_style,
                body.style.cascaded(),
                vcx,
            );
            if has_bg(&donor) {
                // css-backgrounds-3 §2.11.2: initial только used-фон body;
                // его computed CSS остаётся источником явного inherit.
                body.style
                    .set_background_paint(crate::style::background_paint::BackgroundPaint::Initial);
                moved =
                    Some(crate::style::canvas_background::CanvasBackground::from_computed(&donor));
                break;
            }
        }
        if let Some(take) = moved {
            html.style
                .set_canvas_background(Some(std::rc::Rc::new(take)));
            html.style.cascaded_mut().canvas_bg = true;
        }
    }
    nodes
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod font_tests;

#[cfg(test)]
mod suppression_tests;

#[cfg(test)]
mod expression_tests;
