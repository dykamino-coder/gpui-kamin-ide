//! Числовой результат пробы переживает её элементы и отдельный layout engine.

use super::Probe;

impl Probe<'_> {
    pub(super) fn temporary_layout<T>(&mut self, run: impl FnOnce(&mut Probe<'_>) -> T) -> T {
        let Self {
            window,
            cx,
            paint,
            build,
            vert,
            fragment,
            clamp,
            positioning,
            last_geometry,
            last_clamp,
            engine: _,
            last_prepared: _,
        } = self;
        let (result, geometry, clamped) = gpui::with_temporary_element_arena(|| {
            window.with_nested_layout(|window| {
                let mut probe = Probe::new(
                    window,
                    cx,
                    *vert,
                    fragment.as_ref(),
                    paint.clone(),
                    super::fragment::ProbeScopes {
                        clamp: clamp.clone(),
                        positioning: positioning.clone(),
                    },
                    *build,
                );
                let result = run(&mut probe);
                (result, probe.last_geometry, probe.last_clamp)
            })
        });
        *last_geometry = geometry;
        *last_clamp = clamped;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{Hooks, IntoHooked};
    use gpui::{App, LayoutId, Window, div, point, px, size};
    use std::{cell::Cell, rc::Rc};

    struct Tracked(Rc<Cell<usize>>);

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    impl Hooks for Tracked {
        type Layout = ();
        type Prepaint = ();

        fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
            (window.request_layout(Default::default(), [], cx), ())
        }

        fn prepaint(
            &mut self,
            _: gpui::Bounds<gpui::Pixels>,
            _: &mut (),
            _: &mut Window,
            _: &mut App,
        ) {
        }

        fn paint(
            &mut self,
            _: gpui::Bounds<gpui::Pixels>,
            _: &mut (),
            _: &mut (),
            _: &mut Window,
            _: &mut App,
        ) {
        }
    }

    #[gpui::test]
    fn numeric_probe_releases_its_elements_before_the_next_probe(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.), px(0.)),
            size(px(800.), px(600.)),
            |window, app| {
                let drops = Rc::new(Cell::new(0));
                let build: super::super::Build = Rc::new({
                    let drops = drops.clone();
                    move |_, _, _, _, _| Tracked(drops.clone()).hooked()
                });
                let layout = crate::layout::cx::LayoutCx::default();
                let mut probe = Probe::new(
                    window,
                    app,
                    false,
                    None,
                    layout.deferred.paint.clone(),
                    super::super::fragment::ProbeScopes {
                        clamp: None,
                        positioning: None,
                    },
                    layout.contracts.build,
                );
                for expected in 1..=3 {
                    super::super::probe_of(
                        super::super::Kind::Flow,
                        &build,
                        100.,
                        100.,
                        None,
                        None,
                        &mut probe,
                    );
                    assert_eq!(drops.get(), expected);
                }
                super::super::intrinsic_of(&build, &mut probe);
                assert_eq!(drops.get(), 4);
                div()
            },
        );
    }
}
