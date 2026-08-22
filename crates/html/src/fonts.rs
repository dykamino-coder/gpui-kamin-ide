//! Шрифты страницы: правило `@font-face`.
//!
//! Разметка вправе принести СВОЙ шрифт и назвать его как угодно:
//!
//! ```css
//! @font-face { font-family: 'мой'; src: url('/fonts/mplus.woff') }
//! p { font-family: 'мой' }
//! ```
//!
//! Система шрифтов знает файл под его СОБСТВЕННЫМ именем, а разметка просит
//! по своему — поэтому здесь два дела: загрузить файл в систему и запомнить,
//! какое настоящее имя стоит за придуманным. Подмену делает [`alias`], её
//! зовут все места, где семейство уходит в набор.
//!
//! Упаковки `woff` и `woff2` — это тот же sfnt: у первой сжаты таблицы, у
//! второй сверх того перестроены глифы. Распаковку делает крейт `wuff`.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Придуманное разметкой имя → имя, под которым шрифт знает система.
    static ALIASES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Уже загруженные файлы: одно и то же правило встречается на странице
    /// не по разу, а разбор шрифта дорог.
    static LOADED: RefCell<HashMap<String, Option<String>>> =
        RefCell::new(HashMap::new());
    /// Приёмник шрифта: отдаёт системе байты и возвращает её имя семейства.
    static LOADER: RefCell<Option<Loader>> = const { RefCell::new(None) };
}

/// Загрузчик: получает содержимое файла, отдаёт имя семейства в системе.
type Loader = Box<dyn Fn(Vec<u8>) -> Option<String>>;

/// Поставить загрузчик шрифтов. Зовут там, где живёт система шрифтов.
pub fn install_loader(loader: impl Fn(Vec<u8>) -> Option<String> + 'static) {
    LOADER.with(|l| *l.borrow_mut() = Some(Box::new(loader)));
}

/// Настоящее имя семейства за именем из разметки.
pub fn alias(family: &str) -> Option<String> {
    ALIASES.with(|a| a.borrow().get(&family.to_ascii_lowercase()).cloned())
}

/// Разобрать правила `@font-face` из таблицы стилей и загрузить шрифты.
///
/// Путь в `url(...)` берётся как есть: страницу до движка доводит стенд, и
/// адреса в ней уже разрешены в файлы.
pub fn load_faces(css: &str) {
    // Имена семейств придумывает страница, и на соседней странице то же имя
    // значит другой файл — поэтому таблица подмены живёт РОВНО одну страницу.
    ALIASES.with(|a| a.borrow_mut().clear());
    load_faces_into(css);
}

/// Дозагрузка БЕЗ сброса подмен: вложенный документ (`<iframe>`) разбирается
/// посреди отрисовки внешнего — сброс крал бы шрифты хозяина, и весь текст
/// после рамки падал в подстановочный шрифт.
pub fn load_faces_additive(css: &str) {
    load_faces_into(css);
}

fn load_faces_into(css: &str) {
    for block in faces(css) {
        let (Some(family), Some(src)) = (declaration(&block, "font-family"), source(&block)) else {
            continue;
        };
        let real = LOADED.with(|c| c.borrow().get(&src).cloned());
        let real = match real {
            Some(hit) => hit,
            None => {
                let loaded = read_font(&src).and_then(|bytes| {
                    LOADER.with(|l| l.borrow().as_ref().and_then(|load| load(bytes)))
                });
                LOADED.with(|c| c.borrow_mut().insert(src.clone(), loaded.clone()));
                loaded
            }
        };
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("FONT_DBG").is_ok());
            *ON
        } {
            eprintln!("FONT_DBG face family={family:?} src={src:?} real={real:?}");
        }
        if let Some(real) = real {
            ALIASES.with(|a| {
                a.borrow_mut()
                    .insert(family.trim_matches(is_quote).to_ascii_lowercase(), real)
            });
        }
    }
}

fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

/// Тела всех правил `@font-face` в таблице.
fn faces(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Комментарии срезаются ДО поиска: `/* @font-face {...} */` разбирался
    // как живое правило и грузил чужой файл.
    let css = &crate::css::strip_comments(css);
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
        out.push(css[start + open + 1..start + open + close].to_string());
        from = start + open + close;
    }
    out
}

/// Значение свойства внутри правила.
fn declaration(block: &str, name: &str) -> Option<String> {
    block.split(';').find_map(|decl| {
        let (key, value) = decl.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

/// Первый пригодный файл из `src`: берём тот, чью упаковку умеем открыть.
fn source(block: &str) -> Option<String> {
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
fn read_font(path: &str) -> Option<Vec<u8>> {
    let path = path.strip_prefix("file:///").unwrap_or(path);
    let bytes = std::fs::read(path).ok()?;
    let bytes = if bytes.starts_with(b"wOFF") {
        wuff::decompress_woff1(&bytes).ok()?
    } else if bytes.starts_with(b"wOF2") {
        wuff::decompress_woff2(&bytes).ok()?
    } else {
        bytes
    };
    Some(ensure_windows_names(bytes))
}

/// Дописать в name-таблицу записи Windows (platform 3, en-US).
///
/// DirectWrite строит имена семейства только из записей platform 3 (или Mac);
/// шрифт, где имена лежат лишь под platform 0 / language 0 (обычное дело для
/// тестовых шрифтов WPT — FontWithFancyFeatures и родня), получает ПУСТОЙ
/// набор локализованных имён: его не найти по имени, и текст молча рисуется
/// системной подменой. Строки platform 0 — тот же UTF-16BE, так что записи
/// просто дублируются с (3, 1, 0x0409) поверх того же строкового блоба;
/// пересобранная таблица дописывается в конец файла, а запись каталога
/// перенацеливается на неё.
fn ensure_windows_names(bytes: Vec<u8>) -> Vec<u8> {
    fn be16(b: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
    }
    fn be32(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            *b.get(at)?,
            *b.get(at + 1)?,
            *b.get(at + 2)?,
            *b.get(at + 3)?,
        ]))
    }
    let rebuild = || -> Option<Vec<u8>> {
        let num_tables = be16(&bytes, 4)? as usize;
        let mut dir_at = None;
        for i in 0..num_tables {
            let rec = 12 + i * 16;
            if bytes.get(rec..rec + 4)? == b"name" {
                dir_at = Some(rec);
                break;
            }
        }
        let rec = dir_at?;
        let base = be32(&bytes, rec + 8)? as usize;
        let count = be16(&bytes, base + 2)? as usize;
        let str_off = base + be16(&bytes, base + 4)? as usize;
        let mut records: Vec<[u16; 6]> = Vec::with_capacity(count * 2);
        let mut str_end = 0usize;
        for i in 0..count {
            let r = base + 6 + i * 12;
            let rec6 = [
                be16(&bytes, r)?,
                be16(&bytes, r + 2)?,
                be16(&bytes, r + 4)?,
                be16(&bytes, r + 6)?,
                be16(&bytes, r + 8)?,
                be16(&bytes, r + 10)?,
            ];
            if rec6[0] == 3 && rec6[3] == 1 {
                // Имя семейства для Windows уже есть — файл не трогается.
                return None;
            }
            str_end = str_end.max(rec6[4] as usize + rec6[5] as usize);
            records.push(rec6);
        }
        let blob = bytes.get(str_off..str_off + str_end)?.to_vec();
        let mut extra: Vec<[u16; 6]> = records
            .iter()
            .filter(|r| r[0] == 0 && matches!(r[3], 1 | 2 | 4 | 6 | 16 | 17))
            .map(|r| [3, 1, 0x0409, r[3], r[4], r[5]])
            .collect();
        if extra.is_empty() {
            return None;
        }
        records.append(&mut extra);
        records.sort();
        // Новая таблица целиком, в конец файла.
        let mut table = Vec::with_capacity(6 + records.len() * 12 + blob.len());
        for v in [0u16, records.len() as u16, 6 + records.len() as u16 * 12] {
            table.extend_from_slice(&v.to_be_bytes());
        }
        for r in &records {
            for v in r {
                table.extend_from_slice(&v.to_be_bytes());
            }
        }
        table.extend_from_slice(&blob);
        let mut out = bytes.clone();
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let new_off = out.len() as u32;
        out.extend_from_slice(&table);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        // Каталог: offset, length и честная контрольная сумма таблицы.
        let sum = table
            .chunks(4)
            .map(|c| {
                let mut w = [0u8; 4];
                w[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(w)
            })
            .fold(0u32, u32::wrapping_add);
        out[rec + 4..rec + 8].copy_from_slice(&sum.to_be_bytes());
        out[rec + 8..rec + 12].copy_from_slice(&new_off.to_be_bytes());
        out[rec + 12..rec + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        Some(out)
    };
    rebuild().unwrap_or(bytes)
}

/// Имя семейства из name-таблицы sfnt (nameID 16, затем 1): регистрация
/// в системе может НЕ добавить нового имени в общий список (diff пуст —
/// FontWithFancyFeatures терял алиас и весь текст шёл шрифтом-подменой),
/// поэтому имя читается из самого файла.
pub fn sfnt_family(bytes: &[u8]) -> Option<String> {
    let be16 = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let num_tables = be16(4)? as usize;
    let mut name_off = None;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if bytes.get(rec..rec + 4)? == b"name" {
            name_off = Some(be32(rec + 8)? as usize);
            break;
        }
    }
    let base = name_off?;
    let count = be16(base + 2)? as usize;
    let strings = base + be16(base + 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = base + 6 + i * 12;
        let platform = be16(rec)?;
        let name_id = be16(rec + 6)?;
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let len = be16(rec + 8)? as usize;
        let off = strings + be16(rec + 10)? as usize;
        let raw = bytes.get(off..off + len)?;
        let text = match platform {
            // Windows/Unicode: UTF-16BE.
            0 | 3 => String::from_utf16(
                &raw.chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok()?,
            _ => String::from_utf8_lossy(raw).into_owned(),
        };
        if text.trim().is_empty() {
            continue;
        }
        // nameID 16 (typographic family) сильнее 1.
        let rank = if name_id == 16 { 2 } else { 1 };
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_rule_gives_family_and_file() {
        let css = "@font-face { font-family: 'мой'; src: url('/f/a.woff2') format('woff2'), \
                   url('/f/a.woff') format('woff') } p { color: red }";
        let block = faces(css);
        assert_eq!(block.len(), 1);
        assert_eq!(
            declaration(&block[0], "font-family").as_deref(),
            Some("'мой'")
        );
        // Обе упаковки нам по силам, поэтому берётся первая же.
        assert_eq!(source(&block[0]).as_deref(), Some("/f/a.woff2"));
    }

    #[test]
    fn only_font_face_blocks_are_taken() {
        let css = "p { font-family: 'нет' } @font-face { font-family: 'да'; src: url(a.ttf) }";
        let block = faces(css);
        assert_eq!(block.len(), 1);
        assert_eq!(
            declaration(&block[0], "font-family").as_deref(),
            Some("'да'")
        );
    }
}
