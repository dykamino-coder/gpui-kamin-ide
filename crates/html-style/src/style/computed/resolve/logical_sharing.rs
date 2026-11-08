//! Общий результат логических сторон для соседей с одним каскадом (Stylo
//! `style/sharing/mod.rs`: соседи с равными входами делят вычисленный стиль).
//!
//! Подготовка документа раскладывает `margin-block` и другие логические стороны каждого
//! элемента копией его каскадного стиля. У тысяч абзацев с одним правилом `p` выходили
//! тысячи равных копий групп, и общий кэш вычисленной стадии (`materialize::sharing`) уже не
//! узнавал соседей. Ключ — тождество групп входа (`shares_storage`) и письмо коробки; запись
//! держит свои группы, поэтому их адреса не переиспользуются.

use super::*;
use crate::style::computed::writing_mode::WritingMode;
use crate::values::ValueCx;

/// Записей одного прохода: соседей с разными каскадами больше обычно не бывает подряд.
const CAPACITY: usize = 32;

/// Кэш одного прохода `resolve_logical` по документу: кольцо записей на стеке прохода,
/// без выделения памяти (как LRU-кэш соседей Stylo фиксированной ёмкости).
#[derive(Default)]
pub struct LogicalSharing {
    entries: [Option<Entry>; CAPACITY],
    /// Ячейка следующей записи; самые свежие записи стоят перед ней.
    next: usize,
}

struct Entry {
    own: crate::style::computed::identity::Storage,
    writing: (WritingMode, bool, bool),
    value: crate::style::computed::identity::Storage,
}

impl LogicalSharing {
    /// Разложить логические стороны каскадного стиля `own` по `wm`. Попадание берёт
    /// группы общего результата; номер коробки оформления (тождество элемента) остаётся своим.
    pub fn resolve(
        &mut self,
        own: &mut Computed,
        wm: WritingMode,
        parent_vertical: bool,
        is_cell: bool,
        vcx: ValueCx<'_>,
    ) {
        let writing = (wm, parent_vertical, is_cell);
        let recent = (1..=CAPACITY).map(|back| (self.next + CAPACITY - back) % CAPACITY);
        if let Some(hit) = recent
            .filter_map(|slot| self.entries[slot].as_ref())
            .find(|entry| entry.writing == writing && entry.own.matches(own))
        {
            hit.value.share_into(own);
            return;
        }
        let input = crate::style::computed::identity::Storage::of(own);
        own.resolve_logical(wm, parent_vertical, is_cell, vcx);
        self.entries[self.next] = Some(Entry {
            own: input,
            writing,
            value: crate::style::computed::identity::Storage::of(own),
        });
        self.next = (self.next + 1) % CAPACITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn siblings_with_one_cascade_share_resolved_groups() {
        let doc = crate::values::DocumentValues::detached(Default::default());
        let vcx = doc.values();
        let mut own = doc.initial_style();
        own.apply_one("margin-block", "1em", vcx);
        let mut sibling = own.clone();
        sibling.decor_box = 7;
        let mut cache = LogicalSharing::default();
        let wm = WritingMode::default();
        let mut first = own.clone();
        cache.resolve(&mut first, wm, false, false, vcx);
        let mut second = sibling;
        cache.resolve(&mut second, wm, false, false, vcx);
        assert!(first.shares_storage(&second));
        assert_eq!(second.decor_box, 7);
        assert_eq!(first.reset_box_layout.margin.top, Some(Len::Em(1.0)));
        let mut alone = own.clone();
        alone.resolve_logical(wm, false, false, vcx);
        assert_eq!(alone, first);
        let mut vertical = own.clone();
        cache.resolve(&mut vertical, wm, true, false, vcx);
        assert!(!vertical.shares_storage(&first));
    }
}
