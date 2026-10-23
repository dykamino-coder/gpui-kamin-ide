//! Анимация → узел: наложение кадра на настоящий элемент с сохранением его тождества.
//!
//! Вход — элемент дерева и выборка стиля на момент кадра; выход — копия
//! элемента, у которой подменены только анимируемые поля (прозрачность, фон,
//! цвет, размеры, сдвиг, фильтры, тень; `transform`/`rotate`/`scale` — лишь по
//! флагу). `node_id`, поля, отступы и неанимированные свойства остаются
//! прежними: сборка и раскладка должны видеть тот же узел, а не новую коробку.
use crate::dom::Element;
use crate::style::computed::Computed;

pub(super) fn sample(e: &Element, frame: &Computed, transforms: bool) -> Element {
    let mut inner = e.clone();
    let style = inner.style.cascaded_mut();
    style.margin.top = frame.margin.top.or(style.margin.top);
    style.margin.right = frame.margin.right.or(style.margin.right);
    style.margin.bottom = frame.margin.bottom.or(style.margin.bottom);
    style.margin.left = frame.margin.left.or(style.margin.left);
    if frame.opacity.is_some() {
        style.opacity = frame.opacity;
    }
    if frame.background.is_some() {
        style.background = frame.background;
    }
    if frame.color.is_some() {
        style.color = frame.color;
    }
    if frame.width.is_some() {
        style.width = frame.width;
    }
    if frame.font_size.is_some() {
        style.font_size = frame.font_size;
    }
    if frame.height.is_some() {
        style.height = frame.height;
    }
    if frame.translate.is_some() {
        style.translate = frame.translate;
    }
    if frame.filter.is_some() {
        style.filter = frame.filter;
    }
    if frame.backdrop_blur.is_some() {
        style.backdrop_blur = frame.backdrop_blur;
    }
    if frame.backdrop_color.is_some() {
        style.backdrop_color = frame.backdrop_color;
    }
    if frame.drop_shadow.is_some() {
        style.drop_shadow = frame.drop_shadow.clone();
    }
    if transforms {
        if frame.rotate_prop.is_some() {
            style.rotate_prop = frame.rotate_prop;
        }
        if frame.scale_prop.is_some() {
            style.scale_prop = frame.scale_prop;
        }
        if frame.transform.is_some() {
            style.transform = frame.transform;
        }
    }
    inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::{Color, Len};

    #[test]
    fn sampled_height_keeps_identity_placement_edges_and_unanimated_width() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.node_id = 17;
        element.style.cascaded_mut().width = Some(Len::Px(200.0));
        element.style.cascaded_mut().height = Some(Len::Px(100.0));
        element.style.cascaded_mut().margin.top = Some(Len::Px(12.0));
        element.style.cascaded_mut().padding.left = Some(Len::Px(4.0));
        for height in [50.0, 75.0, 100.0] {
            let frame = Computed {
                height: Some(Len::Px(height)),
                ..Computed::default()
            };
            let sampled = sample(&element, &frame, false);
            assert_eq!(sampled.node_id, 17);
            assert_eq!(sampled.style.cascaded().height, Some(Len::Px(height)));
            assert_eq!(
                sampled.style.cascaded().width,
                element.style.cascaded().width
            );
            assert_eq!(
                sampled.style.cascaded().margin.top,
                element.style.cascaded().margin.top
            );
            assert_eq!(
                sampled.style.cascaded().padding.left,
                element.style.cascaded().padding.left
            );
        }
        assert_eq!(element.style.cascaded().height, Some(Len::Px(100.0)));
    }

    #[test]
    fn sampled_percent_and_color_replace_actual_node_declarations() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.style.cascaded_mut().width = Some(Len::Px(200.0));
        element.style.cascaded_mut().height = Some(Len::Px(100.0));
        element.style.cascaded_mut().color = Some(Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        });
        element.style.cascaded_mut().rotate_prop = Some(10.0);
        let frame = Computed {
            width: Some(Len::Pct(0.5)),
            color: Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            rotate_prop: Some(20.0),
            ..Computed::default()
        };
        let sampled = sample(&element, &frame, false);
        assert_eq!(sampled.style.cascaded().width, frame.width);
        assert_eq!(
            sampled.style.cascaded().height,
            element.style.cascaded().height
        );
        assert_eq!(sampled.style.cascaded().color, frame.color);
        assert_eq!(
            sampled.style.cascaded().rotate_prop,
            element.style.cascaded().rotate_prop
        );
        assert_eq!(
            sample(&element, &frame, true).style.cascaded().rotate_prop,
            frame.rotate_prop
        );
    }
}
