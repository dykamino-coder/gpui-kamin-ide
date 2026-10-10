//! Inline `@import` rules of `<style>` blocks with their layer/supports/media conditions.

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
    // Пролог таблицы (css-cascade-5 §6.3): `@import` действует, только пока
    // перед ним нет ничего, кроме `@charset`, `@layer a, b;` и других
    // `@import`. Подключение после правила игнорируется (`cascade-import-001`,
    // `at-rule-013`).
    fn prologue_only(head: &str) -> bool {
        let mut text = String::new();
        let mut rest = head;
        while let Some(a) = rest.find("/*") {
            text.push_str(&rest[..a]);
            rest = rest[a + 2..]
                .find("*/")
                .map_or("", |b| &rest[a + 2 + b + 2..]);
        }
        text.push_str(rest);
        let text = text
            .replace("<!--", "")
            .replace("-->", "")
            .replace("<![CDATA[", "")
            .replace("]]>", "");
        !text.contains('{')
            && text.split(';').all(|part| {
                let t = part.trim().to_ascii_lowercase();
                t.is_empty() || t.starts_with("@charset") || t.starts_with("@layer")
            })
    }
    fn expand(css: &str, base: &std::path::Path, depth: usize) -> String {
        if depth == 0 {
            return css.to_string();
        }
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        let mut prologue = true;
        while let Some(at) = rest.to_ascii_lowercase().find("@import") {
            let (head, tail) = rest.split_at(at);
            out.push_str(head);
            prologue = prologue && prologue_only(head);
            let Some(end) = tail.find(';') else {
                out.push_str(tail);
                return out;
            };
            let rule = tail["@import".len()..end].trim_start();
            rest = &tail[end + 1..];
            if !prologue {
                continue;
            }
            // Адрес — `url(…)` или строка (§6.3); всё после него — условия
            // подключения: `layer(…)`, `supports(…)` и список медиазапросов.
            let low = rule.to_ascii_lowercase();
            let (name, cond) = if low.starts_with("url(") {
                let Some(b) = rule.find(')') else { continue };
                (
                    rule[4..b].trim().trim_matches(|c| c == '\'' || c == '"'),
                    &rule[b + 1..],
                )
            } else if let Some(q) = rule.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let Some(b) = rule[1..].find(q) else { continue };
                (&rule[1..1 + b], &rule[b + 2..])
            } else {
                continue;
            };
            let name = name.trim_start_matches("file:///");
            let file = if std::path::Path::new(name).is_absolute() {
                std::path::PathBuf::from(name)
            } else {
                base.join(kamin_html::css::unescape(name))
            };
            if !file.exists() {
                continue;
            }
            let css = read_stylesheet(&file);
            // Адреса подключённого файла считаются от ЕГО папки — тем же
            // приёмом, что и у `<link rel=stylesheet>`.
            let inner_base = file.parent().unwrap_or(base).to_path_buf();
            let css = rebase_css_urls(&css, &inner_base);
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
            out.push_str(&body);
        }
        out.push_str(rest);
        out
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
