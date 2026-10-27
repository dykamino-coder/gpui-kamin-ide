//! Баланс пробует исходный поток повторно, не наследуя распорки другой высоты.

use super::*;
use crate::layout::fragment::plan::Completion;
use crate::layout::fragment::types::StackAxis;
use gpui::{point, px, size};

fn authored_height(cx: &mut gpui::TestAppContext, style: &str, content: &str) -> f32 {
    let cx = cx.add_empty_window();
    let mut height = 0.;
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
            let tree = crate::dom::parse(&format!("<!doctype html><section style='width:100px;column-count:2;{style}'>{content}</section>"), "");
            let mut opts = RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() };
            opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
            opts.cx.values.viewport = opts.viewport;
            crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
            let host = super::tests::section(&tree);
            let mut el = opts.cx.contracts.build.element(host, &opts.root_style(), &opts);
            let (sz, _) = el.layout_as_root_with_content(size(gpui::AvailableSpace::Definite(px(100.)), gpui::AvailableSpace::MaxContent), window, app);
            height = f32::from(sz.height);
            gpui::div()
        });
    height
}

#[gpui::test]
fn balance_break_avoidance_002_retains_the_earlier_boundary(cx: &mut gpui::TestAppContext) {
    assert_eq!(
        authored_height(
            cx,
            "column-gap:0",
            "<div style='height:25px'></div><div style='height:50px'></div><div style='height:25px;break-before:avoid'></div><div style='height:50px'></div>"
        ),
        100.
    );
}

#[gpui::test]
fn row_flex_037_balances_without_slicing_the_avoided_item(cx: &mut gpui::TestAppContext) {
    assert_eq!(
        authored_height(
            cx,
            "column-gap:0",
            "<div style='display:flex;flex-wrap:wrap'><div style='height:25px;width:50px'></div><div style='height:50px;width:50px'></div><div style='height:5px;width:25px'></div><div style='height:25px;width:25px;break-before:avoid'></div><div style='height:50px;width:50px'></div></div>"
        ),
        100.
    );
}

#[gpui::test]
fn row_flex_balances_without_avoid_and_respects_css_height(cx: &mut gpui::TestAppContext) {
    let content = "<div style='display:flex;flex-wrap:wrap'><div style='height:25px;width:50px'></div><div style='height:50px;width:50px'></div><div style='height:25px;width:50px;break-before:auto'></div><div style='height:50px;width:50px'></div></div>";
    assert_eq!(authored_height(cx, "column-gap:0", content), 75.);
    assert_eq!(
        authored_height(
            cx,
            "column-gap:0;height:75px",
            &content.replace("break-before:auto", "break-before:avoid")
        ),
        75.
    );
}

#[gpui::test]
fn oof_097_keeps_the_row_before_its_owners_spanner(cx: &mut gpui::TestAppContext) {
    assert_eq!(
        authored_height(
            cx,
            "position:relative",
            "<div style='contain:size;height:80px'></div><div style='column-span:all;height:20px'></div><div style='contain:size;height:100px;width:100%'></div><div style='contain:size;height:100px;width:100%'></div><div style='position:relative'><div style='position:absolute;height:100px;width:100%'></div></div><div style='position:absolute;bottom:0;right:0;height:100px;width:90px'></div>"
        ),
        300.
    );
}

#[gpui::test]
fn balance_036_037_count_the_recorded_float_margin_box_once(cx: &mut gpui::TestAppContext) {
    for text in ["&#160;", ""] {
        assert_eq!(
            authored_height(
                cx,
                "line-height:20px;orphans:1;widows:1",
                &format!(
                    "{text}<div style='float:left;width:100%;contain:size;height:20px;margin:40px 0'></div>"
                )
            ),
            100.
        );
    }
}

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
