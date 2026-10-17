//! SVG typed-tree attributes; source cascade remains in its resource scope.
use super::*;
pub(super) fn append_attribute<'input>(
    parent_id: NodeId,
    tag_name: EId,
    aid: AId,
    value: roxmltree::StringStorage<'input>,
    important: bool,
    doc: &mut Document<'input>,
) -> bool {
    match aid {
        // The `style` attribute will be split into attributes, so we don't need it.
        AId::Style |
        // No need to copy a `class` attribute since CSS were already resolved.
        AId::Class => return false,
        _ => {}
    }

    // Ignore `xlink:href` on `tspan` (which was originally `tref` or `a`),
    // because we will convert `tref` into `tspan` anyway.
    if tag_name == EId::Tspan && aid == AId::Href {
        return false;
    }

    if aid.allows_inherit_value() && &*value == "inherit" {
        return resolve_inherit(parent_id, aid, doc);
    }

    doc.append_attribute(aid, value, important);
    true
}

fn resolve_inherit(parent_id: NodeId, aid: AId, doc: &mut Document) -> bool {
    if aid.is_inheritable() {
        // Inheritable attributes can inherit a value from an any ancestor.
        let node_id = doc
            .get(parent_id)
            .ancestors()
            .find(|n| n.has_attribute(aid))
            .map(|n| n.id);
        if let Some(node_id) = node_id {
            if let Some(attr) = doc
                .get(node_id)
                .attributes()
                .iter()
                .find(|a| a.name == aid)
                .cloned()
            {
                doc.attrs.push(Attribute {
                    name: aid,
                    value: attr.value,
                    important: attr.important,
                });

                return true;
            }
        }
    } else {
        // Non-inheritable attributes can inherit a value only from a direct parent.
        if let Some(attr) = doc
            .get(parent_id)
            .attributes()
            .iter()
            .find(|a| a.name == aid)
            .cloned()
        {
            doc.attrs.push(Attribute {
                name: aid,
                value: attr.value,
                important: attr.important,
            });

            return true;
        }
    }

    // Fallback to a default value if possible.
    let value = match aid {
        AId::ImageRendering | AId::ShapeRendering | AId::TextRendering => "auto",

        AId::ClipPath
        | AId::Filter
        | AId::MarkerEnd
        | AId::MarkerMid
        | AId::MarkerStart
        | AId::Mask
        | AId::Stroke
        | AId::StrokeDasharray
        | AId::TextDecoration => "none",

        AId::FontStretch
        | AId::FontStyle
        | AId::FontVariant
        | AId::FontWeight
        | AId::LetterSpacing
        | AId::WordSpacing => "normal",

        AId::Fill | AId::FloodColor | AId::StopColor => "black",

        AId::FillOpacity
        | AId::FloodOpacity
        | AId::Opacity
        | AId::StopOpacity
        | AId::StrokeOpacity => "1",

        AId::ClipRule | AId::FillRule => "nonzero",

        AId::BaselineShift => "baseline",
        AId::ColorInterpolationFilters => "linearRGB",
        AId::Direction => "ltr",
        AId::Display => "inline",
        AId::FontSize => "medium",
        AId::Overflow => "visible",
        AId::StrokeDashoffset => "0",
        AId::StrokeLinecap => "butt",
        AId::StrokeLinejoin => "miter",
        AId::StrokeMiterlimit => "4",
        AId::StrokeWidth => "1",
        AId::TextAnchor => "start",
        AId::Visibility => "visible",
        AId::WritingMode => "lr-tb",
        _ => return false,
    };

    doc.append_attribute(aid, roxmltree::StringStorage::Borrowed(value), false);
    true
}
