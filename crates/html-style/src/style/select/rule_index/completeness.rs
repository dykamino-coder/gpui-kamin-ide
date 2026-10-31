//! Регрессии полноты индекса и порядка кандидатов.

use super::*;
use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Sibs, census_of, matching::matches};
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
    let args = [crate::style::select::has::parse_has_arg("x").unwrap()];
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
}
