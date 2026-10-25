//! Геометрия строки пустого пункта и внешнего маркера.
use super::*;
use crate::style::values::value::Len;
use gpui::{AvailableSpace, Axis, point, px, size};

#[gpui::test]
fn empty_item_keeps_its_font_baseline_beside_a_taller_marker(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let opts = RenderOpts {
                text: Default::default(),
                viewport: (100., 100.),
                normal_line_height: 1.2,
                cx: Default::default(),
                frag: Default::default(),
            };
            let mut style = Computed::default();
            style.font_size = Some(Len::Px(16.));
            let mut marker = Computed::default();
            marker.font_size = Some(Len::Px(40.));
            style.marker_layer = Some(Box::new(marker));
            let merged = BoxStyle::from(style.clone());
            let mut item = Element::synthetic("li", style, Default::default(), vec![]);
            item.list_item = Some(1);
            let mut baseline = |item: &Element| {
                let mut el = render_with_style(item, &Computed::default(), &merged, &opts);
                let id = el.request_layout(window, app);
                window.compute_layout(
                    id,
                    size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::MaxContent,
                    ),
                    app,
                );
                window
                    .first_content_baseline(id, Axis::Vertical)
                    .expect("item font line")
            };
            let empty = baseline(&item);
            item.children.push(Node::Text("\u{a0}".into()));
            assert_eq!(empty, baseline(&item));
            div()
        },
    );
}
