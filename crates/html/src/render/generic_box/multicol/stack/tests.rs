//! Тесты: пустые многоколоночные боксы не получают запасных внутренних промежутков сетки.

use super::*;
use crate::dom::Node;
use crate::layout::fragment::types::StackAxis;

#[test]
fn empty_multicol_keeps_the_original_box_without_column_tracks() {
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    for children in [vec![], vec![Node::Text(" \n\t".into())]] {
        let e = Element::synthetic("div", Computed::default(), Default::default(), children);
        for cols in [1, 3] {
            let result = stack_box(
                gpui::div().w(px(32.)),
                &e,
                crate::style::computed::BoxStyle::from(Computed::default()),
                &Computed::default(),
                &opts,
                cols,
                None,
                16.,
                0.,
                StackAxis::Horizontal,
                false,
                false,
                None,
                None,
                None,
                None,
                None,
                0.,
            );
            let Ok((mut d, _)) = result else {
                panic!("empty multicol must keep its original box");
            };
            let style = d.style();
            assert_eq!(style.size.width, Some(px(32.).into()));
            assert!(style.grid_template_cols.is_none());
            assert!(style.gap.width.is_none());
        }
    }
}
