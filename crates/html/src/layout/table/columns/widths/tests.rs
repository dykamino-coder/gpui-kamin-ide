//! Буферы проб полос колонок: свои на сборку, общие для дорожек одного `<col>`.

use super::push_col_bands;
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Color;

fn col(node_id: u64) -> Element {
    Element {
        marks: Default::default(),
        list_item: None,
        node_id,
        anim: None,
        tag: "col".into(),
        style: Computed {
            background: Some(Color {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                a: 1.0,
            }),
            ..Default::default()
        }
        .into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: (Vec::new()).into(),
        attrs: Vec::new(),
        inline: false,
    }
}

#[test]
fn one_buffer_per_col_element_and_per_build() {
    let cx = crate::layout::cx::LayoutCx::default();
    let (a, b) = (col(7), col(8));
    let els = [Some(&a), Some(&a), Some(&b)];
    let build = || {
        let mut layers: Vec<_> = els
            .iter()
            .map(|&element| crate::layout::table::context::ColumnLayer {
                element,
                rects: None,
            })
            .collect();
        let mut cells = Vec::new();
        push_col_bands(&mut layers, 3, true, &mut cells, &cx);
        assert_eq!(cells.len(), 2, "one painter per element");
        layers
            .into_iter()
            .map(|layer| layer.rects.unwrap())
            .collect::<Vec<_>>()
    };
    let first = build();
    let rect = gpui::Bounds::default();
    first[0].push((rect, true, rect));
    assert_eq!(first[1].borrow().len(), 1);
    assert!(first[2].borrow().is_empty());
    // A rebuilt table (next frame, another document with the same node ids)
    // never sees the previous build's probes.
    let second = build();
    assert!(second[0].borrow().is_empty());
}
