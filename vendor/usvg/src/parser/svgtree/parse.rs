//! Parse SVG into a typed tree before instance import and conversion.
// Copyright 2021 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use roxmltree::Error;
use simplecss::Declaration;
use svgtypes::FontShorthand;

use super::{AId, Attribute, Document, EId, NodeData, NodeId, NodeKind, ShortRange};

mod attributes;
mod css;
mod elements;
mod external;
mod owned;
mod storage;
mod uses;
use attributes::append_attribute;
use css::{XmlNode, resolve_css};
pub(crate) use elements::parse_svg_element;
use uses::parse_svg_use_element;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE_NS: &str = "http://www.w3.org/XML/1998/namespace";

impl<'input> Document<'input> {
    /// Parses a [`Document`] from a [`roxmltree::Document`].
    pub fn parse_tree(
        xml: &roxmltree::Document<'input>,
        injected_stylesheet: Option<&'input str>,
    ) -> Result<Document<'input>, Error> {
        parse(xml, injected_stylesheet, false)
    }
}

fn parse<'input>(
    xml: &roxmltree::Document<'input>,
    injected_stylesheet: Option<&'input str>,
    resource: bool,
) -> Result<Document<'input>, Error> {
    let mut doc = Document {
        nodes: Vec::new(),
        attrs: Vec::new(),
        links: HashMap::new(),
    };

    // build a map of id -> node for resolve_href
    let mut id_map = HashMap::new();
    for node in xml.descendants() {
        if let Some(id) = node.attribute("id") {
            if !id_map.contains_key(id) {
                id_map.insert(id, node);
            }
        }
    }

    // Add a root node.
    doc.nodes.push(NodeData {
        parent: None,
        next_sibling: None,
        children: None,
        kind: NodeKind::Root,
    });

    let style_sheet = resolve_css(xml, injected_stylesheet);

    if resource && parse_tag_name(xml.root_element()).is_none() {
        for node in xml.descendants().filter(|n| {
            parse_tag_name(*n).is_some()
                && n.parent_element()
                    .is_none_or(|p| parse_tag_name(p).is_none())
        }) {
            parse_xml_node(
                node,
                node,
                doc.root().id,
                &style_sheet,
                false,
                0,
                &mut doc,
                &id_map,
            )?;
        }
    } else {
        parse_xml_node_children(
            xml.root(),
            xml.root(),
            doc.root().id,
            &style_sheet,
            false,
            0,
            &mut doc,
            &id_map,
        )?;
    }

    // Check that the root element is `svg`.
    match doc.root().first_element_child() {
        Some(child) => {
            if !resource && child.tag_name() != Some(EId::Svg) {
                return Err(roxmltree::Error::NoRootNode);
            }
        }
        None => return Err(roxmltree::Error::NoRootNode),
    }

    // Collect all elements with `id` attribute.
    let mut links = HashMap::new();
    for node in doc.descendants() {
        if let Some(id) = node.attribute::<&str>(AId::Id) {
            links.insert(id.to_string(), node.id);
        }
    }
    doc.links = links;

    // KaminIDE: one cycle solver over every resource kind and any chain
    // length replaces upstream's per-kind one/two-step checks
    // (fix_recursive_patterns/links/fe_image), which let longer chains
    // overflow the stack.
    super::cycles::fix_resource_cycles(&mut doc);

    Ok(doc)
}

pub(crate) fn parse_tag_name(node: roxmltree::Node) -> Option<EId> {
    if !node.is_element() {
        return None;
    }

    if !matches!(node.tag_name().namespace(), None | Some(SVG_NS)) {
        return None;
    }

    EId::from_str(node.tag_name().name())
}

fn parse_xml_node_children<'input>(
    parent: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    for node in parent.children() {
        parse_xml_node(
            node,
            origin,
            parent_id,
            style_sheet,
            ignore_ids,
            depth,
            doc,
            id_map,
        )?;
    }

    Ok(())
}

fn parse_xml_node<'input>(
    node: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    if depth > 1024 {
        return Err(Error::NodesLimitReached);
    }

    let mut tag_name = match parse_tag_name(node) {
        Some(id) => id,
        None => return Ok(()),
    };

    if tag_name == EId::Style {
        return Ok(());
    }

    // TODO: remove?
    // Treat links as groups.
    if tag_name == EId::A {
        tag_name = EId::G;
    }

    let node_id = parse_svg_element(node, parent_id, tag_name, style_sheet, ignore_ids, doc)?;
    if tag_name == EId::Text {
        super::text::parse_svg_text_element(node, node_id, style_sheet, doc)?;
    } else if tag_name == EId::Use {
        parse_svg_use_element(node, origin, node_id, style_sheet, depth + 1, doc, id_map)?;
    } else {
        parse_xml_node_children(
            node,
            origin,
            node_id,
            style_sheet,
            ignore_ids,
            depth + 1,
            doc,
            id_map,
        )?;
    }

    Ok(())
}
