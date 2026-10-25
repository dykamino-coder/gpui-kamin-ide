//! Публикация computed-стадии: builder владеет записью до finish; читатели
//! удерживают закрытый Rc-handle. Старый used API отделяет COW-копию при записи
//! CSS-операндов (css-cascade-5 §4, CSS2 §6.1); runtime-контекст пока остаётся.

use super::computed::Computed;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct ComputedHandle(Rc<Computed>);

impl ComputedHandle {
    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Deref for ComputedHandle {
    type Target = Computed;

    fn deref(&self) -> &Computed {
        &self.0
    }
}

/// Результат style construction. Legacy-ссылка обслуживает только прежние
/// used consumers; handle не выдаёт её и не принимает правки обратно.
#[derive(Clone, Debug)]
pub(super) struct Published {
    pub(super) handle: ComputedHandle,
    pub(super) legacy: Rc<Computed>,
}

impl Deref for Published {
    type Target = Computed;

    fn deref(&self) -> &Computed {
        &self.handle
    }
}

pub(super) struct ComputedBuilder(Computed);

impl ComputedBuilder {
    pub(super) fn from_cascaded(own: &Computed) -> Self {
        Self(own.clone())
    }

    pub(super) fn finish(self) -> Published {
        let legacy = Rc::new(self.0);
        Published {
            handle: ComputedHandle(legacy.clone()),
            legacy,
        }
    }
}

impl Deref for ComputedBuilder {
    type Target = Computed;

    fn deref(&self) -> &Computed {
        &self.0
    }
}

impl DerefMut for ComputedBuilder {
    fn deref_mut(&mut self) -> &mut Computed {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn retained_publication_survives_legacy_used_cow() {
        let mut cascade = Computed::default();
        cascade.font_size = Some(Len::Px(20.0));
        cascade.width = Some(Len::Pct(0.5));
        let Published { handle, mut legacy } = ComputedBuilder::from_cascaded(&cascade).finish();
        let retained = handle.clone();
        Rc::make_mut(&mut legacy).width = Some(Len::Px(70.0));
        Rc::make_mut(&mut legacy).font_size = Some(Len::Px(7.0));
        assert_eq!(handle.width, Some(Len::Pct(0.5)));
        assert_eq!(retained.font_size, Some(Len::Px(20.0)));
        assert!(handle.ptr_eq(&retained));
        assert!(!std::ptr::eq::<Computed>(&*handle, &*legacy));
    }

    #[test]
    fn handle_clones_share_allocation_and_inherited_group() {
        let published = ComputedBuilder::from_cascaded(&Computed::default()).finish();
        let before = Rc::strong_count(&published.handle.0);
        let handles = vec![published.handle.clone(); 32];
        assert_eq!(Rc::strong_count(&published.handle.0), before + 32);
        for handle in &handles {
            assert!(handle.ptr_eq(&published.handle));
            assert!(Rc::ptr_eq(&handle.inh.0, &published.inh.0));
        }
        drop(handles);
        assert_eq!(Rc::strong_count(&published.handle.0), before);
    }
}
