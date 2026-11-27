//! Память стадии коробки на слоте стиля узла. Стадия коробки (`inherit::box_stage`) —
//! функция своих входов: коробки-родителя (и родителя по дереву элементов у перенесённой
//! коробки), стадий своего стиля, поколения шрифтов, схемы и метрик корня документа и размера
//! окна — последнего, только если значения его прочитали (`values::viewport_reads`). Запись
//! живёт при стиле узла (`dom::SharedStyle`) и годна, пока равны все входы: смена ширины окна
//! пересчитывает только коробки, читавшие окно, и коробки под изменившимся родителем; правка
//! стиля или DOM даёт новые стадии стиля и промах при следующем запросе — синхронный запрос
//! геометрии видит новый стиль без кадра.
//!
//! Так же устроены лучшие движки: Blink держит `ComputedStyle` на элементе и пересчитывает
//! его по `StyleRecalcChange`, Stylo — в данных элемента по restyle hints; общий результат
//! переиспользуется сравнением входов (`MatchedPropertiesCache`, sharing cache), а не
//! идентичностью элемента. Ключи реестров кадра (`self_node` → `cb_node`) в запись не
//! входят: попадание переносит их от родителя этого кадра (`inherit::reidentify`).

use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use std::cell::RefCell;
use std::rc::Rc;

/// Records per style: painted and unpainted stages, a moved box, copies of the element.
const ENTRIES: usize = 4;

mod key;
pub use key::OwnKey;
use key::{DocInputs, Viewport, doc_inputs, viewport};
pub(crate) use key::{Identity, Miss, identity};

/// The inputs of one stage request.
pub(crate) struct Query<'a> {
    pub paint: bool,
    pub layout: &'a BoxStyle,
    /// The box parent by the element tree, for a box layout moved under `layout`.
    pub tree: Option<&'a BoxStyle>,
    pub own: OwnKey,
    pub vcx: ValueCx<'a>,
}

struct Entry {
    paint: bool,
    layout: Rc<BoxStyle>,
    tree: Option<Rc<BoxStyle>>,
    own: OwnKey,
    doc: DocInputs,
    viewport: Option<Viewport>,
    out: BoxStyle,
}

impl Entry {
    fn check(&self, q: &Query<'_>, doc: DocInputs) -> Result<(), Miss> {
        if !self.own.same_stages(&q.own) {
            return Err(Miss::Own);
        }
        if self.own.container != q.own.container {
            return Err(Miss::Container);
        }
        if self.doc.fonts != doc.fonts {
            return Err(Miss::Fonts);
        }
        if self.doc != doc {
            return Err(Miss::Document);
        }
        let tree = match (&self.tree, q.tree) {
            (Some(a), Some(b)) => a.same_stage_parent(b),
            (a, b) => a.is_none() && b.is_none(),
        };
        if !tree || !self.layout.same_stage_parent(q.layout) {
            return Err(Miss::Parent);
        }
        if self.viewport.is_some_and(|v| v != viewport(q.vcx)) {
            return Err(Miss::Viewport);
        }
        Ok(())
    }
}

/// The remembered box stages of one style, shared by the copies of its element: the table
/// exists from the start, so a copy made before the first stage (render works on copies of
/// the document's elements) still reads and fills the document's records.
#[derive(Clone, Default)]
pub struct StageMemo(Rc<RefCell<Vec<Entry>>>);

impl std::fmt::Debug for StageMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0.borrow().len();
        write!(f, "StageMemo({n})")
    }
}

impl StageMemo {
    /// The remembered stage for `q`, or why there is none.
    pub(crate) fn find(&self, q: &Query<'_>) -> Result<BoxStyle, Miss> {
        let entries = &self.0;
        let doc = doc_inputs(q.vcx);
        let mut reason = Miss::First;
        let entries = entries.borrow();
        let same_kind = |e: &&Entry| e.paint == q.paint && e.tree.is_some() == q.tree.is_some();
        for entry in entries.iter().filter(same_kind) {
            match entry.check(q, doc) {
                Ok(()) => {
                    // The values read the viewport for an enclosing computation too.
                    if entry.viewport.is_some() {
                        crate::values::viewport_reads::note(q.vcx);
                    }
                    return Ok(entry.out.clone());
                }
                // A record of this copy of the element names the input that changed.
                Err(miss) if miss != Miss::Own => return Err(miss),
                Err(miss) => reason = miss,
            }
        }
        Err(reason)
    }

    /// Remember `out` computed for `q`; `read_viewport` — its values read the viewport size.
    pub(crate) fn store(&self, q: Query<'_>, read_viewport: bool, out: &BoxStyle) {
        let entry = Entry {
            paint: q.paint,
            layout: shared_parent(q.vcx, q.layout),
            tree: q.tree.map(|t| shared_parent(q.vcx, t)),
            doc: doc_inputs(q.vcx),
            viewport: read_viewport.then(|| viewport(q.vcx)),
            own: q.own,
            out: out.clone(),
        };
        let mut entries = self.0.borrow_mut();
        let same = entries.iter().position(|e| {
            e.paint == entry.paint
                && e.tree.is_some() == entry.tree.is_some()
                && e.own.same_stages(&entry.own)
        });
        match same {
            Some(i) => entries[i] = entry,
            None => {
                if entries.len() == ENTRIES {
                    entries.remove(0);
                }
                // Most styles keep one record: grow by one, not by the default four.
                entries.reserve_exact(1);
                entries.push(entry);
            }
        }
    }

    /// The records of the style, without the values they share (`heap-report`).
    #[cfg(feature = "heap-report")]
    pub(crate) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        let rc = &self.0;
        ops.at("style/box-stage", |ops| {
            let block = ops.shallow(Rc::as_ptr(rc));
            let entries = rc.borrow();
            if block == 0 || entries.capacity() == 0 {
                return block;
            }
            let parents: usize = (entries.iter())
                .flat_map(|e| std::iter::once(&e.layout).chain(&e.tree))
                .map(|p| ops.shallow(Rc::as_ptr(p)))
                .sum();
            block + ops.shallow(entries.as_ptr()) + parents
        })
    }
}

/// The parent as a key, shared with the siblings keyed by an equal one (children are styled
/// one after another under one parent). Its own position-try source is no input of the child,
/// and a style handle there would keep that element's records alive from this one.
fn shared_parent(vcx: ValueCx<'_>, parent: &BoxStyle) -> Rc<BoxStyle> {
    let mut last = vcx.doc.traversal.stage_parent.borrow_mut();
    if let Some(shared) = &*last
        && shared.same_stage_parent(parent)
    {
        return shared.clone();
    }
    let mut key = parent.clone();
    key.try_style = None;
    let key = Rc::new(key);
    *last = Some(key.clone());
    key
}

/// The size of one record.
#[cfg(test)]
pub(crate) const fn entry_bytes() -> usize {
    std::mem::size_of::<Entry>()
}

#[cfg(test)]
#[path = "stage_memo_tests.rs"]
mod tests;
