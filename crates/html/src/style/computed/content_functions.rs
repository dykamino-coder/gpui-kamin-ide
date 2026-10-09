//! Generated-content functions retain the CSS token types of their arguments.
//! CSS Lists 3 #counter-functions: quoted strings cannot name a counter or its style.
use super::{ContentItem, unescape_content};

/// Разбор значения `content` в список составляющих (css-content-3 §2).
///
/// `None` — запись негодна целиком: неизвестная функция, лишний или
/// недостающий аргумент, незакрытая кавычка. Такое объявление применять
/// нельзя, иначе его остатки печатаются литеральным текстом.
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09): `content: url()`/`image-set()` как
// замещаемый строчный атом (`ContentItem::Image`, картинка через
// `background::source`; патч `target/scout-content-2026-09.md` §6). Срез 1509
// пар (lists/pseudo/content/counter-styles/images): 1192 -> 1190, +0/-2 —
// `cross-fade-natural-size` 0.00 -> 38.95, `disclosure-styles` 0.19 -> 0.62;
// ни одна из ожидаемых `element-replacement*` не позеленела. Картинка в
// `content` требует природного размера ДО раскладки строки (`cross-fade` —
// от двух источников), а атом меряется после.
pub(crate) fn parse_content(raw: &str) -> Option<Vec<ContentItem>> {
    let bytes = raw.as_bytes();
    let mut at = 0usize;
    let mut out = vec![];
    while at < bytes.len() {
        let ch = raw[at..].chars().next()?;
        if ch.is_whitespace() {
            at += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' {
            let body = at + ch.len_utf8();
            let len = crate::style::css::skip_string(&raw[body..], ch);
            // Незакрытая строка обрывается переводом строки — значение негодно.
            if !raw[body..body + len].ends_with(ch) {
                return None;
            }
            out.push(ContentItem::Str(unescape_content(
                &raw[body..body + len - ch.len_utf8()],
            )));
            at = body + len;
            continue;
        }
        // Слова кавычек (css-content-3 §4.2). Прежде они не разбирались, и
        // весь `content` с ними выбрасывался (`content-159`, `quotes-*`).
        let rest = &raw[at..];
        let word_end = rest
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '(')
            .unwrap_or(rest.len());
        let quote = match rest[..word_end].to_ascii_lowercase().as_str() {
            "open-quote" => Some((true, true)),
            "close-quote" => Some((false, true)),
            "no-open-quote" => Some((true, false)),
            "no-close-quote" => Some((false, false)),
            _ => None,
        };
        if let Some((open, emit)) = quote {
            out.push(ContentItem::Quote { open, emit });
            at += word_end;
            continue;
        }
        // Дальше только функция: `counter(`, `counters(`, `attr(`.
        let open = rest.find('(')?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let close = at + open + 1 + find_close(&rest[open + 1..])?;
        let args = crate::style::css::split_args(&raw[at + open + 1..close]);
        let arg = |i: usize| -> Option<String> {
            let a = args.get(i)?.trim();
            let unq = a
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix('"'))
                .or_else(|| a.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')));
            Some(unq.map_or_else(|| a.to_string(), unescape_content))
        };
        // CSS Lists 3 §4.7: counter names and styles cannot be strings;
        // only counters()'s separator is a string argument.
        if matches!(name.as_str(), "counter" | "counters") {
            let is_string = |a: &str| a.trim().starts_with(['"', '\'']);
            let style_at = if name == "counter" { 1 } else { 2 };
            // CSS Lists 3 §4.1 and CSS Values 4 §4.2 exclude none, default
            // and CSS-wide keywords from <counter-name>. Compare decoded
            // identifiers case-insensitively, then invalidate the declaration
            // so an earlier valid content value survives the cascade.
            let counter_name = crate::style::css::unescape(args.first()?.trim()).to_ascii_lowercase();
            if matches!(
                counter_name.as_str(),
                "none" | "default" | "initial" | "inherit" | "unset" | "revert" | "revert-layer"
            ) || args.first().is_some_and(|a| is_string(a))
                || args.get(style_at).is_some_and(|a| is_string(a))
                || (name == "counters" && args.get(1).is_some_and(|a| !is_string(a)))
            {
                return None;
            }
        }
        match name.as_str() {
            "counter" if args.len() == 1 || args.len() == 2 => out.push(ContentItem::Counter(
                arg(0)?,
                arg(1).unwrap_or_else(|| "decimal".to_string()),
            )),
            "counters" if args.len() == 2 || args.len() == 3 => out.push(ContentItem::Counters(
                arg(0)?,
                arg(1)?,
                arg(2).unwrap_or_else(|| "decimal".to_string()),
            )),
            "attr" => {
                let body = &raw[at + open + 1..close];
                let comma = super::top_level_comma(body);
                let head = comma.map_or(body, |i| &body[..i]);
                let (name, explicit_type) = attr_name(head)?;
                let fallback = comma
                    .map(|i| body[i + 1..].trim().to_string())
                    .or_else(|| (!explicit_type).then(|| "\"\"".to_string()));
                out.push(ContentItem::Attr(name, fallback));
            }
            "url" if args.len() <= 1 => out.push(ContentItem::Image(arg(0).unwrap_or_default())),
            // ПРОБОВАЛИ И ОТКАТИЛИ: принимать `url()` (§12.2 объявляет его
            // действительным) и класть в псевдоэлемент синтетический `<img>`.
            // Проба по 195 парам `generated-content`: флипов ноль, потеряна
            // `before-after-table-whitespace-001` (0.15 -> 0.58) и просела
            // `before-after-images-001` (0.00 -> 0.41). Обе требуют, чтобы
            // НЕНАЙДЕННАЯ картинка давала коробку НУЛЕВОГО размера — сперва
            // это, потом уже `url()`.
            _ => return None,
        }
        at = close + 1;
    }
    (!out.is_empty()).then_some(out)
}

/// CSS Values 5 §8.7: omitting the type and explicit raw-string both insert
/// the attribute verbatim. The type keyword is not part of its name.
fn attr_name(head: &str) -> Option<(String, bool)> {
    let head = head.trim();
    let ident = crate::style::css::selector_tokens::ident;
    let attr_ident = |raw: &str| {
        ident(raw)
            || raw
                .split_once('|')
                .is_some_and(|(prefix, local)| (prefix.is_empty() || ident(prefix)) && ident(local))
    };
    if attr_ident(head) {
        return Some((crate::style::css::unescape(head), false));
    }
    // A hex escape may consume whitespace within either identifier. Only
    // split where both sides are complete identifier tokens.
    head.char_indices().find_map(|(at, ch)| {
        if !matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{c}') {
            return None;
        }
        let name = head[..at].trim_end();
        let ty = head[at..].trim_start();
        (attr_ident(name)
            && ident(ty)
            && crate::style::css::unescape(ty).eq_ignore_ascii_case("raw-string"))
        .then(|| (crate::style::css::unescape(name), true))
    })
}

/// Индекс парной закрывающей скобки от места ПОСЛЕ открывающей.
fn find_close(after_open: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < after_open.len() {
        let ch = after_open[at..].chars().next()?;
        match ch {
            '"' | '\'' => {
                at += ch.len_utf8();
                at += crate::style::css::skip_string(&after_open[at..], ch);
                continue;
            }
            '(' => depth += 1,
            ')' if depth == 0 => return Some(at),
            ')' => depth -= 1,
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}
