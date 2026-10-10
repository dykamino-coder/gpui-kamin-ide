//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

mod painting;
use painting::paint_body;

use super::GapRulePainter;
use crate::paint::gap_rules::GapLayout;
use crate::paint::gap_rules::geometry::{GridTracks, uncollapsed};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window,
    px,
};

impl Element for GapRulePainter {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (Bounds<Pixels>, Option<GridTracks>);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        // Художник занимает паддинг-бокс контейнера: от него считается поле
        // содержимого (протяжённость главных промежутков строк и лент).
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> (Bounds<Pixels>, Option<GridTracks>) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        let tracks = (self.spec.kind == GapLayout::Grid || self.spec.lines_extent == 2)
            .then(|| window.parent_grid_tracks(*state))
            .flatten()
            .map(|(o, cols, rows)| {
                let (ox, oy) = (f32::from(o.x), f32::from(o.y));
                let gx = self.spec.gap_x.unwrap_or(0.0);
                let gy = self.spec.gap_y.unwrap_or(0.0);
                (
                    uncollapsed(cols.iter().map(|&(a, b)| (ox + a, ox + b)).collect(), gx),
                    uncollapsed(rows.iter().map(|&(a, b)| (oy + a, oy + b)).collect(), gy),
                )
            });
        (bounds, tracks)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        prepaint: &mut (Bounds<Pixels>, Option<GridTracks>),
        window: &mut Window,
        _cx: &mut App,
    ) {
        paint_body(
            self,
            _id,
            _inspector_id,
            _bounds,
            _state,
            prepaint,
            window,
            _cx,
        )
    }
}
