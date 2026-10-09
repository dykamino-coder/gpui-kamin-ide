//! `:has()`.
// owner: B

use crate::style::css::Selector;
use crate::style::select::{Ancestor, Sibs, census_of};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use crate::style::select::matching::matches;

thread_local! {
    pub(crate) static HAS_MARKS: std::cell::RefCell<HashMap<usize, Vec<u64>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Хеш одного аргумента `:has(...)` - ключ отметки.
pub(crate) fn has_id(arg: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    arg.hash(&mut h);
    h.finish()
}

pub(crate) fn has_marks_of(handle: &Handle) -> Vec<u64> {
    let key = std::rc::Rc::as_ptr(handle) as usize;
    HAS_MARKS.with(|m| m.borrow().get(&key).cloned().unwrap_or_default())
}

/// Разобранный аргумент `:has()`: части списка, каждая с ведущим
/// комбинатором, уже пришитым якорем-стражем к самому левому компаунду.
pub(crate) struct HasArg {
    pub(crate) id: u64,
    /// (свой ли уровень: `+`/`~` - братья, иначе поддерево; селектор).
    pub(crate) parts: Vec<(bool, Selector)>,
}

/// Имя атрибута-стража: NUL из html5ever не приходит, коллизий нет.
pub(crate) const HAS_SENTINEL: &str = "\u{0}scope";

/// Пришить якорь-стража к самому левому компаунду цепочки.
pub(crate) fn attach_anchor(sel: &mut Selector, lead: char) {
    if let Some(p) = sel.prev.as_mut() {
        return attach_anchor(&mut p.0, lead);
    }
    if let Some(a) = sel.ancestor.as_mut() {
        return attach_anchor(&mut a.0, lead);
    }
    let sentinel = Selector {
        tag: None,
        id: None,
        classes: vec![],
        attrs: vec![crate::style::css::AttrSel {
            name: HAS_SENTINEL.to_string(),
            op: None,
            ci: false,
        }],
        pseudo: None,
        also: vec![],
        ancestor: None,
        prev: None,
        universal: false,
    };
    match lead {
        '>' => sel.ancestor = Some(Box::new((sentinel, true))),
        '+' => sel.prev = Some(Box::new((sentinel, true))),
        '~' => sel.prev = Some(Box::new((sentinel, false))),
        _ => sel.ancestor = Some(Box::new((sentinel, false))),
    }
}

/// Разобрать аргумент `:has(...)`. Список НЕпрощающий (селекторы-4):
/// битая часть делает недействительным весь аргумент - `None`.
pub(crate) fn parse_has_arg(arg: &str) -> Option<HasArg> {
    let mut parts = vec![];
    for one in crate::style::css::split_selector_list(arg) {
        let one = one.trim();
        let (lead, rest) = match one.chars().next()? {
            c @ ('>' | '+' | '~') => (c, &one[1..]),
            _ => (' ', one),
        };
        let mut sel = Selector::parse(rest)?;
        attach_anchor(&mut sel, lead);
        parts.push((matches!(lead, '+' | '~'), sel));
    }
    (!parts.is_empty()).then(|| HasArg {
        id: has_id(arg),
        parts,
    })
}

/// Собрать строки-аргументы всех `:has()` селектора, включая вложенные в
/// `:not()` и `of S`. Возвращает false, если встретился НЕдопустимый -
/// вложенный `:has` (правило целиком недействительно, спека: cannot be
/// nested).
pub(crate) fn collect_has_args(sel: &Selector, out: &mut Vec<String>) -> bool {
    for p in sel.pseudo.iter().chain(sel.also.iter()) {
        if let Some(rest) = p.strip_prefix("has(").and_then(|r| r.strip_suffix(')')) {
            if rest.contains("has(") {
                return false;
            }
            if !out.iter().any(|a| a == rest) {
                out.push(rest.to_string());
            }
        } else if let Some(inner) = p.strip_prefix("not(").and_then(|r| r.strip_suffix(')')) {
            if let Some(s) = Selector::parse(inner)
                && !collect_has_args(&s, out)
            {
                return false;
            }
        } else if let Some((_, arg)) = p.split_once('(')
            && let Some(arg) = arg.strip_suffix(')')
            && let Some((_, list)) = crate::style::css::nth_of_parts(arg)
        {
            for s in &list {
                if !collect_has_args(s, out) {
                    return false;
                }
            }
        }
    }
    if let Some(a) = &sel.ancestor
        && !collect_has_args(&a.0, out)
    {
        return false;
    }
    if let Some(pr) = &sel.prev
        && !collect_has_args(&pr.0, out)
    {
        return false;
    }
    true
}

/// Паспорт якоря с пришитым атрибутом-стражем.
pub(crate) fn with_sentinel(a: &Ancestor) -> Ancestor {
    let mut out = a.clone();
    out.attrs.push((HAS_SENTINEL.to_string(), String::new()));
    out
}

/// Есть ли в СТРОГОМ поддереве узла предмет селектора с якорем в `path`.
pub(crate) fn has_in_subtree(handle: &Handle, sel: &Selector, path: &mut Vec<Ancestor>) -> bool {
    let children = handle.children.borrow();
    let (spots, all) = census_of(&children);
    let mut pos = 0usize;
    for (child, spot) in children.iter().zip(&spots) {
        if spot.index == 0 {
            continue;
        }
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem: true,
            rc: None,
        };
        if matches(sel, &all[pos], path, sibs) {
            return true;
        }
        path.push(all[pos].clone());
        let hit = has_in_subtree(child, sel, path);
        path.pop();
        if hit {
            return true;
        }
        pos += 1;
    }
    false
}

/// Проход-разметчик `:has()`: на каждый элемент и каждый аргумент решает,
/// найдётся ли предмет - в поддереве либо среди последующих братьев - и
/// складывает отметку. Вложенный `:has` запрещён, поэтому матчи внутри
/// аргумента в отметки не заглядывают и циклов нет.
pub(crate) fn mark_has(handle: &Handle, args: &[HasArg], path: &mut Vec<Ancestor>) {
    let children = handle.children.borrow();
    let (spots, all) = census_of(&children);
    let mut pos = 0usize;
    for (child, spot) in children.iter().zip(&spots) {
        if spot.index == 0 {
            continue;
        }
        let mut marks: Vec<u64> = vec![];
        for arg in args {
            let hit = arg.parts.iter().any(|(sibling, sel)| {
                if *sibling {
                    let mut peers = all.clone();
                    peers[pos] = with_sentinel(&all[pos]);
                    (pos + 1..peers.len()).any(|i| {
                        let sibs = Sibs {
                            all: &peers,
                            pos: i,
                            is_elem: true,
                            rc: None,
                        };
                        matches(sel, &peers[i], path, sibs)
                    })
                } else {
                    path.push(with_sentinel(&all[pos]));
                    let hit = has_in_subtree(child, sel, path);
                    path.pop();
                    hit
                }
            });
            if hit {
                marks.push(arg.id);
            }
        }
        if !marks.is_empty() {
            let key = std::rc::Rc::as_ptr(child) as usize;
            HAS_MARKS.with(|m| m.borrow_mut().insert(key, marks));
        }
        path.push(all[pos].clone());
        mark_has(child, args, path);
        path.pop();
        pos += 1;
    }
}
