//! Inline `@import` rules of `<style>` blocks with their layer/supports/media conditions.

use super::css_scan::{Stmt, scan, split_commas};
use super::stylesheet::{read_stylesheet, rebase_css_urls};

/// Развернуть `@import url(...)` ВНУТРИ `<style>` содержимым файла.
///
/// Страницу до движка доводит стенд, и подключений он не делает сам. Набор
/// WOFF2 подключает свои опорные шрифты ТОЛЬКО через `@import`, и без
/// разворачивания обе стороны каждой пары рисуются системной подменой.
///
/// Разворачивается лишь то, что лежит между `<style>` и `</style>`: первый
/// заход правил весь документ и рвал разметку (css-text 992 → 930).
/// Глубина ограничена: кольцо подключений иначе вешает стенд насмерть.
pub(super) fn expand_style_imports(html: &str, dir: &std::path::Path) -> String {
    fn expand(css: &str, base: &std::path::Path, depth: usize) -> String {
        if depth == 0 {
            return css.to_string();
        }
        let mut out = String::with_capacity(css.len());
        let mut last = 0usize;
        let mut prologue = true;
        for st in scan(css).stmts {
            out.push_str(&css[last..st.start]);
            last = st.end;
            let text = &css[st.start..st.end];
            match st.at.as_deref() {
                // Подключение без блока и только в прологе; остальные —
                // недействительны и выбрасываются целиком.
                Some("import") => {
                    if prologue
                        && !st.block
                        && let Some(body) = import(text, base, depth)
                    {
                        out.push_str(&body);
                    }
                    continue;
                }
                Some("charset") => {}
                Some("layer") if !st.block => {}
                _ if is_valid_rule(&st, text) => prologue = false,
                _ => {}
            }
            out.push_str(text);
        }
        out.push_str(&css[last..]);
        out
    }
    /// Содержимое подключаемого файла с его условиями, `None` — подключения нет.
    fn import(text: &str, base: &std::path::Path, depth: usize) -> Option<String> {
        // Незакрытые строка и `url(` к концу таблицы закрываются (§5.4.1):
        // `@import "a.css` в самом конце `<style>` — полноценное подключение.
        let rule = text["@import".len()..].trim_end_matches(';').trim();
        // Адрес — `url(…)` или строка (§6.3); всё после него — условия
        // подключения: `layer(…)`, `supports(…)` и список медиазапросов.
        let low = rule.to_ascii_lowercase();
        let (name, cond) = if low.starts_with("url(") {
            let b = rule.find(')').unwrap_or(rule.len());
            (
                rule[4..b].trim().trim_matches(|c| c == '\'' || c == '"'),
                rule.get(b + 1..).unwrap_or(""),
            )
        } else if let Some(q) = rule.chars().next().filter(|c| *c == '"' || *c == '\'') {
            let b = rule[1..].find(q).map_or(rule.len(), |b| b + 1);
            (&rule[1..b], rule.get(b + 1..).unwrap_or(""))
        } else {
            return None;
        };
        let name = name.trim_start_matches("file:///");
        let file = if std::path::Path::new(name).is_absolute() {
            std::path::PathBuf::from(name)
        } else {
            base.join(kamin_html::css::unescape(name))
        };
        if !file.exists() {
            return None;
        }
        let css = read_stylesheet(&file);
        // Адреса подключённого файла считаются от ЕГО папки — тем же
        // приёмом, что и у `<link rel=stylesheet>`.
        let inner_base = file.parent().unwrap_or(base).to_path_buf();
        let mut css = rebase_css_urls(&css, &inner_base);
        // Свой конец у подключённого файла: незакрытое в нём не
        // поглощает правила, идущие в `<style>` следом (`at-charset-034`).
        let closer = scan(&css).closer;
        css.push_str(&closer);
        let mut body = expand(&css, &inner_base, depth - 1);
        // Условия разворачиваются обёртками изнутри наружу: медиазапрос,
        // затем `supports()`, затем `layer()` — той же семантикой, что у
        // одноимённых групп (`import-conditional-001/002`).
        let mut cond = cond.trim().to_string();
        let mut layer: Option<String> = None;
        let mut supports: Option<String> = None;
        loop {
            let lc = cond.to_ascii_lowercase();
            if lc.starts_with("layer(") {
                let Some(b) = cond.find(')') else { break };
                layer = Some(cond[6..b].trim().to_string());
                cond = cond[b + 1..].trim().to_string();
            } else if lc == "layer" || lc.starts_with("layer ") {
                layer = Some(String::new());
                cond = cond[5..].trim().to_string();
            } else if lc.starts_with("supports(") {
                let mut depth = 0i32;
                let mut close = None;
                for (i, ch) in cond.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some(b) = close else { break };
                let arg = cond[9..b].trim();
                supports = Some(if arg.starts_with('(') {
                    arg.to_string()
                } else {
                    format!("({arg})")
                });
                cond = cond[b + 1..].trim().to_string();
            } else {
                break;
            }
        }
        if !cond.is_empty() {
            body = format!("@media {cond} {{\n{body}\n}}");
        }
        if let Some(sup) = supports {
            body = format!("@supports {sup} {{\n{body}\n}}");
        }
        if let Some(name) = layer {
            body = format!("@layer {name} {{\n{body}\n}}");
        }
        Some(body)
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.to_ascii_lowercase().find("<style") {
        let Some(open) = rest[at..].find('>') else {
            break;
        };
        let body = at + open + 1;
        out.push_str(&rest[..body]);
        let Some(close) = rest[body..].to_ascii_lowercase().find("</style") else {
            break;
        };
        out.push_str(&expand(&rest[body..body + close], dir, 4));
        rest = &rest[body + close..];
    }
    out.push_str(rest);
    out
}

/// At-правила, действительные только с блоком.
const BLOCK_AT_RULES: &[&str] = &[
    "media",
    "supports",
    "font-face",
    "page",
    "keyframes",
    "-webkit-keyframes",
    "layer",
    "container",
    "counter-style",
    "font-feature-values",
    "font-palette-values",
    "property",
    "scope",
    "starting-style",
    "position-try",
    "view-transition",
];

/// Правило, которое закрывает пролог (css-cascade-5 §6.3): только ДЕЙСТВИТЕЛЬНЫЕ
/// правила. Неизвестное at-правило, `@media;` без блока и правило с негодным
/// селектором отбрасываются разбором и подключений после себя не отменяют
/// (`at-rule-008/009`, `at-import-009/010`).
fn is_valid_rule(st: &Stmt, text: &str) -> bool {
    match st.at.as_deref() {
        Some("namespace") => !st.block,
        Some(name) => st.block && BLOCK_AT_RULES.contains(&name),
        None => {
            let Some(open) = text.find('{').filter(|_| st.block) else {
                return false;
            };
            let mut head = String::new();
            let mut rest = &text[..open];
            while let Some(a) = rest.find("/*") {
                head.push_str(&rest[..a]);
                rest = rest[a + 2..]
                    .find("*/")
                    .map_or("", |b| &rest[a + 2 + b + 2..]);
            }
            head.push_str(rest);
            split_commas(&head).iter().all(|one| {
                !one.trim().is_empty() && kamin_html::css::Selector::parse(one.trim()).is_some()
            })
        }
    }
}
