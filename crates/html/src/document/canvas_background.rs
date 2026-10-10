//! Canvas background for document; split out to keep the owning module within 250 lines.

use crate::dom::Node;
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
                    && (matches!(e.style.overflow_x, Some(o) if o != crate::style::computed::Overflow::Visible)
                        || matches!(e.style.overflow_y, Some(o) if o != crate::style::computed::Overflow::Visible));
                if !own && !root_contained {
                    e.style.overflow_x = None;
                    e.style.overflow_y = None;
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
    e.style.contain_paint == Some(true)
        || e.style.contain_size == Some(true)
        || e.style.contain_layout == Some(true)
        || e.style.contain_style == Some(true)
        || e.style.skip_content == Some(true)
}

/// Главное письмо страницы задаёт `<body>`, а не корень.
///
/// CSS Writing Modes §8.1: сторона письма и направление берутся с корня и
/// управляют областью просмотра целиком — прокруткой, началом отсчёта, сменой
/// страниц. В HTML у правила оговорка: если у корня есть `<body>`, значения
/// берутся С НЕГО, и заданное прямо на `<html>` проигрывает. Без этого шага
/// письмо `body` работало только наследованием вниз, корневая коробка
/// оставалась горизонтальной, а `vertical-rl` начинался от левого края окна
/// вместо правого.
///
/// Вычисленные значения при этом не меняются НИ У КОГО: распространяется
/// только используемое значение корневой коробки. Поэтому собственное письмо
/// `html` заранее прикалывается к тем его детям, которые не `body`
/// (`::before`, `::after`): наследуют они именно его.
/// Отметить узел, чей фон красит канвас (CSS 2.2 §14.2): корневой `html`,
/// а без его фона — `body`. `contain: paint` глушит распространение.
pub(super) fn mark_canvas_background(mut nodes: Vec<Node>) -> Vec<Node> {
    fn has_bg(e: &crate::dom::Element) -> bool {
        // Прозрачный цвет — НЕ краска: `html { background: transparent }`
        // отдаёт канвас телу, как и отсутствие объявления.
        e.style.background.is_some_and(|c| c.a > 0.0)
            || e.style.gradient.is_some()
            // Фон-картинка — та же краска канваса (CSS 2.2 §14.2): без
            // пометки она рисовалась коробкой корня и начиналась с его
            // сдвинутого схлопкой верха (background-size-document-root-vrl).
            || e.style.bg_image.is_some()
    }
    for n in nodes.iter_mut() {
        let Node::Element(html) = n else { continue };
        // §10.5: «A percentage height on the root element is relative to the
        // initial containing block» — высота начального блока определена
        // всегда. Пометка живёт на самом узле: снимаемая обёртка уносит её с
        // собой, и дети `body` верхнего уровня её не получают.
        if html.tag == "html" {
            html.style.root_box = true;
            // css-display-3 §2.7: «a display of contents computes to block on
            // the root element». Без коробки корня слой канваса (`canvas_bg`
            // ниже) не заводился, и фон-картинка корня пропадала целиком
            // (`display-contents-root-background` 99.93).
            if html.style.display == Some(crate::style::computed::Display::Contents) {
                html.style.display = Some(crate::style::computed::Display::Block);
            }
        }
        // Фон переносится только от элемента С КОРОБКОЙ: `display: none` и
        // `display: contents` коробки не дают, и канвас остаётся чистым
        // (`background-color-body-propagation-007`, `-root-propagation-001`).
        let boxed = |e: &crate::dom::Element| {
            !matches!(
                e.style.display,
                Some(crate::style::computed::Display::None)
                    | Some(crate::style::computed::Display::Contents)
            )
        };
        if html.tag != "html" {
            // css-contain-2 §2.1: распространение свойств тела в область
            // просмотра и на канвас глушит ЛЮБОЕ обособление на `html` или
            // `body`, а не только `paint`. Узкая проверка оставляла зелёной
            // одну `contain-{body,html}-bg-002`, где написан именно
            // `contain: paint`; `layout`, `size` и `style` пролезали.
            if html.tag == "body" && has_bg(html) && boxed(html) && !any_containment(html) {
                html.style.canvas_bg = true;
            }
            continue;
        }
        // Обособление на КОРНЕ рвёт ту же цепочку: значение тела уезжает во
        // вьюпорт ЧЕРЕЗ корень (`contain-html-bg-001/003/004`).
        if any_containment(html) {
            continue;
        }
        if has_bg(html) && boxed(html) {
            html.style.canvas_bg = true;
            continue;
        }
        // §14.2: «the propagated values are treated as if they were specified
        // on the root element». Фон тела ПЕРЕЕЗЖАЕТ на корень целиком, а не
        // помечается на месте: иначе область отсчёта плитки считалась бы от
        // полей ТЕЛА.
        let mut moved: Option<crate::style::computed::Computed> = None;
        for c in html.children.iter_mut() {
            let Node::Element(body) = c else { continue };
            // Переезд фона тела НА КОРЕНЬ — это и есть распространение
            // (CSS 2.1 §14.2, «treated as if they were specified on the root
            // element»). Любое обособление тела его отменяет.
            if body.tag == "body" && has_bg(body) && boxed(body) && !any_containment(body) {
                let s = &mut body.style;
                let mut take = crate::style::computed::Computed::default();
                take.background = s.background.take();
                take.background_rcs = s.background_rcs.take();
                take.gradient = s.gradient.take();
                take.gradient_raw = s.gradient_raw.take();
                take.bg_image = s.bg_image.take();
                take.bg_size = std::mem::take(&mut s.bg_size);
                // Шрифтовые единицы позиции решаются по кеглю ТЕЛА, на
                // котором они написаны: после переезда их посчитали бы от
                // кегля корня (`background-position-001`: `6.25ex` при 20px
                // Ahem у тела давало 100 вместо 200).
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разрешать СВОЙ кегль тела по кеглю
                // корня (`body { font-size: 2.5ex }` при `html { font: 20px/1
                // Ahem }` — это 40 точек, и уже от них `6.25ex` = 200).
                // Разрешение написано и считает именно так; `background-
                // position-001` ушла 1.99 → 2.09, то есть 200 точек эталон НЕ
                // ждёт. Значит корень пары не в кегле, а в области отсчёта
                // перенесённого фона. Возвращать вместе с ней.
                let em = match s.font_size {
                    Some(crate::style::values::value::Len::Px(v)) => v,
                    _ => 16.0,
                };
                let fam = s.font_family.clone().unwrap_or_default();
                let to_px = |l: Option<crate::style::values::value::Len>| match l {
                    Some(
                        crate::style::values::value::Len::Em(_)
                        | crate::style::values::value::Len::Ex(_)
                        | crate::style::values::value::Len::Ch(_),
                    ) => Some(crate::style::values::value::Len::Px(
                        crate::text::metrics::spacing_px(l, &fam, em),
                    )),
                    other => other,
                };
                let mut pos = std::mem::take(&mut s.bg_pos);
                pos.x = to_px(pos.x);
                pos.y = to_px(pos.y);
                take.bg_pos = pos;
                take.bg_repeat = s.bg_repeat.take();
                take.bg_clip = s.bg_clip.take();
                take.bg_origin = s.bg_origin.take();
                take.bg_fixed = s.bg_fixed.take();
                // Списки слоёв переезжают вместе с верхним слоем (§14.2).
                take.bg_lists = std::mem::take(&mut s.bg_lists);
                moved = Some(take);
                break;
            }
        }
        if let Some(take) = moved {
            let s = &mut html.style;
            s.background = take.background;
            s.background_rcs = take.background_rcs;
            s.gradient = take.gradient;
            s.gradient_raw = take.gradient_raw;
            s.bg_image = take.bg_image;
            s.bg_size = take.bg_size;
            s.bg_pos = take.bg_pos;
            s.bg_repeat = take.bg_repeat;
            s.bg_clip = take.bg_clip;
            s.bg_origin = take.bg_origin;
            s.bg_fixed = take.bg_fixed;
            s.bg_lists = take.bg_lists;
            s.canvas_bg = true;
        }
    }
    nodes
}
