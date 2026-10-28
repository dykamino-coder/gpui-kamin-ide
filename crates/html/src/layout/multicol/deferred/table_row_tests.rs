//! Ряд таблицы в колонках: «not freely fragmentable» (css-tables-3 §breaking-rules) по
//! записанной раскладке и ширине колонки.

use super::*;
use crate::layout::fragment::types::StackAxis;
use gpui::{point, px, size};

/// Куски таблицы `(колонка, from, h)` в двух колонках `column-fill: auto` высотой 100
/// при ширине стопки `width`.
fn table_pieces(cx: &mut gpui::TestAppContext, width: f32) -> Vec<(usize, f32, f32)> {
    let cx = cx.add_empty_window();
    let mut out = Vec::new();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let rows = "<tr><td style='height:40px;padding:0'></td></tr>".repeat(3);
        let tree = crate::dom::parse(&format!("<section style='columns:2;column-gap:0;column-fill:auto;height:100px'><table style='border-spacing:0'>{rows}</table></section>"), "");
        let mut opts = RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
        let host = super::tests::section(&tree);
        let merged = crate::style::cascade::materialize::box_style(&opts.root_style(), &host.style, opts.cx.vcx());
        let recipe = Recipe::new(host, &merged, &opts, 2, 0., Some(100.), StackAxis::Horizontal, None).expect("table recipe");
        let mut stack = ColumnStack::new(Vec::new(), 2, 0., Some(100.), None, None, None, Default::default());
        recipe.prepare(&mut stack, width / 2., window, app);
        let (_, _, plan, _) = stack.balance(&stack.kid_geoms());
        out = plan.iter().map(|f| (f.col, f.from, f.h)).collect();
        gpui::div()
    });
    out
}

#[gpui::test]
fn a_small_row_crossing_the_column_edge_moves_whole(cx: &mut gpui::TestAppContext) {
    // Ряд 80..120 не выше половины колонки 100×100: перенос целиком.
    assert_eq!(table_pieces(cx, 200.), [(0, 0., 80.), (1, 80., 40.)]);
}

#[gpui::test]
fn a_row_wider_than_half_the_column_width_is_sliced(cx: &mut gpui::TestAppContext) {
    // Колонка шириной 60 < 2×40: ряд freely fragmentable и рвётся внутри себя, а не
    // переносится с границы 80.
    let pieces = table_pieces(cx, 120.);
    assert_eq!(pieces.len(), 2);
    assert!(pieces[0].2 > 80.5 && pieces[1].1 > 80.5, "{pieces:?}");
}
