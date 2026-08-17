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

/// Задаёт ли стиль обёртки её КОРОБКУ, а не только текст внутри.
///
/// `body { width: 600px; position: relative }` — обычный способ задать
/// систему координат странице, и снятая обёртка уносила её с собой: проценты
/// внутри считались от окна, а абсолютные дети — от другого предка.
fn has_box_style(c: &crate::computed::Computed) -> bool {
    c.width.is_some()
        || c.height.is_some()
        || c.min_width.is_some()
        || c.min_height.is_some()
        || c.max_width.is_some()
        || c.max_height.is_some()
        || c.position.is_some()
        || c.display.is_some()
        || c.overflow_x.is_some()
        || c.overflow_y.is_some()
        // Вертикальное письмо задаёт ОСЬ ПОТОКА детей: без коробки её задать
        // некому, а дети об этом знают только через родителя.
        || c.vertical.is_some()
        // Поля и внутренние отступы обёртки сдвигают ВСЁ содержимое: браузер
        // держит на `body` умолчание в 8 точек, и снятая обёртка уносила этот
        // сдвиг с собой — страница прижималась к краю окна.
        || any_side(&c.margin)
        || any_side(&c.padding)
        // Фон обёртки — её собственная краска: снятая обёртка уносила
        // `html { background: … }` с собой, и корневой фон пропадал
        // (страница выходила белой при пустом теле).
        || c.background.is_some()
        || c.gradient.is_some()
        || c.bg_image.is_some()
        || c.background_rcs.is_some()
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
    fn strip(nodes: &mut [Node]) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            if matches!(e.tag.as_str(), "html" | "body") {
                e.style.overflow_x = None;
                e.style.overflow_y = None;
                strip(&mut e.children);
            }
        }
    }
    strip(&mut nodes);
    nodes
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
        e.style.background.is_some_and(|c| c.a > 0.0) || e.style.gradient.is_some()
    }
    for n in nodes.iter_mut() {
        let Node::Element(html) = n else { continue };
        if html.tag != "html" {
            if html.tag == "body" && has_bg(html) && html.style.contain_paint != Some(true) {
                html.style.canvas_bg = true;
            }
            continue;
        }
        if html.style.contain_paint == Some(true) {
            continue;
        }
        if has_bg(html) {
            html.style.canvas_bg = true;
            continue;
        }
        for c in html.children.iter_mut() {
            let Node::Element(body) = c else { continue };
            if body.tag == "body" && has_bg(body) && body.style.contain_paint != Some(true) {
                body.style.canvas_bg = true;
            }
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
    if html.style.contain_paint == Some(true) {
        return nodes;
    }
    let Some(body) = html.children.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) else {
        return nodes;
    };
    let taken = (
        body.style.vertical.or(html.style.vertical),
        body.style.vertical_rl.or(html.style.vertical_rl),
        body.style.rtl.or(html.style.rtl),
    );
    let own = (html.style.vertical, html.style.vertical_rl, html.style.rtl);
    if taken == own {
        return nodes;
    }
    for child in html.children.iter_mut() {
        let Node::Element(e) = child else { continue };
        if e.tag == "body" {
            continue;
        }
        e.style.vertical = e.style.vertical.or(own.0);
        e.style.vertical_rl = e.style.vertical_rl.or(own.1);
        e.style.rtl = e.style.rtl.or(own.2);
    }
    (html.style.vertical, html.style.vertical_rl, html.style.rtl) = taken;
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
    fn walk(nodes: &mut [Node], mode: (Option<bool>, Option<bool>, Option<bool>)) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            // Письмо и направление наследуются; свои значения сильнее.
            let own = (
                e.style.vertical.or(mode.0),
                e.style.vertical_rl.or(mode.1),
                e.style.rtl.or(mode.2),
            );
            let (was_v, was_rl, was_rtl) =
                (e.style.vertical, e.style.vertical_rl, e.style.rtl);
            e.style.vertical = own.0;
            e.style.vertical_rl = own.1;
            e.style.rtl = own.2;
            e.style.resolve_logical();
            // Унаследованное обратно снимается: наследованием занимается
            // сборщик дерева, и оставленное здесь значение завело бы узлу
            // собственную коробку (см. `has_box_style`).
            e.style.vertical = was_v;
            e.style.vertical_rl = was_rl;
            e.style.rtl = was_rtl;
            walk(&mut e.children, own);
        }
    }
    walk(&mut nodes, (None, None, None));
    nodes
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
