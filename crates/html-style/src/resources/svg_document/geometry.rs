//! Запрос SVG-коробок из канонического конвертера; соответствия живут только здесь.

use crate::{dom::Element, values::ValueCx};
use resvg::usvg::{self, GeometryTrace, Rect, Transform};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// Углы объектной коробки в системе координат канвы SVG, без обрезки и stroke.
pub struct SourceBox {
    pub node: u64,
    pub corners: [(f32, f32); 4],
}

/// Тот же разбор, шрифты и загрузчик, что у растеризации документа.
pub fn query(e: &Element, width: f32, height: f32, vcx: ValueCx<'_>) -> Vec<SourceBox> {
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
    let markup = super::serialize::serialize_sized_with_trace(
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
    let _ = vcx.doc.images.borrow().query_svg(&markup, capture.clone());
    std::mem::take(&mut capture.state().boxes)
}

#[derive(Default)]
struct SvgGeometryState {
    root: u64,
    resources: HashSet<u64>,
    offsets: HashMap<usize, u64>,
    nodes: HashMap<usize, (u64, usize)>,
    text_nodes: HashSet<usize>,
    text: Option<SvgGeometryText>,
    groups: Vec<SvgGeometryGroup>,
    boxes: Vec<SourceBox>,
}
struct SvgGeometryGroup {
    node: usize,
    suppressed: bool,
    absolute: Transform,
    local: Transform,
    bounds: Option<Rect>,
}
struct SvgGeometryText {
    root: usize,
    transform: Transform,
    chars: HashMap<(usize, usize), usize>,
    boxes: HashMap<usize, Rect>,
}
#[derive(Clone)]
struct Capture(Rc<RefCell<SvgGeometryState>>);

impl Capture {
    fn state(&self) -> std::cell::RefMut<'_, SvgGeometryState> {
        let state: &RefCell<SvgGeometryState> = &self.0;
        state.borrow_mut()
    }
}

impl SvgGeometryState {
    fn resource(&self, mut node: usize) -> bool {
        while let Some(&(source, parent)) = self.nodes.get(&node) {
            if self.resources.contains(&source) {
                return true;
            }
            if node == parent {
                break;
            }
            node = parent;
        }
        false
    }
    fn object(&mut self, node: usize, rect: Rect, transform: Transform) {
        let Some(&(source, _)) = self.nodes.get(&node) else {
            return;
        };
        if source == self.root {
            return;
        }
        let mut points = [
            usvg::tiny_skia_path::Point::from_xy(rect.left(), rect.top()),
            usvg::tiny_skia_path::Point::from_xy(rect.right(), rect.top()),
            usvg::tiny_skia_path::Point::from_xy(rect.left(), rect.bottom()),
            usvg::tiny_skia_path::Point::from_xy(rect.right(), rect.bottom()),
        ];
        transform.map_points(&mut points);
        self.boxes.push(SourceBox {
            node: source,
            corners: points.map(|p| (p.x, p.y)),
        });
    }
}

impl GeometryTrace for Capture {
    fn group_start(&mut self, node: usize, absolute: Transform, local: Transform) {
        let mut state = self.state();
        let suppressed = state.resource(node);
        state.groups.push(SvgGeometryGroup {
            node,
            suppressed,
            absolute,
            local,
            bounds: None,
        });
    }
    fn group_end(&mut self) {
        let mut state = self.state();
        let Some(group) = state.groups.pop() else {
            return;
        };
        if group.suppressed {
            return;
        }
        if let Some(rect) = group.bounds {
            if !state.text_nodes.contains(&group.node) {
                state.object(group.node, rect, group.absolute);
            }
            if let Some(parent) = state.groups.last_mut() {
                if let Some(rect) = rect.transform(group.local) {
                    extend(&mut parent.bounds, rect);
                }
            }
        }
    }
    fn parsed(&mut self, node: usize, parent: usize, offset: usize) {
        let mut state = self.state();
        if let Some(&source) = state.offsets.get(&offset) {
            state.nodes.insert(node, (source, parent));
        }
    }
    fn object(&mut self, _node: usize, rect: Rect, _transform: Transform) {
        let mut state = self.state();
        if let Some(group) = state.groups.last_mut() {
            if !group.suppressed {
                extend(&mut group.bounds, rect);
            }
        }
    }
    fn text_start(&mut self, node: usize, transform: Transform) {
        let mut state = self.state();
        if state.resource(node) {
            return;
        }
        state.text_nodes.insert(node);
        state.text = Some(SvgGeometryText {
            root: node,
            transform,
            chars: HashMap::new(),
            boxes: HashMap::new(),
        });
    }
    fn text_char(&mut self, node: usize, chunk: usize, byte: usize) {
        if let Some(text) = &mut self.state().text {
            text.chars.insert((chunk, byte), node);
        }
    }
    fn text_cell(&mut self, chunk: usize, byte: usize, rect: Rect, transform: Transform) {
        let mut state = self.state();
        let Some(mut text) = state.text.take() else {
            return;
        };
        if let Some(mut node) = text.chars.get(&(chunk, byte)).copied() {
            if let Some(rect) = rect.transform(transform) {
                loop {
                    text.boxes
                        .entry(node)
                        .and_modify(|r| {
                            if let Some(union) = Rect::from_ltrb(
                                r.left().min(rect.left()),
                                r.top().min(rect.top()),
                                r.right().max(rect.right()),
                                r.bottom().max(rect.bottom()),
                            ) {
                                *r = union;
                            }
                        })
                        .or_insert(rect);
                    if node == text.root {
                        break;
                    }
                    let Some(&(_, parent)) = state.nodes.get(&node) else {
                        break;
                    };
                    if parent == node {
                        break;
                    }
                    node = parent;
                }
            }
        }
        state.text = Some(text);
    }
    fn text_end(&mut self) {
        let mut state = self.state();
        if let Some(text) = state.text.take() {
            let mut boxes: Vec<_> = text.boxes.into_iter().collect();
            boxes.sort_by_key(|&(node, _)| node);
            for (node, rect) in boxes {
                if node == text.root {
                    if let Some(group) = state.groups.last_mut() {
                        extend(&mut group.bounds, rect);
                    }
                }
                state.object(node, rect, text.transform);
            }
        }
    }
}

fn extend(bounds: &mut Option<Rect>, rect: Rect) {
    *bounds = match *bounds {
        Some(old) => Rect::from_ltrb(
            old.left().min(rect.left()),
            old.top().min(rect.top()),
            old.right().max(rect.right()),
            old.bottom().max(rect.bottom()),
        ),
        None => Some(rect),
    };
}
