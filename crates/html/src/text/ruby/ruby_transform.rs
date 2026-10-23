//! Снятие трансформаций с коробок руби: css-transforms-1 §transformable-element исключает
//! незамещённые строчные коробки, а единицы руби внутри рисуются блочными обёртками — это не
//! должно делать их трансформируемыми. Блочный главный бокс руби и элемент без роли
//! трансформации сохраняют.

use crate::dom::Element;
use crate::style::computed::{Computed, Display, inh};

pub(crate) fn used(e: &Element) -> Option<Element> {
    // A block ruby principal box remains transformable. Author display changes
    // that remove a ruby role likewise retain ordinary transform behavior.
    if crate::text::ruby::ruby_role(e).is_none() || e.style.used().display == Some(Display::Block) {
        return None;
    }
    let mut e = e.clone();
    clear(e.style.used_mut());
    Some(e)
}

pub(super) fn clear(style: &mut Computed) {
    style.transform = None;
    style.transform_raw = None;
    style.translate = None;
    style.rotate_prop = None;
    style.scale_prop = None;
    style.perspective = None;
    style.preserve_3d = None;
    style.inherit_bits &= !(inh::TRANSFORM | inh::TRANSFORM_ORIGIN);
}
