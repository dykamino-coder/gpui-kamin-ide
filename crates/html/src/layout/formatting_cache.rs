//! Числовые результаты форматирующих контекстов по дереву и входам сборки.

use crate::dom::{BoxStyles, Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

mod eligibility;
pub(crate) use eligibility::eligible;

type Key = (TypeId, u64, u64);

#[derive(Default)]
pub(crate) struct Store(Cell<Frames>);

#[derive(Default)]
struct Frames {
    current: HashMap<Key, Entry>,
    previous: HashMap<Key, Entry>,
}

struct Entry {
    input: Input,
    value: Rc<dyn Any>,
}

struct Input {
    source: Element,
    parent: BoxStyle,
    constraints: (Option<f32>, Option<f32>, Option<f32>, bool),
    viewport: (f32, f32),
    value_viewport: (f32, f32),
    text: gpui::TextStyle,
    line_height: f32,
    fonts: u64,
    root: crate::style::values::value::RootMetrics,
    inert_clear: bool,
}

impl Input {
    fn new(source: &Element, parent: &BoxStyle, opts: &RenderOpts) -> Self {
        Self {
            source: source.clone(),
            parent: parent.clone(),
            constraints: constraints(opts),
            viewport: opts.viewport,
            value_viewport: opts.cx.values.viewport,
            text: opts.text.clone(),
            line_height: opts.normal_line_height,
            fonts: opts.cx.doc().values.font_generation(),
            root: opts.cx.doc().values.root.borrow().clone(),
            inert_clear: opts.cx.values.inert_clear,
        }
    }

    fn same(&self, source: &Element, parent: &BoxStyle, opts: &RenderOpts) -> bool {
        self.parent.same_layout(parent)
            && self.constraints == constraints(opts)
            && self.viewport == opts.viewport
            && self.value_viewport == opts.cx.values.viewport
            && self.text == opts.text
            && self.line_height == opts.normal_line_height
            && self.fonts == opts.cx.doc().values.font_generation()
            && self.root.same_inputs(&opts.cx.doc().values.root.borrow())
            && self.inert_clear == opts.cx.values.inert_clear
            && crate::dom::same_layout_tree_as(
                &self.source,
                &BoxStyles::of(&self.source),
                &self.source.children,
                source,
            )
    }
}

impl Store {
    pub(crate) fn begin_frame(&self) {
        let mut frames = self.0.take();
        frames.previous = std::mem::take(&mut frames.current);
        self.0.set(frames);
    }

    /// Только числа: T не должен владеть элементами, LayoutId или замыканиями кадра.
    pub(crate) fn get<T: Any + Default>(
        &self,
        source: &Element,
        parent: &BoxStyle,
        opts: &RenderOpts,
    ) -> Option<Rc<T>> {
        if !eligible(source, opts) {
            return None;
        }
        let first = source.children.iter().find_map(|node| match node {
            Node::Element(child) if child.node_id != 0 => Some(child.node_id),
            _ => None,
        })?;
        let key = (TypeId::of::<T>(), source.node_id, first);
        // Перемещаем записи целиком: снимки стиля не получают mutable-проекций.
        let mut frames = self.0.take();
        if let Some(entry) = frames
            .current
            .remove(&key)
            .or_else(|| frames.previous.remove(&key))
            && entry.input.same(source, parent, opts)
        {
            let value = entry.value.clone().downcast().ok();
            frames.current.insert(key, entry);
            self.0.set(frames);
            return value;
        }
        let value = Rc::<T>::default();
        frames.current.insert(
            key,
            Entry {
                input: Input::new(source, parent, opts),
                value: value.clone(),
            },
        );
        self.0.set(frames);
        Some(value)
    }
}

fn constraints(opts: &RenderOpts) -> (Option<f32>, Option<f32>, Option<f32>, bool) {
    let c = &opts.cx.constraints;
    (c.cb_width(), c.avail_w(), c.avail_h, c.cell_bfc)
}
