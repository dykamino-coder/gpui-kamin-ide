//! Коллектор существует только в scoped render context явного запроса.

use super::{LineHeight, Rect, fragments::BlockAxis};
use gpui::{Bounds, Pixels, Window};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

#[derive(Default)]
pub(crate) struct GeometryBoxes {
    pub(crate) border: Vec<Rect>,
    pub(crate) padding: Vec<Rect>,
    pub(crate) content: Vec<Rect>,
    pub(crate) font: Option<(f64, LineHeight)>,
}

pub(crate) struct Collector(
    pub(crate) RefCell<HashMap<u64, GeometryBoxes>>,
    pub(crate) HashMap<u64, Rc<crate::style::computed::Computed>>,
    HashMap<u64, Vec<u64>>,
    Arc<resvg::usvg::fontdb::Database>,
);

struct Silent;

impl Collector {
    pub(crate) fn without_records<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        window.with_render_context(Some(Rc::new(Silent)), f)
    }
    pub(crate) fn record_quad(window: &Window, node: u64, corners: [gpui::Point<Pixels>; 4]) {
        if window.render_context::<Silent>().is_some() {
            return;
        }
        let Some(sink) = window.render_context::<Self>() else {
            return;
        };
        let boxes: &RefCell<HashMap<u64, GeometryBoxes>> = &sink.0;
        let mut all = boxes.borrow_mut();
        let out = all.entry(node).or_default();
        let mut push = |corners, route| {
            let rect = Rect::from_bounds(window.layout_quad_in_viewport(corners, route));
            out.border.push(rect);
            out.padding.push(rect);
            out.content.push(rect);
        };
        if let Some(slices) = window.layout_fragment_slices() {
            for slice in slices.iter() {
                let bounds = Bounds::from_corners(corners[0], corners[0]);
                let bounds = corners.iter().fold(bounds, |bounds, point| {
                    bounds.union(&Bounds::from_corners(*point, *point))
                });
                if !BlockAxis::admits(window, bounds, slice.source_clip) {
                    continue;
                }
                let shift = slice.placement.destination - slice.placement.source;
                push(corners.map(|p| p + shift), Some(slice.placement));
            }
        } else {
            push(corners, None);
        }
    }

    pub(crate) fn for_nodes(
        nodes: &[crate::dom::Node],
        fonts: Arc<resvg::usvg::fontdb::Database>,
    ) -> Self {
        let mut out = Self(RefCell::default(), HashMap::new(), HashMap::new(), fonts);
        let mut pending: Vec<_> = nodes.iter().collect();
        while let Some(node) = pending.pop() {
            if let crate::dom::Node::Element(element) = node {
                out.1.insert(element.node_id, element.style.computed());
                if element.tag == "table" && element.style.used().reset_box_layout.display.is_none()
                    || matches!(
                        element.style.used().reset_box_layout.display,
                        Some(
                            crate::style::computed::Display::Table
                                | crate::style::computed::Display::InlineTable
                        )
                    )
                {
                    let captions = element
                        .children
                        .iter()
                        .filter_map(|child| {
                            let crate::dom::Node::Element(caption) = child else {
                                return None;
                            };
                            (caption.tag == "caption"
                                || caption.style.used().reset_context.is_caption == Some(true))
                            .then_some(caption.node_id)
                        })
                        .collect();
                    out.2.insert(element.node_id, captions);
                }
                pending.extend(&element.children);
            }
        }
        out
    }

    // CSSOM View §6: коробка таблицы и подписи, без анонимной обёртки.
    pub(crate) fn finish_captions(&self) {
        let boxes: &RefCell<HashMap<u64, GeometryBoxes>> = &self.0;
        let mut all = boxes.borrow_mut();
        for (table, captions) in &self.2 {
            for caption in captions {
                let Some(boxes) = all.get(caption) else {
                    continue;
                };
                let (border, padding, content) = (
                    boxes.border.clone(),
                    boxes.padding.clone(),
                    boxes.content.clone(),
                );
                let out = all.entry(*table).or_default();
                out.border.extend(border);
                out.padding.extend(padding);
                out.content.extend(content);
            }
        }
    }

    pub(crate) fn source_style(
        window: &Window,
        node: u64,
    ) -> Option<Rc<crate::style::computed::Computed>> {
        window.render_context::<Self>()?.1.get(&node).cloned()
    }

    pub(crate) fn svg_fonts(window: &Window) -> Option<Arc<resvg::usvg::fontdb::Database>> {
        Some(window.render_context::<Self>()?.3.clone())
    }

    pub(crate) fn record(
        window: &Window,
        node: u64,
        boxes: [Bounds<Pixels>; 3],
        font: (f64, LineHeight),
    ) {
        if window.render_context::<Silent>().is_some() {
            return;
        }
        let Some(sink) = window.render_context::<Self>() else {
            return;
        };
        let sink: Rc<Collector> = sink;
        let collected: &RefCell<HashMap<u64, GeometryBoxes>> = &sink.0;
        let mut all = collected.borrow_mut();
        let out: &mut GeometryBoxes = all.entry(node).or_default();
        out.font = Some(font);
        match window.layout_fragment_slices() {
            Some(slices) => {
                for slice in slices.iter() {
                    // Срез допускает layout-фрагмент, а не обрезает overflow.
                    // Нулевой inline остаётся в своей колонке, но не создаёт
                    // вырожденные копии во всех остальных колонках.
                    let b = boxes[0];
                    let clip = slice.source_clip;
                    if !BlockAxis::admits(window, b, clip) {
                        continue;
                    }
                    let mapped = boxes.map(|r| {
                        let mut r = BlockAxis::slice(window, r, slice.source_clip);
                        r.origin += slice.placement.destination - slice.placement.source;
                        Rect::from_bounds(
                            window.layout_bounds_in_viewport(r, Some(slice.placement)),
                        )
                    });
                    out.border.push(mapped[0]);
                    out.padding.push(mapped[1]);
                    out.content.push(mapped[2]);
                    // Вырожденная коробка на разрыве принадлежит первому
                    // допустившему её фрагменту, а не обоим соседям.
                    if BlockAxis::empty(window, b) {
                        break;
                    }
                }
            }
            None => {
                let mapped =
                    boxes.map(|r| Rect::from_bounds(window.layout_bounds_in_viewport(r, None)));
                out.border.push(mapped[0]);
                out.padding.push(mapped[1]);
                out.content.push(mapped[2]);
            }
        }
    }
}
