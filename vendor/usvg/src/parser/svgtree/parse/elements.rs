//! SVG typed-tree elements; source cascade remains in its resource scope.
use super::*;
pub(crate) fn parse_svg_element<'input>(
    xml_node: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    tag_name: EId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    doc: &mut Document<'input>,
) -> Result<NodeId, Error> {
    let attrs_start_idx = doc.attrs.len();
    let mut href_idx: Option<usize> = None;

    // Copy presentational attributes first.
    for attr in xml_node.attributes() {
        match attr.namespace() {
            None | Some(SVG_NS) | Some(XLINK_NS) | Some(XML_NAMESPACE_NS) => {}
            _ => continue,
        }

        let aid = match AId::from_str(attr.name()) {
            Some(v) => v,
            None => continue,
        };

        // During a `use` resolving, all `id` attributes must be ignored.
        // Otherwise we will get elements with duplicated id's.
        if ignore_ids && aid == AId::Id {
            continue;
        }

        // For some reason those properties are allowed only inside a `style` attribute and CSS.
        if matches!(aid, AId::MixBlendMode | AId::Isolation | AId::FontKerning) {
            continue;
        } else if aid == AId::ImageRendering
            && matches!(
                attr.value(),
                "smooth" | "high-quality" | "crisp-edges" | "pixelated"
            )
        {
            continue;
        }

        // SVG 2: when both `href` and `xlink:href` are present, the unprefixed
        // `href` takes precedence and the XLink one must be ignored, regardless
        // of their order in the source document.
        // https://www.w3.org/TR/SVG2/linking.html#XLinkRefAttrs
        if aid == AId::Href {
            let is_unprefixed = attr.namespace().is_none();
            let is_xlink = attr.namespace() == Some(XLINK_NS);
            if !is_unprefixed && !is_xlink {
                continue;
            }

            if let Some(idx) = href_idx {
                // An `href` was already stored. Only an unprefixed `href` is
                // allowed to override it; an `xlink:href` is ignored.
                if is_unprefixed {
                    doc.attrs[idx].value = attr.value_storage().clone();
                }
                continue;
            }
        }

        let added = append_attribute(
            parent_id,
            tag_name,
            aid,
            attr.value_storage().clone(),
            false,
            doc,
        );
        if added && aid == AId::Href {
            href_idx = Some(doc.attrs.len() - 1);
        }
    }

    let mut insert_attribute = |aid, value: &str, important: bool| {
        // Check that attribute already exists.
        let idx = doc.attrs[attrs_start_idx..]
            .iter_mut()
            .position(|a| a.name == aid);

        // Append an attribute as usual.
        let added = append_attribute(
            parent_id,
            tag_name,
            aid,
            roxmltree::StringStorage::new_owned(value),
            important,
            doc,
        );

        // Check that attribute was actually added, because it could be skipped.
        if added {
            if let Some(idx) = idx {
                let last_idx = doc.attrs.len() - 1;
                let existing_idx = attrs_start_idx + idx;

                // See https://developer.mozilla.org/en-US/docs/Web/CSS/important
                // When a declaration is important, the order of precedence is reversed.
                // Declarations marked as important in the user-agent style sheets override
                // all important declarations in the user style sheets. Similarly, all important
                // declarations in the user style sheets override all important declarations in the
                // author's style sheets. Finally, all important declarations take precedence over
                // all animations.
                //
                // Which means:
                // 1) Existing is not important, new is not important -> swap
                // 2) Existing is important, new is not important -> don't swap
                // 3) Existing is not important, new is important -> swap
                // 4) Existing is important, new is important -> don't swap (since the order
                // is reversed, so existing important attributes take precedence over new
                // important attributes)
                let has_precedence = !doc.attrs[existing_idx].important;

                if has_precedence {
                    doc.attrs.swap(existing_idx, last_idx);
                }

                // Remove last.
                doc.attrs.pop();
            }
        }
    };

    let mut write_declaration = |declaration: &Declaration| {
        // TODO: perform XML attribute normalization
        let imp = declaration.important;
        let val = declaration.value;

        if declaration.name == "marker" {
            insert_attribute(AId::MarkerStart, val, imp);
            insert_attribute(AId::MarkerMid, val, imp);
            insert_attribute(AId::MarkerEnd, val, imp);
        } else if declaration.name == "font" {
            if let Ok(shorthand) = FontShorthand::from_str(val) {
                // First we need to reset all values to their default.
                insert_attribute(AId::FontStyle, "normal", imp);
                insert_attribute(AId::FontVariant, "normal", imp);
                insert_attribute(AId::FontWeight, "normal", imp);
                insert_attribute(AId::FontStretch, "normal", imp);
                insert_attribute(AId::LineHeight, "normal", imp);
                insert_attribute(AId::FontSizeAdjust, "none", imp);
                insert_attribute(AId::FontKerning, "auto", imp);
                insert_attribute(AId::FontVariantCaps, "normal", imp);
                insert_attribute(AId::FontVariantLigatures, "normal", imp);
                insert_attribute(AId::FontVariantNumeric, "normal", imp);
                insert_attribute(AId::FontVariantEastAsian, "normal", imp);
                insert_attribute(AId::FontVariantPosition, "normal", imp);

                // Then, we set the properties that have been declared.
                shorthand
                    .font_stretch
                    .map(|s| insert_attribute(AId::FontStretch, s, imp));
                shorthand
                    .font_weight
                    .map(|s| insert_attribute(AId::FontWeight, s, imp));
                shorthand
                    .font_variant
                    .map(|s| insert_attribute(AId::FontVariant, s, imp));
                shorthand
                    .font_style
                    .map(|s| insert_attribute(AId::FontStyle, s, imp));
                insert_attribute(AId::FontSize, shorthand.font_size, imp);
                insert_attribute(AId::FontFamily, shorthand.font_family, imp);
            } else {
                log::warn!(
                    "Failed to parse {} value: '{}'",
                    AId::Font,
                    declaration.value
                );
            }
        } else if let Some(aid) = AId::from_str(declaration.name) {
            // Parse only the presentation attributes.
            if aid.is_presentation() {
                insert_attribute(aid, val, imp);
            }
        }
    };

    // Apply CSS.
    for rule in &style_sheet.rules {
        if rule.selector.matches(&XmlNode(xml_node)) {
            for declaration in &rule.declarations {
                write_declaration(declaration);
            }
        }
    }

    // Split a `style` attribute.
    if let Some(value) = xml_node.attribute("style") {
        for declaration in simplecss::DeclarationTokenizer::from(value) {
            write_declaration(&declaration);
        }
    }

    if doc.nodes.len() > 1_000_000 {
        return Err(Error::NodesLimitReached);
    }

    let node_id = doc.append(
        parent_id,
        NodeKind::Element {
            tag_name,
            attributes: ShortRange::new(attrs_start_idx as u32, doc.attrs.len() as u32),
        },
    );

    Ok(node_id)
}
