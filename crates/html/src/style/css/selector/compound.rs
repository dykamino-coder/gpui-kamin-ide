//! Разбор составного селектора (Selector::parse_compound): тип, классы, id, атрибуты, псевдоклассы и псевдоэлементы.

use super::*;

impl Selector {
    pub(crate) fn parse_compound(raw: &str) -> Option<Selector> {
        let s = raw.trim();
        if s.is_empty() || s == "*" || s == "*|*" {
            return Some(Selector {
                tag: None,
                id: None,
                classes: vec![],
                attrs: vec![],
                pseudo: None,
                also: vec![],
                ancestor: None,
                prev: None,
                universal: !s.is_empty(),
            });
        }
        let mut sel = Selector {
            tag: None,
            id: None,
            classes: vec![],
            attrs: vec![],
            pseudo: None,
            also: vec![],
            ancestor: None,
            prev: None,
            universal: false,
        };
        // Разделитель ищется ВНЕ скобок: в `:not(:first-child)` двоеточие и
        // точка — часть записи псевдокласса, а не начало следующего куска.
        // Пока это не учитывалось, `:not(...)` разбирался на два бессмысленных
        // псевдокласса и правило не совпадало ни с чем.
        let delim = |s: &str| {
            let mut depth = 0i32;
            let mut escaped = false;
            s.char_indices()
                .find(|(_, ch)| {
                    if escaped {
                        escaped = false;
                        return false;
                    }
                    match ch {
                        '\\' => escaped = true,
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '.' | '#' | ':' | '[' if depth == 0 => return true,
                        _ => {}
                    }
                    false
                })
                .map_or(s.len(), |(i, _)| i)
        };
        // Разбираем слева направо: имя тега идёт первым, дальше .класс/#id/:псевдо.
        let mut rest = s;
        let head_end = delim(rest);
        if head_end > 0 {
            let raw_name = rest[..head_end].trim();
            let local = raw_name
                .rsplit_once('|')
                .map_or(raw_name, |(_, local)| local);
            if local != "*" && !selector_tokens::ident(local) {
                return None;
            }
            let name = unescape(raw_name).to_ascii_lowercase();
            // Пространство имён нам чуждо: `*|div` — тот же div, `*|*` —
            // универсал (селекторы-4 §type-nmsp).
            // Пространство имён нам чуждо, но НЕОБЪЯВЛЕННЫЙ префикс делает
            // селектор недействительным (css-namespaces-3 §5: «A type selector
            // … containing a namespace prefix that has not been previously
            // declared is an invalid selector»), а с ним и весь список:
            // `.test1, y|div { red }` после неверного `@namespace y` не красит
            // (`at-media-003`, `at-supports-045`, `at-supports-namespace-001/002`).
            let name = match name.rsplit_once('|') {
                Some((ns, t)) => {
                    if !ns_declared(ns) {
                        return None;
                    }
                    t.to_string()
                }
                None => name,
            };
            sel.universal = name == "*";
            if !name.is_empty() && name != "*" {
                sel.tag = Some(name);
            }
        }
        rest = &rest[head_end..];
        while !rest.is_empty() {
            let kind = rest.as_bytes()[0] as char;
            let body = &rest[1..];
            let end = delim(body);
            let name = &body[..end];
            // Псевдоэлемент стоит ПОСЛЕДНИМ в составной части (Selectors §3):
            // `p:first-line.two` — недействительный селектор, а разбирался как
            // годный, и правило красило чужой абзац (`c25-pseudo-elmnt-000`).
            if sel.pseudo.as_deref().is_some_and(is_pseudo_element) {
                return None;
            }
            if kind == '[' {
                // Атрибутное условие тянется до закрывающей скобки, кавычки
                // внутри — со своим содержимым.
                let mut end = 0usize;
                let mut quote: Option<char> = None;
                for (i, ch) in body.char_indices() {
                    match (quote, ch) {
                        (Some(q), c) if c == q => quote = None,
                        (Some(_), _) => {}
                        (None, '"') | (None, '\'') => quote = Some(ch),
                        (None, ']') => {
                            end = i;
                            break;
                        }
                        _ => {}
                    }
                }
                if end == 0 && !body.starts_with(']') {
                    return None;
                }
                sel.attrs.push(selector_tokens::attr(&body[..end])?);
                rest = &body[end + 1..];
                continue;
            }
            if matches!(kind, '.' | '#') && !selector_tokens::ident(name) {
                return None;
            }
            match kind {
                '.' => sel.classes.push(unescape(name)),
                '#' => sel.id = Some(unescape(name)),
                // `:hover` и `::before` дают одно и то же имя: различать их
                // незачем — псевдоэлементы отбираются по имени.
                ':' => {
                    // Неизвестный псевдокласс или псевдоэлемент делает
                    // селектор недействительным, а с ним и ВЕСЬ список
                    // (Selectors §3.1): `p:invalidPseudoClass, p.test1`
                    // не красит ни того, ни другого. Прежде неизвестное имя
                    // просто не совпадало, и вторая часть списка работала.
                    // Имя псевдокласса сравнивается без учёта регистра
                    // (`:FiRSt-cHIlD` — тот же `:first-child`).
                    let bare = name.trim_start_matches(':').to_ascii_lowercase();
                    if !bare.is_empty() && !known_pseudo(&bare) {
                        return None;
                    }
                    if bare.split('(').next() == Some("lang") {
                        let args = bare.strip_prefix("lang(")?.strip_suffix(')')?;
                        if split_top_level(args, ',')
                            .iter()
                            .any(|arg| selector_tokens::value(arg.trim()).is_none())
                        {
                            return None;
                        }
                    }
                    // Пустышка от второго двоеточия `::after` — не
                    // псевдокласс, копить её нельзя.
                    if let Some(prev) = sel.pseudo.take()
                        && !prev.is_empty()
                    {
                        sel.also.push(prev);
                    }
                    sel.pseudo = Some(unescape(name.trim_start_matches(':')).to_ascii_lowercase())
                }
                _ => return None,
            }
            rest = &body[end..];
        }
        Some(sel)
    }
}
