//! Функции anchor()/anchor-size() в длинах: стороны, размеры, пул выражений и их разбор (в том числе внутри min/max/calc).

use super::*;

/// Сторона якоря в `anchor()` (css-anchor-position-1 §anchor-fn);
/// `center` разбирается как `Pct(0.5)` — спека так и определяет.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnchorSide {
    Inside,
    Outside,
    Top,
    Right,
    Bottom,
    Left,
    Start,
    End,
    SelfStart,
    SelfEnd,
    Pct(f32),
}

/// Мера якоря в `anchor-size()` (css-anchor-position-1 §anchor-size-fn);
/// `Implicit` — ключевое слово опущено: берётся ось свойства.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnchorSize {
    Implicit,
    Width,
    Height,
    Block,
    Inline,
    SelfBlock,
    SelfInline,
}

/// Разобранная `anchor()` или `anchor-size()`: имя (нет — якорь по умолчанию
/// из `position-anchor`), сторона, запасное значение и довесок в точках из
/// `calc(anchor(…) + 10px)`. `size` задан — это `anchor-size()`, `side` тогда
/// не читается.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorFn {
    pub name: Option<String>,
    pub side: AnchorSide,
    pub size: Option<AnchorSize>,
    pub fallback: Option<Len>,
    pub add: f32,
    /// `min(anchor(…), anchor(…), …)` / `max(…)` (css-values-4
    /// §comparison-functions): остальные доводы; сама запись — первый.
    /// Пусто — обычная `anchor()`.
    pub alts: Vec<AnchorFn>,
    /// `max()` при непустых `alts`, иначе `min()`.
    pub max: bool,
}

/// Арена `anchor()` — тем же порядком, что `CALC_POOL`: append-only, `Len`
/// несёт индекс, тело живёт здесь.
static ANCHOR_POOL: std::sync::Mutex<Vec<AnchorFn>> = std::sync::Mutex::new(Vec::new());

pub fn anchor_store(f: AnchorFn) -> u32 {
    let mut pool = ANCHOR_POOL.lock().unwrap();
    pool.push(f);
    (pool.len() - 1) as u32
}

pub fn anchor_get(i: u32) -> Option<AnchorFn> {
    ANCHOR_POOL.lock().unwrap().get(i as usize).cloned()
}

/// Тело `anchor(...)` без скобок: `[<name> || <side>] , <fallback>?`.
/// Запятая ищется на верхнем уровне — запасным значением бывает вложенный
/// `anchor(--a1 bottom)` (`position-anchor-none-pseudo-element-named`).
pub(super) fn parse_anchor(inner: &str, add: f32, size_fn: bool) -> Option<Len> {
    let mut depth = 0i32;
    let mut cut = None;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                cut = Some(i);
                break;
            }
            _ => {}
        }
    }
    let (head, tail) = match cut {
        Some(i) => (&inner[..i], Some(inner[i + 1..].trim())),
        None => (inner, None),
    };
    let mut name = None;
    let mut side = None;
    let mut size = None;
    for tok in head.split_whitespace() {
        if tok.starts_with("--") {
            name = Some(tok.to_string());
            continue;
        }
        let t = tok.to_ascii_lowercase();
        // `anchor-size()`: вместо стороны — мера (§anchor-size-fn).
        if size_fn {
            size = Some(match t.as_str() {
                "width" => AnchorSize::Width,
                "height" => AnchorSize::Height,
                "block" => AnchorSize::Block,
                "inline" => AnchorSize::Inline,
                "self-block" => AnchorSize::SelfBlock,
                "self-inline" => AnchorSize::SelfInline,
                _ => return None,
            });
            continue;
        }
        side = Some(match t.as_str() {
            "inside" => AnchorSide::Inside,
            "outside" => AnchorSide::Outside,
            "top" => AnchorSide::Top,
            "right" => AnchorSide::Right,
            "bottom" => AnchorSide::Bottom,
            "left" => AnchorSide::Left,
            "start" => AnchorSide::Start,
            "end" => AnchorSide::End,
            "self-start" => AnchorSide::SelfStart,
            "self-end" => AnchorSide::SelfEnd,
            "center" => AnchorSide::Pct(0.5),
            _ => AnchorSide::Pct(css_number(t.strip_suffix('%')?)? / 100.0),
        });
    }
    let fallback = match tail {
        Some(t) if !t.is_empty() => Some(Len::parse(t)?),
        _ => None,
    };
    Some(Len::Anchor(anchor_store(AnchorFn {
        name,
        side: if size_fn { AnchorSide::Inside } else { side? },
        size: size_fn.then(|| size.unwrap_or(AnchorSize::Implicit)),
        fallback,
        add,
        alts: Vec::new(),
        max: false,
    })))
}

/// `min(anchor(…), …)` / `max(anchor(…), …)` во вставке абсолюта
/// (`anchor-in-css-min-max-function`: `top: min(anchor(--a1 bottom),
/// anchor(--a2 bottom), anchor(--a3 top))`). Доводы режутся запятыми
/// верхнего уровня, каждый обязан быть `anchor()` (в том числе
/// `calc(anchor() ± px)`); `anchor-size()` и смесь с длинами — негодны:
/// крайний член сравнивается с краем якоря только в одной системе отсчёта.
pub(super) fn parse_anchor_minmax(s: &str, max: bool) -> Option<Len> {
    let inner = &s[4..s.len() - 1];
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    args.push(inner[start..].trim());
    let mut fns = Vec::with_capacity(args.len());
    for a in args {
        let Len::Anchor(i) = Len::parse(a)? else {
            return None;
        };
        let f = anchor_get(i)?;
        if f.size.is_some() || !f.alts.is_empty() {
            return None;
        }
        fns.push(f);
    }
    let mut first = fns.remove(0);
    first.alts = fns;
    first.max = max;
    Some(Len::Anchor(anchor_store(first)))
}

/// `calc(anchor(…) ± <length>)`: функция вырезается и заменяется нулём,
/// остаток обязан свернуться в точки — он и становится довеском. Доля или
/// шрифтовая единица рядом с якорем честно не разбирается (запись падает).
pub(super) fn parse_anchor_calc(s: &str) -> Option<Len> {
    let lower = s.to_ascii_lowercase();
    // `anchor-size(` не содержит подстроки `anchor(` — ветки не путаются.
    let (at, head, size_fn) = match lower.find("anchor-size(") {
        Some(i) => (i, 12, true),
        None => (lower.find("anchor(")?, 7, false),
    };
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in s[at..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(at + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let body = &s[at + head..end];
    let rest = format!("{}0px{}", &s[..at], &s[end + 1..]);
    let add = match Len::parse(&rest)? {
        Len::Px(v) => v,
        _ => return None,
    };
    parse_anchor(body, add, size_fn)
}
