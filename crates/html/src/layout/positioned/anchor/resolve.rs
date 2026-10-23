//! Выбранная запасная позиция и `anchor-size()` в стиле коробки.

use super::settle::default_anchor_of;
use crate::document::state::ValueCx;
use crate::style::computed::{Align, Computed, Position};
use crate::style::values::value::{AnchorSize, Len, anchor_get, px_lossy_zero};

mod current;
mod insets;
pub(crate) use current::{SizeInputs, current_inputs, needs_host, prepare};
mod try_rule;
use insets::fill_anchor_pairs;
pub(crate) use try_rule::apply_try_rule;

/// Размеры разрешаются только из реестра текущего кадра.
pub fn resolve_sizes(c: &mut Computed, inherited: &Computed, vcx: ValueCx<'_>) {
    resolve_with(c, inherited, None, vcx);
}

fn resolve_with(
    c: &mut Computed,
    inherited: &Computed,
    current: Option<SizeInputs>,
    vcx: ValueCx<'_>,
) {
    if !matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        return;
    }
    let default_anchor = default_anchor_of(c);
    let cb_vertical = inherited.wm().vertical();
    let own_vertical = c.wm().vertical();
    let plan = super::AnchorPlan::of(c, inherited, vcx);
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
                        && !(inherited.transform_ancestor || inherited.transform.is_some())
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
    if c.position_area.is_some()
        && default_anchor.is_some()
        && let Some((cw, ch)) = current.and_then(|v| v.cell)
    {
        let of = |l: Option<Len>, base: f32| match l {
            Some(Len::Pct(p)) => Some(Len::Px(p * base)),
            other => other,
        };
        let inline = if cb_vertical { ch } else { cw };
        c.width = of(c.width, cw);
        c.min_width = of(c.min_width, cw);
        c.max_width = of(c.max_width, cw);
        c.height = of(c.height, ch);
        c.min_height = of(c.min_height, ch);
        c.max_height = of(c.max_height, ch);
        c.margin.top = of(c.margin.top, inline);
        c.margin.right = of(c.margin.right, inline);
        c.margin.bottom = of(c.margin.bottom, inline);
        c.margin.left = of(c.margin.left, inline);
        c.padding.top = of(c.padding.top, inline);
        c.padding.right = of(c.padding.right, inline);
        c.padding.bottom = of(c.padding.bottom, inline);
        c.padding.left = of(c.padding.left, inline);
    }
    c.width = one(c.width, false);
    c.min_width = one(c.min_width, false);
    c.max_width = one(c.max_width, false);
    c.height = one(c.height, true);
    c.min_height = one(c.min_height, true);
    c.max_height = one(c.max_height, true);
    // §position-area: без default anchor box область не действует;
    // явные anchor()-вставки продолжают задавать обычный IMCB.
    let area_active = c.position_area.is_some()
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
    let block_stretch = c.align_self == Some(Align::Stretch);
    let inline_stretch = c.justify_self == Some(Align::Stretch);
    let (x_stretch, y_stretch) = if cb_vertical {
        (block_stretch, inline_stretch)
    } else {
        (inline_stretch, block_stretch)
    };
    let b = c.borders();
    if x_stretch {
        let extra = pxv(c.margin.left)
            + pxv(c.margin.right)
            + pxv(b.left)
            + pxv(b.right)
            + pxv(c.padding.left)
            + pxv(c.padding.right);
        c.width = Some(Len::Px((w - extra).max(0.0)));
    }
    if y_stretch {
        let extra = pxv(c.margin.top)
            + pxv(c.margin.bottom)
            + pxv(b.top)
            + pxv(b.bottom)
            + pxv(c.padding.top)
            + pxv(c.padding.bottom);
        c.height = Some(Len::Px((h - extra).max(0.0)));
    }
}

fn is_grid(c: &Computed) -> bool {
    matches!(
        c.display,
        Some(crate::style::computed::Display::Grid)
            | Some(crate::style::computed::Display::InlineGrid)
    )
}
