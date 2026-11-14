//! Регрессии полноты индекса и порядка кандидатов.

use super::*;
use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Sibs, census_of, matching::matches};
use html5ever::interface::TreeSink;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

fn visit(parent: &Handle, path: &[Ancestor], tree: &TreeIndex, rules: &[Rule], index: &RuleIndex) {
    let children = parent.children.borrow();
    let (_, all) = census_of(&children, tree);
    let all = Rc::new(all);
    for (pos, child) in children.iter().enumerate() {
        let mut me = all[pos].clone();
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem: true,
            rc: Some(&all),
        };
        let full: Vec<_> = rules
            .iter()
            .enumerate()
            .filter_map(|(i, r)| matches(&r.sel, &me, path, sibs).then_some(i))
            .collect();
        let indexed: Vec<_> = index
            .candidates(&me, path)
            .iter()
            .copied()
            .filter(|&i| matches(&rules[i].sel, &me, path, sibs))
            .collect();
        assert_eq!(indexed, full, "{} {:?}", me.tag, me.id);
        me.peers = Some((all.clone(), pos));
        me.ancestor_bloom = Some(
            path.last()
                .and_then(|a| a.ancestor_bloom)
                .unwrap_or_default()
                .with_node(&me),
        );
        let mut next = path.to_vec();
        next.push(me);
        visit(child, &next, tree, rules, index);
    }
}

#[test]
fn indexed_matches_equal_full_scan_through_ancestor_and_sibling_chains() {
    let dom = crate::dom::xml::xml_dom("<r class='top'><a id='first' class='a a'/><b class='b' data-x='yes'><x id='leaf' class='leaf'/><x class='other'/></b><c/></r>").unwrap();
    let mut tree = TreeIndex::new(
        crate::values::DocumentValues::detached(Default::default()),
        true,
    );
    let args: Vec<_> = ["*", "x", "y", ".absent", "+ b", "~ c"]
        .into_iter()
        .map(|raw| crate::style::select::has::parse_has_arg(raw).unwrap())
        .collect();
    crate::style::select::has::mark_has(&dom.document, &args, &mut vec![], &mut tree).unwrap();
    let rules: Vec<_> = [
        "*",
        "#leaf",
        ".leaf",
        "x",
        "[data-x]",
        "[data-x=no]",
        ".top .leaf",
        "#absent .leaf",
        "#first + b .leaf",
        "a ~ b > x",
        "r > b + c",
        ":is(.leaf,.other)",
        ":where(x)",
        ":not(.missing)",
        "x:nth-child(2 of x)",
        "b:has(x) .leaf",
        "r:has(*) b:has(x) .leaf",
        "a:has(+ b) + b:has(x) > x",
        "a:has(~ c) ~ b > x:nth-child(2)",
        "r:has(.absent) > b .leaf",
        "b:has(y) .leaf",
        "b:has(x):not(.missing) .leaf",
    ]
    .into_iter()
    .enumerate()
    .map(|(order, raw)| Rule {
        sel: Selector::parse(raw).unwrap(),
        decls: Default::default(),
        order,
        origin: crate::style::css::Origin::Author,
        layer: vec![u32::MAX],
        link: crate::style::css::LinkMatch::Any,
    })
    .collect();
    let index = RuleIndex::new(&rules, &Admission::default()).unwrap();
    visit(&dom.document, &[], &tree, &rules, &index);
    let rules: Vec<_> = rules.iter().cycle().take(180).cloned().collect();
    let index = RuleIndex::new(&rules, &Admission::default()).unwrap();
    for mutation in [false, true] {
        let mut prepared = TreeIndex::new(
            crate::values::DocumentValues::detached(Default::default()),
            true,
        );
        index
            .prepare_document(&dom.document, &rules, &args, &mut prepared, mutation)
            .unwrap();
        assert!(prepared.has_marks == tree.has_marks, "prepared :has marks");
        assert_eq!(index.bulk.borrow().is_some(), !mutation);
        visit(&dom.document, &[], &tree, &rules, &index);
    }
}

#[test]
fn edits_and_tree_changes_rebuild_candidates_without_stale_rows() {
    let dom =
        crate::dom::xml::xml_dom("<r><a class='on'/><b data-x='yes'><x/></b><c/></r>").unwrap();
    let root = dom.document.children.borrow()[0].clone();
    let a = root.children.borrow()[0].clone();
    let b = root.children.borrow()[1].clone();
    let rules: Vec<_> = [".on ~ b > x", "b[data-x=yes] > x", "r > c:last-child"]
        .into_iter()
        .cycle()
        .take(72)
        .enumerate()
        .map(|(order, raw)| Rule {
            sel: Selector::parse(raw).unwrap(),
            decls: Default::default(),
            order,
            origin: crate::style::css::Origin::Author,
            layer: vec![u32::MAX],
            link: crate::style::css::LinkMatch::Any,
        })
        .collect();
    let index = RuleIndex::new(&rules, &Admission::default()).unwrap();
    for step in 0..5 {
        match step {
            1 | 2 => {
                let handle = if step == 1 { &a } else { &b };
                let markup5ever_rcdom::NodeData::Element { attrs, .. } = &handle.data else {
                    unreachable!();
                };
                attrs.borrow_mut()[0].value = "off".into();
            }
            3 => {
                let child = b.children.borrow()[0].clone();
                dom.remove_from_parent(&child);
                dom.append(&root, html5ever::interface::NodeOrText::AppendNode(child));
            }
            4 => {
                let child = root.children.borrow().last().unwrap().clone();
                dom.remove_from_parent(&child);
            }
            _ => {}
        }
        let mut tree = TreeIndex::new(
            crate::values::DocumentValues::detached(Default::default()),
            true,
        );
        for mutation in [false, true] {
            index
                .prepare_document(&dom.document, &rules, &[], &mut tree, mutation)
                .unwrap();
            assert_eq!(index.bulk.borrow().is_some(), !mutation);
            visit(&dom.document, &[], &tree, &rules, &index);
        }
    }
}
