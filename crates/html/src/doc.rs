//! Разобранный документ: то, что живёт между кадрами.
//!
//! Ключевое требование к переводчику — не быть дороже нативной вёрстки. В
//! GPUI элементы пересоздаются каждый кадр, и это нормально: сборка `div` с
//! готовыми числами дёшева. Дорого другое — разбор разметки, каскад и
//! растеризация рисунков. Всё это обязано случиться ОДИН раз на документ, а
//! не на кадр.
//!
//! Поэтому вызывающий держит у себя `Document`, а на кадре зовёт только
//! `render`. Пересборка происходит лишь когда сменилась сама разметка — что
//! проверяется по хэшу, а не по строке целиком.

#[path = "doc/box_style.rs"]
mod box_style;
use box_style::has_box_style;

use crate::dom::Node;
use crate::value::Len;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Документ, разобранный один раз.
pub struct Document {
    nodes: Vec<Node>,
    /// Текстовый стиль `body`: им набирается текст верхнего уровня, у
    /// которого своего элемента нет.
    root: crate::computed::Computed,
    /// Хэш разметки и темы: по нему видно, нужен ли повторный разбор.
    key: u64,
}

impl Document {
    pub fn new(html: &str, theme_css: &str) -> Self {
        // Свои шрифты страницы грузятся ДО разбора: иначе первый же замер
        // ширины пойдёт по подстановке, а перерисовки под новый шрифт нет.
        crate::fonts::load_faces(html);
        crate::color_space::load_profiles(html);
        crate::lines::forget_measures();
        crate::interact::forget_row_rects();
        crate::interact::forget_vt_measures();
        let (nodes, root) = unwrap_document(mark_canvas_background(resolve_logical(
            propagate_writing_mode(viewport_overflow(crate::dom::parse(html, theme_css))),
        )));
        Document {
            nodes,
            root,
            key: hash_of(html, theme_css),
        }
    }

    /// Текстовый стиль страницы (`html`/`body`): им набирается текст, у
    /// которого своего элемента нет.
    pub fn root_style(&self) -> &crate::computed::Computed {
        &self.root
    }

    /// Разобрать заново, только если разметка действительно изменилась.
    ///
    /// Для стриминга (текст дописывается по кусочку) это и есть главный
    /// рубеж: пока пришедший кусок не изменил разметку, дерево остаётся тем
    /// же, и кадр стоит ровно столько же, сколько нативный.
    pub fn update(&mut self, html: &str, theme_css: &str) -> bool {
        let key = hash_of(html, theme_css);
        if key == self.key {
            return false;
        }
        let (nodes, root) = unwrap_document(mark_canvas_background(resolve_logical(
            propagate_writing_mode(viewport_overflow(crate::dom::parse(html, theme_css))),
        )));
        self.nodes = nodes;
        self.root = root;
        self.key = key;
        true
    }

    /// Хэш содержимого — соль для `RenderOpts::doc_salt`.
    pub fn salt(&self) -> u64 {
        self.key
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Сколько узлов в документе — для решения о виртуализации: раскладка в
    /// GPUI считается заново каждый кадр, поэтому длинный документ обязан
    /// рисоваться по видимым блокам, а не целиком.
    pub fn node_count(&self) -> usize {
        fn walk(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|n| match n {
                    Node::Text(_) => 1,
                    Node::Element(e) => 1 + walk(&e.children),
                })
                .sum()
        }
        walk(&self.nodes)
    }

    /// Блоки верхнего уровня — единица виртуализации: их можно раздать в
    /// список GPUI, который раскладывает только видимое.
    pub fn top_level_blocks(&self) -> usize {
        self.nodes.len()
    }
}

/// Есть ли у обёртки НЕНУЛЕВОЙ отступ хоть с одной стороны.
///
/// Нулевой не считается: `body { margin: 0 }` пишут именно затем, чтобы
/// обёртка ничего не делала, — держать её ради этого значит терять
/// виртуализацию на ровном месте.
fn any_side(s: &crate::computed::Sides) -> bool {
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
fn viewport_overflow(mut nodes: Vec<Node>) -> Vec<Node> {
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
                    && (matches!(e.style.overflow_x, Some(o) if o != crate::computed::Overflow::Visible)
                        || matches!(e.style.overflow_y, Some(o) if o != crate::computed::Overflow::Visible));
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
fn any_containment(e: &crate::dom::Element) -> bool {
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
fn mark_canvas_background(mut nodes: Vec<Node>) -> Vec<Node> {
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
            if html.style.display == Some(crate::computed::Display::Contents) {
                html.style.display = Some(crate::computed::Display::Block);
            }
        }
        // Фон переносится только от элемента С КОРОБКОЙ: `display: none` и
        // `display: contents` коробки не дают, и канвас остаётся чистым
        // (`background-color-body-propagation-007`, `-root-propagation-001`).
        let boxed = |e: &crate::Element| {
            !matches!(
                e.style.display,
                Some(crate::computed::Display::None) | Some(crate::computed::Display::Contents)
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
        let mut moved: Option<crate::computed::Computed> = None;
        for c in html.children.iter_mut() {
            let Node::Element(body) = c else { continue };
            // Переезд фона тела НА КОРЕНЬ — это и есть распространение
            // (CSS 2.1 §14.2, «treated as if they were specified on the root
            // element»). Любое обособление тела его отменяет.
            if body.tag == "body" && has_bg(body) && boxed(body) && !any_containment(body) {
                let s = &mut body.style;
                let mut take = crate::computed::Computed::default();
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
                    Some(crate::value::Len::Px(v)) => v,
                    _ => 16.0,
                };
                let fam = s.font_family.clone().unwrap_or_default();
                let to_px = |l: Option<crate::value::Len>| match l {
                    Some(
                        crate::value::Len::Em(_)
                        | crate::value::Len::Ex(_)
                        | crate::value::Len::Ch(_),
                    ) => Some(crate::value::Len::Px(crate::metrics::spacing_px(
                        l, &fam, em,
                    ))),
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

fn propagate_writing_mode(mut nodes: Vec<Node>) -> Vec<Node> {
    let [Node::Element(html)] = nodes.as_mut_slice() else {
        return nodes;
    };
    if html.tag != "html" {
        return nodes;
    }
    // Ограничение на корне гасит распространение: корень с ним — сам себе
    // область, и наружу его письмо не выходит.
    if any_containment(html) {
        // Тело своё письмо СОХРАНЯЕТ — гасится распространение, а не
        // вычисленное значение (css-writing-modes §3.1). Значит главным
        // потоком тело не стало, и полагающийся главному потоку прижим к
        // краю окна ему не положен. Рисователь обособления КОРНЯ уже не
        // видит, поэтому пометку ставим здесь
        // (contain-html-w-m-001..004).
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
            {
                b.style.wm_contained = true;
            }
        }
        return nodes;
    }
    // `html::before`/`::after` — СОСЕДИ body в потоке страницы: растяжка
    // body на весь вьюпорт (min_h в render) выталкивала их за нижний край,
    // и текст псевдо пропадал (wm-propagation-body-044). Свой нулевой
    // минимум гасит только навязанный, заданного не трогает.
    let has_root_pseudo = html
        .children
        .iter()
        .any(|n| matches!(n, Node::Element(p) if p.tag == "::after" || p.tag == "::before"));
    if has_root_pseudo {
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
                && b.style.height.is_none()
                && b.style.min_height.is_none()
            {
                b.style.min_height = Some(crate::value::Len::Px(0.0));
            }
        }
    }
    let Some(body) = html.children.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) else {
        return nodes;
    };
    // Ограничение на теле оставляет письмо ему: наверх идёт только
    // собственное письмо корня (contain-body-{w-m,t-o}-001..004).
    let body_contained = any_containment(body);
    let taken = if body_contained {
        (
            html.style.vertical,
            html.style.vertical_rl,
            html.style.rtl,
            html.style.sideways,
        )
    } else {
        (
            body.style.vertical.or(html.style.vertical),
            body.style.vertical_rl.or(html.style.vertical_rl),
            body.style.rtl.or(html.style.rtl),
            body.style.sideways.or(html.style.sideways),
        )
    };
    // Та же пометка, что и при обособлении корня: письмо тела осталось при
    // теле, область просмотра его не приняла (css-contain-2
    // §containment-types), и тело — обычный блок в потоке корня. Ставится
    // ПОСЛЕ вычисления `taken`, чтобы неизменяемый заём `body` уже кончился
    // (contain-body-w-m-001..004).
    if body_contained {
        for n in html.children.iter_mut() {
            if let Node::Element(b) = n
                && b.tag == "body"
            {
                b.style.wm_contained = true;
            }
        }
    }
    let own = (
        html.style.vertical,
        html.style.vertical_rl,
        html.style.rtl,
        html.style.sideways,
    );
    // Главное вертикальное письмо: строчная ось корня занимает всё окно
    // (§8.2) — И при письме, заданном на самом `<html>` (taken == own):
    // эталон wm-propagation-047 ставит sideways-lr на корень, и без 100vh
    // прижимать содержимое к низу не от чего.
    // До раннего выхода — только sideways-lr (нужен низ, wm-prop-047-ref);
    // обычные вертикальные корни с письмом на самом html так теряли
    // скроллер-структуру (available-size-022/023 замерено).
    // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2): якорь самим
    // стилем; корню с фоном-картинкой не ставится — гасил canvas-слой
    // (замерено на background-size-document-root-vrl-*).
    if taken.1 == Some(true)
        && html.style.align_self.is_none()
        && std::env::var("ANCH_BG").map_or(html.style.bg_image.is_none(), |_| true)
    {
        html.style.align_self = Some(crate::computed::Align::End);
    }
    // Вертикальный корень с ФОНОМ-КАРТИНКОЙ: без минимума высоты его
    // коробка при пустом теле нулевая, и краске негде лечь
    // (background-size-document-root-vrl-*). Обычным вертикальным корням
    // минимум не ставится — терялась скроллер-структура (av-size-022/023).
    if taken.0 == Some(true)
        && html.style.bg_image.is_some()
        && html.style.height.is_none()
        && html.style.min_height.is_none()
    {
        html.style.min_height = Some(crate::value::Len::Vh(1.0));
    }
    let side_lr = taken.3 == Some(true) && taken.1 != Some(true);
    if taken.0 == Some(true)
        && side_lr
        && html.style.height.is_none()
        && html.style.min_height.is_none()
    {
        html.style.min_height = Some(crate::value::Len::Vh(1.0));
    }
    if taken == own {
        return nodes;
    }
    // Вычисленные значения не меняются ни у кого (§8.2): распространяется
    // только used корневой коробки. Псевдо-дети html (`::before`/`::after`)
    // наследуют СОБСТВЕННОЕ письмо html — оно прикалывается им заранее
    // (wm-propagation-body-044: html vlr + body htb, текст псевдо вертикален).
    for child in html.children.iter_mut() {
        let Node::Element(e) = child else { continue };
        if !(e.tag == "::after" || e.tag == "::before") {
            continue;
        }
        e.style.vertical = e.style.vertical.or(own.0);
        e.style.vertical_rl = e.style.vertical_rl.or(own.1);
        e.style.rtl = e.style.rtl.or(own.2);
        e.style.sideways = e.style.sideways.or(own.3);
    }
    if taken.0 == Some(true) && html.style.height.is_none() && html.style.min_height.is_none() {
        html.style.min_height = Some(crate::value::Len::Vh(1.0));
    }
    (
        html.style.vertical,
        html.style.vertical_rl,
        html.style.rtl,
        html.style.sideways,
    ) = taken;
    nodes
}

/// Разложить логические стороны и размеры по физическим — по всему дереву.
///
/// Какая сторона логического начала физическая, знает только письмо, а оно
/// НАСЛЕДУЕТСЯ. Пока перевод шёл при разборе, он молча считал письмо
/// горизонтальным: `margin-block` в вертикальном тексте разворачивал отступ
/// не по той оси, а `inline-size` задавал не ту сторону коробки.
///
/// Проход идёт после каскада и после распространения письма с `<body>` —
/// то есть в единственной точке, где письмо узла уже окончательно.
fn resolve_logical(mut nodes: Vec<Node>) -> Vec<Node> {
    fn walk(
        nodes: &mut [Node],
        mode: (Option<bool>, Option<bool>, Option<bool>, Option<bool>),
    ) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            // Письмо и направление наследуются; свои значения сильнее.
            // `sideways` — часть ЗНАЧЕНИЯ `writing-mode`, а оно наследуемое
            // (css-writing-modes-4 §3.1, `Inherited: yes`). Без него потомок
            // блока `sideways-lr`, у которого своего письма нет, разбирал
            // логические стороны по строке `vertical-lr` таблицы
            // §Abstract-Physical Mapping: `inline-start` уезжал с НИЖНЕГО
            // края на верхний (эталон `initial-letter-block-position-
            // margins-slr-ref` ставил `margin-inline-start: 15px` сверху).
            let own = (
                e.style.vertical.or(mode.0),
                e.style.vertical_rl.or(mode.1),
                e.style.rtl.or(mode.2),
                e.style.sideways.or(mode.3),
            );
            let (was_v, was_rl, was_rtl, was_sw) = (
                e.style.vertical,
                e.style.vertical_rl,
                e.style.rtl,
                e.style.sideways,
            );
            e.style.vertical = own.0;
            e.style.vertical_rl = own.1;
            e.style.rtl = own.2;
            e.style.sideways = own.3;
            if {
                static ON: std::sync::LazyLock<bool> =
                    std::sync::LazyLock::new(|| std::env::var("LOG_DBG").is_ok());
                *ON
            } && e
                .style
                .logical
                .as_ref()
                .is_some_and(|l| l.border.iter().any(|b| b.is_some()))
            {
                eprintln!(
                    "LOG tag={} cls={:?} v={:?} rl={:?} rtl={:?} sw={:?} border={:?}",
                    e.tag,
                    e.attr("class"),
                    e.style.vertical,
                    e.style.vertical_rl,
                    e.style.rtl,
                    e.style.sideways,
                    e.style.logical.as_ref().map(|l| l.border.clone())
                );
            }
            // Табличность ячейки на этом шаге держится ТЕГОМ:
            // `Display::TableCell` приходит только из авторского CSS
            // (замеренный откат в шапке `resolve_logical`), поэтому
            // проверяются оба признака.
            let is_cell = matches!(e.tag.as_str(), "td" | "th")
                || e.style.display == Some(crate::computed::Display::TableCell);
            e.style.resolve_logical(mode.0, is_cell);
            // Слои `:hover`, `::first-letter` и `::first-line` — ТЕМ ЖЕ
            // проходом. Слой собирается копией стиля элемента ДО этого
            // прохода (`dom.rs:2473 layer()`), поэтому логические стороны
            // остаются у него в `Computed::logical` и на физические поля не
            // ложатся НИКОГДА. Из-за этого `::first-letter
            // { margin-block-start: 10px }` не доезжал до буквицы:
            // `render::initial_letter_float` читает у слоя `margin.top` и
            // родню, а там `None`.
            for layer in [
                e.hover.as_mut(),
                e.first_letter.as_mut(),
                e.first_line.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                let (lv, lrl, lrtl, lsw) =
                    (layer.vertical, layer.vertical_rl, layer.rtl, layer.sideways);
                layer.vertical = own.0;
                layer.vertical_rl = own.1;
                layer.rtl = own.2;
                layer.sideways = own.3;
                layer.resolve_logical(mode.0, is_cell);
                layer.vertical = lv;
                layer.vertical_rl = lrl;
                layer.rtl = lrtl;
                layer.sideways = lsw;
            }
            // Унаследованное обратно снимается: наследованием занимается
            // сборщик дерева, и оставленное здесь значение завело бы узлу
            // собственную коробку (см. `has_box_style`).
            e.style.vertical = was_v;
            e.style.vertical_rl = was_rl;
            e.style.rtl = was_rtl;
            e.style.sideways = was_sw;
            walk(&mut e.children, own);
        }
    }
    walk(&mut nodes, (None, None, None, None));
    // `zoom` (css-viewport-1): длины под зумом домножаются ЗДЕСЬ, отдельным
    // проходом по собственным стилям — до слияния и до раскладки, а не в
    // `inline::inherit` (см. ★ перед ней). Идёт после `walk`: логические
    // стороны уже физические. Страницу без `zoom` проход не меняет: у неё ни
    // одного элемента с `zoom`, и ни одна ветка записи не исполняется.
    crate::zoom::resolve(&mut nodes);
    // `z-index: inherit` и `clip: inherit` разбор выражает только разрядом
    // `inherit_bits`, а значение родителя кладёт слияние (`inline::inherit`) —
    // в СЛИТЫЙ стиль. Сборщик же дерева решает слой и обрезку по
    // СОБСТВЕННОМУ (`defers(&e.style, …)`, `below`, `grouped(…, &e.style)`),
    // и там оставалось прежнее объявление: `z-index: -1; z-index: inherit`
    // клал зелёный под поток (`z-index-014`), `clip: inherit` не резал
    // ничего (`clip-102`). CSS 2.1 §6.2.1: «the property takes the same
    // computed value as the property for the element's parent».
    settle_explicit_inherit(&mut nodes, None);
    // Якорные вставки, которым не разрешиться никогда (нет имени и якоря
    // по умолчанию; имя, которого в документе нет), сводятся к запасному
    // значению или к `auto` ЗДЕСЬ — сборщик дерева выбирает статическую
    // позицию по `edge_set`, а логические вставки к этому шагу уже легли на
    // физические стороны (`Computed::resolve_logical` выше).
    crate::anchor::settle_static(&mut nodes);
    // motion-1: offset-трансформ — вторым проходом по СОБРАННОМУ дереву, где у
    // каждой коробки есть родитель. Идёт после `zoom::resolve` (длины уже
    // домножены) и после `resolve_logical` (стороны уже физические).
    crate::motion::settle(&mut nodes);
    nodes
}

/// Явное `inherit` у `z-index` и `clip` — в собственный стиль элемента.
///
/// Оба свойства ненаследуемые, поэтому собственный стиль родителя и есть его
/// вычисленное значение (как у `zoom::explicit`). Проход сверху вниз: у
/// родителя цепочка `inherit` к этому шагу уже разрешена.
fn settle_explicit_inherit(nodes: &mut [Node], parent: Option<&crate::computed::Computed>) {
    use crate::computed::inh;
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        if let Some(p) = parent {
            if e.style.inherit_bits & inh::Z_INDEX != 0 {
                e.style.z_index = p.z_index;
            }
            if e.style.inherit_bits & inh::CLIP != 0 {
                e.style.clip_rect = p.clip_rect;
                e.style.clip_len = p.clip_len;
            }
            if e.style.grid_areas_inherit {
                e.style.grid_areas = p.grid_areas.clone();
            }
        }
        settle_explicit_inherit(&mut e.children, Some(&e.style));
    }
}

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
    crate::fonts::load_faces_additive(html);
    crate::color_space::load_profiles(html);
    // Пулы `@page` ВНЕШНЕГО документа: `parse_media` начинает с их очистки
    // (`take_page_decls`), а рамка разбирается на КАЖДОМ кадре — лист
    // печатной пары со второго кадра терял size/margin/фон. Правила `@page`
    // самой рамки к листам внешнего документа не относятся (css-page-3: page
    // context — только у корневого документа), поэтому пулы возвращаются.
    let outer_rules = crate::css::page_rules_snapshot();
    let media = crate::css::Media {
        width: viewport.0,
        height: viewport.1,
        ..crate::css::Media::default()
    };
    let parsed = crate::dom::parse_media(html, theme_css, media);
    *crate::css::PAGE_RULES.lock().unwrap() = outer_rules;
    let (mut nodes, _root) = unwrap_document(mark_canvas_background(resolve_logical(
        propagate_writing_mode(viewport_overflow(parsed)),
    )));
    // Вложенному документу offset-трансформ нужен ровно так же: проход по
    // дереву переехал сюда из разбора стиля (`dom.rs`), и без этой строки
    // `<iframe>` потерял бы всё, что раньше работало.
    crate::motion::settle(&mut nodes);
    (nodes, hash_of(html, theme_css))
}

/// Снять обёртки `<html>`/`<body>`, которые парсер добавляет всегда.
///
/// Без этого «блоков верхнего уровня» ровно один — весь документ — и
/// виртуализация теряет смысл: список спрашивает единственный элемент и
/// раскладывает вместе с ним всё содержимое.
fn unwrap_document(nodes: Vec<Node>) -> (Vec<Node>, crate::computed::Computed) {
    let mut root = crate::computed::Computed::default();
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
        root = crate::inline::inherit(&root, &e.style);
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
                    child.style = crate::inline::inherit(&e.style, &child.style);
                    Node::Element(child)
                }
                other => other,
            })
            .collect();
    }
}

fn hash_of(html: &str, theme: &str) -> u64 {
    let mut h = DefaultHasher::new();
    html.hash(&mut h);
    theme.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_after_survives_unwrap() {
        let doc = Document::new(
            "<style>html::after { content: \"AFTER\"; display: block; }</style>             <body><div>x</div></body>",
            "",
        );
        let tags: Vec<String> = doc
            .nodes()
            .iter()
            .map(|n| match n {
                Node::Element(e) => e.tag.clone(),
                Node::Text(t) => format!("txt[{t}]"),
            })
            .collect();
        eprintln!("TOP: {tags:?}");
        fn has_after(nodes: &[Node]) -> bool {
            nodes.iter().any(|n| match n {
                Node::Element(e) => e.tag.contains("after") || has_after(&e.children),
                _ => false,
            })
        }
        assert!(has_after(doc.nodes()), "html::after потерян: {tags:?}");
    }

    #[test]
    fn same_markup_is_not_reparsed() {
        let mut doc = Document::new("<p>раз</p>", "");
        assert!(
            !doc.update("<p>раз</p>", ""),
            "разметка та же — разбора нет"
        );
        assert!(
            doc.update("<p>два</p>", ""),
            "разметка иная — разобрали заново"
        );
    }

    #[test]
    fn theme_change_also_triggers_a_reparse() {
        let mut doc = Document::new("<p>раз</p>", "");
        assert!(
            doc.update("<p>раз</p>", "p { color: red }"),
            "сменилась тема"
        );
    }

    #[test]
    fn document_wrappers_are_unwrapped() {
        // Парсер всегда добавляет <html><body>; для виртуализации нужны
        // настоящие блоки документа, а не одна обёртка.
        let doc = Document::new("<div>раз</div><div>два</div>", "");
        assert_eq!(
            doc.top_level_blocks(),
            2,
            "получено {}",
            doc.top_level_blocks()
        );
    }

    #[test]
    fn node_count_walks_the_whole_tree() {
        let doc = Document::new("<div><p>раз</p><p>два</p></div>", "");
        // html + body + div + два абзаца + два текста — важна не точная цифра,
        // а то, что счёт идёт вглубь.
        assert!(doc.node_count() >= 5, "получено {}", doc.node_count());
    }
}
