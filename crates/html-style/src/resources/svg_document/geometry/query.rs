//! Разбор исходного SVG для синхронного запроса с базой шрифтов документа.

use super::{Capture, SourceBox, SvgGeometryState};
use crate::{dom::Element, values::ValueCx};
use resvg::usvg;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

/// Тот же разбор, шрифты и загрузчик, что у растеризации документа.
pub fn query(
    e: &Element,
    width: f32,
    height: f32,
    vcx: ValueCx<'_>,
    fonts: Arc<usvg::fontdb::Database>,
) -> Vec<SourceBox> {
    let mut resources = HashSet::new();
    let mut pending = vec![e];
    while let Some(element) = pending.pop() {
        if matches!(
            element.tag.as_str(),
            "defs" | "clipPath" | "mask" | "marker" | "pattern" | "filter" | "symbol"
        ) {
            resources.insert(element.node_id);
        }
        pending.extend(element.children.iter().filter_map(|node| match node {
            crate::dom::Node::Element(child) => Some(child),
            _ => None,
        }));
    }
    let mut offsets = HashMap::new();
    let markup = super::super::serialize::serialize_sized_with_trace(
        e,
        width,
        height,
        0.,
        0.,
        vcx,
        &mut |node, at| {
            offsets.insert(at, node);
        },
    );
    let capture = Capture(Rc::new(RefCell::new(SvgGeometryState {
        root: e.node_id,
        offsets,
        resources,
        ..SvgGeometryState::default()
    })));
    let _ = vcx
        .doc
        .images
        .borrow()
        .query_svg(&markup, fonts, capture.clone());
    std::mem::take(&mut capture.state().boxes)
}
