//! Полные зависимости кэша продолжения.

use super::*;
use crate::layout::cx::{BoxConstraints, FragmentConstraints};
use crate::layout::fragment::geometry::Node;
use gpui::{AvailableSpace, Style, point, px, size};

#[gpui::test]
fn cached_continuation_requires_the_complete_producing_inputs(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let id = window.request_layout(Style::default(), [], app);
            let node = Node::capture(id, window);
            let available = size(
                AvailableSpace::Definite(px(100.)),
                AvailableSpace::MaxContent,
            );
            window.compute_layout(id, available, app);
            let inputs = Inputs {
                constraints: FragmentConstraints {
                    area: (100., 200.),
                    parent: BoxConstraints {
                        cb_width: Some(100.),
                        avail_w: Some(100.),
                        avail_h: Some(300.),
                        cell_bfc: false,
                    },
                },
                source: Some(node.read(window).unwrap().space),
                page: None,
            };
            let mut cached = CopyGeometry {
                inputs: inputs.clone(),
                from: 40.,
                offset: 60.,
                kid: Kid::default(),
                inline: Default::default(),
                laid: Laid::default(),
                parallel: None,
                continued: true,
            };
            assert!(cached.reusable(&inputs, 40.));
            assert!(cached.reusable(&inputs, 60.));
            assert!(!cached.reusable(&inputs, 80.));
            let mut changed = inputs.clone();
            changed.constraints.area.1 = 180.;
            assert!(!cached.reusable(&changed, 40.));
            changed = inputs.clone();
            changed.constraints.parent.avail_h = Some(400.);
            assert!(!cached.reusable(&changed, 40.));
            changed = inputs.clone();
            changed.constraints.parent.cb_width = Some(120.);
            assert!(!cached.reusable(&changed, 40.));
            changed = inputs.clone();
            changed.page = Some(crate::layout::fragment::token::space::PageIdentity {
                index: 1,
                name: Some(0),
                side: crate::layout::fragment::token::space::PageSide::Left,
                blank: false,
            });
            assert!(!cached.reusable(&changed, 40.));
            changed = inputs.clone();
            changed.source = None;
            assert!(!cached.reusable(&changed, 40.));
            window.compute_layout(id, available, app);
            changed.source = Some(node.read(window).unwrap().space);
            assert!(!cached.reusable(&changed, 40.));
            cached.continued = false;
            assert!(cached.reusable(&inputs, 80.));
            assert!(!cached.reusable(&changed, 80.));
            gpui::div()
        },
    );
}
