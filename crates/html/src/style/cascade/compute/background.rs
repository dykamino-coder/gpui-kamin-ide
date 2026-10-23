//! inherit_stage, явное `inherit` длинных свойств фона (css-cascade-5
//! §7.3.1): каждое свойство берёт у родителя своё вычисленное значение —
//! поля верхнего слоя и свой список слоёв (`bg_lists`); `background-color`
//! копирует вычисленный цвет и решает его используемое значение.

use super::*;
use crate::style::computed::{BgListProp, inh};

pub(super) fn inherit_background(parent: &Computed, own: &Computed, c: &mut Computed) {
    let on = |b: u32| own.inherit_bits & b != 0;
    let list = |c: &mut Computed, prop: BgListProp| {
        c.bg_lists.remove(prop);
        if let Some(layers) = parent.bg_lists.get(prop) {
            c.bg_lists.set_layers(prop, layers.to_vec());
        }
    };
    if on(inh::BG_IMAGE) {
        c.bg_image = parent.bg_image.clone();
        c.gradient = parent.gradient.clone();
        c.gradient_raw = parent.gradient_raw.clone();
        c.gradient_current = parent.gradient_current.clone();
        list(c, BgListProp::Image);
    }
    if on(inh::BG_POS) {
        c.bg_pos.x = parent.bg_pos.x;
        c.bg_position_math[0] = parent.bg_position_math[0].clone();
        list(c, BgListProp::PositionX);
    }
    if on(inh::BG_POS_Y) {
        c.bg_pos.y = parent.bg_pos.y;
        c.bg_position_math[1] = parent.bg_position_math[1].clone();
        list(c, BgListProp::PositionY);
    }
    if on(inh::BG_REPEAT) {
        c.bg_repeat = parent.bg_repeat;
        list(c, BgListProp::Repeat);
    }
    if on(inh::BG_ORIGIN) {
        c.bg_origin = parent.bg_origin;
        list(c, BgListProp::Origin);
    }
    if on(inh::BG_CLIP) {
        c.bg_clip = parent.bg_clip;
        list(c, BgListProp::Clip);
    }
    if on(inh::BG_SIZE) {
        c.bg_size = parent.bg_size;
        list(c, BgListProp::Size);
    }
    if on(inh::BG_ATTACH) {
        c.bg_fixed = parent.bg_fixed;
        list(c, BgListProp::Attachment);
    }
    if own.background_inherit {
        c.background_color = parent.background_color.clone();
    }
    // `currentcolor` фона решается ЗДЕСЬ: только теперь известен цвет самого
    // элемента. Вычисленное значение остаётся словом — его унаследуют дети
    // (`background-color: inherit`) и решат своим цветом заново.
    if own.background_inherit
        || c.background_color
            .as_ref()
            .is_some_and(|value| value.depends_on_current())
    {
        let current = c.current_color();
        c.derive_background(Some(current));
    }
}
