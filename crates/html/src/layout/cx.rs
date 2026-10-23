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
mod deferred;
mod fragmentation;
mod inline;
mod position;
mod ruby;
mod scroll;
mod values;

pub(crate) use clamp::ClampContext;
pub(crate) use constraints::BoxConstraints;
pub(crate) use contracts::BuildContext;
pub use deferred::DeferredPaintContext;
pub(crate) use fragmentation::FragmentationContext;
pub(crate) use inline::InlineContext;
pub(crate) use position::PositioningContext;
pub(crate) use ruby::RubyContext;
pub(crate) use scroll::ScrollContext;
pub(crate) use values::ValueContext;

use crate::document::state::ValueCx;
use crate::layout::options::RenderOpts;
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
        Self {
            values: self.values.clone(),
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

/// The document of a build. `Default` is an empty document: a host builds a
/// `Document`'s nodes through `Document::render`, which sets its own.
#[derive(Clone)]
pub(crate) struct BuildDoc(pub(crate) Rc<crate::document::state::DocState>);

impl Default for BuildDoc {
    fn default() -> Self {
        BuildDoc(crate::document::state::DocState::new(
            crate::style::css::Media::default(),
        ))
    }
}

impl LayoutCx {
    /// The document being built.
    pub(crate) fn doc(&self) -> &Rc<crate::document::state::DocState> {
        &self.values.doc.0
    }

    /// Values of the build: `calc()`/`anchor()` bodies parsed while building
    /// the frame go to the document's frame arena.
    pub(crate) fn vcx(&self) -> ValueCx<'_> {
        self.values
            .doc
            .0
            .frame_values()
            .with_lines(self.fragmentation.lines.as_ref())
            .with_viewport(self.values.viewport)
            .with_paint(Some(&self.deferred.paint))
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
