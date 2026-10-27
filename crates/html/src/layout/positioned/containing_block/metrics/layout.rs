//! Исходный normal layout при известной ширине; снимок отделён от живых LayoutId.

use crate::dom::Element;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, LayoutId, Window};
use std::rc::Rc;

mod source;
pub(crate) use source::{collect, collect_paged, contains_positioned};

pub(crate) type PageSource = Rc<dyn Fn((f32, f32), &mut Window, &mut App) -> Rc<super::Snapshot>>;

type Build = Box<dyn FnOnce(RenderOpts) -> AnyElement>;

/// Построение, которому нужен Window, переносится в request_layout, без prepaint.
pub(crate) fn defer(
    kids: Vec<Element>,
    parent: BoxStyle,
    width: Option<f32>,
    mut opts: RenderOpts,
    build: impl FnOnce(RenderOpts) -> AnyElement + 'static,
) -> AnyElement {
    if opts.cx.position.geometry_record.is_some()
        || !kids.iter().any(contains_positioned)
        || width.is_none()
    {
        return build(opts);
    }
    // Callback строит потомков после закрытия исходных LayerScope. Их очереди
    // принадлежат прежним CB, включая transform-предка для вложенного fixed.
    opts.cx.position.state = opts.cx.position.state.retain().resume();
    Original {
        kids,
        parent,
        width: width.unwrap(),
        opts,
        build: Some(Box::new(build)),
        child: None,
    }
    .hooked()
}

struct Original {
    kids: Vec<Element>,
    parent: BoxStyle,
    width: f32,
    opts: RenderOpts,
    build: Option<Build>,
    child: Option<AnyElement>,
}

impl Wrap for Original {
    fn child(&mut self) -> &mut AnyElement {
        self.child
            .as_mut()
            .expect("original layout built the fragment tree")
    }
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let geometry = collect(&self.kids, &self.parent, self.width, &self.opts, window, cx);
        self.opts.cx.position.geometry = Some(geometry);
        let build = self
            .build
            .take()
            .expect("one original geometry pass per layout element");
        self.child = Some(build(self.opts.clone()));
        self.child().request_layout(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::positioned::containing_block::LayerScope;
    use gpui::{AvailableSpace, IntoElement, div, px, size};
    use std::cell::Cell;

    #[gpui::test]
    fn deferred_multicol_keeps_the_fixed_destination_beyond_an_absolute_cb(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        cx.draw(
            gpui::point(px(0.), px(0.)),
            size(px(400.), px(400.)),
            |window, app| {
                let mut kid = crate::layout::table::anon::anon_element("div", Vec::new());
                kid.style.used_mut().position = Some(crate::style::computed::Position::Absolute);
                let opts = RenderOpts {
                    viewport: (400., 400.),
                    normal_line_height: 1.2,
                    text: Default::default(),
                    cx: Default::default(),
                    frag: Default::default(),
                };
                let state = opts.cx.position.state.clone();
                let transform = LayerScope::cb(&state, true);
                let absolute = LayerScope::cb(&state, false);
                let built = Rc::new(Cell::new(false));
                let observed = built.clone();
                let mut deferred = defer(
                    vec![kid],
                    opts.root_style(),
                    Some(100.),
                    opts,
                    move |opts| {
                        assert!(opts.cx.position.state.can_defer(true, 0, false));
                        observed.set(true);
                        div().into_any_element()
                    },
                );
                assert!(!built.get());
                let _absolute_host = absolute.finish();
                let _transform_host = transform.finish();
                assert!(!state.cb_active(true));
                deferred.layout_as_root(
                    size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::MaxContent,
                    ),
                    window,
                    app,
                );
                assert!(built.get());
                div()
            },
        );
    }
}
