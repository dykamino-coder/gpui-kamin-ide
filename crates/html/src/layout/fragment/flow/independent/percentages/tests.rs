//! Процентные ограничения разрешаются от исходного CB, независимо от фрагмента.
use super::*;
mod continuation;

#[test]
fn definite_and_indefinite_cb_have_distinct_percentage_rules() {
    let original = Computed {
        height: Some(Len::Pct(0.5)),
        min_height: Some(Len::Pct(0.25)),
        max_height: Some(Len::Pct(0.75)),
        ..Computed::default()
    };
    let mut definite = original.clone();
    freeze(&mut definite, Some(400.0));
    assert_eq!(definite.height, Some(Len::Px(200.0)));
    assert_eq!(definite.min_height, Some(Len::Px(100.0)));
    assert_eq!(definite.max_height, Some(Len::Px(300.0)));
    let mut indefinite = original;
    freeze(&mut indefinite, None);
    assert_eq!(indefinite.height, None);
    assert_eq!(indefinite.min_height, Some(Len::Px(0.0)));
    assert_eq!(indefinite.max_height, None);
}

fn opts(tree: &crate::dom::Parsed) -> RenderOpts {
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    opts
}

fn target(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "section" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("section in test document")
}

#[test]
fn original_cb_survives_wrapper_removal_and_changes_with_page_area() {
    let tree = crate::dom::parse(
        "<html style='height:100%'><body style='height:100%;margin:0'><div style='height:50%;box-sizing:border-box;padding:10px'><section style='height:200%;min-height:150%;max-height:250%'></section></div></body></html>",
        "",
    );
    let opts = opts(&tree);
    let (nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    assert_eq!(target(&nodes).style.used().height, Some(Len::Px(560.0)));
    let context = contexts.context(target(&nodes).node_id);
    for (page, expected) in [(600.0, 560.0), (400.0, 360.0)] {
        let (leaf, _) = context.leaf(
            crate::layout::fragment::token::Space {
                inline_size: Some(800.0),
                block_size: Some(page),
                ..crate::layout::fragment::token::Space::pages()
            },
            &opts,
        );
        assert_eq!(leaf.content_block, expected);
        let mut continuation = nodes.clone();
        let Node::Element(root) = &mut continuation[0] else {
            unreachable!()
        };
        contexts.refresh(
            root,
            crate::layout::fragment::token::Space {
                inline_size: Some(800.0),
                block_size: Some(page),
                ..crate::layout::fragment::token::Space::pages()
            },
            &opts,
        );
        assert_eq!(
            target(&continuation).style.used().height,
            Some(Len::Px(expected))
        );
    }
}

#[test]
fn parent_minimum_does_not_supply_a_definite_percentage_cb() {
    let tree = crate::dom::parse(
        "<div style='min-height:400px'><section style='height:50%;min-height:25%;max-height:75%'></section></div>",
        "",
    );
    let opts = opts(&tree);
    let (nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    let style = target(&nodes).style.used();
    assert_eq!(style.height, None);
    assert_eq!(style.min_height, Some(Len::Px(0.0)));
    assert_eq!(style.max_height, None);
    let (leaf, _) = contexts.context(target(&nodes).node_id).leaf(
        crate::layout::fragment::token::Space {
            inline_size: Some(800.0),
            block_size: Some(600.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    assert_eq!(leaf.content_block, 0.0);
}

#[test]
fn column_cb_is_not_the_remaining_fragment_height() {
    let tree = crate::dom::parse(
        "<section style='height:50%;min-height:25%;max-height:75%'></section>",
        "",
    );
    let opts = opts(&tree);
    let mut parent = opts.root_style();
    parent.icb = false;
    parent.height = Some(Len::Px(400.0));
    parent.column_count = Some(2);
    // Обёртки документа не участвуют в изолированном контракте колонки.
    let section = target(&tree).clone();
    let columns = column_percentages(
        &[Node::Element(section.clone())],
        &parent,
        Some(200.0),
        &opts,
    );
    assert_eq!(target(&columns).style.used().height, Some(Len::Px(200.0)));
    assert_eq!(
        target(&columns).style.used().min_height,
        Some(Len::Px(100.0))
    );
    assert_eq!(
        target(&columns).style.used().max_height,
        Some(Len::Px(300.0))
    );
    let mut span = section;
    span.style.used_mut().column_span = Some(true);
    let spanning = column_percentages(&[Node::Element(span)], &parent, Some(200.0), &opts);
    assert_eq!(target(&spanning).style.used().height, Some(Len::Px(200.0)));
}

#[test]
fn font_lengths_and_border_box_limits_are_resolved_before_consumption() {
    let tree = crate::dom::parse(
        "<section style='font-size:10px;height:10em;max-height:9em;min-height:8em;box-sizing:border-box;padding:1em;box-decoration-break:clone'></section>",
        "",
    );
    let opts = opts(&tree);
    let (nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    let (leaf, _) = contexts.context(target(&nodes).node_id).leaf(
        crate::layout::fragment::token::Space {
            inline_size: Some(800.0),
            block_size: Some(600.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    assert_eq!(leaf.content_block, 70.0);
    assert_eq!(
        (leaf.edges.start, leaf.edges.end, leaf.edges.clone),
        (10.0, 10.0, true)
    );
}

#[test]
fn viewport_leaf_limits_and_edges_keep_the_initial_page_basis() {
    let tree = crate::dom::parse(
        "<section style='height:150vh;min-height:110vh;max-height:120vh;box-sizing:border-box;padding:2vh;border:1vh solid'></section>",
        "",
    );
    let opts = opts(&tree);
    let (nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    for page in [600.0, 400.0] {
        let (leaf, _) = contexts.context(target(&nodes).node_id).leaf(
            crate::layout::fragment::token::Space {
                inline_size: Some(800.0),
                block_size: Some(page),
                ..crate::layout::fragment::token::Space::pages()
            },
            &opts,
        );
        assert_eq!(leaf.content_block, 684.0);
        assert_eq!((leaf.edges.start, leaf.edges.end), (18.0, 18.0));
    }
}

#[test]
fn viewport_parent_supplies_a_definite_percentage_containing_block() {
    let tree = crate::dom::parse(
        "<div style='height:50vh'><section style='height:50%'></section></div>",
        "",
    );
    let opts = opts(&tree);
    let (nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    assert_eq!(target(&nodes).style.used().height, Some(Len::Px(150.0)));
    let (leaf, _) = contexts.context(target(&nodes).node_id).leaf(
        crate::layout::fragment::token::Space {
            inline_size: Some(800.0),
            block_size: Some(400.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    assert_eq!(leaf.content_block, 150.0);
}

#[test]
fn vertical_inline_percentages_remain_owned_by_vertical_layout() {
    let tree = crate::dom::parse(
        "<div style='writing-mode:vertical-rl;height:400px'><section style='height:50%;min-height:25%;max-height:75%'></section></div>",
        "",
    );
    let opts = opts(&tree);
    let (mut nodes, contexts) = page_percentages(&tree, &opts.root_style(), &opts);
    let original = target(&tree).style.used();
    let Node::Element(root) = &mut nodes[0] else {
        unreachable!()
    };
    contexts.refresh(
        root,
        crate::layout::fragment::token::Space {
            inline_size: Some(800.0),
            block_size: Some(300.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        &opts,
    );
    let actual = target(&nodes).style.used();
    assert_eq!(actual.height, original.height);
    assert_eq!(actual.min_height, original.min_height);
    assert_eq!(actual.max_height, original.max_height);
}
