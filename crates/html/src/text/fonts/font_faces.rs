//! Font faces for fonts; split out to keep the owning module within 250 lines.

use super::ensure_windows_names;

pub(super) fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

/// Тела всех правил `@font-face` в таблице.
pub(super) fn faces(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Комментарии срезаются ДО поиска: `/* @font-face {...} */` разбирался
    // как живое правило и грузил чужой файл.
    let css = &crate::style::css::strip_comments(css);
    let lower = css.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find("@font-face") {
        let start = from + at;
        let Some(open) = css[start..].find('{') else {
            break;
        };
        let Some(close) = css[start + open..].find('}') else {
            break;
        };
        let body = css[start + open + 1..start + open + close].to_string();
        from = start + open + close;
        // Правило внутри ложного `@media`/`@supports` не действует
        // (css-conditional-3 §2): иначе вторая, «запасная» грань того же
        // семейства перебивала первую (`at-media-content-002`,
        // `at-supports-content-002`: `local('Arial')` вместо Ahem). Сюда
        // приходит вся разметка — стек скобок считается от начала своего
        // `<style>`.
        let base = lower[..start].rfind("<style").map_or(0, |s| {
            lower[s..start].find('>').map_or(start, |g| s + g + 1)
        });
        if crate::style::css::in_false_group(
            &css[base..],
            start - base,
            crate::style::css::Media::default(),
        ) {
            continue;
        }
        out.push(body);
    }
    out
}

/// Значение свойства внутри правила.
pub(super) fn declaration(block: &str, name: &str) -> Option<String> {
    block.split(';').find_map(|decl| {
        let (key, value) = decl.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

/// Первый пригодный файл из `src`: берём тот, чью упаковку умеем открыть.
pub(super) fn source(block: &str) -> Option<String> {
    let src = declaration(block, "src")?;
    let mut fallback = None;
    for part in src.split(',') {
        let Some(open) = part.find("url(") else {
            continue;
        };
        let rest = &part[open + 4..];
        let Some(close) = rest.find(')') else {
            continue;
        };
        let path = rest[..close].trim().trim_matches(is_quote).to_string();
        let known = matches!(
            std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("ttf" | "otf" | "woff" | "woff2")
        );
        if known {
            return Some(path);
        }
        fallback.get_or_insert(path);
    }
    fallback
}

/// Прочитать файл шрифта и, если он упакован, распаковать.
///
/// Обе упаковки — это тот же sfnt: в `woff` таблицы просто сжаты, в `woff2`
/// вдобавок перестроены таблицы глифов. Разбор второй руками не пишут, он
/// взят крейтом.
/// Годен ли sfnt: первые четыре байта — его версия.
///
/// Распаковка обязана дать именно sfnt. Иначе в систему уходит мусор, она
/// молча подменяет его своим шрифтом, и страница выглядит так, будто чужой
/// шрифт ПРИНЯТ — ровно то, чего негодный файл не должен добиваться.
pub(super) fn sfnt_ok(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some(b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"typ1" | b"ttcf")
    )
}

pub(super) fn read_font(path: &str) -> Option<Vec<u8>> {
    let path = path.strip_prefix("file:///").unwrap_or(path);
    let bytes = std::fs::read(path).ok()?;
    // Упаковку задаёт АДРЕС, а подпись внутри файла — то, что файл о себе
    // заявляет. Их расхождение — отказ (§4.1 WOFF2): файл `.woff2` с
    // подписью `XXXX` прежде проваливался мимо обеих веток распаковки и
    // уходил в систему сырым, будто это голый sfnt.
    let packing = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let bytes = match packing.as_deref() {
        Some("woff") => {
            if !bytes.starts_with(b"wOFF") {
                return None;
            }
            wuff::decompress_woff1(&bytes).ok()?
        }
        Some("woff2") => {
            if !bytes.starts_with(b"wOF2") {
                return None;
            }
            wuff::decompress_woff2(&bytes).ok()?
        }
        // Адрес без расширения (`data:`-подобные пути набора) — судим по
        // подписи, как прежде.
        _ if bytes.starts_with(b"wOFF") => wuff::decompress_woff1(&bytes).ok()?,
        _ if bytes.starts_with(b"wOF2") => wuff::decompress_woff2(&bytes).ok()?,
        _ => bytes,
    };
    if !sfnt_ok(&bytes) {
        return None;
    }
    Some(ensure_windows_names(bytes))
}
