//! @page (селекторы страниц, поля), @property, @position-try.

use crate::style::css::*;
use std::collections::HashMap;

/// Селектор страницы (css-page-3 §page-selectors): имя типа страницы и
/// счётчики псевдоклассов. `:blank` хранится ради специфичности — пустых
/// листов стопка не рождает, и такой селектор ни с чем не совпадает.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageSel {
    pub name: Option<String>,
    pub first: u8,
    pub blank: u8,
    pub left: u8,
    pub right: u8,
}

/// Правило `@page` целиком: список селекторов и объявления в порядке записи.
#[derive(Clone, Debug, Default)]
pub struct PageRule {
    pub sels: Vec<PageSel>,
    pub decls: Vec<(String, String)>,
    /// Вложенные правила марджин-боксов (`@top-left { … }`, css-page-3
    /// §margin-at-rules): имя коробки в нижнем регистре → объявления.
    pub margins: Vec<(String, Vec<(String, String)>)>,
}

/// Шестнадцать марджин-боксов листа (css-page-3 §margin-boxes, Table 1) — по
/// часовой стрелке от левого верхнего угла.
pub const MARGIN_BOXES: [&str; 16] = [
    "top-left-corner",
    "top-left",
    "top-center",
    "top-right",
    "top-right-corner",
    "right-top",
    "right-middle",
    "right-bottom",
    "bottom-right-corner",
    "bottom-right",
    "bottom-center",
    "bottom-left",
    "bottom-left-corner",
    "left-bottom",
    "left-middle",
    "left-top",
];

/// Все правила `@page` документа в порядке появления — с псевдоклассами и
/// именами. Из них `page_decls_for` собирает объявления конкретного листа.
pub static PAGE_RULES: std::sync::Mutex<Vec<PageRule>> = std::sync::Mutex::new(Vec::new());

/// Снимок без очистки: рендер зовётся на каждом кадре, а правила должны
/// пережить все кадры документа (очистка — на разборе следующего).
pub fn page_rules_snapshot() -> Vec<PageRule> {
    PAGE_RULES.lock().unwrap().clone()
}

/// Голова `@page` → список селекторов; `None` — голова неверна, и правило
/// отбрасывается целиком (css-page-3 §syntax-page-selector, как у обычного
/// списка селекторов). Имя регистрозависимо, псевдоклассы — нет.
pub(crate) fn parse_page_selectors(head: &str) -> Option<Vec<PageSel>> {
    let head = head.trim();
    if head.is_empty() {
        return Some(vec![PageSel::default()]);
    }
    let mut out = Vec::new();
    for part in head.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let (ident, rest) = match part.find(':') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        let ident = ident.trim_end();
        if !ident
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
        {
            return None;
        }
        let mut sel = PageSel {
            name: (!ident.is_empty()).then(|| ident.to_string()),
            ..PageSel::default()
        };
        for pc in rest.split(':').skip(1) {
            match pc.trim().to_ascii_lowercase().as_str() {
                "first" => sel.first += 1,
                "blank" => sel.blank += 1,
                "left" => sel.left += 1,
                "right" => sel.right += 1,
                _ => return None,
            }
        }
        out.push(sel);
    }
    Some(out)
}

/// Объявления листа `index` (с нуля) с именем типа `name` (`""` — без
/// имени): каскад css-page-3 §cascading-and-page-context. Совпавшие правила
/// идут по возрастанию специфичности (f, g, h) — f: имя типа, g: `:first` и
/// `:blank`, h: `:left` и `:right`, — при равной по порядку записи; объявления
/// сливаются в этом порядке, последнее побеждает. Левые/правые — по
/// направлению прогрессии страниц: при ltr первый лист ПРАВЫЙ
/// (§page-selectors: «if the root element's direction is ltr, then the first
/// page is a right page»), при rtl — левый.
pub fn page_decls_for(index: usize, name: &str, rtl: bool) -> Vec<(String, String)> {
    page_decls_in(&PAGE_RULES.lock().unwrap(), index, name, rtl)
}

/// То же по снимку правил (`page_rules_snapshot`).
pub fn page_decls_in(rules: &[PageRule], index: usize, name: &str, rtl: bool) -> Vec<(String, String)> {
    matching_rules(rules, index, name, rtl)
        .into_iter()
        .flat_map(|i| rules[i].decls.clone())
        .collect()
}

/// Марджин-боксы листа: каскад тот же, что у объявлений листа
/// (`page_decls_in`), объявления каждой коробки сливаются по возрастанию
/// специфичности. Порядок — `MARGIN_BOXES`; коробки без правил не попадают.
pub fn page_margins_in(
    rules: &[PageRule],
    index: usize,
    name: &str,
    rtl: bool,
) -> Vec<(String, Vec<(String, String)>)> {
    let hits = matching_rules(rules, index, name, rtl);
    MARGIN_BOXES
        .iter()
        .filter_map(|b| {
            let list: Vec<(String, String)> = hits
                .iter()
                .flat_map(|&i| {
                    rules[i]
                        .margins
                        .iter()
                        .filter(|(n, _)| n == b)
                        .flat_map(|(_, d)| d.clone())
                })
                .collect();
            (!list.is_empty()).then(|| (b.to_string(), list))
        })
        .collect()
}

/// Номера совпавших с листом правил по возрастанию (специфичность, порядок).
pub(crate) fn matching_rules(rules: &[PageRule], index: usize, name: &str, rtl: bool) -> Vec<usize> {
    let right = (index % 2 == 0) != rtl;
    let mut hits: Vec<((u8, u8, u8), usize)> = Vec::new();
    for (order, r) in rules.iter().enumerate() {
        let spec = r
            .sels
            .iter()
            .filter(|s| {
                s.name.as_deref().is_none_or(|n| n == name)
                    && (s.first == 0 || index == 0)
                    && s.blank == 0
                    && (s.left == 0 || !right)
                    && (s.right == 0 || right)
            })
            .map(|s| {
                (
                    s.name.is_some() as u8,
                    s.first + s.blank,
                    s.left + s.right,
                )
            })
            .max();
        if let Some(sp) = spec {
            hits.push((sp, order));
        }
    }
    hits.sort();
    hits.into_iter().map(|(_, i)| i).collect()
}

/// Забрать правила `@page` прошлого документа (чистка перед разбором
/// следующего, `dom::parse_media`).
pub fn take_page_decls() -> Vec<PageRule> {
    std::mem::take(&mut PAGE_RULES.lock().unwrap())
}

/// Правила `@position-try <dashed-ident> { … }` документа (css-anchor-position-1
/// §fallback-rule): имя → объявления. Тот же пул, что `PAGE_RULES`: копится
/// при разборе листов, чистится на разборе следующего документа
/// (`take_try_rules`), читается на сборке кадра (`anchor::place`). Повтор
/// имени перекрывает — «the last one in document order wins».
pub static TRY_RULES: std::sync::Mutex<Option<HashMap<String, Decls>>> =
    std::sync::Mutex::new(None);

/// Зарегистрированное свойство `@property` (css-properties-values-api-1 §3):
/// синтаксис, наследуется ли, начальное значение.
#[derive(Clone, Debug)]
pub struct Registered {
    pub syntax: String,
    pub inherits: bool,
    pub initial: Option<String>,
}

/// Реестр `@property` последнего разобранного документа.
pub static PROPERTY_RULES: std::sync::Mutex<Option<HashMap<String, Registered>>> =
    std::sync::Mutex::new(None);

pub fn take_property_rules() -> HashMap<String, Registered> {
    PROPERTY_RULES.lock().unwrap().take().unwrap_or_default()
}

/// Копия реестра `@property` — его читает сборка переменных узла.
pub fn property_rules() -> HashMap<String, Registered> {
    PROPERTY_RULES.lock().unwrap().clone().unwrap_or_default()
}

pub fn take_try_rules() -> HashMap<String, Decls> {
    TRY_RULES.lock().unwrap().take().unwrap_or_default()
}

/// Объявления правила `@position-try` по имени (`--x`), копией.
pub fn try_rule(name: &str) -> Option<Decls> {
    TRY_RULES.lock().unwrap().as_ref()?.get(name).cloned()
}
