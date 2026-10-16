//! `:has()`.
// owner: B

mod collect;
pub(crate) use collect::collect_has_args;

use crate::style::css::Selector;
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs, census_of};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;

thread_local! {
    pub(crate) static HAS_MARKS: std::cell::RefCell<HashMap<usize, Vec<u64>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Хеш одного аргумента `:has(...)` - ключ отметки.
pub(super) fn has_id(arg: &str) -> u64 {
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
const HAS_SENTINEL: &str = "\u{0}scope";

/// Пришить якорь-стража к самому левому компаунду цепочки.
fn attach_anchor(sel: &mut Selector, lead: char) {
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

/// Паспорт якоря с пришитым атрибутом-стражем.
fn with_sentinel(a: &Ancestor) -> Ancestor {
    let mut out = a.clone();
    out.attrs.push((HAS_SENTINEL.to_string(), String::new()));
    out
}

/// Есть ли в СТРОГОМ поддереве узла предмет селектора с якорем в `path`.
fn has_in_subtree(handle: &Handle, sel: &Selector, path: &mut Vec<Ancestor>) -> bool {
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

/// Relational matches anchored on the host but searching its shadow tree.
pub(crate) fn shadow_marks(root: &Handle, args: &[HasArg], host: &Ancestor) -> Vec<u64> {
    let mut path = vec![with_sentinel(host)];
    args.iter()
        .filter(|arg| {
            arg.parts
                .iter()
                .any(|(sibling, sel)| !sibling && has_in_subtree(root, sel, &mut path))
        })
        .map(|arg| arg.id)
        .collect()
}
