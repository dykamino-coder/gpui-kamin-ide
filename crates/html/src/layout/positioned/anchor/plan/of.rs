//! Построение плана якорного размещения из стиля (AnchorPlan::of).

use super::super::area::physical_area;
use super::super::settle::default_anchor_of;
use super::super::{AnchorPlan, SidePlan};
use super::flipped;
use crate::style::computed::{Align, BoxStyle, Computed, Position};
use crate::style::values::value::{Len, anchor_get, px_lossy_zero};
use crate::values::ValueCx;

impl AnchorPlan {
    /// План есть у абсолюта хотя бы с одной `anchor()`-вставкой, с
    /// `position-area` или `anchor-center` при якоре по умолчанию, со
    /// списком `position-try-fallbacks` либо с `position-visibility`
    /// (проверка переполнения нужна и без якоря — `no-overflow`).
    pub(in crate::layout::positioned::anchor) fn of(
        own: &BoxStyle,
        inherited: &Computed,
        vcx: ValueCx<'_>,
        layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
    ) -> Option<AnchorPlan> {
        if !matches!(
            own.reset_positioned.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        ) {
            return None;
        }
        let m = px_lossy_zero;
        let margin = [
            m(own.reset_box_layout.margin.top),
            m(own.reset_box_layout.margin.right),
            m(own.reset_box_layout.margin.bottom),
            m(own.reset_box_layout.margin.left),
        ];
        let side = |l: Option<Len>, margin: f32| -> Option<SidePlan> {
            let Some(Len::Anchor(i)) = l else { return None };
            let f = anchor_get(i, vcx)?;
            // `anchor-size()` во вставке — величина, не край: сдвига не даёт.
            if f.size.is_some() {
                return None;
            }
            Some(SidePlan { f, margin })
        };
        let inset = [
            own.reset_positioned.inset.top,
            own.reset_positioned.inset.right,
            own.reset_positioned.inset.bottom,
            own.reset_positioned.inset.left,
        ];
        let sides = [
            side(inset[0], margin[0]),
            side(inset[1], margin[1]),
            side(inset[2], margin[2]),
            side(inset[3], margin[3]),
        ];
        let default_anchor = default_anchor_of(own);
        let area = own
            .reset_positioned
            .position_area
            .filter(|_| default_anchor.is_some())
            .map(|a| physical_area(a, inherited, own));
        let anchor_center = own.reset_flex.align_self == Some(Align::AnchorCenter)
            || own.reset_flex.justify_self == Some(Align::AnchorCenter);
        // §position-visibility, anchor-valid: «If the box references the
        // default anchor box (e.g. using 'position-area', 'anchor()' or
        // 'anchor-size()' functions, or 'anchor-center'), but the default
        // anchor box cannot be resolved…» — ссылка это САМА запись
        // `position-area`. `area` выше уже отфильтрована якорем по умолчанию,
        // и коробка без якоря (`position-visibility-anchor-valid`, #target2:
        // `position-area: block-end`, имени нет) не пряталась никогда.
        let anchor_lengths = [
            own.reset_box_layout.width,
            own.reset_box_layout.height,
            own.reset_box_layout.min_width,
            own.reset_box_layout.min_height,
            own.reset_box_layout.max_width,
            own.reset_box_layout.max_height,
            own.reset_positioned.inset.top,
            own.reset_positioned.inset.right,
            own.reset_positioned.inset.bottom,
            own.reset_positioned.inset.left,
        ];
        // anchor-size() тоже требует плана: иначе зависимости размера
        // исчезают, а текущая геометрия заменяется запасным значением.
        let has_anchor = anchor_lengths
            .into_iter()
            .flatten()
            .any(|l| matches!(l, Len::Anchor(_)));
        let refs_default = own.reset_positioned.position_area.is_some()
            || anchor_center
            || anchor_lengths.into_iter().flatten().any(|l| match l {
                Len::Anchor(i) => anchor_get(i, vcx).is_some_and(|f| f.name.is_none()),
                _ => false,
            });
        if !has_anchor
            && sides.iter().all(Option::is_none)
            && area.is_none()
            && !(anchor_center && default_anchor.is_some())
            && own.reset_positioned.position_try_fallbacks.is_empty()
            && own.reset_positioned.position_visibility == 0
        {
            return None;
        }
        Some(AnchorPlan {
            key: own.anchor_key,
            values: vcx.keep(),
            layout: layout.clone(),
            sides,
            default_anchor,
            cb_flipped: flipped(inherited),
            own_flipped: flipped(own),
            area,
            inset,
            margin,
            align_self: own.reset_flex.align_self,
            justify_self: own.reset_flex.justify_self,
            safe_align: own.reset_flex.align_self_safe,
            safe_justify: own.reset_flex.justify_self_safe,
            cb_vertical: inherited.wm().vertical(),
            cb_node: own.cb_node,
            fixed: own.reset_positioned.position == Some(Position::Fixed)
                && !(inherited.reset_transforms.transform_ancestor
                    || inherited.reset_transforms.transform.is_some()
                    || inherited.reset_context.contain_layout == Some(true)
                    || inherited.reset_context.contain_paint == Some(true)
                    || inherited.reset_effects.will_change & crate::style::computed::wc::CB_FIXED
                        != 0),
            seq: own.anchor_seq,
            refs_default,
        })
    }
}
