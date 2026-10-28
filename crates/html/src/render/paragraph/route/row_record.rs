//! Записи физического ряда во фрагментации: каждый кусок сообщает свой узел раскладки, сам
//! ряд — себя вместе с ними (`FlowMeasure::record_row`). Обёртки прозрачны для раскладки и
//! краски.

use crate::hooks::{IntoHooked, Wrap};
use crate::layout::fragment::flow::FlowMeasure;
use crate::layout::fragment::types::StackAxis;
use gpui::{AnyElement, App, LayoutId, Window};
use std::cell::RefCell;
use std::rc::Rc;

type Pieces = Rc<RefCell<Vec<LayoutId>>>;

/// Рекордер одного ряда.
#[derive(Clone)]
pub(super) struct RowRecord {
    flow: Rc<FlowMeasure>,
    axis: StackAxis,
    keep: (usize, usize),
    pieces: Pieces,
}

impl RowRecord {
    pub(super) fn new(flow: Rc<FlowMeasure>, axis: StackAxis, keep: (usize, usize)) -> Self {
        Self {
            flow,
            axis,
            keep,
            pieces: Rc::default(),
        }
    }

    /// Кусок ряда, чей узел войдёт в его линии.
    pub(super) fn piece(&self, child: AnyElement) -> AnyElement {
        Piece {
            child,
            pieces: self.pieces.clone(),
        }
        .hooked()
    }

    /// Сам ряд.
    pub(super) fn row(self, child: AnyElement) -> AnyElement {
        Row {
            child,
            record: self,
        }
        .hooked()
    }
}

impl RowRecord {
    /// Recorder of a row built under a fragmentation recorder; a turned row
    /// is measured as its own root.
    pub(super) fn for_row(
        inherited: &crate::style::computed::BoxStyle,
        opts: &crate::layout::options::RenderOpts,
    ) -> Option<Self> {
        let watch = opts.frag.watch.as_ref()?;
        (!inherited.wm().turned()).then(|| {
            Self::new(
                watch.flow.clone(),
                StackAxis::of(inherited.wm()),
                (
                    usize::from(inherited.orphans.unwrap_or(2)),
                    usize::from(inherited.widows.unwrap_or(2)),
                ),
            )
        })
    }
}

/// Atomic pieces report their nodes to the row's recorder.
pub(super) fn pieces(
    record: Option<&RowRecord>,
    pieces: Vec<crate::text::inline::Piece>,
) -> Vec<crate::text::inline::Piece> {
    use crate::text::inline::Piece;
    match record {
        Some(r) => pieces
            .into_iter()
            .map(|p| match p {
                Piece::Atom(el) => Piece::Atom(r.piece(el)),
                p => p,
            })
            .collect(),
        None => pieces,
    }
}
struct Piece {
    child: AnyElement,
    pieces: Pieces,
}

impl Wrap for Piece {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.pieces.borrow_mut().push(id);
        id
    }
}

struct Row {
    child: AnyElement,
    record: RowRecord,
}

impl Wrap for Row {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        self.record.pieces.borrow_mut().clear();
        let id = self.child.request_layout(window, cx);
        let r = &self.record;
        r.flow
            .record_row(id, &r.pieces.borrow(), r.axis, r.keep, window);
        id
    }
}
