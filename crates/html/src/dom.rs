//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

use crate::computed::{Computed, Display, Position};
use crate::css::{
    Decls, Keyframes, Media, Rule, Selector, parse_decls, parse_keyframes, parse_stylesheet_media,
};
use crate::value::Len;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    /// Устойчивый номер узла в документе.
    ///
    /// Нужен анимации: GPUI хранит её состояние по идентификатору элемента, а
    /// он обязан совпадать от кадра к кадру, иначе анимация каждый раз
    /// начинается заново.
    pub node_id: u64,
    /// Кадры анимации, уже разрешённые в стиль: доля времени → стиль.
    pub anim: Option<Vec<(f32, Computed)>>,
    pub tag: String,
    pub style: Computed,
    /// Стиль наведения, собранный из правил с `:hover`. Пустой, если таких
    /// правил не было.
    pub hover: Option<Computed>,
    /// Стиль первой буквы абзаца (`::first-letter`) — отдельным слоем поверх
    /// базового: это буквица, а не стиль всего блока.
    pub first_letter: Option<Computed>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Computed>,
    pub children: Vec<Node>,
    /// Атрибуты, которые нужны при отрисовке: `src`, `href`, `colspan`.
    pub attrs: Vec<(String, String)>,
    /// Инлайн ли элемент по своей природе (`<span>`, `<code>`, `<a>`): от
    /// этого зависит, попадёт ли он в строку текста или станет блоком.
    pub inline: bool,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Теги, которые в HTML участвуют в строке текста, а не разрывают её.
const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "kbd", "mark",
    "q", "rp", "rt", "ruby", "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u",
    "var", "wbr", "img", "svg",
    // Правки текста: без них `~~зачёркнутое~~` из markdown разрывало абзац.
    "del", "ins",
    // Управление формой стоит В СТРОКЕ: иначе «Согласен» уезжает под флажок,
    // а ряд кнопок выстраивается в столбик.
    "button", "label", "input", "select", "textarea", "output", "meter", "progress",
];

/// Теги, содержимое которых не рисуется.
const DROP_TAGS: &[&str] = &[
    "script", "style", "head", "title", "meta", "link", "noscript",
];

/// Стиль по умолчанию для тега — то, что браузер берёт из своей таблицы.
/// Без него `<b>` не жирный, а `<h1>` неотличим от абзаца.
fn user_agent_css() -> &'static str {
    r#"
    h1 { font-size: 24px; font-weight: 700; margin: 12px 0 6px }
    h2 { font-size: 20px; font-weight: 700; margin: 10px 0 5px }
    h3 { font-size: 17px; font-weight: 600; margin: 9px 0 4px }
    h4 { font-size: 15px; font-weight: 600; margin: 8px 0 4px }
    h5, h6 { font-size: 13px; font-weight: 600; margin: 8px 0 4px }
    p { margin: 6px 0 }
    b, strong { font-weight: 700 }
    del, s { text-decoration: line-through }
    ins { text-decoration: underline }
    mark { background: #ffe066; color: #1a1c23 }
    button { background: #3d3f51; border: 1px solid #4a4a5a; color: #e6e6ee }
    dd { margin-left: 32px }
    dt { font-weight: 700; margin: 6px 0 2px }
    figure { margin: 8px 0 }
    figcaption { font-size: 11px; color: #9aa0b4; margin: 4px 0 0 }
    caption { font-weight: 600; margin: 0 0 4px }
    canvas { background: #2a2b36; border: 1px dashed #4a4a5a }
    i, em { font-style: italic }
    u { text-decoration: underline }
    s, del { text-decoration: line-through }
    small { font-size: 11px }
    a { color: #8ab4f8; text-decoration: underline }
    code, kbd, samp { font-family: monospace; font-size: 12px }
    pre { font-family: monospace; margin: 6px 0; padding: 8px; overflow-x: auto }
    /* Заранее размеченный текст зазоров `text-autospace` не получает: правка
       ширины ломает выравнивание в столбик, ради которого его и пишут
       (css-text-4 §7, таблица стилей агента). */
    pre, code, kbd, samp, tt, textarea, input { text-autospace: no-autospace }
    ul, ol { margin: 6px 0; padding-left: 18px }
    li { margin: 2px 0 }
    blockquote { margin: 6px 0; padding-left: 10px; border-left: 3px solid #4a4a5a }
    hr { height: 1px; margin: 8px 0; background: #4a4a5a }
    table { margin: 6px 0 }
    th { font-weight: 700; padding: 4px 8px; text-align: left }
    td { padding: 4px 8px }
    button { padding: 4px 10px; border-radius: 4px }
    "#
}

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn parse(html: &str, extra_css: &str) -> Vec<Node> {
    parse_media(html, extra_css, Media::default())
}

/// То же, но с известными условиями окружения для `@media`.
pub fn parse_media(html: &str, extra_css: &str, media: Media) -> Vec<Node> {
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });

    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    let mut rules = parse_stylesheet_media(user_agent_css(), media);
    let base = rules.len();
    for (i, r) in parse_stylesheet_media(extra_css, media)
        .into_iter()
        .enumerate()
    {
        rules.push(Rule {
            order: base + i,
            ..r
        });
    }
    let mut doc_css = String::new();
    collect_style_tags(&dom.document, &mut doc_css);
    let base = rules.len();
    for (i, r) in parse_stylesheet_media(&doc_css, media)
        .into_iter()
        .enumerate()
    {
        rules.push(Rule {
            order: base + i,
            // Таблица ДОКУМЕНТА: её происхождение старше нашего умолчания и
            // темы приложения, поэтому она перебивает их независимо от
            // специфичности (CSS Cascade §6.4.4).
            origin: 1,
            ..r
        });
    }

    // Переменные темы: `:root { --x: … }` и `--x` в инлайн-стиле корня.
    // Собираются до обхода, потому что нужны каждому узлу.
    // Пользовательские свойства НАСЛЕДУЮТСЯ и каскадируют, поэтому корень
    // стартует пустым: каждый узел добавляет свои и передаёт вниз.
    let vars = Decls::new();

    let mut out = vec![];
    // Наборы кадров собираются из тех же источников, что и правила.
    let mut frames = parse_keyframes(user_agent_css());
    frames.extend(parse_keyframes(extra_css));
    frames.extend(parse_keyframes(&doc_css));
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters: HashMap<String, i32> = HashMap::new();
    walk_children(
        &dom.document,
        &rules,
        &vars,
        &frames,
        &mut counter,
        &mut counters,
        &[],
        false,
        &mut out,
    );
    hoist_grid_abspos(&mut out);
    content_box_static_position(&mut out);
    out
}

/// Абсолютный ребёнок СЕТКИ или ГИБКОГО контейнера без заданных краёв стоит
/// на статической позиции, а она отсчитывается от СОДЕРЖИМОГО контейнера
/// (css-grid-2 §9.1, css-flexbox-1 §4.1), тогда как раскладка под нами кладёт
/// такого ребёнка в коробку ПОЛЕЙ. Разницу забирает поле элемента: при
/// выравнивании к началу оно даёт левый отступ, к концу — правый, по центру —
/// сдвиг на половину разницы, при растяжении — обе стороны сразу.
fn content_box_static_position(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        content_box_static_position(&mut el.children);
        if !matches!(
            el.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            continue;
        }
        let pad = el.style.padding;
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if child.style.position != Some(Position::Absolute) {
                continue;
            }
            // Элемент с заданными линиями стоит не на статической позиции, а в
            // СВОЕЙ области сетки — поля туда добавлять нечего.
            if child.style.grid_col.is_some() || child.style.grid_row.is_some() {
                continue;
            }
            // `left: auto` — это ОТСУТСТВИЕ края, а не заданный край: именно
            // при `auto` с обеих сторон элемент стоит на статической позиции.
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let inset = child.style.inset;
            if auto(inset.left) && auto(inset.right) {
                child.style.margin.left = add_len(child.style.margin.left, pad.left);
                child.style.margin.right = add_len(child.style.margin.right, pad.right);
            }
            if auto(inset.top) && auto(inset.bottom) {
                child.style.margin.top = add_len(child.style.margin.top, pad.top);
                child.style.margin.bottom = add_len(child.style.margin.bottom, pad.bottom);
            }
        }
    }
}

/// Сумма двух длин. Складываются только точки: смешивать доли и кегли здесь
/// не с чем — контейнера в этот момент нет.
fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}

/// Абсолютный ПОТОМОК сетки размещается по её линиям, а не по статической
/// позиции: если содержащий блок такого элемента — сама сетка, то `grid-row`
/// и `grid-column` задают ему прямоугольник области (css-grid-2 §9). Раскладка
/// знает только ПРЯМЫХ детей сетки, поэтому потомок поднимается к ней. Стиль к
/// этому моменту уже вычислен, и переезд по дереву его не меняет.
fn hoist_grid_abspos(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        hoist_grid_abspos(&mut el.children);
        if !is_grid(&el.style) || !own_containing_block(&el.style) {
            continue;
        }
        let mut taken = vec![];
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if own_containing_block(&child.style) || is_grid(&child.style) {
                continue;
            }
            steal_placed(&mut child.children, &mut taken);
        }
        el.children.extend(taken);
    }
}

/// Забрать из поддерева абсолютные элементы с заданными линиями сетки.
fn steal_placed(children: &mut Vec<Node>, out: &mut Vec<Node>) {
    let mut kept = Vec::with_capacity(children.len());
    for mut node in children.drain(..) {
        if let Node::Element(el) = &mut node {
            let placed = el.style.grid_col.is_some() || el.style.grid_row.is_some();
            if el.style.position == Some(Position::Absolute) && placed {
                out.push(node);
                continue;
            }
            // Свой содержащий блок — дальше уже чужие абсолютные элементы.
            // Останавливает и ПОДСЕТКА: она размещает своих абсолютных детей
            // сама, и счёт с конца (`grid-column: 3 / -1`) идёт по её
            // собственному числу дорожек (`subgrid/abs-pos-001`). А вот обычная
            // вложенная сетка без своего отсчёта помехой не служит: её потомок
            // по-прежнему принадлежит внешней сетке (css-grid-2 §9).
            if !own_containing_block(&el.style) && !el.style.subgrid {
                steal_placed(&mut el.children, out);
            }
        }
        kept.push(node);
    }
    *children = kept;
}

/// Контейнер сетки — это и `inline-grid`: разница только в том, как коробка
/// встаёт в поток снаружи.
fn is_grid(c: &Computed) -> bool {
    matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid))
}

/// Задаёт ли элемент отсчёт для абсолютных потомков.
fn own_containing_block(c: &Computed) -> bool {
    matches!(
        c.position,
        Some(Position::Relative)
            | Some(Position::Absolute)
            | Some(Position::Fixed)
            | Some(Position::Sticky)
    )
}

// Кастомные свойства из правил. Селектор не важен: в документе переменные
// почти всегда объявлены на корне, а разбирать их область видимости — это

/// Содержимое всех `<style>` документа — html5ever кладёт его текстом внутрь.
fn collect_style_tags(handle: &Handle, out: &mut String) {
    if let NodeData::Element { name, .. } = &handle.data
        && name.local.as_ref() == "style"
    {
        for child in handle.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                // Обёртка `<![CDATA[ … ]]>` встречается в эталонах XHTML: там
                // она прячет стиль от разбора XML. Разбору CSS она мусор, и
                // правила до первой закрывающей скобки пропадали вместе с ней.
                let text = contents.borrow();
                let trimmed = text.trim();
                let body = trimmed
                    .strip_prefix("<![CDATA[")
                    .and_then(|rest| rest.strip_suffix("]]>"))
                    .unwrap_or(&text);
                out.push_str(body);
                out.push('\n');
            }
        }
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
}

/// Цепочка предков для сопоставления `.card .title`: тег + классы + id.
#[derive(Clone)]
struct Ancestor {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    /// Место среди соседей: нужно структурным псевдоклассам.
    spot: Spot,
    /// Адрес ссылки: нужен `:link`/`:visited`.
    href: Option<String>,
    /// Атрибут `dir` самого узла (true = rtl): нужен `:dir()`.
    dir: Option<bool>,
}

/// Направление письма, заданное АТРИБУТОМ: `<div dir="rtl">`.
///
/// В разметке направление задают именно атрибутом, а не стилем: он и есть
/// обычный способ написать страницу справа налево. Тег `<bdo>` вдобавок
/// ОТМЕНЯЕТ разбор двунаправленности — знаки идут ровно в заданную сторону.
fn apply_direction(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let Some((_, value)) = attrs.iter().find(|(k, _)| k == "dir") else {
        if tag == "bdo" {
            style.bidi_override = Some(true);
        }
        return;
    };
    match value.to_ascii_lowercase().as_str() {
        "rtl" => {
            if style.rtl.is_none() {
                style.rtl = Some(true)
            }
        }
        "ltr" if style.rtl.is_none() => style.rtl = Some(false),
        // `dir="auto"` — сторону выбирает первый сильный знак текста; это
        // делает разбор двунаправленности сам, поэтому здесь ничего не ставим.
        _ => {}
    }
    if tag == "bdo" {
        style.bidi_override = Some(true);
    }
}

/// Размер, заданный АТРИБУТОМ: `<img width="100" height="36">`.
///
/// В HTML это «представленческая подсказка» — стиль самого слабого веса, и
/// без него картинка в разметке без CSS выходит по своему пикселю, а не по
/// заявленному размеру. Атрибут проигрывает любому правилу CSS, поэтому
/// применяется, только если размера ещё нет.
fn apply_presentational_size(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    if !matches!(
        tag,
        "img" | "canvas" | "embed" | "iframe" | "video" | "object"
    ) {
        return;
    }
    let value = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| match v.strip_suffix('%') {
                Some(pct) => pct.trim().parse::<f32>().ok().map(|p| Len::Pct(p / 100.0)),
                None => v.trim().parse::<f32>().ok().map(Len::Px),
            })
    };
    style.attr_width = value("width");
    style.attr_height = value("height");
    // Своего пикселя у холста нет, но размер по умолчанию задан разметкой:
    // 300 на 150 (HTML §4.12.5). Без него `<canvas width="20">` выходил
    // нулевой высоты, а холст без атрибутов — пустым местом.
    if tag == "canvas" {
        style.attr_width = style.attr_width.or(Some(Len::Px(300.0)));
        style.attr_height = style.attr_height.or(Some(Len::Px(150.0)));
    }
    if style.width.is_none() {
        style.width = style.attr_width;
    }
    if style.height.is_none() {
        style.height = style.attr_height;
    }
}

/// Место элемента среди соседей — по нему считаются структурные псевдоклассы.
#[derive(Clone, Copy, Default)]
struct Spot {
    /// Номер среди соседей-элементов, с единицы.
    index: usize,
    /// Сколько всего соседей-элементов.
    total: usize,
    /// То же, но среди соседей с ТЕМ ЖЕ тегом (`:nth-of-type`).
    of_type: usize,
    of_type_total: usize,
}

/// Обойти детей узла, посчитав каждому его место среди соседей.
#[allow(clippy::too_many_arguments)]
fn walk_children(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut HashMap<String, i32>,
    path: &[Ancestor],
    preserve: bool,
    out: &mut Vec<Node>,
) {
    let children = handle.children.borrow();
    // Сначала перепись: сколько всего элементов и сколько с каждым тегом.
    let tags: Vec<Option<String>> = children
        .iter()
        .map(|c| match &c.data {
            NodeData::Element { name, .. } => Some(name.local.to_string()),
            _ => None,
        })
        .collect();
    let total = tags.iter().filter(|t| t.is_some()).count();
    let mut seen = 0usize;
    let mut seen_of_type: HashMap<String, usize> = HashMap::new();
    // Предыдущие соседи-элементы: по ним считаются `.a + .b` и `.a ~ .b`.
    let mut sibs: Vec<Ancestor> = vec![];
    for (child, tag) in children.iter().zip(&tags) {
        let spot = match tag {
            Some(tag) => {
                seen += 1;
                let of_type = seen_of_type.entry(tag.clone()).or_insert(0);
                *of_type += 1;
                Spot {
                    index: seen,
                    total,
                    of_type: *of_type,
                    of_type_total: tags.iter().filter(|t| t.as_deref() == Some(tag)).count(),
                }
            }
            None => Spot::default(),
        };
        walk(
            child, rules, vars, frames, counter, counters, path, spot, preserve, &sibs, out,
        );
        if let NodeData::Element { name, attrs, .. } = &child.data {
            let attrs = attrs.borrow();
            let find = |key: &str| {
                attrs
                    .iter()
                    .find(|a| a.name.local.as_ref() == key)
                    .map(|a| a.value.to_string())
            };
            sibs.push(Ancestor {
                tag: name.local.to_string(),
                id: find("id"),
                classes: find("class")
                    .map(|v| v.split_whitespace().map(str::to_string).collect())
                    .unwrap_or_default(),
                spot,
                href: find("href"),
                dir: find("dir").and_then(|v| match v.to_ascii_lowercase().as_str() {
                    "rtl" => Some(true),
                    "ltr" => Some(false),
                    _ => None,
                }),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut HashMap<String, i32>,
    path: &[Ancestor],
    spot: Spot,
    preserve: bool,
    sibs: &[Ancestor],
    out: &mut Vec<Node>,
) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            // Пустой узел ПО CSS — только схлопываемые пробелы
            // (`space`/`tab`/`CR`/`LF`). `str::trim` снимает весь юникодный
            // пробел, и узел из идеографических U+3000 отбрасывался прямо на
            // разборе: строка из них не доезжала до раскладки вовсе
            // (`trailing-ideographic-space-017`).
            let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
            // Под `white-space: pre*` схлопывания нет вовсе: узел из одного
            // перевода строки — это жёсткий разрыв, и выбрасывать его нельзя
            // (`word-space-transform-011`: `あ<wbr>い<wbr>\n<wbr>う` шло одной
            // строкой, потому что перевод пропадал ещё на разборе).
            if preserve || !text.chars().all(collapsible) || text.contains(' ') {
                out.push(Node::Text(text));
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            if DROP_TAGS.contains(&tag.as_str()) {
                return;
            }
            let attrs: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.to_string(), a.value.to_string()))
                .collect();
            let id = attrs
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.clone());
            let classes: Vec<String> = attrs
                .iter()
                .find(|(k, _)| k == "class")
                .map(|(_, v)| v.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            let me = Ancestor {
                tag: tag.clone(),
                id: id.clone(),
                classes: classes.clone(),
                spot,
                href: attrs
                    .iter()
                    .find(|(k, _)| k == "href")
                    .map(|(_, v)| v.clone()),
                dir: attrs.iter().find(|(k, _)| k == "dir").and_then(|(_, v)| {
                    match v.to_ascii_lowercase().as_str() {
                        "rtl" => Some(true),
                        "ltr" => Some(false),
                        _ => None,
                    }
                }),
            };

            let inline_decls: Decls = attrs
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| parse_decls(v))
                .unwrap_or_default();
            let mut matched: Vec<&Rule> = rules
                .iter()
                .filter(|r| matches(&r.sel, &me, path, sibs))
                .collect();
            // Свои переменные: родительские, поверх них объявления
            // совпавших правил в порядке каскада, поверх — свои же в
            // атрибуте. Пока словарь был один на документ, `:root{--c:red}`
            // и `.dark{--c:blue}` складывались в него подряд, и последнее
            // объявление красило ВЕСЬ документ — переключение темы классом
            // не работало в принципе.
            let own_vars = {
                let mut own = vars.clone();
                let mut by_cascade: Vec<&&Rule> = matched.iter().collect();
                by_cascade.sort_by_key(|r| (r.origin, r.sel.specificity(), r.order));
                for rule in by_cascade {
                    for (k, v) in &rule.decls {
                        if k.starts_with("--") {
                            own.insert(k.clone(), v.clone());
                        }
                    }
                }
                for (k, v) in &inline_decls {
                    if k.starts_with("--") {
                        own.insert(k.clone(), v.clone());
                    }
                }
                own
            };
            let vars = &own_vars;
            let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
            apply_presentational_size(&mut style, &tag, &attrs);
            // Язык — свойство узла, а не CSS: по нему выбираются образцы
            // слогораздела (`hyphens: auto`).
            if let Some((_, v)) = attrs.iter().find(|(k, _)| k == "lang") {
                style.lang = Some(v.clone());
            }
            apply_direction(&mut style, &tag, &attrs);
            // Правила с `:hover` собираются отдельным слоем: в базовый стиль
            // им нельзя, иначе элемент выглядел бы всегда наведённым.
            let mut hovered: Vec<&Rule> = rules
                .iter()
                .filter(|r| r.sel.pseudo.as_deref() == Some("hover"))
                .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                .collect();
            hovered.sort_by_key(|r| (r.sel.specificity(), r.order));
            // Слой наведения собирается ПОВЕРХ базового стиля, а не с нуля:
            // правило `:hover` меняет два-три свойства, а не весь стиль. Со
            // сборкой «с нуля» плавный переход на середине пути показывал
            // голый огрызок — без отступов, размеров и шрифта.
            let hover = (!hovered.is_empty()).then(|| {
                let mut merged = style.clone();
                for rule in hovered.iter() {
                    merged.apply_decls_with_vars(&rule.decls, vars);
                }
                merged
            });
            // Псевдоэлементы первой буквы и первой строки — тем же слоем
            // поверх базового стиля: они меняют начертание куска, а не блок.
            let layer = |name: &str| {
                let mut found: Vec<&Rule> = rules
                    .iter()
                    .filter(|r| r.sel.pseudo.as_deref() == Some(name))
                    .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                    .collect();
                found.sort_by_key(|r| (r.sel.specificity(), r.order));
                (!found.is_empty()).then(|| {
                    let mut merged = style.clone();
                    for rule in found.iter() {
                        merged.apply_decls_with_vars(&rule.decls, vars);
                    }
                    merged
                })
            };
            let first_letter = layer("first-letter");
            let first_line = layer("first-line");

            if style.display == Some(Display::None) {
                return;
            }

            // Счётчики: сброс и увеличение действуют на узле, до его детей.
            for decl in [&style.counter_reset, &style.counter_increment] {
                let Some(text) = decl else { continue };
                let reset = std::ptr::eq(decl, &style.counter_reset);
                let mut it = text.split_whitespace();
                while let Some(name) = it.next() {
                    let value: i32 = it
                        .clone()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(if reset { 0 } else { 1 });
                    if it
                        .clone()
                        .next()
                        .and_then(|n| n.parse::<i32>().ok())
                        .is_some()
                    {
                        it.next();
                    }
                    let slot = counters.entry(name.to_string()).or_insert(0);
                    if reset {
                        *slot = value;
                    } else {
                        *slot += value;
                    }
                }
            }

            let mut path2 = path.to_vec();
            path2.push(me.clone());
            let mut children = vec![];
            walk_children(
                handle,
                rules,
                vars,
                frames,
                counter,
                counters,
                &path2,
                style.preserve_newlines.unwrap_or(preserve),
                &mut children,
            );
            // Псевдоэлементы: коробка появляется, только если у правила есть
            // `content`. Значками, стрелками и разделителями в вёрстке
            // занимаются именно они, и без них разметка теряет часть смысла.
            if let Some(el) = pseudo_box(rules, vars, counters, &me, path, &[], "after", &attrs) {
                children.push(Node::Element(el));
            }
            if let Some(el) = pseudo_box(rules, vars, counters, &me, path, &[], "before", &attrs) {
                children.insert(0, Node::Element(el));
            }

            *counter += 1;
            // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
            // уже нет, а интерполировать нужно готовые стили, а не текст.
            let anim = style.animation.as_ref().and_then(|a| {
                let track = frames.get(&a.name)?;
                let resolved: Vec<(f32, Computed)> = track
                    .iter()
                    .map(|(at, decls)| {
                        let mut c = style.clone();
                        c.apply_decls_with_vars(decls, vars);
                        (*at, c)
                    })
                    .collect();
                (resolved.len() >= 2).then_some(resolved)
            });
            // `dir="auto"` — сторону задаёт ПЕРВЫЙ СИЛЬНЫЙ знак содержимого
            // (HTML §3.2.6.4). Раньше здесь не ставилось ничего в расчёте на
            // разбор двунаправленности, но он берёт сторону абзаца, а не
            // куска: строка `1;234;56א;` внутри `<span dir=auto>` выходила
            // слева направо (`empty-span-001`).
            if attrs
                .iter()
                .any(|(k, v)| k == "dir" && v.eq_ignore_ascii_case("auto"))
            {
                // `dir="auto"` в HTML — это `unicode-bidi: plaintext`: сторона
                // решается для КАЖДОГО абзаца между жёсткими разрывами, а не
                // для элемента целиком (`text-align-end-016`).
                style.bidi_plaintext = Some(true);
                if style.rtl.is_none()
                    && let Some(rtl) = first_strong(&children)
                {
                    style.rtl = Some(rtl);
                }
            }
            out.push(Node::Element(Element {
                node_id: *counter,
                anim,
                inline: INLINE_TAGS.contains(&tag.as_str()),
                tag,
                style,
                hover,
                first_letter,
                first_line,
                children,
                attrs,
            }));
        }
        _ => walk_children(
            handle, rules, vars, frames, counter, counters, path, preserve, out,
        ),
    }
}

/// Коробка псевдоэлемента `::before`/`::after`, если правила её создают.
///
/// В CSS это настоящий потомок с собственным стилем; так его и собираем —
/// обычным инлайновым элементом с текстовым содержимым. `attr(имя)`
/// подставляется значением атрибута хозяина.
fn pseudo_box(
    rules: &[Rule],
    vars: &Decls,
    counters: &HashMap<String, i32>,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: &[Ancestor],
    which: &str,
    attrs: &[(String, String)],
) -> Option<Element> {
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(which))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    let raw = style.content.clone()?;
    // `counter(имя)` — значение счётчика на этом узле.
    if let Some(name) = raw
        .strip_prefix("counter(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let value = counters.get(name.trim()).copied().unwrap_or(0);
        return Some(Element {
            node_id: 0,
            anim: None,
            inline: true,
            tag: format!("::{which}"),
            style,
            hover: None,
            first_letter: None,
            first_line: None,
            children: vec![Node::Text(value.to_string())],
            attrs: vec![],
        });
    }
    let text = match raw.strip_prefix("attr(").and_then(|r| r.strip_suffix(')')) {
        Some(name) => attrs
            .iter()
            .find(|(k, _)| k == name.trim())
            .map(|(_, v)| v.clone())
            .unwrap_or_default(),
        None => raw,
    };
    if style.display == Some(Display::None) {
        return None;
    }
    Some(Element {
        // Псевдоэлемент своей анимации не несёт: правило `::before` задаёт
        // содержимое, а не движение.
        node_id: 0,
        anim: None,
        inline: true,
        tag: format!("::{which}"),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![Node::Text(text)],
        attrs: vec![],
    })
}

/// Сопоставление селектора с узлом и его цепочкой предков.
///
/// `sibs` — предыдущие соседи-элементы узла в порядке разметки: по ним
/// решаются соседние комбинаторы `+` и `~`.
fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor], sibs: &[Ancestor]) -> bool {
    if let Some(pseudo) = &sel.pseudo {
        // `:not(...)` — отрицание вложенного селектора. Разбирается здесь, а
        // не среди структурных: внутри скобок может стоять тег или класс, а им
        // нужен сам узел, а не только его место среди соседей.
        if let Some(inner) = pseudo
            .strip_prefix("not(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let Some(inner) = Selector::parse(inner) else {
                return false;
            };
            if matches(&inner, me, path, sibs) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // Структурный псевдокласс — часть обычного каскада: он зависит только
        // от места узла в дереве. Остальные (`:hover`, `::before`) сюда не
        // попадают: их применяет отдельный слой при отрисовке.
        // `:root` — корень документа, то есть `<html>`. Он не структурный по
        // месту среди соседей, поэтому решается здесь: без него объявления
        // вроде `:root { font: 25px/1 Ahem }` не доезжали НИКУДА, и страница
        // набиралась шрифтом по умолчанию (`text-align-last-015`).
        // Ссылки: `:visited` — адрес уже в истории. Свою страницу браузер в
        // историю кладёт по определению, поэтому пустой `href` и якорь на
        // себя — посещённые; остальное для нас непосещённое.
        if pseudo == "link" || pseudo == "visited" {
            let Some(href) = &me.href else { return false };
            let visited = href.is_empty() || href.starts_with('#');
            if (pseudo == "visited") != visited {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // `:dir(rtl|ltr)` — направление узла: свой атрибут `dir`, иначе
        // ближайшего предка с ним; по умолчанию письмо слева направо
        // (селекторы-4 §direction-pseudo).
        if let Some(want) = pseudo
            .strip_prefix("dir(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let rtl = me
                .dir
                .or_else(|| path.iter().rev().find_map(|a| a.dir))
                .unwrap_or(false);
            if want.trim().eq_ignore_ascii_case("rtl") != rtl {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if pseudo == "root" {
            if me.tag != "html" {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        let Some(ok) = structural(pseudo, me.spot) else {
            return false;
        };
        if !ok {
            return false;
        }
    }
    matches_ignoring_pseudo(sel, me, path, sibs)
}

/// Структурные псевдоклассы: место узла среди соседей.
///
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
fn structural(pseudo: &str, spot: Spot) -> Option<bool> {
    let (name, arg) = match pseudo.split_once('(') {
        Some((n, rest)) => (n, rest.trim_end_matches(')').trim()),
        None => (pseudo, ""),
    };
    let (index, total) = match name {
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-last-child" => {
            (spot.index, spot.total)
        }
        "first-of-type" | "last-of-type" | "only-of-type" | "nth-of-type" | "nth-last-of-type" => {
            (spot.of_type, spot.of_type_total)
        }
        _ => return None,
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return Some(false);
    }
    Some(match name {
        "first-child" | "first-of-type" => index == 1,
        "last-child" | "last-of-type" => index == total,
        "only-child" | "only-of-type" => total == 1,
        "nth-child" | "nth-of-type" => nth_matches(arg, index),
        "nth-last-child" | "nth-last-of-type" => nth_matches(arg, total + 1 - index),
        _ => false,
    })
}

/// Запись `an+b` из `:nth-child()`: подходит ли номер.
fn nth_matches(arg: &str, index: usize) -> bool {
    let arg = arg.trim().to_ascii_lowercase();
    let (a, b) = match arg.as_str() {
        "odd" => (2i64, 1i64),
        "even" => (2, 0),
        _ => match arg.split_once('n') {
            None => match arg.parse::<i64>() {
                Ok(b) => (0, b),
                Err(_) => return false,
            },
            Some((head, tail)) => {
                let a = match head.trim() {
                    "" | "+" => 1,
                    "-" => -1,
                    other => match other.parse::<i64>() {
                        Ok(v) => v,
                        Err(_) => return false,
                    },
                };
                let tail = tail.replace(' ', "");
                let b = if tail.is_empty() {
                    0
                } else {
                    match tail.parse::<i64>() {
                        Ok(v) => v,
                        Err(_) => return false,
                    }
                };
                (a, b)
            }
        },
    };
    let index = index as i64;
    if a == 0 {
        return index == b;
    }
    let diff = index - b;
    diff % a == 0 && diff / a >= 0
}

/// То же сопоставление, но без отсева по псевдоклассу — для слоя наведения.
fn matches_ignoring_pseudo(
    sel: &Selector,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: &[Ancestor],
) -> bool {
    if !matches_compound(sel, me) {
        return false;
    }
    // Соседний комбинатор: `+` — ровно предыдущий сосед-элемент, `~` — любой
    // раньше. Сосед проверяется ПОЛНЫМ сопоставлением со своими соседями
    // слева и тем же путём предков (соседи его делят).
    if let Some(prev) = &sel.prev {
        let (prev_sel, adjacent) = (&prev.0, prev.1);
        let hit = |i: usize| matches(prev_sel, &sibs[i], path, &sibs[..i]);
        let found = if adjacent {
            !sibs.is_empty() && hit(sibs.len() - 1)
        } else {
            (0..sibs.len()).rev().any(hit)
        };
        if !found {
            return false;
        }
    }
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    // Сосед ПРЕДКА: его соседей здесь уже не восстановить — такое правило
    // честно не совпадает, чем совпадать наугад.
    if parent_sel.prev.is_some() {
        return false;
    }
    if direct {
        return path.last().is_some_and(|p| {
            matches_compound(parent_sel, p) && matches_chain(parent_sel, path, path.len() - 1)
        });
    }
    (0..path.len())
        .rev()
        .any(|i| matches_compound(parent_sel, &path[i]) && matches_chain(parent_sel, path, i))
}

/// Продолжение цепочки вверх для `.a .b .c`.
fn matches_chain(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if parent_sel.prev.is_some() {
        return false;
    }
    if direct {
        return at > 0
            && matches_compound(parent_sel, &path[at - 1])
            && matches_chain(parent_sel, path, at - 1);
    }
    (0..at)
        .rev()
        .any(|i| matches_compound(parent_sel, &path[i]) && matches_chain(parent_sel, path, i))
}

fn matches_compound(sel: &Selector, node: &Ancestor) -> bool {
    if let Some(t) = &sel.tag
        && t != &node.tag
    {
        return false;
    }
    if let Some(id) = &sel.id
        && node.id.as_deref() != Some(id.as_str())
    {
        return false;
    }
    // Структурный псевдокласс НЕ-предметного компаунда
    // (`td:nth-child(2) div`): место предка среди братьев известно из
    // `spot` — без проверки любой `td` подходил под любой номер, и
    // последнее правило перекрашивало все колонки
    // (logical-physical-mapping-001). Нестуктурные (`:hover`) здесь
    // по-прежнему пропускаются.
    if let Some(p) = &sel.pseudo
        && let Some(ok) = structural(p, node.spot)
        && !ok
    {
        return false;
    }
    sel.classes.iter().all(|c| node.classes.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Цвета детей по порядку — короткая запись для проверок каскада.
    fn child_colors(html: &str) -> Vec<Option<crate::value::Color>> {
        fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.attr("id") == Some(id) {
                        return Some(e);
                    }
                    if let Some(found) = find(&e.children, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let nodes = parse(html, "");
        find(&nodes, "box")
            .map(|e| {
                e.children
                    .iter()
                    .filter_map(|n| match n {
                        Node::Element(c) => Some(c.style.color),
                        Node::Text(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn custom_properties_cascade_and_inherit() {
        let red = crate::value::Color::parse("red");
        let blue = crate::value::Color::parse("blue");
        // Переключение темы классом: у потомка внутри `.dark` своё значение
        // переменной, у остальных — корневое. Пока переменные собирались в
        // один плоский словарь на документ, последнее объявление красило ВЕСЬ
        // документ, и тема классом не переключалась в принципе.
        let colors = child_colors(
            "<style>:root { --c: red } .dark { --c: blue } i { color: var(--c) }</style>             <div id=box><i></i><i class=dark></i></div>",
        );
        assert_eq!(colors, vec![red, blue], "получено {colors:?}");
    }

    #[test]
    fn nth_child_selects_by_position() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2) { color: red }</style>\
             <div id=box><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red, None], "получено {colors:?}");
    }

    #[test]
    fn nth_child_understands_an_plus_b() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2n+1) { color: red }</style>\
             <div id=box><i></i><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![red, None, red, None], "получено {colors:?}");
    }

    #[test]
    fn last_child_counts_from_the_end() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:last-child { color: red }</style>\
             <div id=box><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red], "получено {colors:?}");
    }

    #[test]
    fn of_type_counts_only_the_same_tag() {
        let red = crate::value::Color::parse("red");
        // Второй `<i>` — четвёртый ребёнок, но второй своего тега.
        let colors = child_colors(
            "<style>i:nth-of-type(2) { color: red }</style>\
             <div id=box><b></b><i></i><b></b><i></i></div>",
        );
        assert_eq!(colors, vec![None, None, None, red], "получено {colors:?}");
    }

    fn first_element(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "body" || e.tag == "html" {
                        if let Some(inner) = find(&e.children) {
                            return Some(inner);
                        }
                        continue;
                    }
                    return Some(e);
                }
            }
            None
        }
        find(nodes).expect("нет элементов")
    }

    #[test]
    fn tag_defaults_apply() {
        let nodes = parse("<h1>Заголовок</h1>", "");
        let h1 = first_element(&nodes);
        assert_eq!(h1.tag, "h1");
        assert_eq!(h1.style.font_weight, Some(700));
    }

    #[test]
    fn inline_style_beats_stylesheet() {
        let nodes = parse(
            r#"<style>.c { color: red }</style><div class="c" style="color: #00ff00">x</div>"#,
            "",
        );
        let div = first_element(&nodes);
        assert_eq!(div.style.color.map(|c| c.g), Some(1.0));
    }

    #[test]
    fn descendant_selector_needs_the_ancestor() {
        let html = r#"<style>.card .t { color: #0000ff }</style>
            <div class="card"><span class="t">внутри</span></div><span class="t">снаружи</span>"#;
        let nodes = parse(html, "");
        let mut found = vec![];
        collect_spans(&nodes, &mut found);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].style.color.map(|c| c.b),
            Some(1.0),
            "внутри карточки — покрашен"
        );
        assert_eq!(
            found[1].style.color, None,
            "снаружи — правило не применяется"
        );
    }

    fn collect_spans<'a>(nodes: &'a [Node], out: &mut Vec<&'a Element>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "span" {
                    out.push(e);
                }
                collect_spans(&e.children, out);
            }
        }
    }

    #[test]
    fn css_variables_are_substituted() {
        // На переменных построены все современные темы: без подстановки такое
        // объявление терялось молча.
        let nodes = parse(":root { --brand: #00ff00 } .b { color: var(--brand) }", "");
        let _ = &nodes;
        let nodes = parse(
            "<style>:root { --brand: #00ff00 } .b { color: var(--brand) }</style>             <div class=\"b\">текст</div>",
            "",
        );
        assert_eq!(first_element(&nodes).style.color.map(|c| c.g), Some(1.0));
    }

    #[test]
    fn variable_fallback_is_used_when_undefined() {
        let nodes = parse(
            "<style>.b { color: var(--missing, #0000ff) }</style><div class=\"b\">t</div>",
            "",
        );
        assert_eq!(first_element(&nodes).style.color.map(|c| c.b), Some(1.0));
    }

    #[test]
    fn hover_rules_form_a_separate_layer() {
        let nodes = parse(
            "<style>.b { color: #ffffff } .b:hover { color: #ff0000 }</style>             <div class=\"b\">кнопка</div>",
            "",
        );
        let d = first_element(&nodes);
        assert_eq!(d.style.color.map(|c| c.r), Some(1.0), "базовый цвет белый");
        assert_eq!(
            d.style.color.map(|c| c.g),
            Some(1.0),
            "и не покрашен наведением"
        );
        let hover = d.hover.as_ref().expect("слой наведения собран");
        assert_eq!(hover.color.map(|c| c.g), Some(0.0), "в наведении — красный");
    }

    #[test]
    fn no_hover_rules_means_no_layer() {
        let nodes = parse("<div class=\"b\">без наведения</div>", "");
        assert!(first_element(&nodes).hover.is_none());
    }

    #[test]
    fn script_and_style_content_is_dropped() {
        let nodes = parse(
            "<script>alert(1)</script><style>.a{}</style><p>текст</p>",
            "",
        );
        let p = first_element(&nodes);
        assert_eq!(p.tag, "p");
        assert!(matches!(p.children.first(), Some(Node::Text(t)) if t == "текст"));
    }

    #[test]
    fn display_none_removes_the_subtree() {
        let nodes = parse(
            r#"<div style="display:none"><p>невидимо</p></div><p>видно</p>"#,
            "",
        );
        let first = first_element(&nodes);
        assert_eq!(first.tag, "p");
        assert!(matches!(first.children.first(), Some(Node::Text(t)) if t == "видно"));
    }

    #[test]
    fn unclosed_tags_are_recovered_by_the_parser() {
        let nodes = parse("<div><p>раз<p>два</div>", "");
        let div = first_element(&nodes);
        let ps = div
            .children
            .iter()
            .filter(|n| matches!(n, Node::Element(e) if e.tag == "p"))
            .count();
        assert_eq!(ps, 2, "html5ever закрывает <p> сам");
    }
}

#[cfg(test)]
mod presentational_tests {
    use super::*;

    /// `<img width=100>` — представленческая подсказка, и без неё картинка
    /// набирается по своему пикселю вместо заявленного размера.
    #[test]
    fn image_size_attributes_reach_the_style() {
        let nodes = parse(r#"<img src="x.png" width="100" height="40">"#, "");
        fn find(nodes: &[Node]) -> Option<&Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "img" {
                        return Some(e);
                    }
                    if let Some(found) = find(&e.children) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let img = find(&nodes).expect("картинка в дереве");
        assert_eq!(img.style.width, Some(Len::Px(100.0)));
        assert_eq!(img.style.height, Some(Len::Px(40.0)));
    }
}

#[cfg(test)]
mod nth_child_tests {
    use super::*;

    fn spans(nodes: &[Node], out: &mut Vec<Computed>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "span" {
                    out.push(e.style.clone());
                }
                spans(&e.children, out);
            }
        }
    }

    /// `:nth-child(1)` адресует ПЕРВОГО ребёнка — на нём держатся эталоны
    /// целого набора тестов гибкой раскладки.
    #[test]
    fn first_child_is_addressable() {
        let html = r#"<style>
            span { background: white }
            span:nth-child(1) { background: yellow }
            span:first-child { color: red }
        </style><div style="display:flex"><span>a</span><span>b</span><span>c</span></div>"#;
        let mut found = vec![];
        spans(&parse(html, ""), &mut found);
        assert_eq!(found.len(), 3, "три куска");
        let first = &found[0];
        assert_ne!(
            first.background, found[1].background,
            "фон первого куска обязан отличаться от второго"
        );
        assert!(first.color.is_some(), ":first-child тоже обязан сработать");
    }
}

#[cfg(test)]
mod white_space_tests {
    use super::*;

    /// `white-space: break-spaces` обязан доехать до правил переноса: от него
    /// зависит, считается ли хвостовой пробел в ширину строки.
    #[test]
    fn break_spaces_reaches_the_wrap_rules() {
        let nodes = parse(r#"<div style="white-space: break-spaces">X XX X</div>"#, "");
        fn find(nodes: &[Node]) -> Option<&Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "div" {
                        return Some(e);
                    }
                    if let Some(f) = find(&e.children) {
                        return Some(f);
                    }
                }
            }
            None
        }
        let div = find(&nodes).expect("блок в дереве");
        assert_eq!(div.style.break_after_spaces, Some(true), "разбор");
        let wrap = crate::lines::rules(&div.style).expect("правила");
        assert!(wrap.break_spaces, "правила переноса");
        assert!(wrap.keep_spaces, "сохранение пробелов");
    }
}

/// Первый СИЛЬНЫЙ знак содержимого: `true` — справа налево, `None` — сильных
/// знаков нет вовсе и сторона остаётся унаследованной.
fn first_strong(nodes: &[Node]) -> Option<bool> {
    use unicode_bidi::BidiClass::*;
    for node in nodes {
        match node {
            Node::Text(t) => {
                for ch in t.chars() {
                    match unicode_bidi::bidi_class(ch) {
                        L => return Some(false),
                        R | AL => return Some(true),
                        _ => {}
                    }
                }
            }
            Node::Element(e) => {
                // Внутри куска со СВОЕЙ стороной искать нечего: он сам себе
                // абзац для этого правила.
                if e.style.rtl.is_some() {
                    continue;
                }
                if let Some(found) = first_strong(&e.children) {
                    return Some(found);
                }
            }
        }
    }
    None
}
