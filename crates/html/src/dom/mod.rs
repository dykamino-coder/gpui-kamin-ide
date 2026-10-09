//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

pub mod encoding;
mod subgrid_axes;
mod grid_static_position;
mod replaced_display;
mod display_inheritance;
use crate::dom::display_inheritance::resolve_display_inherit;
mod initial_pseudos;
mod containment;
pub(super) mod language;
mod counter_decls;
mod presentational_hints;
mod float_tail;
pub(crate) use counter_decls::{
    apply_counter_decls, apply_value_hint, counter_snapshot, inherit_counter_decls,
};
pub(super) mod content;
pub(crate) use content::{content_text, host_content, resolve_content_attributes};

use crate::style::computed::Computed;
use crate::style::css::{Decls, Media, Rule, parse_keyframes, parse_stylesheet_media};
use crate::style::select::has::{HAS_MARKS, HasArg, collect_has_args, mark_has, parse_has_arg};
use crate::style::select::{QUIRKS, quirks};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;
use std::rc::Rc;
use html5ever::tendril::TendrilSink;
pub(super) mod xhtml;
use crate::dom::xhtml::*;
pub(super) mod fixup_tree;
use crate::dom::fixup_tree::*;
pub(super) mod fixup_grid;
pub(crate) use crate::dom::fixup_grid::*;
pub(crate) mod shadow;
pub(crate) use crate::dom::shadow::*;
pub(super) mod element_style;
use crate::dom::element_style::*;
pub(super) mod walk;
use crate::dom::walk::*;
pub(super) mod scroll_markers;
use crate::dom::scroll_markers::*;
pub(super) mod pseudo;
pub(crate) use crate::dom::pseudo::*;

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    /// Номер пункта списка из счётчика `list-item`; `None` — не пункт.
    pub list_item: Option<i32>,
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
pub(super) const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "kbd", "mark",
    // Все руби-теги — строчные (css-ruby-1 §2.1.1: контейнер и внутренние
    // коробки руби неатомарны и строчного уровня).
    "q", "rb", "rbc", "rp", "rt", "rtc", "ruby", "s", "samp", "small", "span", "strong", "sub",
    "sup", "time", "u", "var", "wbr", "img", "svg",
    // Правки текста: без них `~~зачёркнутое~~` из markdown разрывало абзац.
    "del", "ins",
    // Управление формой стоит В СТРОКЕ: иначе «Согласен» уезжает под флажок,
    // а ряд кнопок выстраивается в столбик.
    "button", "label", "input", "select", "textarea", "output", "meter", "progress",
];

/// Теги, которым таблица агента даёт блочный вид (HTML §15.3.2-15.3.12).
///
/// Правило для НЕИЗВЕСТНОГО тега — строчный: своей записи в листе агента у
/// него нет, а начальное значение `display` — `inline`. Прежде блочным
/// становилось всё, чего нет в `INLINE_TAGS`, и `<foo>` внутри абзаца рвал
/// строку (`line-breaking-font-size-zero-001`).
const BLOCK_TAGS: &[&str] = &[
    "html", "body", "address", "blockquote", "center", "div", "figure", "figcaption", "footer",
    "form", "header", "hr", "legend", "listing", "main", "p", "plaintext", "pre", "xmp", "article",
    "aside", "h1", "h2", "h3", "h4", "h5", "h6", "hgroup", "nav", "section", "search", "dir", "dd",
    "dl", "dt", "ol", "ul", "menu", "li", "table", "caption", "colgroup", "col", "thead", "tbody",
    "tfoot", "tr", "td", "th", "fieldset", "details", "summary", "dialog", "optgroup", "option",
    "frameset", "frame", "noframes", "head", "title", "meta", "link", "base", "script", "style",
    "noscript", "template", "slot", "map", "area", "source", "track", "param",
];

/// Теги, содержимое которых не рисуется НИКОГДА (код и стили).
///
/// `head`/`title`/`meta`/`link` сюда не входят: их прячет таблица агента
/// `display: none`, и авторское `head { display: block }` её перебивает
/// (CSS2/generated-content content-067 и родня).
pub(super) const DROP_TAGS: &[&str] = &["script", "style", "noscript"];

/// Имя тега без пространственного префикса.
///
/// В XHTML рисунок и формулы часто пишут с префиксом (`<svg:svg
/// xmlns:svg="…">`), а разборщик HTML держит двоеточие частью имени. Движок
/// сверяет теги по коротким именам, поэтому `svg:svg` не опознавался как
/// рисунок вовсе — вся семья замещаемых тестов CSS2 рисовала пустоту.
pub(super) fn local_name(name: &str) -> String {
    match name.split_once(':') {
        Some((_, local)) if !local.is_empty() => local.to_string(),
        _ => name.to_string(),
    }
}

/// Стиль по умолчанию для тега — то, что браузер берёт из своей таблицы.
/// Без него `<b>` не жирный, а `<h1>` неотличим от абзаца.
/// ПРОВЕРЕНО ДВАЖДЫ: этот лист чатовый (`pre { padding: 8px }`,
/// `td { padding: 4px 8px }`), но на своде он не сказывается НИ НА ЧЁМ —
/// стенд подаёт `BROWSER_CSS` вторым листом того же происхождения, и по
/// порядку каскада выигрывает он. Проба с браузерными значениями здесь:
/// 71 пара семей `white-space-pre-*`, `text-indent-intrinsic-*`,
/// `border-style-applies-to-*`, `table-visual-layout-*` — ни одного сдвига.
/// Красное в этих семьях держит что-то другое.
fn user_agent_css() -> &'static str {
    r#"
head, title, meta, link, template { display: none }
    slot { display: contents }
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
    i, em { font-style: italic }
    u { text-decoration: underline }
    s, del { text-decoration: line-through }
    small { font-size: 11px }
    a[href] { color: #8ab4f8; text-decoration: underline }
    code, kbd, samp { font-family: monospace; font-size: 12px }
    pre { font-family: monospace; margin: 6px 0; padding: 8px; overflow-x: auto }
    /* Заранее размеченный текст зазоров `text-autospace` не получает: правка
       ширины ломает выравнивание в столбик, ради которого его и пишут
       (css-text-4 §7, таблица стилей агента). */
    pre, code, kbd, samp, tt, textarea, input { text-autospace: no-autospace }
    ul, ol { margin: 6px 0; padding-inline-start: 18px }
    li { margin: 2px 0 }
    blockquote { margin: 6px 0; padding-left: 10px; border-left: 3px solid #4a4a5a }
    hr { height: 1px; margin: 8px 0; background: #4a4a5a }
    table { margin: 6px 0 }
    th { font-weight: 700; padding: 4px 8px; text-align: left }
    td { padding: 4px 8px }
    button { padding: 4px 10px; border-radius: 4px }
    /* Руби (css-ruby-1, Appendix A.1): скобки `rp` — только для движков без
       руби; аннотация вполовину кегля, одной строкой, без знака акцента.
       Пара правил равносильна спековому `rtc, :not(rtc) > rt { font-size: 50% }`.
       `unicode-bidi: isolate` пока не ставится — мерить отдельно (`ruby-bidi-001`). */
    /* css-content-3 §4.2, HTML §15.3.3: `q` берёт кавычки из `quotes`. */
    q::before { content: open-quote }
    q::after { content: close-quote }
    rp { display: none }
    rb, rt, rtc { white-space: nowrap }
    rt, rtc { font-size: 50%; line-height: 1; text-emphasis: none; text-justify: ruby }
    rtc > rt { font-size: 100% }
    /* Языковые правила A.1: чжуинь (zh-TW) — 30% кегля, у китайского
       аннотация по центру. Без них строка под аннотацию росла на кегль
       50% (`ruby-lang-specific-style-001`). */
    rt:lang(zh-TW), rtc:lang(zh-TW) { font-size: 30% }
    rtc:lang(zh-TW) > rt { font-size: 100% }
    rt:lang(zh), rtc:lang(zh) { ruby-align: center }
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
    let _content_document = content::document(html);
    // Правила `@page` — от последнего РАЗОБРАННОГО документа: почистить,
    // чтобы прошлый лист не красил страницу нового. `@position-try` — тот же
    // пул и та же чистка.
    let _ = crate::style::css::take_page_decls();
    let _ = crate::style::css::take_try_rules();
    let _ = crate::style::css::take_property_rules();
    crate::style::generated::counter_style_rules::reset();
    crate::style::css::reset_layers();
    crate::style::values::value::set_dark_scheme(false);
    // Корневые метрики (`rem`, `rlh`) — тоже от прошлого документа: у рамки
    // и у страницы свой корень, и чужие четыре точки на кегль испортили бы
    // весь разбор. Пишет их `walk` ниже, на элементе `html`.
    crate::style::values::value::reset_root_metrics();
    let html = expand_xhtml_self_closing(html);
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });
    // Режим quirks (HTML §13.2.6.4.1, «initial» insertion mode: документ без
    // DOCTYPE). Документ XHTML в quirks не бывает никогда (HTML §2.1, «XML
    // documents … always in no-quirks mode»), а мы разбираем и его HTML-
    // разборщиком, поэтому пространство имён корня его исключает.
    QUIRKS.with(|q| {
        q.set(
            dom.quirks_mode.get() == html5ever::tree_builder::QuirksMode::Quirks
                && !html.contains("http://www.w3.org/1999/xhtml"),
        )
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
    // Лист агента и тема — общие для ВСЕХ областей дерева: каждая тень
    // получает их копию, а правила документа в тень не попадают
    // (css-shadow-1 §3.2: селекторы сопоставляются в своей области).
    let agent_rules = rules.clone();
    // Каждый `<style>` — ОТДЕЛЬНАЯ таблица: конец каждой закрывает свои
    // незакрытые конструкции (CSS 2.1 §4.2, `uri-017`). В склейке незакрытая
    // запись первой таблицы съедала правила второй.
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(&dom.document, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_stylesheet_media(css, media).into_iter().enumerate() {
            rules.push(Rule {
                order: base + i,
                // Таблица ДОКУМЕНТА: её происхождение старше нашего умолчания и
                // темы приложения, поэтому она перебивает их независимо от
                // специфичности (CSS Cascade §6.4.4).
                origin: 1,
                ..r
            });
        }
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
    let agent = Scope {
        rules: agent_rules,
        frames: frames.clone(),
    };
    for css in &sheets {
        frames.extend(crate::style::css::parse_keyframes_in(css, Some(media)));
    }
    // `:has()`: аргументы собираются со всех селекторов, правила с
    // вложенным `:has` выкидываются (спека: cannot be nested), отметки
    // считаются отдельным проходом до обхода.
    let mut has_args_raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut has_args_raw));
    let has_args: Vec<HasArg> = has_args_raw
        .iter()
        .filter_map(|a| parse_has_arg(a))
        .collect();
    HAS_MARKS.with(|m| m.borrow_mut().clear());
    if !has_args.is_empty() {
        mark_has(&dom.document, &has_args, &mut vec![]);
    }
    // Тени (`<template shadowrootmode>`): области стилей и распределение
    // слотов считаются ДО обхода, как отметки `:has`; обход читает их по
    // адресу узла. Документ без теней предпроход не платит.
    SHADOWS.with(|m| m.borrow_mut().clear());
    SLOTS.with(|m| m.borrow_mut().clear());
    let doc = Rc::new(Scope { rules, frames });
    if html.contains("shadowrootmode") {
        let mut drafts: HashMap<usize, SlotInfo> = HashMap::new();
        let top: Vec<Handle> = dom.document.children.borrow().clone();
        scan_shadows(&top, &mut vec![], &doc, &agent, media, &mut drafts);
        finish_slots(drafts);
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::style::generated::counters::Counters::default();
    walk_children(
        &dom.document,
        &doc.rules,
        &vars,
        &doc.frames,
        &mut counter,
        &mut counters,
        &[],
        false,
        &mut out,
    );
    // ПЕРВЫМ проходом: табличная починка и подъёмы ниже читают `display`.
    resolve_display_inherit(&mut out, (None, None, None, None, None));
    resolve_rule_color_inherit(&mut out, &RuleColors::default());
    // Anonymous inline-table around orphan table boxes inside inline boxes
    // (CSS 2.1 §17.2.1 step 3); block parents are fixed up by `blocks()`.
    crate::layout::table::anon::inline_anon_tables(&mut out);
    // Лунки, которые умеет taffy, — на путь сетки ДО подъёмов и среза
    // подсетки: дальше они идут тем же кодом, что и сетка.
    lanes_as_grid(&mut out);
    hoist_grid_abspos(&mut out);
    grid_static_position::adjust(&mut out);
    flex_items_lose_float(&mut out);
    align_self_from_dom_parent(&mut out, None);
    grid_table_items_keep_stretch(&mut out);
    subgrid_takes_parent_tracks(&mut out);
    filter_ref_only_empty(&mut out);
    fold_run_ins(&mut out, None);
    float_tail::mark(&mut out);
    if quirks() {
        quirks_percent_heights(&mut out, None);
    }
    out
}

// Кастомные свойства из правил. Селектор не важен: в документе переменные
// почти всегда объявлены на корне, а разбирать их область видимости — это

/// Содержимое `<style>`-тегов документа, каждый — отдельной таблицей;
/// html5ever кладёт его текстом внутрь.
/// ПРОБОВАЛИ И ОТКАТИЛИ: раскрывать ссылки на знаки (`&gt;`, `&amp;`) в
/// содержимом `<style>`, когда документ XHTML. По разбору XML это верно —
/// построитель HTML держит `<style>` сырым текстом, и селектор `body &gt; div`
/// не совпадает ни с чем; в наборе таких файлов 103. Замерено по всему CSS2:
/// 5233 -> 5230, приобретено 0, потеряно 3. Раскрытые правила доезжают до
/// раскладки и вскрывают ошибки НИЖЕ (`text-indent-intrinsic-001` 0.00 -> 1.10,
/// `-003` 0.00 -> 0.68), а `content-177` 0.42 -> «красное видно»: раскрытая
/// `&quot;` закрывает строку CSS раньше времени. Возвращаться сюда после
/// разбора этих трёх корней.
fn collect_style_tags(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, .. } = &handle.data
        && &*name.local == "style"
    {
        let mut sheet = String::new();
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
                // Preserve child text content: an invented LF makes EOF strings invalid.
                sheet.push_str(body);
            }
        }
        out.push(sheet);
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Цвета детей по порядку — короткая запись для проверок каскада.
    fn child_colors(html: &str) -> Vec<Option<crate::style::values::value::Color>> {
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

    /// Тексты псевдоэлементов документа в порядке обхода.
    fn pseudo_texts(html: &str) -> Vec<String> {
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag.starts_with("::") {
                        let text = e.children.iter().find_map(|c| match c {
                            Node::Text(t) => Some(t.clone()),
                            _ => None,
                        });
                        out.push(text.unwrap_or_default());
                    }
                    walk(&e.children, out);
                }
            }
        }
        let mut out = vec![];
        walk(&parse(html, ""), &mut out);
        out
    }

    #[test]
    fn list_item_counter_is_implicit() {
        // Пункт списка двигает `list-item` сам; `start` задаёт начало,
        // `value` — номер конкретного пункта (css-lists-3 §ua-stylesheet).
        let texts = pseudo_texts(
            "<style>li::after { content: counter(list-item) }</style>             <ol start=\"5\"><li></li><li value=\"9\"></li><li></li></ol>",
        );
        assert_eq!(texts, vec!["5", "9", "10"]);
        // Явное упоминание `list-item` отменяет неявное увеличение.
        let texts = pseudo_texts(
            "<style>li { counter-increment: list-item 3 } li::after { content: counter(list-item) }</style>             <ol><li></li><li></li></ol>",
        );
        assert_eq!(texts, vec!["3", "6"]);
    }

    #[test]
    fn has_relational_pseudo() {
        let red = crate::style::values::value::Color::parse("red");
        let green = crate::style::values::value::Color::parse("green");
        // Предметная позиция: якорь с потомком-предметом.
        let colors = child_colors(
            "<style>div { color: red } div:has(span) { color: green }</style>             <div id=\"box\"><div><span></span></div><div><b></b></div></div>",
        );
        assert_eq!(colors, vec![green, red]);
        // Ведущий `>`: только прямой ребёнок; `+`: следующий брат.
        let colors = child_colors(
            "<style>p { color: red } p:has(> em) { color: green }             p:has(+ p) { background: yellow }</style>             <div id=\"box\"><p><i><em>x</em></i></p><p><em>y</em></p></div>",
        );
        assert_eq!(colors, vec![red, green]);
        // Непредметная позиция: `div:has(.x) b` красит b только в div с .x.
        let colors = child_colors(
            "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i class=\"x\"></i><b></b></div></div>",
        );
        assert_eq!(colors, vec![None, green]);
        let colors = child_colors(
            "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i></i><b></b></div></div>",
        );
        assert_eq!(colors, vec![None, red]);
    }

    #[test]
    fn nth_child_of_selector_list() {
        let red = crate::style::values::value::Color::parse("red");
        let green = crate::style::values::value::Color::parse("green");
        // Индекс считается среди совпавших с S братьев, а не среди всех:
        // второй `.a` — это :nth-child(2 of .a), хотя среди детей он третий.
        let colors = child_colors(
            "<style>p { color: red } p:nth-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
        );
        assert_eq!(colors, vec![red, red, green]);
        // nth-last-child(of S): совпавшие считаются с конца.
        let colors = child_colors(
            "<style>p { color: red } p:nth-last-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
        );
        assert_eq!(colors, vec![green, red, red]);
    }

    #[test]
    fn custom_properties_cascade_and_inherit() {
        let red = crate::style::values::value::Color::parse("red");
        let blue = crate::style::values::value::Color::parse("blue");
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
        let red = crate::style::values::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2) { color: red }</style>\
             <div id=box><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red, None], "получено {colors:?}");
    }

    #[test]
    fn nth_child_understands_an_plus_b() {
        let red = crate::style::values::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2n+1) { color: red }</style>\
             <div id=box><i></i><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![red, None, red, None], "получено {colors:?}");
    }

    #[test]
    fn last_child_counts_from_the_end() {
        let red = crate::style::values::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:last-child { color: red }</style>\
             <div id=box><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red], "получено {colors:?}");
    }

    #[test]
    fn of_type_counts_only_the_same_tag() {
        let red = crate::style::values::value::Color::parse("red");
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
    fn declarative_shadow_scopes_styles_and_slots() {
        let red = crate::style::values::value::Color::parse("red");
        let green = crate::style::values::value::Color::parse("green");
        // Плоские дети хоста: `b` тени (её правило, не документное) и слот
        // (правило тени). Документное `b { red }` в тень не протекает.
        let colors = child_colors(
            "<style>b { color: red } slot { color: green }</style>             <div id=\"box\"><template shadowrootmode=\"open\"><style>b { color: green } slot { color: red }</style>             <b></b><slot></slot></template><i></i></div>",
        );
        assert_eq!(colors, vec![green, red]);
        // Распределённый ребёнок стилизуется таблицей ДОКУМЕНТА, не тени.
        let colors = child_colors(
            "<style>i { color: green }</style>             <div><template shadowrootmode=\"open\"><style>i { color: red }</style>             <slot id=\"box\"></slot></template><i></i></div>",
        );
        assert_eq!(colors, vec![green]);
        // `:host` красит хост, `:has-slotted` — слот с распределёнными.
        let colors = child_colors(
            "<div id=\"box\"><div><template shadowrootmode=\"open\"><style>:host { color: green } slot { color: red } :has-slotted { color: green }</style>             <slot></slot></template><i></i></div></div>",
        );
        assert_eq!(colors, vec![green]);
        // Обычный `<template>` как прежде не рисуется.
        let colors = child_colors("<div id=\"box\"><template><b></b></template><i></i></div>");
        assert_eq!(colors, vec![None]);
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
    use crate::style::values::value::Len;

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
        let wrap = crate::text::paragraph::rules(&div.style).expect("правила");
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

/// Сжать пробелы внутри скобок: `reversed( x )` — одна запись значения,
/// а разбор идёт по словам.
fn squeeze_parens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                depth -= 1;
                out.push(ch);
            }
            c if c.is_whitespace() && depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}
