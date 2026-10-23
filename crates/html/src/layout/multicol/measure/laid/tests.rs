//! Геометрия копий не требует предварительных высот и сохраняет поля потока.

use super::*;

fn root(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored div")
}

#[test]
fn copies_export_collapsed_margins_before_root_layout() {
    let tree = crate::dom::parse(
        "<div><div style='margin-top:3px'><div style='height:20px;margin-top:7px'>x</div></div></div>",
        "",
    );
    let measured = RefCell::default();
    let kids = children(
        root(&tree),
        &BoxStyle::default(),
        tree.values(),
        100.0,
        &measured,
    )
    .expect("all copies are recorded");
    assert_eq!(kids[0].1.margin_top, 7.0);
    assert_eq!(kids[0].1.height, 0.0);
    let Node::Element(inner) = &kids[0].0.children[0] else {
        panic!("inner")
    };
    assert_eq!(
        inner.style.computed().margin.top,
        Some(crate::style::values::value::Len::Px(0.0))
    );
    assert_eq!(*measured.borrow(), vec![(kids[0].0.node_id, 100.0)]);
}

#[test]
fn sibling_with_its_own_fragment_state_keeps_the_whole_preplan() {
    for special in [
        "<div style='columns:2'>nested</div>",
        "<div style='box-decoration-break:clone'>clone</div>",
        "<div style='text-box-trim:trim-both'>trim</div>",
        "<div><div style='position:absolute'>parallel</div></div>",
        "<div style='position:absolute'>parallel sibling</div>",
    ] {
        let tree = crate::dom::parse(&format!("<div><div>ordinary</div>{special}</div>"), "");
        let measured = RefCell::default();
        assert!(
            children(
                root(&tree),
                &BoxStyle::default(),
                tree.values(),
                100.0,
                &measured
            )
            .is_none(),
            "{special}"
        );
        assert!(measured.borrow().is_empty());
    }
}

#[test]
fn pending_tree_waits_for_width_without_resolving_percentages() {
    let tree = crate::dom::parse(
        "<div><div style='margin-top:10%;height:20px'>x</div></div>",
        "",
    );
    let host = root(&tree);
    let pending = pending(host, &BoxStyle::default(), tree.values()).expect("deferred tree");
    assert_eq!(
        pending[0].0.style.computed().margin.top,
        Some(crate::style::values::value::Len::Pct(0.1))
    );
    let measured = RefCell::default();
    let kids = children(host, &BoxStyle::default(), tree.values(), 120.0, &measured)
        .expect("definite width");
    assert_eq!(kids[0].1.margin_top, 12.0);
    assert_eq!(kids[0].1.height, 0.0);
}

#[test]
fn vertical_copies_export_logical_margins_without_estimating_width() {
    for reverse in [false, true] {
        let mode = if reverse {
            "vertical-rl"
        } else {
            "vertical-lr"
        };
        let tree = crate::dom::parse(
            &format!(
                "<div><div style='writing-mode:{mode};width:20px;margin-left:3px;margin-right:7px'>x</div></div>"
            ),
            "",
        );
        let host = root(&tree);
        let measured = RefCell::default();
        let kids = vertical_children(
            &host,
            &BoxStyle::default(),
            tree.values(),
            100.0,
            reverse,
            &measured,
        )
        .expect("vertical copy");
        assert_eq!(kids[0].1.height, 0.0);
        assert_eq!(kids[0].1.margin_top, if reverse { 7.0 } else { 3.0 });
        assert_eq!(kids[0].1.margin_bottom, if reverse { 3.0 } else { 7.0 });
        assert_eq!(*measured.borrow(), vec![(kids[0].0.node_id, 100.0)]);
    }
}

#[test]
fn full_width_float_converted_to_block_uses_a_measured_copy() {
    let tree = crate::dom::parse(
        "<div><div style='float:left;width:100%;height:20px'>float</div></div>",
        "",
    );
    let host = super::super::block_floats(root(&tree), |_| false, Some(100.0))
        .expect("full-width float is a block in the stack");
    let measured = RefCell::default();
    let kids = children(&host, &BoxStyle::default(), tree.values(), 100.0, &measured)
        .expect("converted float has one ordinary block flow");
    assert!(kids[0].0.marks.float_block);
    assert!(kids[0].0.style.computed().float.is_none());
    assert_eq!(kids[0].1.height, 0.0);
    assert_eq!(*measured.borrow(), vec![(kids[0].0.node_id, 100.0)]);
}

#[test]
fn calc_margins_wait_for_width_and_use_the_same_collapse_basis() {
    let tree = crate::dom::parse(
        "<div><div style='margin:calc(10% + 3px) 0 0'><div style='height:20px;margin:calc(20% + 7px) 0 0'>x</div></div></div>",
        "",
    );
    let host = root(&tree);
    let pending = pending(host, &BoxStyle::default(), tree.values()).expect("deferred calc");
    assert!(matches!(
        pending[0].0.style.computed().margin.top,
        Some(crate::style::values::value::Len::Calc(_))
    ));
    for width in [100.0, 200.0] {
        let measured = RefCell::default();
        let kids = children(host, &BoxStyle::default(), tree.values(), width, &measured)
            .expect("definite calc basis");
        assert_eq!(kids[0].1.margin_top, width * 0.2 + 7.0);
        assert_eq!(kids[0].1.height, 0.0);
        let Node::Element(inner) = &kids[0].0.children[0] else {
            panic!("inner")
        };
        assert_eq!(
            inner.style.computed().margin.top,
            Some(crate::style::values::value::Len::Px(0.0))
        );
    }
}

#[test]
fn vertical_calc_margins_resolve_only_after_inline_size_is_known() {
    for (mode, reverse) in [("vertical-lr", false), ("vertical-rl", true)] {
        let tree = crate::dom::parse(
            &format!(
                "<div><div style='writing-mode:{mode};width:20px;margin-left:calc(10% + 3px);margin-right:calc(20% + 7px)'>x</div></div>"
            ),
            "",
        );
        let host = root(&tree);
        let pending = pending_vertical(host, &BoxStyle::default(), tree.values(), reverse)
            .expect("deferred vertical calc");
        assert!(matches!(
            pending[0].0.style.computed().margin.left,
            Some(crate::style::values::value::Len::Calc(_))
        ));
        let measured = RefCell::default();
        let kids = vertical_children(
            host,
            &BoxStyle::default(),
            tree.values(),
            100.0,
            reverse,
            &measured,
        )
        .expect("definite inline size");
        assert_eq!(kids[0].1.margin_top, if reverse { 27.0 } else { 13.0 });
        assert_eq!(kids[0].1.margin_bottom, if reverse { 13.0 } else { 27.0 });
    }
}

#[test]
fn cross_copy_clearance_keeps_the_existing_f2_preplan() {
    let tree = crate::dom::parse(
        "<div><div><div style='float:left;width:15px;height:250px'></div></div><div style='clear:left'></div></div>",
        "",
    );
    let host = root(&tree);
    let merged = BoxStyle::default();
    let measured = RefCell::default();
    assert!(children(host, &merged, tree.values(), 100.0, &measured).is_none());
    assert!(pending(host, &merged, tree.values()).is_none());
    assert!(
        super::super::native_float::children(host, &merged, tree.values(), 100.0, &measured)
            .is_none()
    );
    assert!(super::super::native_float::pending(host, &merged, tree.values()).is_none());
    assert!(measured.borrow().is_empty());
}
