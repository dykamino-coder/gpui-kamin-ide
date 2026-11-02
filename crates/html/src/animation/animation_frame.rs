//! Анимация → узел: наложение кадра на настоящий элемент с сохранением его тождества.
//!
//! Вход — элемент дерева и выборка стиля на момент кадра; выход — копия
//! элемента, у которой подменены только анимируемые поля (прозрачность, фон,
//! цвет, размеры, сдвиг, фильтры, тень; `transform`/`rotate`/`scale` — лишь по
//! флагу). `node_id`, поля, отступы и неанимированные свойства остаются
//! прежними: сборка и раскладка должны видеть тот же узел, а не новую коробку.
use crate::dom::Element;
use crate::style::computed::Computed;

pub(super) fn sample(
    e: &Element,
    frame: &Computed,
    transforms: bool,
    vcx: crate::values::ValueCx<'_>,
) -> Element {
    let mut inner = e.clone();
    // Animations are an origin of the cascade (css-cascade-5 §6.2): the sampled
    // values enter the cascaded style and are computed with it.
    let style = inner.style.cascaded_mut();
    style.reset_box_layout.margin.top = frame
        .reset_box_layout
        .margin
        .top
        .or(style.reset_box_layout.margin.top);
    style.reset_box_layout.margin.right = frame
        .reset_box_layout
        .margin
        .right
        .or(style.reset_box_layout.margin.right);
    style.reset_box_layout.margin.bottom = frame
        .reset_box_layout
        .margin
        .bottom
        .or(style.reset_box_layout.margin.bottom);
    style.reset_box_layout.margin.left = frame
        .reset_box_layout
        .margin
        .left
        .or(style.reset_box_layout.margin.left);
    if frame.reset_effects.opacity.is_some() {
        style.reset_effects.opacity = frame.reset_effects.opacity;
    }
    if frame.reset_background.background.is_some() {
        style.reset_background.background = frame.reset_background.background;
    }
    if frame.inh.ui.color.is_some() {
        style.inh.ui.color = frame.inh.ui.color;
    }
    if frame.reset_box_layout.width.is_some() {
        style.reset_box_layout.width = frame.reset_box_layout.width;
    }
    if frame.inh.font.font_size.is_some() {
        style.inh.font.font_size = frame.inh.font.font_size;
    }
    if frame.reset_box_layout.height.is_some() {
        style.reset_box_layout.height = frame.reset_box_layout.height;
    }
    if frame.reset_transforms.translate.is_some() {
        style.reset_transforms.translate = frame.reset_transforms.translate;
    }
    if frame.reset_effects.filter.is_some() {
        style.set_animated_filter(frame);
    }
    if frame.reset_effects.backdrop_blur.is_some() {
        style.reset_effects.backdrop_blur = frame.reset_effects.backdrop_blur;
    }
    if frame.reset_effects.backdrop_color.is_some() {
        style.reset_effects.backdrop_color = frame.reset_effects.backdrop_color;
    }
    if frame.reset_effects.drop_shadow.is_some() {
        style.reset_effects.drop_shadow = frame.reset_effects.drop_shadow.clone();
    }
    if transforms {
        if frame.reset_transforms.rotate_prop.is_some() {
            style.reset_transforms.rotate_prop = frame.reset_transforms.rotate_prop;
        }
        if frame.reset_transforms.scale_prop.is_some() {
            style.reset_transforms.scale_prop = frame.reset_transforms.scale_prop;
        }
        if frame.reset_transforms.transform.is_some() {
            style.reset_transforms.transform = frame.reset_transforms.transform;
        }
    }
    crate::style::cascade::materialize::rematerialize(&e.style, inner, vcx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::{Color, Len};

    fn test_vcx() -> crate::values::ValueCx<'static> {
        let doc = Box::leak(Box::new(crate::document::state::DocState::new(
            crate::style::css::Media::default(),
        )));
        doc.values()
    }

    #[test]
    fn sampled_height_keeps_identity_placement_edges_and_unanimated_width() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.node_id = 17;
        element.style.used_mut().reset_box_layout.width = Some(Len::Px(200.0));
        element.style.used_mut().reset_box_layout.height = Some(Len::Px(100.0));
        element.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(12.0));
        element.style.used_mut().reset_box_layout.padding.left = Some(Len::Px(4.0));
        for height in [50.0, 75.0, 100.0] {
            let frame = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        height: Some(Len::Px(height)),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            let sampled = sample(&element, &frame, false, test_vcx());
            assert_eq!(sampled.node_id, 17);
            assert_eq!(
                sampled.style.used().reset_box_layout.height,
                Some(Len::Px(height))
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.width,
                element.style.used().reset_box_layout.width
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.margin.top,
                element.style.used().reset_box_layout.margin.top
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.padding.left,
                element.style.used().reset_box_layout.padding.left
            );
        }
        assert_eq!(
            element.style.used().reset_box_layout.height,
            Some(Len::Px(100.0))
        );
    }

    #[test]
    fn sampled_percent_and_color_replace_actual_node_declarations() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.style.used_mut().reset_box_layout.width = Some(Len::Px(200.0));
        element.style.used_mut().reset_box_layout.height = Some(Len::Px(100.0));
        element.style.used_mut().inh.ui.color = Some(Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        });
        element.style.used_mut().reset_transforms.rotate_prop = Some(10.0);
        let frame = {
            let mut with_inherited = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        width: Some(Len::Pct(0.5)),
                        ..Default::default()
                    },
                ),
                reset_transforms: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetTransforms {
                        rotate_prop: Some(20.0),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.ui.color = Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
            with_inherited
        };
        let sampled = sample(&element, &frame, false, test_vcx());
        assert_eq!(
            sampled.style.used().reset_box_layout.width,
            frame.reset_box_layout.width
        );
        assert_eq!(
            sampled.style.used().reset_box_layout.height,
            element.style.used().reset_box_layout.height
        );
        assert_eq!(sampled.style.cascaded().inh.ui.color, frame.inh.ui.color);
        assert_eq!(
            sampled.style.used().reset_transforms.rotate_prop,
            element.style.used().reset_transforms.rotate_prop
        );
        assert_eq!(
            sample(&element, &frame, true, test_vcx())
                .style
                .used()
                .reset_transforms
                .rotate_prop,
            frame.reset_transforms.rotate_prop
        );
    }
}
