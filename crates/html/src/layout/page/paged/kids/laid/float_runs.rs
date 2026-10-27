//! Общий BFC печатного float-пробега сохраняет рекордер и адреса роста.
use super::*;
use crate::layout::fragment::{ShapeCx, flow};

#[test]
fn shared_page_float_run_records_clear_block_with_atomic_inline_content() {
    let tree = crate::dom::parse(
        "<div><br style='clear:both'><div style='float:left;height:115px;break-inside:avoid'>1</div><div style='height:115px;clear:both'>A<div style='float:left;height:115px;break-inside:avoid'>2</div><span>X<span style='display:inline-block;height:100%;width:1px'></span><br>Y</span></div></div>",
        "",
    );
    let source = &super::tests::authored(&tree).children;
    let p = group(
        source,
        &BoxStyle::default(),
        384.,
        0.,
        (false, false),
        tree.values(),
    )
    .expect("shared measured float run");
    assert!(p.blocks && p.float_extent);
    let Node::Element(root) = &p.nodes[0] else {
        panic!("shared flow root")
    };
    assert!(!crate::layout::box_kind::own_context(root));
    assert!(flow::regrow_copy(root));
    assert!(!flow::independent_copy(root));
    let watched = super::super::breaks::recorder(
        &p.nodes,
        &p.nodes[0],
        None,
        false,
        p.blocks,
        ShapeCx::COLUMNS,
        tree.values(),
    );
    let blocks = watched.blocks().expect("measured block boundaries");
    assert_eq!(blocks.kids.len(), 1);
    let Node::Element(clear) = &source[2] else {
        panic!("clear block")
    };
    assert_eq!(blocks.kids[0].id, clear.node_id);
    assert_eq!(root.children.len(), source.len());
}

#[test]
fn unrelated_inline_run_does_not_acquire_a_block_copy() {
    let tree = crate::dom::parse("<div><span>one</span><span>two</span></div>", "");
    let source = &super::tests::authored(&tree).children;
    assert!(
        group(
            source,
            &BoxStyle::default(),
            384.,
            0.,
            (false, false),
            tree.values()
        )
        .is_none()
    );
}

#[test]
fn anonymous_float_run_retains_original_percentage_cb_and_forced_end() {
    use crate::layout::options::RenderOpts;
    use crate::style::values::value::Len;
    let tree = crate::dom::parse(
        "<!doctype html><html style='height:100%'><body style='height:100%;margin:0'><div style='height:100%'><br><div style='float:left;height:60%;break-after:page'>1</div><div style='clear:both;height:60%'><div style='float:left;height:100%'>2</div><span style='height:10px'><span style='display:inline-block;width:1px;height:100%'></span></span></div></div></body></html>",
        "",
    );
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (384., 192.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    let (nodes, contexts) = flow::page_percentages(&tree, &opts.root_style(), &opts);
    let source = &super::tests::authored(&nodes).children;
    let height = |n: &Node| match n {
        Node::Element(e) => e.style.used().height,
        _ => panic!("float box"),
    };
    assert_eq!(height(&source[1]), Some(Len::Px(115.200005)));
    let (_, after, _, _, _) = super::super::names::group_break_names(
        "",
        &mut None,
        |n| matches!(n, Node::Element(e) if e.style.used().float.is_some_and(|f| f != 0)),
        &source[..2],
        &source[0],
        tree.values(),
    );
    let p = group(
        &source[..2],
        &BoxStyle::default(),
        384.,
        0.,
        (false, after),
        tree.values(),
    )
    .unwrap();
    let Node::Element(root) = &p.nodes[0] else {
        panic!("measured root")
    };
    assert_eq!(root.style.used().height, None);
    assert_eq!(height(&root.children[1]), height(&source[1]));
    let blocks = flow::blocks::BlockBox::collect(root, ShapeCx::COLUMNS, tree.values());
    assert!(blocks.edge(true, &Default::default()).0);
    let mut resumed = root.clone();
    contexts.refresh(
        &mut resumed,
        crate::layout::fragment::token::Space {
            inline_size: Some(384.),
            block_size: Some(320.),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    assert_eq!(resumed.style.used().height, None);
    assert_eq!(height(&resumed.children[1]), Some(Len::Px(192.)));
    let mut continuation = super::tests::authored(&nodes).clone();
    contexts.refresh(
        &mut continuation,
        crate::layout::fragment::token::Space {
            inline_size: Some(384.),
            block_size: Some(320.),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    assert_eq!(height(&continuation.children[1]), Some(Len::Px(192.)));
    let Node::Element(clear) = &continuation.children[2] else {
        panic!("clear box")
    };
    assert_eq!(height(&clear.children[0]), Some(Len::Px(192.)));
    let Node::Element(span) = &clear.children[1] else {
        panic!("inline ancestor")
    };
    assert_eq!(height(&span.children[0]), Some(Len::Px(192.)));
    let Node::Element(original_clear) = &source[2] else {
        panic!("clear box")
    };
    let Node::Element(original_span) = &original_clear.children[1] else {
        panic!("inline ancestor")
    };
    assert_eq!(
        height(&original_span.children[0]),
        Some(Len::Px(115.200005))
    );
}
