//! Reftest metadata of a test file: compared print pages, scope, match and mismatch references.

use super::reference_result;

/// Листы из `<meta name="reftest-pages" content="1,3-4">` (протокол печатных
/// reftest WPT, docs/writing-tests/print-reftests.md: сравниваются только
/// перечисленные страницы, номера с единицы, диапазоны `a-b`, `-b`, `a-`).
pub(super) fn reftest_pages(html: &str) -> Option<Vec<usize>> {
    let lower = html.to_ascii_lowercase();
    let at = lower.find("reftest-pages")?;
    let tag_start = lower[..at].rfind('<')?;
    let tag_end = at + lower[at..].find('>')?;
    let tag = &lower[tag_start..tag_end];
    if !tag.starts_with("<meta") {
        return None;
    }
    let c = tag.find("content")?;
    let rest = tag[c + 7..].trim_start().strip_prefix('=')?.trim_start();
    let (q, rest) = match rest.chars().next()? {
        q @ ('"' | '\'') => (q, &rest[1..]),
        _ => (' ', rest),
    };
    let value = &rest[..rest.find(q).unwrap_or(rest.len())];
    let mut pages = Vec::new();
    for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (
                a.trim().parse::<usize>().unwrap_or(1),
                b.trim().parse::<usize>().unwrap_or(1000),
            ),
            None => {
                let n = part.parse::<usize>().ok()?;
                (n, n)
            }
        };
        for n in a.max(1)..=b.min(1000) {
            if !pages.contains(&(n - 1)) {
                pages.push(n - 1);
            }
        }
    }
    pages.sort_unstable();
    Some(pages)
}

/// Тест, которому нужен JavaScript, стенд исполнить не может.
/// `<meta name="variant">` применяется скриптом
/// (`support/variant-class.js` читает `location.search` и вешает
/// класс на `<html>`); без класса тест и эталон рисуют дефолт, и
/// пара сходится, не проверив ничего (`dominant-baseline-auto`:
/// снимки побайтово равны). Хуже: `text-box-trim-start-001` был
/// 0.00, пока трима не было, и стал 25.62, когда трим появился —
/// эталон без класса остаётся нетримленным. Такая пара — вне
/// цели, а не зелёная и не красная; рисовать её незачем.
/// Владелец (2026-10-16): вне цели ВЕСЬ JavaScript, не только `<script>` в
/// тесте. Разбор красных integ-30 нашёл 71 такую пару: скрипт в эталоне (26),
/// обработчики `onload=…` (17; ещё 13 таких были «зелёными» случайно —
/// обработчик просто не исполнялся), тесты для сервера WPT `.sub.` (14),
/// вложенные документы со скриптом (7), ручные `-manual` (6).
pub(super) fn out_of_scope(test: &str, reference: &str) -> Option<&'static str> {
    let name = std::path::Path::new(test)
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.contains(".sub.") {
        return Some("вне цели: сервер WPT");
    }
    if name.contains("-manual.") {
        return Some("вне цели: ручной тест");
    }
    let head_lower = std::fs::read_to_string(test)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if head_lower.contains("name=\"variant\"") || head_lower.contains("name='variant'") {
        return Some("вне цели: вариант");
    }
    if let Some(why) = script_use(&head_lower) {
        return Some(why);
    }
    let reference_lower = std::fs::read_to_string(reference)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if script_use(&reference_lower).is_some() {
        return Some("вне цели: скрипт в эталоне");
    }
    let dir = std::path::Path::new(test)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let scripted_subdocument = subdocument_sources(&head_lower).into_iter().any(|src| {
        let text = std::fs::read_to_string(dir.join(src)).unwrap_or_default();
        script_use(&text.to_ascii_lowercase()).is_some()
    });
    scripted_subdocument.then_some("вне цели: скрипт во вложенном документе")
}

/// Исполняемый JavaScript в тексте документа (уже в нижнем регистре).
fn script_use(lower: &str) -> Option<&'static str> {
    if lower.contains("<script") {
        Some("вне цели: скрипт")
    } else if lower.contains("javascript:") {
        Some("вне цели: javascript-ссылка")
    } else if has_event_handler(lower) {
        Some("вне цели: обработчик on*")
    } else {
        None
    }
}

/// Атрибут-обработчик `on<имя>=` внутри тега (`<body onload="…">`). Текст и
/// CSS между тегами не смотрим: `online` в тексте — не обработчик.
fn has_event_handler(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut in_tag = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'<' => in_tag = true,
            b'>' => in_tag = false,
            _ if in_tag && b.is_ascii_whitespace() && bytes[i + 1..].starts_with(b"on") => {
                let rest = &bytes[i + 3..];
                let letters = rest.iter().take_while(|c| c.is_ascii_lowercase()).count();
                let after = rest[letters..].iter().find(|c| !c.is_ascii_whitespace());
                if letters >= 3 && after == Some(&b'=') {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Источники вложенных документов: `<iframe src>`, `<object data>`, `<embed src>`.
fn subdocument_sources(lower: &str) -> Vec<&str> {
    let mut out = vec![];
    for (tag, attr) in [("<iframe", "src"), ("<object", "data"), ("<embed", "src")] {
        for chunk in lower.split(tag).skip(1) {
            let head = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
            let Some(at) = head.find(&format!("{attr}=")) else {
                continue;
            };
            let value = head[at + attr.len() + 1..].trim_start();
            let value = match value.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    &value[1..value[1..].find(q).map_or(value.len(), |e| e + 1)]
                }
                _ => &value[..value.find(char::is_whitespace).unwrap_or(value.len())],
            };
            let value = value.split(['#', '?']).next().unwrap_or("");
            if !value.is_empty() && !value.contains(':') {
                out.push(value);
            }
        }
    }
    out
}

/// Эталоны пары из `<link rel=match|mismatch>` теста.
pub(super) struct References {
    /// Запасные эталоны (`rel=match`), кроме основного.
    pub(super) alternates: Vec<String>,
    /// Анти-эталоны (`rel=mismatch`).
    pub(super) mismatches: Vec<String>,
    /// Есть ли у теста хоть один `rel=match`.
    pub(super) has_match: bool,
}

/// WPT разрешает тесту НЕСКОЛЬКО эталонов (`rel=match`):
/// совпадение с любым — зачёт. Стенд сравнивает с первым, а
/// остальные проверяет, только если первый не сошёлся
/// (`hyphens-manual-011`: два эталона — с дефисом-минусом и с
/// настоящим знаком переноса).
/// ГОЧА: `rel="mismatch"` СОДЕРЖИТ подстроку `match`, а значит
/// при поиске по подстроке анти-эталон шёл запасным эталоном —
/// и тест, совпавший с ним (то есть по-настоящему провалившийся),
/// получал маленькое число и красился зелёным. Разбираем `rel`
/// списком слов и держим анти-эталоны отдельным оракулом.
pub(super) fn references(test: &str, reference: &str) -> References {
    let mut alternates: Vec<String> = vec![];
    let mut mismatches: Vec<String> = vec![];
    let mut has_match = false;
    let source_for_refs = std::fs::read_to_string(test).unwrap_or_default();
    for tag in source_for_refs.to_ascii_lowercase().split("<link").skip(1) {
        let head = &tag[..tag.find('>').unwrap_or(tag.len())];
        let Some(rel_at) = head.find("rel") else {
            continue;
        };
        let rel = head[rel_at + 3..]
            .trim_start()
            .strip_prefix('=')
            .map(|r| r.trim_start())
            .unwrap_or("");
        let rel = match rel.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let rest = &rel[1..];
                &rest[..rest.find(q).unwrap_or(rest.len())]
            }
            _ => &rel[..rel.find(char::is_whitespace).unwrap_or(rel.len())],
        };
        let anti = rel.split_whitespace().any(|t| t == "mismatch");
        if !anti && !rel.split_whitespace().any(|t| t == "match") {
            continue;
        }
        has_match |= rel.split_whitespace().any(|t| t == "match");
        let Some(at) = head.find("href") else {
            continue;
        };
        let rest = &head[at + 4..];
        let Some(open) = rest.find(['"', '\'']) else {
            continue;
        };
        let quote = rest.as_bytes()[open] as char;
        let Some(close) = rest[open + 1..].find(quote) else {
            continue;
        };
        let href = &rest[open + 1..open + 1 + close];
        let dir = std::path::Path::new(test)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let full = dir.join(href);
        let full = full.to_string_lossy().replace('/', "\\");
        if anti {
            if !mismatches.contains(&full) {
                mismatches.push(full);
            }
        } else if !reference_result::same_file(&full, reference) && !alternates.contains(&full) {
            alternates.push(full);
        }
    }
    References {
        alternates,
        mismatches,
        has_match,
    }
}
