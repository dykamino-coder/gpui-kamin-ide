//! Сопоставление селекторов: реляционный псевдокласс `:has()`. Аргумент
//! разбирается в `HasArg` с якорем-стражем у самого левого компаунда; отдельный
//! проход до обхода (`mark_has`) решает для каждого элемента и аргумента, есть
//! ли предмет в поддереве или среди последующих братьев, и складывает отметки
//! в индекс дерева — сопоставление потом лишь читает их по хешу.
//! Вложенный `:has()` запрещён, поэтому циклов нет; `shadow_marks` — то же для
//! хоста по его дереву теней.

mod candidates;
mod collect;
pub(in crate::style::select) mod index;
mod marks;
pub(in crate::style::select) mod scope;
mod storage;
#[cfg(test)]
mod tests;
mod walk;
pub use collect::retain_valid_rules;

use crate::budget::admission::Error;
use crate::dom::tree_index::TreeIndex;
use crate::style::css::Selector;
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs};
use markup5ever_rcdom::Handle;
use walk::Walk;

/// Хеш одного аргумента `:has(...)` - ключ отметки.
pub(super) fn has_id(arg: &str) -> u64 {
    crate::style::css::has_argument_id(arg)
}

/// Разобранный аргумент `:has()`: части списка, каждая с ведущим
/// комбинатором, уже пришитым якорем-стражем к самому левому компаунду.
pub struct HasArg {
    pub id: u64,
    /// (свой ли уровень: `+`/`~` - братья, иначе поддерево; селектор).
    pub parts: Vec<(bool, Selector)>,
}

/// Имя атрибута-стража: NUL из html5ever не приходит, коллизий нет.
pub(super) const HAS_SENTINEL: &str = "\u{0}scope";

/// Пришить якорь-стража к самому левому компаунду цепочки.
fn attach_anchor(mut sel: &mut Selector, lead: char) {
    loop {
        if sel.prev.is_some() {
            sel = &mut sel.prev.as_mut().expect("previous compound").0;
        } else if sel.ancestor.is_some() {
            sel = &mut sel.ancestor.as_mut().expect("ancestor compound").0;
        } else {
            break;
        }
    }
    let sentinel = Selector {
        arguments: vec![],
        tag: None,
        tag_namespace: Default::default(),
        argument_namespace: Default::default(),
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
pub fn parse_has_arg(arg: &str) -> Option<HasArg> {
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

/// Один индекс области используется всеми якорями; ledger оплачивает реальные кандидаты.
pub fn mark_has(
    handle: &Handle,
    args: &[HasArg],
    path: &mut Vec<Ancestor>,
    tree: &mut TreeIndex,
) -> Result<(), Error> {
    if args.is_empty() {
        return Ok(());
    }
    let scope = scope::Scope::new(handle, path, tree)?;
    let admission = tree.admission();
    let mut found = marks::Marks::new(scope.records.len(), &admission)?;
    for arg in args {
        found.argument();
        for (sibling, sel) in &arg.parts {
            found.alternative()?;
            if super::sets::eligible(sel) {
                let anchors = super::sets::anchors(&scope, sel, tree)?;
                for i in anchors.ones() {
                    found.propagate(i, &admission)?;
                }
                continue;
            }
            let candidates = candidates::candidates(&scope, sel, tree)?;
            if let Some((relation, subject)) = candidates::simple(sel) {
                candidates::mark_simple(&scope, relation, &subject, &candidates, &mut found, tree)?;
            } else {
                candidates::mark_complex(&scope, *sibling, sel, &candidates, &mut found, tree)?;
            }
        }
        for &i in found.touched.iter() {
            admission.work(1)?;
            let record = &scope.records[i];
            tree.insert_has_mark(&record.handle, arg.id)?;
        }
    }
    Ok(())
}

/// Хост — единственный внешний якорь; братья корня тени ему не принадлежат.
pub fn shadow_marks(
    root: &Handle,
    args: &[HasArg],
    host: &Ancestor,
    tree: &TreeIndex,
) -> Result<Vec<u64>, Error> {
    let scope = scope::Scope::new(root, std::slice::from_ref(host), tree)?;
    let _overlay = tree.matching.anchor(host.key.clone());
    let mut marks = Vec::new();
    for arg in args {
        for (sibling, sel) in &arg.parts {
            if *sibling {
                continue;
            }
            let candidates = candidates::candidates(&scope, sel, tree)?;
            let mut hit = false;
            for &i in candidates.iter() {
                if scope.match_at(sel, i, tree)? {
                    hit = true;
                    break;
                }
            }
            if hit {
                marks.push(arg.id);
                break;
            }
        }
    }
    Ok(marks)
}
