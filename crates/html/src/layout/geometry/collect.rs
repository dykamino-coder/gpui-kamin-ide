//! Коллектор существует только в scoped render context явного запроса.

use super::{LineHeight, Rect};
use gpui::{Bounds, Pixels, Window};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Default)]
pub(crate) struct Boxes {
    pub(crate) border: Vec<Rect>,
    pub(crate) padding: Vec<Rect>,
    pub(crate) content: Vec<Rect>,
    pub(crate) font: Option<(f64, LineHeight)>,
}

#[derive(Default)]
pub(crate) struct Collector(pub(crate) RefCell<HashMap<u64, Boxes>>);

impl Collector {
    pub(crate) fn record(
        window: &Window,
        node: u64,
        boxes: [Bounds<Pixels>; 3],
        font: (f64, LineHeight),
    ) {
        let Some(sink) = window.render_context::<Self>() else {
            return;
        };
        let sink: Rc<Collector> = sink;
        let mut all = sink.0.borrow_mut();
        let out: &mut Boxes = all.entry(node).or_default();
        out.font = Some(font);
        match window.layout_fragment_slices() {
            Some(slices) => {
                for slice in slices.iter() {
                    // Срез допускает layout-фрагмент, а не обрезает overflow.
                    // Нулевой inline остаётся в своей колонке, но не создаёт
                    // вырожденные копии во всех остальных колонках.
                    let b = boxes[0];
                    let clip = slice.source_clip;
                    if b.right() < clip.left()
                        || b.left() > clip.right()
                        || b.bottom() <= clip.top()
                        || b.top() >= clip.bottom()
                    {
                        continue;
                    }
                    let mapped = boxes.map(|r| {
                        let mut r = r.intersect(&slice.source_clip);
                        r.origin += slice.placement.destination - slice.placement.source;
                        Rect::from_bounds(
                            window.layout_bounds_in_viewport(r, Some(slice.placement)),
                        )
                    });
                    out.border.push(mapped[0]);
                    out.padding.push(mapped[1]);
                    out.content.push(mapped[2]);
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
