//! Входы layout, которые бокс передаёт боксам, строящимся внутри него.
//!
//! `ConstraintSpace` в Blink / `ContainingBlock` в Servo: родитель решает, что
//! потомки вправе предполагать (ширина containing block, доступные размеры), и
//! передаёт это вместе с вызовом построения. Здесь контекст едет в
//! [`RenderOpts::cx`], который и так доходит до каждой функции построения и
//! клонируется в отложенные замыкания построения — замыкание видит контекст места,
//! которое его создало.
//!
//! [`RenderOpts::cx`]: crate::layout::options::RenderOpts::cx

mod clamp;
mod constraints;
mod contracts;
pub(crate) mod deferred;
mod fragmentation;
mod frame;
mod inline;
mod position;
mod ruby;
pub(crate) mod scope;
mod scroll;
pub(crate) mod subtree;
mod values;

pub(crate) use clamp::ClampContext;
pub(crate) use constraints::{BoxConstraints, FragmentConstraints, WidthBasis};
pub(crate) use contracts::BuildContext;
pub use deferred::DeferredPaintContext;
pub(crate) use fragmentation::FragmentationContext;
pub(crate) use frame::FrameCx;
pub(crate) use inline::InlineContext;
pub(crate) use position::PositioningContext;
pub(crate) use ruby::RubyContext;
pub(crate) use scroll::ScrollContext;
pub(crate) use values::ValueContext;

use crate::layout::options::RenderOpts;
use crate::values::ValueCx;
use std::rc::Rc;

/// Доменные входы поддерева. Чтение заимствует группу, изменение получает `&mut`.
/// Копия сохраняет только скаляры и общие владельцы; данные кадра не копируются.
#[derive(Default)]
pub struct LayoutCx {
    pub(crate) values: ValueContext,
    pub(crate) constraints: BoxConstraints,
    pub(crate) position: PositioningContext,
    pub(crate) clamp: ClampContext,
    pub(crate) fragmentation: FragmentationContext,
    pub deferred: DeferredPaintContext,
    pub(crate) scroll: ScrollContext,
    pub(crate) inline: InlineContext,
    pub(crate) ruby: RubyContext,
    pub(crate) contracts: BuildContext,
}

impl Clone for LayoutCx {
    fn clone(&self) -> Self {
        self.with_doc(self.values.doc.clone())
    }
}

impl LayoutCx {
    /// A copy that builds into `doc`: the replaced document is not made or shared.
    pub(crate) fn with_doc(&self, doc: BuildDoc) -> Self {
        Self {
            values: ValueContext {
                doc,
                viewport: self.values.viewport,
                inert_clear: self.values.inert_clear,
            },
            constraints: self.constraints.clone(),
            position: self.position.clone(),
            clamp: self.clamp.clone(),
            fragmentation: self.fragmentation.clone(),
            deferred: self.deferred.clone(),
            scroll: self.scroll.clone(),
            inline: self.inline.clone(),
            ruby: self.ruby.clone(),
            contracts: self.contracts.clone(),
        }
    }
}

/// The document of a build. `Default` is an empty document, made on first use: a host
/// builds a `Document`'s nodes through `Document::render`, which sets its own, so the
/// host's `LayoutCx::default()` of every frame costs no document state. A copy shares the
/// document with the original (the empty one is made before it is shared).
#[derive(Default)]
pub(crate) struct BuildDoc(std::cell::OnceCell<Rc<crate::document::state::DocState>>);

impl BuildDoc {
    pub(crate) fn of(doc: Rc<crate::document::state::DocState>) -> Self {
        BuildDoc(std::cell::OnceCell::from(doc))
    }

    pub(crate) fn get(&self) -> &Rc<crate::document::state::DocState> {
        self.0.get_or_init(|| {
            crate::document::state::DocState::new(crate::style::css::Media::default())
        })
    }
}

impl Clone for BuildDoc {
    fn clone(&self) -> Self {
        BuildDoc::of(self.get().clone())
    }
}

impl LayoutCx {
    /// The document being built.
    pub(crate) fn doc(&self) -> &Rc<crate::document::state::DocState> {
        &self.values.doc.get()
    }

    /// Layout-phase frame state of the document being built.
    pub(crate) fn layout(&self) -> &Rc<crate::layout::layout_state::LayoutState> {
        &self.values.doc.get().layout
    }

    /// Text backend font state of the document being built.
    pub(crate) fn text_fonts(&self) -> &Rc<crate::text::text_fonts::TextFonts> {
        &self.values.doc.get().text
    }

    /// Text-phase measurements of the document being built.
    pub(crate) fn paragraph(&self) -> &crate::text::paragraph_state::ParagraphState {
        &self.values.doc.get().paragraph
    }

    /// Values of the build: `calc()`/`anchor()` bodies parsed while building
    /// the frame go to the document's frame arena.
    pub(crate) fn vcx(&self) -> ValueCx<'_> {
        let mut values = self
            .values
            .doc
            .get()
            .frame_values()
            .with_viewport(self.values.viewport);
        values.clearance_possible = !self.values.inert_clear;
        values
    }

    /// Values of the build with its frame facts: the nearest fragmented ancestor's line
    /// measure and the memoized length and subtree facts.
    pub(crate) fn frame(&self) -> FrameCx<'_> {
        FrameCx {
            values: self.vcx(),
            lines: self.fragmentation.lines.as_ref(),
            lengths: Some(&self.contracts.lengths),
            subtrees: Some(&self.contracts.subtrees),
            paint: Some(&self.deferred.paint),
            layout: self.layout(),
        }
    }

    /// The next step-8 paint key: taken on entering an element, before its
    /// children are built.
    pub(crate) fn next_paint_key(&self) -> u64 {
        let v = self.deferred.keys.get().wrapping_add(1);
        self.deferred.keys.set(v);
        v
    }
}

impl RenderOpts {
    /// Options for the boxes inside the current one: same document options,
    /// context changed by `f`. Boxed — the build recursion is deep and its
    /// stack frames are tight.
    pub(crate) fn inner(&self, f: impl FnOnce(&mut LayoutCx)) -> Box<RenderOpts> {
        let mut opts = Box::new(self.clone());
        f(&mut opts.cx);
        opts
    }
}
