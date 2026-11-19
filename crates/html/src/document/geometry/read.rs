//! CSSOM View lookups in one completed layout of one document instance.
use super::*;

pub(super) struct Fragments<'a> {
    pub(super) root: &'a Handle,
    pub(super) ids: &'a HashMap<usize, u64>,
    pub(super) sink: &'a Collector,
    pub(super) base: f32,
    pub(super) state: &'a DocState,
}

/// A synchronous geometry read scope. Its fragment lists are in content order.
/// The diagnostic enumeration additionally includes elements without boxes.
pub struct GeometryRead<'a>(pub(super) Option<Fragments<'a>>);

impl GeometryRead<'_> {
    fn at_path(&self, path: &[usize]) -> Option<Handle> {
        let mut node = self.0.as_ref()?.root.clone();
        for index in path {
            let child = node.children.borrow().get(*index)?.clone();
            node = child;
        }
        Some(node)
    }

    fn with_rects<R>(&self, node: &Handle, read: impl FnOnce(&[Rect]) -> R) -> R {
        let Some(f) = &self.0 else {
            return read(&[]);
        };
        let boxes = f.sink.0.borrow();
        let b = f
            .ids
            .get(&(Rc::as_ptr(node) as usize))
            .and_then(|id| boxes.get(id));
        read(b.map_or(&[], |b| b.border.as_slice()))
    }

    /// Read the ordered fragments for a script-held source node.
    pub fn client_rects_for(&self, node: &Handle) -> Vec<Rect> {
        self.with_rects(node, <[Rect]>::to_vec)
    }

    /// Read the union without copying the node's fragments.
    pub fn bounding_rect_for(&self, node: &Handle) -> Rect {
        self.with_rects(node, Rect::bounding)
    }

    /// CSSOM View §6. The returned list is a static copy.
    pub fn get_client_rects(&self, path: &[usize]) -> Vec<Rect> {
        self.at_path(path)
            .map_or_else(Vec::new, |n| self.client_rects_for(&n))
    }

    /// CSSOM View §6. Bounding requires no intermediate list allocation.
    pub fn get_bounding_client_rect(&self, path: &[usize]) -> Rect {
        self.at_path(path)
            .map_or_else(Rect::default, |n| self.bounding_rect_for(&n))
    }

    pub(super) fn elements(self) -> Vec<ElementGeometry> {
        let Some(Fragments {
            root,
            ids,
            sink,
            base,
            state,
        }) = self.0
        else {
            return Vec::new();
        };
        let mut boxes = sink.0.borrow_mut();
        let mut out = Vec::new();
        let mut pending = vec![(root.clone(), Vec::new())];
        while let Some((node, path)) = pending.pop() {
            if let NodeData::Element { name, attrs, .. } = &node.data {
                let id = ids.get(&(Rc::as_ptr(&node) as usize));
                let b = id.and_then(|id| boxes.remove(id)).unwrap_or_default();
                let attr = |key: &str| {
                    attrs
                        .borrow()
                        .iter()
                        .find(|a| &*a.name.local == key)
                        .map(|a| a.value.to_string())
                        .unwrap_or_default()
                };
                let (font_size, line_height) = id
                    .and_then(|id| sink.1.get(id))
                    .map(|style| crate::layout::geometry::font(style, base, state.values()))
                    .or(b.font)
                    .unwrap_or((16., LineHeight::Normal));
                out.push(ElementGeometry {
                    path: path.clone(),
                    tag: name.local.to_string(),
                    id: attr("id"),
                    class: attr("class"),
                    border_box: Rect::bounding(&b.border),
                    client_rects: b.border,
                    padding_boxes: b.padding,
                    content_boxes: b.content,
                    font_size,
                    line_height,
                });
            }
            for (i, child) in node.children.borrow().iter().enumerate().rev() {
                let mut path = path.clone();
                path.push(i);
                pending.push((child.clone(), path));
            }
        }
        out
    }
}
