//! Заместитель блочного ребёнка корневого потока по его записи.

use super::super::placeholder::{Placeholder, Slot};
use super::{Counters, FlowCx};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;
use std::rc::Rc;

impl FlowCx {
    /// Заместитель ребёнка `e`, если его запись (блока, бывшего далеко от
    /// экрана) подходит.
    /// `edge` — первый или последний элемент потока: его базовые линии может
    /// читать контейнер, такой ребёнок всегда собирается.
    pub(crate) fn placeholder(
        &self,
        e: &Element,
        edge: bool,
        opts: &RenderOpts,
    ) -> Option<AnyElement> {
        if !self.reuse.allow || edge && !self.edges || e.node_id == 0 || self.floats.get() {
            return None;
        }
        if self.reuse.store.refused(e.node_id) {
            return None;
        }
        let (record, damage) = self.reuse.store.same_tree(e)?;
        let width = || {
            self.resized && record.fits_width(&self.key) && record.key.same_except_width(&self.key)
        };
        let same = record.key.same(&self.key);
        // Растянутый блок при другой ширине — та же раскладка в коробке
        // другой ширины: лист по потоку, краска записи не годна.
        let stretched = !same && record.stretch;
        // Блок с другими значениями краски: раскладка записи годна, краска нет.
        let repaint = damage == super::super::Damage::Paint;
        // Ближний блок без свежего повтора почти наверняка попадёт на экран,
        // а отказ заместителя на экране собирает кадр второй раз. Растянутый
        // и перекрашенный блоки не повторяются, поэтому заменяется только
        // далёкий. Блок, уже отказанный на экране при этом ключе
        // (`Store::banned`), замещается только далёким: повтор на месте его
        // снова сдвинул бы.
        let paintable = record.far
            || (!stretched
                && !repaint
                && !self.reuse.store.banned(e.node_id)
                && record.current(&self.reuse));
        let fits = (same || width()) && paintable && Counters::read(&opts.cx) == record.start;
        if !fits {
            return None;
        }
        // The block's paint changed here: its record now compares with this
        // tree and holds no recorded paint, so it keeps neither the previous
        // tree nor its stale output alive (`Record::moved_to`).
        // A record compared at a rebuild takes this build's block as its
        // source for the next builds.
        let record = match !record.source.holds() || repaint && !record.repaint.get() {
            true => Rc::new(record.moved_to(self.reuse.shells.source(e), repaint)),
            false => record,
        };
        record.end.write(&opts.cx);
        let mut next = self.reuse.next.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next.replace(next);
        let slot = Rc::new(Slot::new(record, stretched, repaint));
        super::super::push(&self.reuse.placeholders, slot.clone());
        Some(Placeholder::new(slot).hooked())
    }
}
