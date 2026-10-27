//! Глубокие прозрачные и непрозрачные raster scopes на нативном стеке.

use super::draw_at_scale;
use crate::text::raster::Background;
use gpui::{IntoElement, ParentElement, Styled, canvas, div};
use std::{cell::Cell, rc::Rc};

#[test]
fn deep_background_scopes_paint_on_eight_mib() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for opaque in [false, true] {
                let painted = Rc::new(Cell::new(false));
                let observed = painted.clone();
                draw_at_scale(Some(1.0), || {
                    let mut tree = canvas(
                        |_, _, _| (),
                        move |_, (), window, _| {
                            assert_eq!(window.raster_context::<Background>().is_some(), opaque);
                            observed.set(true);
                        },
                    )
                    .size_full()
                    .into_any_element();
                    for _ in 0..1026 {
                        let mut parent = div().size_full().child(tree);
                        parent.style().snap_to_device_pixels = Some(true);
                        if opaque {
                            parent = parent.bg(gpui::rgb(0xffffff));
                        }
                        tree = parent.into_any_element();
                    }
                    tree
                });
                assert!(painted.get());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
