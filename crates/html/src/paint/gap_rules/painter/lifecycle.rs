//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

use super::painting::paint_body;
use crate::hooks::Hooks;

use super::GapRulePainter;
use crate::paint::gap_rules::GapLayout;
use crate::paint::gap_rules::geometry::{GridTracks, uncollapsed};
use gpui::{App, Bounds, LayoutId, Pixels, Window, px};

impl Hooks for GapRulePainter {
    type Layout = LayoutId;
    type Prepaint = (Bounds<Pixels>, Option<GridTracks>);

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        // Художник занимает паддинг-бокс контейнера: от него считается поле
        // содержимого (протяжённость главных промежутков строк и лент).
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::Prepaint {
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
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        _cx: &mut App,
    ) {
        paint_body(self, _bounds, _state, prepaint, window, _cx)
    }
}
