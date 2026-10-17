//! SVG typed-tree uses; source cascade remains in its resource scope.
use super::*;
fn resolve_href<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
    id_map: &HashMap<&str, roxmltree::Node<'a, 'input>>,
) -> Option<roxmltree::Node<'a, 'input>> {
    // See the comment in `parse_svg_element` about `href` precedence.
    //
    // Note: `roxmltree::Node::attribute("href")` matches by local name only and
    // would return whichever `href`/`xlink:href` comes first, so we have to
    // filter by namespace explicitly.
    let link_value = node
        .attributes()
        .find(|a| a.name() == "href" && a.namespace().is_none())
        .or_else(|| {
            node.attributes()
                .find(|a| a.name() == "href" && a.namespace() == Some(XLINK_NS))
        })
        .map(|a| a.value())?;

    let link_id = svgtypes::IRI::from_str(link_value).ok()?.0;

    id_map.get(link_id).copied()
}

pub(super) fn parse_svg_use_element<'input>(
    node: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    let link = match resolve_href(node, id_map) {
        Some(v) => v,
        None => return Ok(()),
    };

    if link == node || link == origin {
        log::warn!(
            "Recursive 'use' detected. '{}' will be skipped.",
            node.attribute((SVG_NS, "id")).unwrap_or_default()
        );
        return Ok(());
    }

    // Make sure we're linked to an SVG element.
    if parse_tag_name(link).is_none() {
        return Ok(());
    }

    // Check that none of the linked node's children reference current `use` node
    // via other `use` node.
    //
    // Example:
    // <g id="g1">
    //     <use xlink:href="#use1" id="use2"/>
    // </g>
    // <use xlink:href="#g1" id="use1"/>
    //
    // `use2` should be removed.
    //
    // Also, child should not reference its parent:
    // <g id="g1">
    //     <use xlink:href="#g1" id="use1"/>
    // </g>
    //
    // `use1` should be removed.
    //
    // KaminIDE: the check is transitive (any chain length), like Blink's
    // SVGUseElement cycle check: `use` is skipped when expanding its target
    // can reach `use` itself again through nested `use` elements. Upstream
    // looked one step deep and otherwise ran into the 1024 depth limit.
    let is_recursive = use_reaches_itself(node, link, id_map);

    if is_recursive {
        log::warn!(
            "Recursive 'use' detected. '{}' will be skipped.",
            node.attribute((SVG_NS, "id")).unwrap_or_default()
        );
        return Ok(());
    }

    parse_xml_node(
        link,
        node,
        parent_id,
        style_sheet,
        true,
        depth + 1,
        doc,
        id_map,
    )
}

/// Whether expanding `link` (the target of `use_node`) reaches `use_node`
/// again, directly or through other `use` targets. Each target is walked once.
fn use_reaches_itself<'input>(
    use_node: roxmltree::Node<'_, 'input>,
    link: roxmltree::Node<'_, 'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> bool {
    let mut seen = std::collections::HashSet::from([link.id()]);
    let mut stack = vec![link];
    while let Some(target) = stack.pop() {
        for inner in target
            .descendants()
            .filter(|n| n.has_tag_name((SVG_NS, "use")))
        {
            if inner == use_node {
                return true;
            }
            if let Some(next) = resolve_href(inner, id_map) {
                if seen.insert(next.id()) {
                    stack.push(next);
                }
            }
        }
    }
    false
}
