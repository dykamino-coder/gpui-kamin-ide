//! Чтение файла шрифта для регистрации в системе: адрес `data:` или путь, распаковка `woff`/
//! `woff2` в sfnt, проверка совпадения упаковки адреса с подписью и версии sfnt. Негодный файл
//! отвергается, чтобы система не подменила его своим шрифтом молча.

use super::ensure_windows_names;

pub(super) fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

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

/// Прочитать файл шрифта и, если он упакован, распаковать.
///
/// Обе упаковки — это тот же sfnt: в `woff` таблицы просто сжаты, в `woff2`
/// вдобавок перестроены таблицы глифов. Разбор второй руками не пишут, он
/// взят крейтом.
pub(super) fn read_font(path: &str, revision: &crate::budget::RevisionBudget) -> Option<Vec<u8>> {
    // Адрес `data:` (RFC 2397) — шрифт в самой таблице; упаковку тогда
    // покажет подпись (ветки ниже без расширения).
    let bytes = crate::document::resource_url::read(path, revision).ok()?;
    let path = if path.starts_with("data:") { "" } else { path };
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
