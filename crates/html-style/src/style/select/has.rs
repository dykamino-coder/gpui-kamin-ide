//! Сопоставление селекторов: реляционный псевдокласс `:has()`. Аргумент
//! разбирается в `HasArg` с якорем-стражем у самого левого компаунда; отдельный
//! проход до обхода (`mark_has`) решает для каждого элемента и аргумента, есть
//! ли предмет в поддереве или среди последующих братьев, и складывает отметки
//! в индекс дерева — сопоставление потом лишь читает их по хешу.
//! Вложенный `:has()` запрещён, поэтому циклов нет; `shadow_marks` — то же для
//! хоста по его дереву теней.

mod candidates;
mod collect;
mod index;
mod marks;
mod scope;
mod storage;
mod walk;
#[cfg(test)]
mod tests {

    use super::*;

    fn marked(xml: &str, raw: &str) -> Vec<String> {
        let dom = crate::dom::xml::xml_dom(xml).unwrap();
        let mut tree = TreeIndex::new(
            crate::values::DocumentValues::detached(Default::default()),
            true,
        );
        let arg = parse_has_arg(raw).unwrap();
        mark_has(&dom.document, &[arg], &mut vec![], &mut tree).unwrap();
        assert!(!tree.matching.has_anchor());
        let scope = scope::Scope::new(&dom.document, &[], &tree).unwrap();
        scope
            .records
            .iter()
            .filter_map(|r| {
                let node = &r.all[r.pos];
                (!tree.has_marks_of(&r.handle).is_empty())
                    .then(|| node.id.clone())
                    .flatten()
            })
            .collect()
    }

    #[test]
    fn relative_relations_keep_their_bounds_and_sibling_identity() {
        let xml = "<root id='r'><a id='a'><x/></a><b id='b'><x/></b><c id='c'/></root>";
        for (arg, expected) in [
            ("x", vec!["r", "a", "b"]),
            ("> x", vec!["a", "b"]),
            ("+ b", vec!["a"]),
            ("~ c", vec!["a", "b"]),
            ("+ b > x", vec!["a"]),
            ("> a + b > x", vec!["r"]),
            ("> x + b", vec![]),
            ("+ c", vec!["b"]),
        ] {
            assert_eq!(marked(xml, arg), expected, "{arg}");
        }
    }

    #[test]
    fn alternative_results_do_not_stop_another_relatives_propagation() {
        assert_eq!(
            marked(
                "<r id='r'><a id='a'><z/><b id='b'><x/></b></a></r>",
                "> z, x"
            ),
            ["r", "a", "b"]
        );
    }

    #[test]
    fn filtered_sibling_positions_use_original_siblings_under_overlay() {
        let xml = "<r id='r'><a id='a'/><b class='keep'/><b class='keep'><x/></b><c/></r>";
        assert_eq!(marked(xml, "+ b:nth-child(1 of .keep) + b > x"), ["a"]);
        assert!(marked(xml, "+ b:nth-child(2 of .keep) + b > x").is_empty());
    }

    #[test]
    fn shadow_host_overlay_is_scoped_and_cannot_claim_siblings() {
        let dom = crate::dom::xml::xml_dom("<host><x/></host>").unwrap();
        let tree = TreeIndex::new(
            crate::values::DocumentValues::detached(Default::default()),
            true,
        );
        let host = dom.document.children.borrow()[0].clone();
        let passport = super::super::ancestor_of(&host, Default::default(), &tree).unwrap();
        let yes = parse_has_arg("> x").unwrap();
        let no = parse_has_arg("+ x").unwrap();
        assert_eq!(
            shadow_marks(&host, &[yes, no], &passport, &tree).unwrap(),
            [has_id("> x")]
        );
        assert!(!tree.matching.has_anchor());
    }
}
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
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    arg.hash(&mut h);
    h.finish()
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
