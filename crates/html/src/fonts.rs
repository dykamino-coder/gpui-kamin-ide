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
        if std::env::var_os("FONT_DBG").is_some() {
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
    if bytes.starts_with(b"wOFF") {
        return wuff::decompress_woff1(&bytes).ok();
    }
    if bytes.starts_with(b"wOF2") {
        return wuff::decompress_woff2(&bytes).ok();
    }
    Some(bytes)
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
