//! Taking a kept paragraph for this frame (`super`), and why a record did not hold.

use super::{AtomBuilder, Counters, FlowReuse, ParaRecord};
use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::paragraph::{AtomSource, KeptAtoms, LazyBuild, Paragraph};
use std::rc::Rc;

/// Why the record of a paragraph did not give this frame its element: the
/// paragraph is built and recorded again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Miss {
    /// The build takes no stand-ins (`FlowReuse::allow`).
    Blocked,
    /// No record of this id: the paragraph was not recorded, or was pruned.
    NoRecord,
    /// A box below the owner was published after the record (`BoxTree::build_epoch`).
    Epoch,
    /// The owner's size follows a sibling whose content changed.
    Siblings,
    /// An input from outside the nodes changed (`ParaKey::changed`, journal `paragraph.key`).
    Key,
    /// The slot is empty: the recorded element was not painted, so it never moved there.
    Empty,
    /// An atom's node is not where the record found it.
    Atoms,
}

/// The number of misses (`KeepStats::missed`).
pub(super) const MISSES: usize = Miss::Atoms as usize + 1;

impl Miss {
    fn note(self) {
        match self {
            Miss::Blocked => {
                crate::work::add!(ParagraphsMissBlocked, 1);
            }
            Miss::NoRecord => {
                crate::work::add!(ParagraphsMissNoRecord, 1);
            }
            Miss::Epoch => {
                crate::work::add!(ParagraphsMissEpoch, 1);
            }
            Miss::Siblings => {
                crate::work::add!(ParagraphsMissSiblings, 1);
            }
            Miss::Key => {
                crate::work::add!(ParagraphsMissKey, 1);
            }
            Miss::Empty => {
                crate::work::add!(ParagraphsMissEmpty, 1);
            }
            Miss::Atoms => {
                crate::work::add!(ParagraphsMissAtoms, 1);
            }
        }
    }
}

/// The kept paragraph of record `id`, if it holds (see `super`); the miss is
/// counted in the journal.
pub(super) fn resume(
    flow: &Rc<FlowReuse>,
    id: u64,
    (nodes, inherited, opts): (&[Node], &BoxStyle, &RenderOpts),
    start: Counters,
    builder: AtomBuilder,
) -> Option<Paragraph> {
    match take(flow, id, (nodes, inherited, opts), start, builder) {
        Ok(para) => Some(para),
        Err(miss) => {
            flow.store.paragraphs.miss(miss);
            miss.note();
            None
        }
    }
}

fn take(
    flow: &Rc<FlowReuse>,
    id: u64,
    (nodes, inherited, opts): (&[Node], &BoxStyle, &RenderOpts),
    start: Counters,
    builder: AtomBuilder,
) -> Result<Paragraph, Miss> {
    // A second build of the frame (`FlowReuse::again`) takes the records too: what the
    // discarded first build took went back to its slot (`Paragraph::keep_painted`).
    if !flow.allow {
        return Err(Miss::Blocked);
    }
    let record: Rc<ParaRecord> = (flow.store.paragraphs)
        .with(|map| map.get(&id).cloned())
        .ok_or(Miss::NoRecord)?;
    {
        let boxes = opts.cx.doc().boxes.borrow();
        let epoch = boxes.build_epoch(record.owner).ok_or(Miss::Epoch)?;
        if epoch > record.at || record.at > boxes.clock() {
            return Err(Miss::Epoch);
        }
        if boxes.resized_by_siblings(record.owner, record.at) {
            return Err(Miss::Siblings);
        }
    }
    if let Some(field) = record.key.changed(inherited, opts) {
        crate::why::note("paragraph.key", || field.to_string());
        return Err(Miss::Key);
    }
    let para = record.slot.take().ok_or(Miss::Empty)?;
    let source = match para.kept_atoms() {
        Some(atoms) => AtomSource::of(nodes, &atoms),
        None => AtomSource::of(nodes, &KeptAtoms::default()),
    };
    let Some(source) = source else {
        // The element stays in the slot for no one: the record is made again.
        return Err(Miss::Atoms);
    };
    let (from, to) = record.counters;
    start.advanced(from, to).write(&opts.cx);
    record.seen.set(flow.store.epoch.get());
    let lazy = LazyBuild::new(source, builder(inherited, opts, record.flow_text));
    let para = para
        .resume(record.slot.clone(), lazy)
        .measure_context(&opts.cx);
    Ok(para.rebind_selection(opts.selection_color()))
}
