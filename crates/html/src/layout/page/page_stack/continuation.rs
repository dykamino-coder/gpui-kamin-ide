//! Продолжение при смене ширины: новая мера и адрес содержимого каждого flex-потока.
use super::{PageStack, measure::measure, resume::Resume};
use crate::layout::fragment::flow::blocks::Laid;
use crate::layout::fragment::types::Kid;
use crate::tolerance::COORD_EPS;
use gpui::{App, Window};

#[derive(Clone)]
pub(super) struct CopyGeometry {
    pub(super) width: f32,
    pub(super) kid: Kid,
    pub(super) laid: Laid,
    pub(super) continued: bool,
}

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn remeasure_copy(
        &mut self,
        index: usize,
        copy: usize,
        area: (f32, f32),
        _current: &Kid,
        from: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<(Kid, f32)> {
        if self
            .measured
            .get(&(index, copy))
            .is_some_and(|m| (m.width - area.0).abs() < COORD_EPS)
        {
            return None;
        }
        let old = self
            .measured
            .get(&(index, copy))
            .or_else(|| self.measured.get(&(index, copy.saturating_sub(1))))?
            .clone();
        if (old.width - area.0).abs() < COORD_EPS && !old.continued {
            self.measured.entry((index, copy)).or_insert(old);
            return None;
        }
        let token = Resume::at(&old.laid, from);
        let k = &mut self.kids[index];
        let original = k.regrow.as_ref()?.laid.clone();
        let main = k.watched.clone()?;
        let rec = if copy == 0 {
            main.clone()
        } else {
            main.continuation_recorder(copy)
        };
        let base = k.regrow.as_ref()?.clone();
        let mut own = self
            .continuations
            .remove(&(index, copy))
            .unwrap_or_else(|| {
                let mut r = base;
                r.dom = r.source.clone();
                r.watched = rec.clone();
                r
            });
        rec.clear_consumed(window);
        rec.resume_content(&old.laid, from);
        k.watched = Some(rec.clone());
        rebuild_copy(k, copy, &own);
        let mut next = measure(k, self.axis, area, copy, window, cx);
        let consumed = token.consumed(&k.regrow.as_ref()?.laid);
        if consumed.values().any(|v| v.0 > COORD_EPS || v.1) {
            rec.consume_flex_items(consumed, window);
            rebuild_copy(k, copy, &own);
            next = measure(k, self.axis, area, copy, window, cx);
        }
        k.watched = Some(main);
        let fresh = std::mem::replace(&mut k.regrow.as_mut()?.laid, original);
        let offset = if from <= COORD_EPS {
            0.0
        } else {
            token.offset(&fresh, next.h)
        };
        own.laid = fresh.clone();
        self.continuations.insert((index, copy), own);
        self.measured.insert(
            (index, copy),
            CopyGeometry {
                width: area.0,
                kid: next.clone(),
                laid: fresh,
                continued: true,
            },
        );
        Some((next, offset))
    }
}

fn rebuild_copy(k: &mut super::PageKid, copy: usize, r: &crate::layout::fragment::push::Regrow) {
    r.watched.rewind_flows();
    r.watched.reset();
    {
        let element = (r.build)(&r.dom, copy);
        if copy == 0 {
            k.el = element;
        } else if let Some(slot) = k.frags.get_mut(copy - 1) {
            *slot = element;
        }
    }
}
