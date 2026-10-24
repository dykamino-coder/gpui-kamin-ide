//! Продолжение при смене ограничений: новая мера и адрес содержимого каждого flex-потока.
mod inline;
use super::{PageStack, measure::measure, resume::Resume};
use crate::layout::fragment::flow::blocks::Laid;
use crate::layout::fragment::types::Kid;
use crate::tolerance::COORD_EPS;
use gpui::{App, Window};

/// Кэш привязан к породившему снимок проходу и всем входам поддерева.
/// Рост/новая раскладка меняют source даже при прежней ширине.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Inputs {
    pub(super) constraints: crate::layout::cx::FragmentConstraints,
    pub(super) source: Option<gpui::LayoutSpace>,
    pub(super) page: Option<crate::layout::fragment::token::space::PageIdentity>,
}

#[derive(Clone)]
pub(super) struct CopyGeometry {
    pub(super) inputs: Inputs,
    pub(super) from: f32,
    pub(super) offset: f32,
    pub(super) kid: Kid,
    pub(super) inline: crate::layout::fragment::flow::inline::Geometry,
    pub(super) laid: Laid,
    pub(super) continued: bool,
}

impl CopyGeometry {
    /// Нефрагментированный снимок можно резать в любом месте; продолжение
    /// сохраняет только исходный адрес и его результат в новой геометрии.
    fn reusable(&self, inputs: &Inputs, from: f32) -> bool {
        inputs.source.is_some()
            && self.inputs == *inputs
            && (!self.continued || self.from == from || self.offset == from)
    }
}

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn remeasure_copy(
        &mut self,
        index: usize,
        copy: usize,
        area: (f32, f32),
        from: f32,
        page: Option<crate::layout::fragment::token::space::PageIdentity>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<(Kid, f32)> {
        let base = self.kids[index].regrow.as_ref()?.clone();
        let inputs = Inputs {
            constraints: crate::layout::cx::FragmentConstraints {
                area,
                parent: base.opts.cx.constraints.clone(),
            },
            source: base.space,
            page,
        };
        if let Some(m) = self.measured.get(&(index, copy))
            && m.reusable(&inputs, from)
        {
            if !m.continued {
                return None;
            }
            if m.from == from || m.offset == from {
                return Some((m.kid.clone(), m.offset));
            }
        }
        let old = self
            .measured
            .get(&(index, copy))
            .or_else(|| self.measured.get(&(index, copy.saturating_sub(1))))?
            .clone();
        if !old.continued && old.reusable(&inputs, from) {
            self.measured.entry((index, copy)).or_insert(old);
            return None;
        }
        let token = Resume::at(&old.laid, from);
        let k = &mut self.kids[index];
        let original = k.regrow.as_ref()?.laid.clone();
        let original_space = k.regrow.as_ref()?.space;
        let main = k.watched.clone()?;
        let rec = if copy == 0 {
            main.clone()
        } else {
            main.continuation_recorder(copy)
        };
        let mut own = self
            .continuations
            .remove(&(index, copy))
            .unwrap_or_else(|| {
                let mut r = base;
                r.dom = r.source.clone();
                r.watched = rec.clone();
                r
            });
        if let Some(percentages) = &k.percentages {
            let mut space = crate::layout::fragment::token::Space::pages();
            (space.inline_size, space.block_size, space.page) = (Some(area.0), Some(area.1), page);
            percentages.refresh(&mut own.dom, space, &own.opts);
        }
        rec.clear_consumed(window);
        rec.resume_content(&old.laid, from);
        let inline_addresses = inline::resume(&rec.flow, &old.inline.paragraphs, from);
        k.watched = Some(rec.clone());
        rebuild_copy(k, copy, &own);
        let (mut next, mut next_inline) = measure(k, self.axis, area, copy, window, cx);
        let consumed = token.consumed(&k.regrow.as_ref()?.laid);
        if consumed.values().any(|v| v.0 > COORD_EPS || v.1) {
            rec.consume_flex_items(consumed, window);
            rebuild_copy(k, copy, &own);
            (next, next_inline) = measure(k, self.axis, area, copy, window, cx);
        }
        k.watched = Some(main);
        let fresh = std::mem::replace(&mut k.regrow.as_mut()?.laid, original);
        let fresh_space = std::mem::replace(&mut k.regrow.as_mut()?.space, original_space);
        let offset = if from <= COORD_EPS {
            0.0
        } else {
            let root = inline::offset(&inline_addresses, &next_inline.paragraphs);
            root.into_iter()
                .chain(std::iter::once(token.offset(&fresh, next.h)))
                .min_by(f32::total_cmp)?
        };
        own.laid = fresh.clone();
        own.space = fresh_space;
        self.continuations.insert((index, copy), own);
        self.measured.insert(
            (index, copy),
            CopyGeometry {
                inputs,
                from,
                offset,
                kid: next.clone(),
                inline: next_inline,
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

#[cfg(test)]
mod tests;
