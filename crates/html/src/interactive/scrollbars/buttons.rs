//! Кнопки-стрелки классической полосы прокрутки на обоих концах дорожки: треугольник в
//! центре кнопки, отключённое состояние по текущему положению в диапазоне прокрутки,
//! авторский цвет ползунка с приглушённой альфой для отключённой стрелки.
use gpui::{Bounds, Hsla, PathBuilder, Pixels, Window, point, px};

pub(super) fn paint(
    lane: Bounds<Pixels>,
    axis: usize,
    button: f32,
    offset: f32,
    maximum: f32,
    (arrows, custom): ([Hsla; 2], Option<Hsla>),
    window: &mut Window,
) {
    if button < 6.0 {
        return;
    }
    let length = f32::from(if axis == 0 {
        lane.size.width
    } else {
        lane.size.height
    });
    for (index, position) in [button / 2.0, length - button / 2.0]
        .into_iter()
        .enumerate()
    {
        let sign = if index == 0 { -1.0 } else { 1.0 };
        let disabled = if index == 0 {
            offset <= 0.0
        } else {
            offset >= maximum
        };
        let mut color = custom.unwrap_or(arrows[usize::from(!disabled)]);
        if custom.is_some() && disabled {
            color.a *= 0.4;
        }
        let center = if axis == 0 {
            point(
                lane.left() + px(position),
                lane.top() + lane.size.height / 2.0,
            )
        } else {
            point(
                lane.left() + lane.size.width / 2.0,
                lane.top() + px(position),
            )
        };
        let vertex = |main, cross| {
            if axis == 0 {
                point(center.x + px(main), center.y + px(cross))
            } else {
                point(center.x + px(cross), center.y + px(main))
            }
        };
        let mut path = PathBuilder::fill();
        path.move_to(vertex(sign * 2.0, 0.0));
        path.line_to(vertex(-sign * 2.0, -3.0));
        path.line_to(vertex(-sign * 2.0, 3.0));
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }
}
