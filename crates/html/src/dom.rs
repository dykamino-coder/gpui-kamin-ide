//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

use crate::computed::{Computed, Display};
use crate::css::{Decls, Rule, Selector, parse_decls, parse_stylesheet};
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    pub tag: String,
    pub style: Computed,
    /// Стиль наведения, собранный из правил с `:hover`. Пустой, если таких
    /// правил не было.
    pub hover: Option<Computed>,
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
    i, em { font-style: italic }
    u { text-decoration: underline }
    s, del { text-decoration: line-through }
    small { font-size: 11px }
    a { color: #8ab4f8; text-decoration: underline }
    code, kbd, samp { font-family: monospace; font-size: 12px }
    pre { font-family: monospace; margin: 6px 0; padding: 8px; overflow-x: auto }
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
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });

    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    let mut rules = parse_stylesheet(user_agent_css());
    let base = rules.len();
    for (i, r) in parse_stylesheet(extra_css).into_iter().enumerate() {
        rules.push(Rule {
            order: base + i,
            ..r
        });
    }
    let mut doc_css = String::new();
    collect_style_tags(&dom.document, &mut doc_css);
    let base = rules.len();
    for (i, r) in parse_stylesheet(&doc_css).into_iter().enumerate() {
        rules.push(Rule {
            order: base + i,
            ..r
        });
    }

    // Переменные темы: `:root { --x: … }` и `--x` в инлайн-стиле корня.
    // Собираются до обхода, потому что нужны каждому узлу.
    let vars = collect_vars(&rules);

    let mut out = vec![];
    walk(&dom.document, &rules, &vars, &[], &mut out);
    out
}

/// Кастомные свойства из правил. Селектор не важен: в документе переменные
/// почти всегда объявлены на корне, а разбирать их область видимости — это
/// уже полноценный каскад, которого мы намеренно избегаем.
fn collect_vars(rules: &[Rule]) -> Decls {
    let mut vars = Decls::new();
    for rule in rules {
        for (k, v) in &rule.decls {
            if k.starts_with("--") {
                vars.insert(k.clone(), v.clone());
            }
        }
    }
    vars
}

/// Содержимое всех `<style>` документа — html5ever кладёт его текстом внутрь.
fn collect_style_tags(handle: &Handle, out: &mut String) {
    if let NodeData::Element { name, .. } = &handle.data
        && name.local.as_ref() == "style"
    {
        for child in handle.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                out.push_str(&contents.borrow());
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
}

fn walk(handle: &Handle, rules: &[Rule], vars: &Decls, path: &[Ancestor], out: &mut Vec<Node>) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if !text.trim().is_empty() || text.contains(' ') {
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
            };

            let inline_decls: Decls = attrs
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| parse_decls(v))
                .unwrap_or_default();
            let mut matched: Vec<&Rule> = rules
                .iter()
                .filter(|r| matches(&r.sel, &me, path))
                .collect();
            let style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
            // Правила с `:hover` собираются отдельным слоем: в базовый стиль
            // им нельзя, иначе элемент выглядел бы всегда наведённым.
            let mut hovered: Vec<&Rule> = rules
                .iter()
                .filter(|r| r.sel.pseudo.as_deref() == Some("hover"))
                .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path))
                .collect();
            let hover =
                (!hovered.is_empty()).then(|| Computed::resolve(&mut hovered, &Decls::new()));

            if style.display == Some(Display::None) {
                return;
            }

            let mut path2 = path.to_vec();
            path2.push(me);
            let mut children = vec![];
            for child in handle.children.borrow().iter() {
                walk(child, rules, vars, &path2, &mut children);
            }

            out.push(Node::Element(Element {
                inline: INLINE_TAGS.contains(&tag.as_str()),
                tag,
                style,
                hover,
                children,
                attrs,
            }));
        }
        _ => {
            for child in handle.children.borrow().iter() {
                walk(child, rules, vars, path, out);
            }
        }
    }
}

/// Сопоставление селектора с узлом и его цепочкой предков.
fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor]) -> bool {
    // Псевдоклассы (`:hover`) применяются отдельным слоем при отрисовке —
    // в базовый стиль они попадать не должны.
    if sel.pseudo.is_some() {
        return false;
    }
    matches_ignoring_pseudo(sel, me, path)
}

/// То же сопоставление, но без отсева по псевдоклассу — для слоя наведения.
fn matches_ignoring_pseudo(sel: &Selector, me: &Ancestor, path: &[Ancestor]) -> bool {
    if !matches_compound(sel, me) {
        return false;
    }
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
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
    sel.classes.iter().all(|c| node.classes.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

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
