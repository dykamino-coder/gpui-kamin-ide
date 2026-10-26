//! Доказуемые высоты блоков при схлопывании полей: когда высота не равна высоте содержимого,
//! поля ребёнка и родителя не сливаются (CSS 2.2 §10.7, Blink `block_layout_algorithm.cc`).
//! `lowers` — `max-height` режет определённую стопку детей; `raises` — `min-height`
//! тянет коробку выше содержимого; `separate` отделяет конечное поле последнего ребёнка. Нулевая высота блока
//! из одних флоатов — `zero_floats`.

use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::block::struts::{margin_px, pin_inherited_margins};
use crate::layout::box_kind::{in_flow, inline_level_box};
use crate::style::values::value::Len;

mod lowers;
pub(super) use lowers::lowers;
mod zero_floats;
pub(crate) use zero_floats::{used_style, zero_float_blocks};

pub(super) fn separate(e: &mut Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    if !lowers(e, vcx) {
        return false;
    }
    // Конечное поле ребёнка не меняет используемую высоту ограниченного
    // родителя. Отрицательное поле в flex-адаптере снова уменьшило бы auto-
    // высоту, отменив ограничение, которое разделило поля.
    for node in e.children.iter_mut().rev() {
        if let Node::Element(child) = node
            && in_flow(&child.style.used())
        {
            pin_inherited_margins(child, false, true, vcx);
            child.style.used_mut().margin.bottom = Some(Len::Px(0.0));
            break;
        }
    }
    true
}

pub(super) fn raises(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let st = e.style.used();
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    match margin_px(st.min_height, &st, vcx) {
        None => !zero(st.min_height),
        Some(mh) if mh <= 0.0 => false,
        Some(mh) => {
            let mut sum = 0.0f32;
            let mut known = true;
            for c in &e.children {
                match c {
                    Node::Text(t) if blank_text(t) => {}
                    Node::Text(_) => known = false,
                    Node::Element(ch) if !in_flow(&ch.style.used()) => {}
                    Node::Element(ch) if ch.inline && inline_level_box(ch) => known = false,
                    Node::Element(ch) => {
                        let cs = ch.style.used();
                        let b = cs.borders();
                        let side = |l: Option<Len>| match l {
                            None => Some(0.0),
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        };
                        let own = match (
                            margin_px(cs.height, &cs, vcx),
                            side(cs.padding.top),
                            side(cs.padding.bottom),
                            side(b.top),
                            side(b.bottom),
                        ) {
                            (Some(h), Some(pt), Some(pb), Some(bt), Some(bb)) => {
                                Some(h + pt + pb + bt + bb)
                            }
                            _ => None,
                        };
                        match own {
                            Some(v) => sum += v,
                            None => known = false,
                        }
                    }
                }
                if !known {
                    break;
                }
            }
            !known || mh > sum + crate::tolerance::COORD_EPS
        }
    }
}
