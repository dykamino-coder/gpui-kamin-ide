//! Построение плана якорного размещения из стиля (AnchorPlan::of).

use super::super::area::physical_area;
use super::super::settle::default_anchor_of;
use super::super::{AnchorPlan, SidePlan};
use super::flipped;
use crate::document::state::ValueCx;
use crate::style::computed::{Align, Computed, Position};
use crate::style::values::value::{Len, anchor_get, px_lossy_zero};

impl AnchorPlan {
    /// План есть у абсолюта хотя бы с одной `anchor()`-вставкой, с
    /// `position-area` или `anchor-center` при якоре по умолчанию, со
    /// списком `position-try-fallbacks` либо с `position-visibility`
    /// (проверка переполнения нужна и без якоря — `no-overflow`).
    pub(in crate::layout::positioned::anchor) fn of(
        own: &Computed,
        inherited: &Computed,
        vcx: ValueCx<'_>,
    ) -> Option<AnchorPlan> {
        if !matches!(
            own.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        ) {
            return None;
        }
        let m = px_lossy_zero;
        let margin = [
            m(own.margin.top),
            m(own.margin.right),
            m(own.margin.bottom),
            m(own.margin.left),
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
            own.inset.top,
            own.inset.right,
            own.inset.bottom,
            own.inset.left,
        ];
        let sides = [
            side(inset[0], margin[0]),
            side(inset[1], margin[1]),
            side(inset[2], margin[2]),
            side(inset[3], margin[3]),
        ];
        let default_anchor = default_anchor_of(own);
        let area = own
            .position_area
            .filter(|_| default_anchor.is_some())
            .map(|a| physical_area(a, inherited, own));
        let anchor_center = own.align_self == Some(Align::AnchorCenter)
            || own.justify_self == Some(Align::AnchorCenter);
        // §position-visibility, anchor-valid: «If the box references the
        // default anchor box (e.g. using 'position-area', 'anchor()' or
        // 'anchor-size()' functions, or 'anchor-center'), but the default
        // anchor box cannot be resolved…» — ссылка это САМА запись
        // `position-area`. `area` выше уже отфильтрована якорем по умолчанию,
        // и коробка без якоря (`position-visibility-anchor-valid`, #target2:
        // `position-area: block-end`, имени нет) не пряталась никогда.
        let refs_default = own.position_area.is_some()
            || anchor_center
            || [
                own.width,
                own.height,
                own.min_width,
                own.min_height,
                own.max_width,
                own.max_height,
                own.inset.top,
                own.inset.right,
                own.inset.bottom,
                own.inset.left,
            ]
            .into_iter()
            .flatten()
            .any(|l| match l {
                Len::Anchor(i) => anchor_get(i, vcx).is_some_and(|f| f.name.is_none()),
                _ => false,
            });
        if sides.iter().all(Option::is_none)
            && area.is_none()
            && !(anchor_center && default_anchor.is_some())
            && own.position_try_fallbacks.is_empty()
            && own.position_visibility == 0
            && ![
                own.width,
                own.height,
                own.min_width,
                own.min_height,
                own.max_width,
                own.max_height,
            ]
            .into_iter()
            .any(|l| matches!(l, Some(Len::Anchor(_))))
        {
            return None;
        }
        Some(AnchorPlan {
            key: own.anchor_key,
            values: vcx.keep(),
            sides,
            default_anchor,
            cb_flipped: flipped(inherited),
            own_flipped: flipped(own),
            area,
            inset,
            margin,
            align_self: own.align_self,
            justify_self: own.justify_self,
            safe_align: own.align_self_safe,
            safe_justify: own.justify_self_safe,
            cb_vertical: inherited.wm().vertical(),
            cb_node: own.cb_node,
            fixed: own.position == Some(Position::Fixed)
                && !(inherited.transform_ancestor
                    || inherited.transform.is_some()
                    || inherited.contain_layout == Some(true)
                    || inherited.contain_paint == Some(true)
                    || inherited.will_change & crate::style::computed::wc::CB_FIXED != 0),
            seq: own.anchor_seq,
            refs_default,
        })
    }
}
