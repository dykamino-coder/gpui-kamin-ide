//! Локальная правка разметки для режима `edit` (ширину меняет режим `relayout`).
//! Правка детерминирована и не зависит от страницы, кроме места: первый
//! подходящий узел после середины разметки.

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edit {
    /// Одна буква текстового узла после середины документа.
    Text,
    /// Новый атрибут `data-edit` у элемента после середины документа.
    Attr,
    /// Класс `alt` у корневого `html` (так переключают стиль пробы браузеров).
    Class,
    /// Новый узел: копия первого листового элемента после середины, сразу за ним.
    Insert,
    /// Удаление первого листового элемента после середины документа.
    Remove,
}

/// Готовая вторая разметка (`--edit-markup FILE`) вместо правки `Edit`. Хранится
/// вне `Args`: их размер входит в кучу стенда (замыкание и задача цикла событий),
/// а аудит сравнивает живые байты кандидата и базы точно.
static MARKUP_FILE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub(crate) fn set_markup_file(path: std::path::PathBuf) -> Result<()> {
    MARKUP_FILE
        .set(path)
        .map_err(|_| "--edit-markup is given twice".into())
}

pub(crate) fn markup_file() -> Option<&'static std::path::Path> {
    MARKUP_FILE.get().map(std::path::PathBuf::as_path)
}

impl Edit {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "text" => Self::Text,
            "attr" => Self::Attr,
            "class" => Self::Class,
            "insert" => Self::Insert,
            "remove" => Self::Remove,
            _ => return Err("edit must be text, attr, class, insert or remove".into()),
        })
    }

    /// Разметка второго варианта.
    pub(crate) fn markup(self, html: &str) -> Result<String> {
        let edited = match self {
            Self::Text => text(html),
            Self::Attr => attr(html),
            Self::Class => class(html),
            Self::Insert => leaf(html).map(|leaf| {
                let mut out = html.to_owned();
                out.insert_str(leaf.end, &html[leaf.start..leaf.end]);
                out
            }),
            Self::Remove => leaf(html).map(|leaf| {
                let mut out = html.to_owned();
                out.replace_range(leaf, "");
                out
            }),
        };
        edited.ok_or_else(|| format!("no place for a {self:?} edit in the input").into())
    }
}

/// Позиция внутри `<script>`/`<style>`: правка там не меняет отрисовку.
fn in_raw_text(html: &str, at: usize) -> bool {
    let before = &html[..at];
    ["script", "style"].iter().any(|tag| {
        let open = before.rfind(&format!("<{tag}"));
        let close = before.rfind(&format!("</{tag}"));
        open.is_some_and(|open| close.is_none_or(|close| close < open))
    })
}

fn middle(html: &str) -> usize {
    let start = html.find("<body").unwrap_or(0);
    let mut at = start + (html.len() - start) / 2;
    while !html.is_char_boundary(at) {
        at += 1;
    }
    at
}

fn text(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut at = middle(html);
    while let Some(offset) = html[at..].find('>') {
        let open = at + offset + 1;
        let end = html[open..].find('<').map_or(html.len(), |e| open + e);
        if !in_raw_text(html, open)
            && let Some(letter) = (open..end).find(|&i| bytes[i].is_ascii_lowercase())
        {
            let replacement = if bytes[letter] == b'a' { "o" } else { "a" };
            let mut out = html.to_owned();
            out.replace_range(letter..letter + 1, replacement);
            return Some(out);
        }
        at = end;
    }
    None
}

/// Начало имени открывающего тега (позиция после `<`) вне `<script>`/`<style>`.
fn start_tag_at(html: &str, open: usize) -> bool {
    html.as_bytes()
        .get(open)
        .is_some_and(u8::is_ascii_alphabetic)
        && !in_raw_text(html, open)
}

/// Первый открывающий тег после середины; если во второй половине одни
/// закрывающие теги (короткая страница, глубокая вложенность) — последний до неё.
fn attr(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let middle = middle(html);
    let after = html[middle..]
        .match_indices('<')
        .map(|(i, _)| middle + i + 1)
        .find(|&open| start_tag_at(html, open));
    let open = after.or_else(|| {
        html[..middle]
            .rmatch_indices('<')
            .map(|(i, _)| i + 1)
            .find(|&open| start_tag_at(html, open))
    })?;
    let name = (open..html.len())
        .find(|&i| !bytes[i].is_ascii_alphanumeric())
        .unwrap_or(html.len());
    let mut out = html.to_owned();
    out.insert_str(name, " data-edit=\"1\"");
    Some(out)
}

/// Первый листовой элемент после середины: `<имя ...>текст</имя>` без вложенных
/// тегов. Копия такого элемента рядом с ним допустима в любом родителе (ячейка
/// таблицы, пункт списка, строчный элемент), поэтому вставка и удаление меняют
/// ровно один узел дерева, а не то, как разбирается окружение.
fn leaf(html: &str) -> Option<std::ops::Range<usize>> {
    let bytes = html.as_bytes();
    let mut at = middle(html);
    while let Some(offset) = html[at..].find('<') {
        let start = at + offset;
        let open = start + 1;
        at = open;
        if !bytes.get(open).is_some_and(u8::is_ascii_alphabetic) || in_raw_text(html, open) {
            continue;
        }
        let name_end = (open..html.len())
            .find(|&i| !bytes[i].is_ascii_alphanumeric())
            .unwrap_or(html.len());
        let name = &html[open..name_end];
        if ["script", "style", "template", "title"]
            .iter()
            .any(|raw| name.eq_ignore_ascii_case(raw))
        {
            continue;
        }
        let Some(tag_end) = html[name_end..].find('>').map(|e| name_end + e) else {
            return None;
        };
        if bytes[tag_end - 1] == b'/' {
            continue;
        }
        let content = tag_end + 1;
        let Some(next) = html[content..].find('<').map(|e| content + e) else {
            return None;
        };
        let close = &html[next..];
        let closes = close.len() > name.len() + 2
            && close.starts_with("</")
            && close[2..2 + name.len()].eq_ignore_ascii_case(name)
            && close.as_bytes()[2 + name.len()] == b'>';
        if closes {
            return Some(start..next + name.len() + 3);
        }
    }
    None
}

/// Класс на корневом элементе. Без явного `<html>` корень неявный: тег
/// `<html class="alt">` ставится после doctype (до него он сменил бы режим
/// документа), и разбор остального не меняется.
fn class(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = html.to_owned();
    match lower.find("<html") {
        Some(at) => {
            let open = at + "<html".len();
            let tag_end = open + lower[open..].find('>')?;
            if lower[open..tag_end].contains("class") {
                return None;
            }
            out.insert_str(open, " class=\"alt\"");
        }
        None => {
            let lead = lower.len() - lower.trim_start().len();
            let at = if lower[lead..].starts_with("<!doctype") {
                lead + lower[lead..].find('>')? + 1
            } else {
                lead
            };
            out.insert_str(at, "<html class=\"alt\">");
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
