//! Проверка без записи: есть ли у стиля длины в единицах окна или контейнера, которые решает
//! `Computed::resolve_viewport`. Стиль коробки общий: копия нужна, только когда решать есть что.

use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    /// Есть ли что решать `resolve_viewport`: длины в единицах окна или
    /// контейнера. Типизированные образы проверяются без копирования; для
    /// эллиптических радиусов и отступов линий проверка консервативна.
    pub fn has_viewport_lengths(&self, vcx: ValueCx<'_>) -> bool {
        let vp = |l: &Option<Len>| l.is_some_and(|l| l.needs_viewport_resolution(vcx));
        let sides = |s: &Sides| [&s.top, &s.right, &s.bottom, &s.left].into_iter().any(vp);
        [
            &self.reset_box_layout.width,
            &self.reset_box_layout.height,
            &self.reset_box_layout.min_width,
            &self.reset_box_layout.min_height,
            &self.reset_box_layout.max_width,
            &self.reset_box_layout.max_height,
            &self.reset_flex.flex_basis,
            &self.reset_svg_math.svg_path_length,
            &self.inh.font.font_size,
            &self.reset_border.radius.tl,
            &self.reset_border.radius.tr,
            &self.reset_border.radius.br,
            &self.reset_border.radius.bl,
        ]
        .into_iter()
        .any(vp)
            || [
                &self.reset_box_layout.padding,
                &self.reset_box_layout.snap.padding,
                &self.reset_box_layout.snap.margin,
                &self.reset_box_layout.margin,
                &self.reset_border.border_width,
                &self.reset_positioned.inset,
            ]
            .into_iter()
            .any(sides)
            || self
                .reset_flex
                .gap
                .is_some_and(|(row, col)| vp(&row) || vp(&col))
            || self.has_decor_insets()
            || self.reset_border.radius_ell.is_some()
            || [
                self.reset_background.gradient.as_ref(),
                self.reset_background.gradient_current.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|gradient| gradient.needs_length_resolution(&vp))
            || self
                .reset_background
                .bg_lists
                .iter()
                .flat_map(|(_, layers)| layers)
                .any(|layer| layer.needs_length_resolution(&vp))
            || self
                .image_operands_ref()
                .any(|image| image.needs_length_resolution(&vp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::BoxStyle;
    use crate::style::decoration_paint::DecorationPaint;
    use std::rc::Rc;

    #[test]
    fn resolved_images_keep_style_and_paint_shared() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        for image in [
            "url(a.png)",
            "cross-fade(red 25%, filter(url(a.png), brightness(50%)))",
            "linear-gradient(90deg, #def, #fff)",
            "radial-gradient(20px 30px at 10% 20%, red, blue)",
            "conic-gradient(at 20% 30%, red, blue)",
            "url(a.png), linear-gradient(red 10px, blue 50%)",
        ] {
            let mut c = Computed::default();
            c.apply_one("background-image", image, vcx);
            // Тот же этап нормализации, который предшествует сборке коробок.
            c.resolve_viewport((800.0, 600.0), vcx);
            let style = Rc::new(c);
            let paint = Rc::new(DecorationPaint::from_computed(&style));
            let mut b = BoxStyle::shared(style.clone());
            b.paint = Some(paint.clone());
            b.resolve_viewport((800.0, 600.0), vcx);
            assert!(Rc::ptr_eq(b.style_rc(), &style), "{image}");
            assert!(Rc::ptr_eq(b.paint.as_ref().unwrap(), &paint), "{image}");
        }
    }

    #[test]
    fn image_viewport_resolution_matches_unconditional_walk_and_isolates_siblings() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        for (property, value) in [
            ("background-image", "linear-gradient(red 1vw, blue 10vh)"),
            (
                "background-image",
                "radial-gradient(10vw 20vh at 1vw 2vh, red, blue)",
            ),
            ("background-image", "conic-gradient(at 1vw 2vh, red, blue)"),
            (
                "background-image",
                "url(a.png), radial-gradient(at 1vw 2vh, red, blue)",
            ),
            (
                "content",
                "filter(cross-fade(red 25%, url(a.png)), blur(2vw))",
            ),
            (
                "background-image",
                "filter(url(a.png), drop-shadow(1vw 2vh 0 red))",
            ),
            ("mask-image", "linear-gradient(red 1vw, blue 10vh)"),
            ("border-image-source", "linear-gradient(red 1vw, blue 10vh)"),
            ("shape-outside", "linear-gradient(red 1vw, blue 10vh)"),
            (
                "background-image",
                "linear-gradient(red calc(1vw + 2px), blue)",
            ),
        ] {
            let mut c = Computed::default();
            c.apply_one(property, value, vcx);
            assert!(c.has_viewport_lengths(vcx), "{property}: {value}");
            let mut expected = c.clone();
            expected.resolve_viewport((800.0, 600.0), vcx);
            let style = Rc::new(c);
            let paint = Rc::new(DecorationPaint::from_computed(&style));
            let mut expected_paint = (*paint).clone();
            expected_paint
                .background
                .resolve_viewport(&style, (800.0, 600.0), vcx);
            let mut b = BoxStyle::shared(style.clone());
            b.paint = Some(paint.clone());
            let sibling = b.clone();
            b.resolve_viewport((800.0, 600.0), vcx);
            assert_eq!(&**b.style_rc(), &expected, "{property}: {value}");
            assert_eq!(&**b.paint.as_ref().unwrap(), &expected_paint);
            assert!(Rc::ptr_eq(sibling.style_rc(), &style));
            assert!(Rc::ptr_eq(sibling.paint.as_ref().unwrap(), &paint));
            let mut other = sibling;
            other.resolve_viewport((400.0, 300.0), vcx);
            assert_ne!(&**other.style_rc(), &expected, "{property}: {value}");
        }
    }
}
