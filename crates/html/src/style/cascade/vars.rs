//! `var()`, `attr()`, `sibling-index()`.
// owner: B

use crate::style::computed::top_level_comma;
use crate::style::css::Decls;
use crate::style::values::value::{Color, Len};

/// Подстановка `var(--x)` и `var(--x, запасное)`.
/// Сколько раз раскрывать переменные внутри переменных.
///
/// Тема обычно ссылается на тему: `--btn: var(--accent)`. Один проход такую
/// цепочку не раскрывал, и объявление уходило в разбор строкой `var(--accent)`.
/// Потолок нужен от кольцевых ссылок.
pub(crate) const VAR_DEPTH: usize = 8;

/// Смещение скобки, парной той, что открыла запись.
pub(crate) fn balanced_close(after_open: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in after_open.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

thread_local! {
    /// Атрибуты элемента, чей стиль сейчас собирается, — источник для
    /// типизированного `attr()` (css-values-5 §7.7). Ставит `dom::walk` на
    /// время `resolve_with_vars` и сразу снимает: у стилей вне этого окна
    /// (наведение, кадры анимации, псевдоэлементы) хозяина нет, и там берётся
    /// запасное значение.
    pub(crate) static CURRENT_ATTRS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// Номер элемента среди братьев и их число — для `sibling-index()` и
    /// `sibling-count()` (css-values-5 §tree-counting). Ставит `dom::walk`
    /// на время каскада элемента, как и атрибуты.
    pub(crate) static CURRENT_SIBLING: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

pub fn set_current_sibling(at: Option<(usize, usize)>) {
    CURRENT_SIBLING.with(|c| c.set(at));
}

/// `sibling-index()` / `sibling-count()` — целым числом (css-values-5
/// §tree-counting: «sibling-index() … returns an <integer> … the index of the
/// element among its inclusive siblings, starting at 1»). Без хозяина
/// (вне каскада элемента) запись остаётся как есть и роняет объявление.
pub(crate) fn resolve_sibling(value: String) -> String {
    if !value.contains("sibling-") {
        return value;
    }
    match CURRENT_SIBLING.with(|c| c.get()) {
        Some((i, n)) => value
            .replace("sibling-index()", &i.to_string())
            .replace("sibling-count()", &n.to_string()),
        None => value,
    }
}

pub fn set_current_attrs(attrs: &[(String, String)]) {
    CURRENT_ATTRS.with(|a| *a.borrow_mut() = attrs.to_vec());
}

pub fn clear_current_attrs() {
    CURRENT_ATTRS.with(|a| a.borrow_mut().clear());
}

/// Значение атрибута по имени из `attr()`.
///
/// Префикс пространства имён (`foo|bar`): у атрибутов HTML пространства нет,
/// а реестра `@namespace` здесь не видно — атрибут считается отсутствующим, и
/// берётся запасное значение (`attr-namespace-non-existing`). `|bar` — по
/// локальному имени; `*|bar` сюда не доходит (`resolve_attrs`). Сравнение
/// ASCII-регистронезависимое: HTML-парсер опускает в нижний регистр только
/// ASCII, и запрос опускается так же, а не-ASCII знаки сравниваются как есть
/// (`html-attr-case-insensitivity`).
pub(crate) fn attr_value(name: &str) -> Option<String> {
    let local = match name.split_once('|') {
        Some(("", local)) => local,
        Some(_) => return None,
        None => name,
    };
    CURRENT_ATTRS.with(|a| {
        a.borrow()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(local))
            .map(|(_, v)| v.clone())
    })
}

/// Значение атрибута под типом `attr()`; `None` — не разбирается этим типом.
pub(crate) fn attr_cast(value: &str, ty: &str) -> Option<String> {
    let v = value.trim();
    let ty: String = ty
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // `<length>`: голое число длиной не бывает, кроме нуля (css-values-4
    // §6.1); `Len::parse` принимает его точками — отсекаем здесь.
    let length = |v: &str| match Len::parse(v) {
        Some(Len::Px(n)) if v.parse::<f32>().is_ok() => (n == 0.0).then(|| v.to_string()),
        Some(
            Len::Pct(_)
            | Len::Auto
            | Len::MinContent
            | Len::MaxContent
            | Len::FitContent
            | Len::Anchor(_),
        )
        | None => None,
        Some(_) => Some(v.to_string()),
    };
    let percentage = |v: &str| {
        v.strip_suffix('%')
            .and_then(|n| n.trim().parse::<f32>().ok())
            .map(|_| v.to_string())
    };
    match ty.as_str() {
        // Любые токены; `url()` из атрибута запрещён (attr-tainted).
        "type(*)" => (!v.is_empty() && !v.to_ascii_lowercase().contains("url("))
            .then(|| v.to_string()),
        "type(<length>)" => length(v),
        "type(<percentage>)" => percentage(v),
        "type(<length-percentage>)" => length(v).or_else(|| percentage(v)),
        "type(<number>)" | "number" => v.parse::<f32>().ok().map(|_| v.to_string()),
        "type(<integer>)" => v.parse::<i64>().ok().map(|_| v.to_string()),
        "type(<color>)" => Color::parse(v).map(|_| v.to_string()),
        // `attr(x px)` — число из атрибута с единицей из записи.
        unit if !unit.is_empty()
            && (unit == "%" || unit.chars().all(|c| c.is_ascii_alphabetic())) =>
        {
            v.parse::<f32>().ok().map(|_| format!("{v}{unit}"))
        }
        _ => None,
    }
}

/// Типизированный `attr()` (css-values-5 §7.7): `attr(<имя> <тип>, <запас>?)`
/// подставляется ДО разбора значения, как `var()`. Нетипизированная запись
/// (`attr(x)` — строка) остаётся как есть: её разбирают `content` и счётчики.
/// Негодный атрибут без запаса делает объявление недействительным на
/// вычислении — значение становится `unset`.
pub(crate) fn resolve_attrs(key: &str, value: &str) -> String {
    if key == "content"
        || !value
            .as_bytes()
            .windows(5)
            .any(|w| w.eq_ignore_ascii_case(b"attr("))
    {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    loop {
        // ASCII-опускание не сдвигает байтов: индекс годен и для `rest`.
        let Some(at) = rest.to_ascii_lowercase().find("attr(") else {
            break;
        };
        let after = &rest[at + 5..];
        let Some(close) = balanced_close(after) else {
            break;
        };
        let inner = &after[..close];
        let (head, fallback) = match top_level_comma(inner) {
            Some(i) => (inner[..i].trim(), Some(inner[i + 1..].trim())),
            None => (inner.trim(), None),
        };
        let (name, ty) = match head.split_once(char::is_whitespace) {
            Some((n, t)) => (n.trim(), t.trim()),
            None => (head, ""),
        };
        // `<attr-name>` — как `<wq-name>`, «but without the possibility of a
        // wildcard prefix» (css-values-5 §attr-notation, Overview.bs:2057-2059):
        // `attr(*|bar …)` негоден при разборе, запас не спасает
        // (`attr-namespace-wildcard`). Выброс объявления здесь выражается
        // `unset`, как у негодного атрибута без запаса.
        if name.starts_with("*|") {
            return "unset".to_string();
        }
        out.push_str(&rest[..at]);
        if ty.is_empty() {
            out.push_str(&rest[at..at + 5 + close + 1]);
        } else {
            match (attr_value(name).and_then(|v| attr_cast(&v, ty)), fallback) {
                (Some(v), _) => out.push_str(&v),
                (None, Some(f)) => out.push_str(f),
                (None, None) => return "unset".to_string(),
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

pub(crate) fn resolve_vars(value: &str, vars: &Decls) -> String {
    let mut out = value.to_string();
    for _ in 0..VAR_DEPTH {
        let Some(next) = crate::style::css::variable_values::substitute(
            &out, &mut |name| vars.get(name).cloned(),
        ) else {
            return "unset".into();
        };
        if next == out {
            break;
        }
        out = next;
        if !crate::style::css::variable_values::has_var(&out) {
            break;
        }
    }
    if out.trim().is_empty() && crate::style::css::variable_values::has_var(value) {
        "unset".into()
    } else {
        out
    }
}
