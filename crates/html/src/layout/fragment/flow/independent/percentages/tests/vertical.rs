//! Сохранение владения процентами в вертикальном письме.
use super::*;

#[test]
fn vertical_inline_percentages_remain_owned_by_vertical_layout() {
    let tree = crate::document::parsed::parse(
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
    assert_eq!(
        actual.reset_box_layout.height,
        original.reset_box_layout.height
    );
    assert_eq!(
        actual.reset_box_layout.min_height,
        original.reset_box_layout.min_height
    );
    assert_eq!(
        actual.reset_box_layout.max_height,
        original.reset_box_layout.max_height
    );
}
