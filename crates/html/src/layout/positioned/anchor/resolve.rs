//! Выбранная запасная позиция и `anchor-size()` в стиле коробки.

use super::settle::default_anchor_of;
use crate::style::computed::{Align, BoxStyle, Computed, Position};
use crate::style::values::value::{AnchorSize, Len, anchor_get, px_lossy_zero};
use crate::values::ValueCx;

mod current;
mod insets;
pub(crate) use current::{SizeInputs, current_inputs, needs_deferred, needs_host, prepare};
mod try_rule;
use insets::fill_anchor_pairs;
pub(crate) use try_rule::try_rule_onto;

/// Размеры разрешаются только из реестра текущего кадра.
pub(crate) fn resolve_sizes(
    c: &mut BoxStyle,
    inherited: &BoxStyle,
    vcx: ValueCx<'_>,
    layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
) {
    resolve_with(c, inherited, None, vcx, layout);
}

fn resolve_with(
    c: &mut BoxStyle,
    inherited: &BoxStyle,
    current: Option<SizeInputs>,
    vcx: ValueCx<'_>,
    layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
) {
    if !matches!(
        c.reset_positioned.position,
        Some(Position::Absolute) | Some(Position::Fixed)
    ) {
        return;
    }
    let default_anchor = default_anchor_of(c);
    let cb_vertical = inherited.wm().vertical();
    let own_vertical = c.wm().vertical();
    let plan = super::AnchorPlan::of(c, inherited, vcx, layout);
    let one = |l: Option<Len>, y_axis: bool| -> Option<Len> {
        let Some(Len::Anchor(i)) = l else { return l };
        let mut f = anchor_get(i, vcx);
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            // `anchor()` в размере негодна (§anchor-fn: только вставки).
            let kind = cur.size?;
            let hit = plan
                .as_ref()
                .and_then(|p| p.rec_current(cur.name.as_deref()))
                .map(|r| {
                    // Размер решается ДО раскладки, стека трансформов коробки ещё
                    // нет: под трансформированным предком считаем цепочку общей
                    // (до-трансформная рамка), без него — отображённую
                    // (`transform-002/009`: `anchor-size(width)` = 100, а не 50).
                    let b = if r.tf_top != 0
                        && !(inherited.reset_transforms.transform_ancestor
                            || inherited.reset_transforms.transform.is_some())
                    {
                        r.rect_tf
                    } else {
                        r.rect
                    };
                    let width = match kind {
                        AnchorSize::Width => true,
                        AnchorSize::Height => false,
                        AnchorSize::Implicit => !y_axis,
                        AnchorSize::Block => cb_vertical,
                        AnchorSize::Inline => !cb_vertical,
                        AnchorSize::SelfBlock => own_vertical,
                        AnchorSize::SelfInline => !own_vertical,
                    };
                    let v = if width { b.size.width } else { b.size.height };
                    f32::from(v) + cur.add
                });
            match (hit, cur.fallback) {
                (Some(v), _) => return Some(Len::Px(v)),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j, vcx),
                (None, fb) => return fb,
            }
        }
        None
    };
    // §position-area: клетка «makes that the box's containing block», и
    // доли размеров, полей и отбивок решаются от неё, а не от исходного
    // содержащего блока (заметка спеки: «like max-height: 100%»). Раскладка
    // клетки не знает — доли переводятся в точки здесь, по клетке текущей
    // пробы, ДО растяжки ниже (она вычитает уже решённые поля и отбивки).
    // Поля и отбивки — от строчного размера клетки по письму содержащего
    // блока (`position-area-percents-001`: 5% от 80 в горизонтальном, от 40
    // в `vertical-rl`); размеры — по своей оси (`transform-001`:
    // `width: 100%` = ширина якоря, а не 800 окна).
    if c.reset_positioned.position_area.is_some()
        && default_anchor.is_some()
        && let Some((cw, ch)) = current.and_then(|v| v.cell)
    {
        let of = |l: Option<Len>, base: f32| match l {
            Some(Len::Pct(p)) => Some(Len::Px(p * base)),
            other => other,
        };
        let inline = if cb_vertical { ch } else { cw };
        c.reset_box_layout.width = of(c.reset_box_layout.width, cw);
        c.reset_box_layout.min_width = of(c.reset_box_layout.min_width, cw);
        c.reset_box_layout.max_width = of(c.reset_box_layout.max_width, cw);
        c.reset_box_layout.height = of(c.reset_box_layout.height, ch);
        c.reset_box_layout.min_height = of(c.reset_box_layout.min_height, ch);
        c.reset_box_layout.max_height = of(c.reset_box_layout.max_height, ch);
        c.reset_box_layout.margin.top = of(c.reset_box_layout.margin.top, inline);
        c.reset_box_layout.margin.right = of(c.reset_box_layout.margin.right, inline);
        c.reset_box_layout.margin.bottom = of(c.reset_box_layout.margin.bottom, inline);
        c.reset_box_layout.margin.left = of(c.reset_box_layout.margin.left, inline);
        c.reset_box_layout.padding.top = of(c.reset_box_layout.padding.top, inline);
        c.reset_box_layout.padding.right = of(c.reset_box_layout.padding.right, inline);
        c.reset_box_layout.padding.bottom = of(c.reset_box_layout.padding.bottom, inline);
        c.reset_box_layout.padding.left = of(c.reset_box_layout.padding.left, inline);
    }
    c.reset_box_layout.width = one(c.reset_box_layout.width, false);
    c.reset_box_layout.min_width = one(c.reset_box_layout.min_width, false);
    c.reset_box_layout.max_width = one(c.reset_box_layout.max_width, false);
    c.reset_box_layout.height = one(c.reset_box_layout.height, true);
    c.reset_box_layout.min_height = one(c.reset_box_layout.min_height, true);
    c.reset_box_layout.max_height = one(c.reset_box_layout.max_height, true);
    // §position-area: без default anchor box область не действует;
    // явные anchor()-вставки продолжают задавать обычный IMCB.
    let area_active = c.reset_positioned.position_area.is_some()
        && default_anchor.is_some()
        && current.is_none_or(|v| v.cell.is_some());
    if !area_active {
        fill_anchor_pairs(c, Some(is_grid(inherited)), current.and_then(|v| v.area));
        return;
    }
    let Some((w, h)) = current.and_then(|v| v.area) else {
        return;
    };
    let pxv = px_lossy_zero;
    let block_stretch = c.reset_flex.align_self == Some(Align::Stretch);
    let inline_stretch = c.reset_flex.justify_self == Some(Align::Stretch);
    let (x_stretch, y_stretch) = if cb_vertical {
        (block_stretch, inline_stretch)
    } else {
        (inline_stretch, block_stretch)
    };
    let b = c.borders();
    if x_stretch {
        let extra = pxv(c.reset_box_layout.margin.left)
            + pxv(c.reset_box_layout.margin.right)
            + pxv(b.left)
            + pxv(b.right)
            + pxv(c.reset_box_layout.padding.left)
            + pxv(c.reset_box_layout.padding.right);
        c.reset_box_layout.width = Some(Len::Px((w - extra).max(0.0)));
    }
    if y_stretch {
        let extra = pxv(c.reset_box_layout.margin.top)
            + pxv(c.reset_box_layout.margin.bottom)
            + pxv(b.top)
            + pxv(b.bottom)
            + pxv(c.reset_box_layout.padding.top)
            + pxv(c.reset_box_layout.padding.bottom);
        c.reset_box_layout.height = Some(Len::Px((h - extra).max(0.0)));
    }
}

fn is_grid(c: &Computed) -> bool {
    matches!(
        c.reset_box_layout.display,
        Some(crate::style::computed::Display::Grid)
            | Some(crate::style::computed::Display::InlineGrid)
    )
}
