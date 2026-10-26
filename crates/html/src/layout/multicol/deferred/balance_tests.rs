//! Баланс пробует исходный поток повторно, не наследуя распорки другой высоты.

use super::*;
use crate::layout::fragment::plan::Completion;
use crate::layout::fragment::types::StackAxis;
use gpui::{point, px, size};

#[gpui::test]
fn float_balance_trials_keep_the_source_and_repeated_layout_stable(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for (style, content) in [
        (
            "font:50px/1 Ahem",
            "<div>A<div style='float:left'>X</div></div><div>B<div style='float:left'>X</div></div><div>C<div style='float:left'>X</div></div>",
        ),
        (
            "font:20px/1 Ahem",
            "<div style='height:20px'></div><div style='float:left;width:100%;height:20px;margin:40px 0;contain:size'></div>",
        ),
        (
            "font:20px/1 Ahem",
            "<div style='float:left;width:100%;height:20px;margin:40px 0;contain:size'></div>",
        ),
    ] {
        cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
            let tree = crate::dom::parse(&format!("<section style='width:200px;column-count:2;column-gap:0;{style}'>{content}</section>"), "");
            let mut opts = RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() };
            opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
            opts.cx.values.viewport = opts.viewport;
            crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
            let host = super::tests::section(&tree);
            let before = format!("{:?}", host.style.used());
            let merged = crate::style::cascade::materialize::box_style(&opts.root_style(), &host.style, opts.cx.vcx());
            let recipe = Recipe::new(host, &merged, &opts, 2, 0., None, StackAxis::Horizontal, None).expect("recorded float recipe");
            let mut stack = ColumnStack::new(Vec::new(), 2, 0., None, None, None, None, None, Default::default());
            let mut heights = Vec::new();
            for width in [100., 80., 100.] {
                recipe.prepare(&mut stack, width, window, app);
                assert_eq!(stack.balance_failure, None);
                let (height, _, plan, _) = stack.balance(&stack.kid_geoms());
                assert_eq!(plan.completion, Completion::Complete);
                heights.push(height);
            }
            assert_eq!(heights[0], 100.);
            assert_eq!(heights[0], heights[2]);
            assert_eq!(format!("{:?}", host.style.used()), before);
            gpui::div()
        });
    }
}
