//! Цветовые входы подготовки документа без сборки коробок (Quirks Mode §3.12).

use crate::document::state::ValueCx;
use crate::dom::SharedStyle;
use crate::style::computed::Computed;
use std::rc::Rc;

/// Вычисленный CSS для распространения свойств документа до сборки коробок:
/// фильтры и смешивание текста не меняют цвет исходного элемента.
pub(crate) fn computed(
    parent: &Computed,
    own: &Computed,
    vcx: ValueCx<'_>,
) -> crate::style::ComputedHandle {
    super::compute::compute(parent, own, vcx).handle
}

/// Цвет подготовки с прежним смешиванием background-clip: text.
/// Подготовка документа не создаёт контекст форматирования и стиль коробки.
pub(crate) fn colors(parent: &Computed, own: &SharedStyle, vcx: ValueCx<'_>) -> Rc<Computed> {
    let published = own.slot().any();
    let element = published.is_some();
    let view = own.used();
    let mut computed = match published {
        Some(_) => Rc::clone(&view),
        None => super::compute::compute(parent, own.cascaded(), vcx).legacy,
    };
    let declared = if element { own.cascaded() } else { &view };
    let colors = super::color_inputs::text_clip_colors(
        parent,
        declared,
        &view,
        &computed,
        element,
        view.background_paint,
        &vcx.background_images(),
    );
    if computed.color != colors.color
        || computed.text_clip_raw != colors.raw
        || computed.text_clip_fill != colors.fill
    {
        let computed = Rc::make_mut(&mut computed);
        computed.color = colors.color;
        computed.text_clip_raw = colors.raw;
        computed.text_clip_fill = colors.fill;
    }
    computed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::cascade::materialize::{box_style_unpainted, materialize};
    use crate::style::computed::BoxStyle;

    #[test]
    fn preparation_preserves_root_and_body_colors_without_box_construction() {
        for style in [
            "color:green",
            "color:rgba(255,0,0,.5);background:blue;background-clip:text",
            "color:currentColor;filter:invert(1)",
            "color:inherit;position:absolute",
        ] {
            for published in [false, true] {
                let parsed = crate::dom::parse(
                    &format!(
                        r#"<html style="color:red;background:green;background-clip:text"><body style="{style}">x</body></html>"#
                    ),
                    "",
                );
                let vcx = parsed.values();
                if published {
                    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
                }
                let root =
                    crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "html").unwrap();
                let body =
                    crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "body").unwrap();
                let old_root = box_style_unpainted(&BoxStyle::default(), &root.style, vcx);
                let new_root = colors(&Computed::default(), &root.style, vcx);
                let old = box_style_unpainted(&old_root, &body.style, vcx);
                let new = colors(&new_root, &body.style, vcx);
                assert_eq!(
                    (
                        new_root.color,
                        new_root.text_clip_raw,
                        new_root.text_clip_fill
                    ),
                    (
                        old_root.color,
                        old_root.text_clip_raw,
                        old_root.text_clip_fill
                    )
                );
                assert_eq!(
                    (new.color, new.text_clip_raw, new.text_clip_fill),
                    (old.color, old.text_clip_raw, old.text_clip_fill),
                    "{style}: {published}"
                );
            }
        }
    }
}
