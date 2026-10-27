//! Мера раскладкой копии: сетка, flex, таблица и обёртка вложенных колонок.

use super::*;
use crate::layout::multicol::measure::native_dispatch_tests::{host, opts, two_columns};

mod children;

#[gpui::test]
fn empty_grid_uses_measured_copy_at_known_and_deferred_widths(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    use crate::dom::Node;
    let tree = crate::dom::parse(
        "<div><div style='display:grid;grid-template-rows:20px 30px'></div></div>",
        "",
    );
    let opts = opts();
    let host = host(&tree);
    let recipe = two_columns(host, &opts).unwrap();
    for width in [100.0, 120.0] {
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(400.), gpui::px(400.)),
            |window, app| {
                let kids = recipe.children(width, window, app);
                assert_eq!(kids.len(), 1);
                assert_eq!(kids[0].measure, Some(width));
                let Node::Element(grid) = &host.children[0] else {
                    panic!("grid")
                };
                assert!(gate(grid, true, Some(width)));
                assert!(kids[0].watched.as_ref().is_some_and(|w| w.has_blocks()));
                gpui::div()
            },
        );
    }
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[test]
fn percent_margins_use_the_copy_inline_size() {
    let tree = crate::dom::parse(
        "<div style='display:grid;margin-top:10%;margin-bottom:5%'><div>x</div></div>",
        "",
    );
    let c = host(&tree);
    let registry = RefCell::default();
    let (_, shape) =
        measured(c, &BoxStyle::default(), tree.values(), 120.0, &registry).expect("copy");
    assert_eq!((shape.margin_top, shape.margin_bottom), (12.0, 6.0));
    assert_eq!(shape.height, 0.0);
    assert_eq!(*registry.borrow(), vec![(c.node_id, 120.0)]);
}
