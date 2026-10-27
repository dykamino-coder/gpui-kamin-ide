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

/// CSS для наследования и отдельный результат окраски подготовки документа.
#[derive(Default)]
pub(crate) struct Colors {
    css: Rc<Computed>,
    pub foreground: crate::style::decoration_paint::ForegroundPaint,
}

pub(crate) fn colors(parent: &Colors, own: &SharedStyle, vcx: ValueCx<'_>) -> Colors {
    let published = own.slot().any();
    let element = published.is_some();
    let view = own.used();
    let css = match published {
        Some(_) => Rc::clone(&view),
        None => super::compute::compute(&parent.css, own.cascaded(), vcx).legacy,
    };
    let declared = if element { own.cascaded() } else { &view };
    let input = if !element && !declared.declares_color() {
        parent.foreground
    } else {
        crate::style::decoration_paint::ForegroundPaint {
            color: css.color,
            ..Default::default()
        }
    };
    let foreground = super::color_inputs::text_clip_colors(
        super::color_inputs::TextClipInput {
            parent: parent.foreground,
            foreground: input,
            element,
            declares_color: declared.declares_color(),
            out_of_flow: crate::style::flow::absolutely_positioned(view.position),
            background: view.background_paint,
        },
        &crate::style::background_paint::BackgroundValues::from_computed(&css),
        &vcx.background_images(),
    );
    Colors { css, foreground }
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
                let new_root = colors(&Colors::default(), &root.style, vcx);
                let old = box_style_unpainted(&old_root, &body.style, vcx);
                let new = colors(&new_root, &body.style, vcx);
                assert_eq!(new_root.foreground, old_root.foreground_paint());
                assert_eq!(
                    new.foreground,
                    old.foreground_paint(),
                    "{style}: {published}"
                );
            }
        }
    }
}
