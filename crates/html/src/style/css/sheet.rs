//! Таблица стилей: правила, каскадные слои, пространства имён.

use crate::style::css::*;
use std::collections::HashMap;

mod rules;
use rules::sheet_rules;

thread_local! {
    /// Реестр слоёв документа: полное имя → путь индексов (порядок —
    /// по ПЕРВОМУ объявлению, css-cascade-5 §6.4.3).
    static LAYERS: std::cell::RefCell<HashMap<String, Vec<u32>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Следующий индекс ребёнка у каждого родителя (ключ — полное имя).
    static LAYER_NEXT: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
    /// Текущий слой разбора: полное имя и путь.
    static LAYER_NOW: std::cell::RefCell<(String, Vec<u32>)> =
        const { std::cell::RefCell::new((String::new(), Vec::new())) };
    static LAYER_ANON: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Сбросить реестр слоёв — на входе разбора документа.
pub fn reset_layers() {
    crate::text::fonts::alternates::reset();
    LAYERS.with(|l| l.borrow_mut().clear());
    LAYER_NEXT.with(|l| l.borrow_mut().clear());
    LAYER_NOW.with(|l| *l.borrow_mut() = (String::new(), Vec::new()));
    LAYER_ANON.with(|c| c.set(0));
}

/// Путь слоя по имени (возможно с точками) внутри текущего; регистрирует
/// незнакомые звенья. Пустое имя — анонимный слой, всегда новый.
fn layer_enter_path(name: &str) -> (String, Vec<u32>) {
    let (mut full, mut path) = LAYER_NOW.with(|l| l.borrow().clone());
    let segs: Vec<String> = if name.trim().is_empty() {
        let n = LAYER_ANON.with(|c| {
            let v = c.get();
            c.set(v + 1);
            v
        });
        vec![format!("\u{1}anon{n}")]
    } else {
        name.split('.').map(|s| s.trim().to_string()).collect()
    };
    for seg in segs {
        let child = if full.is_empty() {
            seg
        } else {
            format!("{full}.{seg}")
        };
        let known = LAYERS.with(|l| l.borrow().get(&child).cloned());
        path = match known {
            Some(p) => p,
            None => {
                let idx = LAYER_NEXT.with(|n| {
                    let mut n = n.borrow_mut();
                    let e = n.entry(full.clone()).or_insert(0);
                    let v = *e;
                    *e += 1;
                    v
                });
                let mut p = path.clone();
                p.push(idx);
                LAYERS.with(|l| l.borrow_mut().insert(child.clone(), p.clone()));
                p
            }
        };
        full = child;
    }
    (full, path)
}

/// Путь для правил ТЕКУЩЕГО места разбора (с хвостом «собственные»).
fn layer_of_rules() -> Vec<u32> {
    let mut p = LAYER_NOW.with(|l| l.borrow().1.clone());
    p.push(u32::MAX);
    p
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    parse_stylesheet_media(css, Media::default())
}

thread_local! {
    /// Префиксы `@namespace` разбираемой таблицы; `None` — разбор идёт не из
    /// таблицы, и префиксы не проверяются (прежнее поведение).
    static NS_PREFIXES: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Снимает префиксы, когда верхний вызов разбора таблицы кончился (и при панике).
struct NsScope(bool);

impl Drop for NsScope {
    fn drop(&mut self) {
        if self.0 {
            NS_PREFIXES.with(|n| *n.borrow_mut() = None);
        }
    }
}

/// Объявлен ли префикс. Пустой (`|div`) и `*` объявлены всегда.
pub(super) fn ns_declared(ns: &str) -> bool {
    ns.is_empty()
        || ns == "*"
        || NS_PREFIXES.with(|n| n.borrow().as_ref().is_none_or(|s| s.contains(ns)))
}

/// Префиксы действительных `@namespace` (css-namespaces-3 §2): «must follow
/// all @charset and @import rules and precede all other non-ignored at-rules
/// and style rules … Otherwise the @namespace rule is invalid». Даже пустой
/// `@media {}` или `@supports (…) {}` закрывает пролог (`at-media-003`,
/// `at-supports-045`).
fn declared_prefixes(css: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    loop {
        rest = stylesheet_tokens::start(rest);
        let Some((piece, tail)) = next_piece(rest) else {
            break;
        };
        rest = tail;
        let Piece::Statement { head } = piece else {
            break;
        };
        let low = head.trim().to_ascii_lowercase();
        if low.starts_with("@charset") || low.starts_with("@import") || low.starts_with("@layer") {
            continue;
        }
        let Some(r) = low.strip_prefix("@namespace") else {
            break;
        };
        let first = r.split_whitespace().next().unwrap_or("");
        if !first.is_empty()
            && !first.starts_with('"')
            && !first.starts_with('\'')
            && !first.starts_with("url(")
        {
            set.insert(first.to_string());
        }
    }
    set
}

/// `attr(ns|name)` с необъявленным префиксом делает объявление негодным —
/// для оракула `@supports` (`at-supports-namespace-001`: `attr(y|href)`).
pub(super) fn attr_prefixes_declared(v: &str) -> bool {
    let low = v.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = low[from..].find("attr(") {
        let open = from + at + 5;
        let arg = low[open..]
            .trim_start()
            .split(|ch: char| ch == ')' || ch == ',' || ch.is_whitespace())
            .next()
            .unwrap_or("");
        if let Some((ns, _)) = arg.split_once('|')
            && !ns_declared(ns)
        {
            return false;
        }
        from = open;
    }
    true
}

/// То же, но с известными условиями окружения.
pub fn parse_stylesheet_media(css: &str, media: Media) -> Vec<Rule> {
    // Префиксы считает ВЕРХНИЙ вызов: вложенные группы (`@media`,
    // `@supports`, `@layer`) разбираются тем же входом рекурсивно и видят
    // пролог своей таблицы.
    let top = NS_PREFIXES.with(|n| n.borrow().is_none());
    if top {
        let declared = declared_prefixes(css);
        NS_PREFIXES.with(|n| *n.borrow_mut() = Some(declared));
    }
    let _scope = NsScope(top);
    sheet_rules(css, media, top)
}
