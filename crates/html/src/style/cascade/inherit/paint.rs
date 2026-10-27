//! Результат оформления коробки; наследование читает исходные CSS-значения.

use super::*;
use crate::style::decoration_paint::{DecorationPaint, PaintInput};
use std::rc::Rc;
mod gradient_filter;

pub(super) fn inherit_paint(
    parent: &BoxStyle,
    declared: &Computed,
    own: &Computed,
    c: &mut BoxStyle,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) {
    // layout_made возвращает ту же коробку, а не нового CSS-потомка.
    if c.paint.as_ref().is_some_and(|p| p.preparation.is_none()) {
        return;
    }
    let input = c
        .paint
        .as_ref()
        .and_then(|p| p.preparation)
        .unwrap_or_else(|| {
            let element = c.element_style.is_some();
            let declares_color = declared.declares_color();
            PaintInput {
                parent: parent.foreground_paint(),
                foreground: if !element && !declares_color {
                    parent.foreground_paint()
                } else {
                    crate::style::decoration_paint::ForegroundPaint {
                        color: c.color,
                        ..Default::default()
                    }
                },
                element,
                declares_color,
            }
        });
    let mut paint = DecorationPaint::from_computed(c);
    let own_filter = c
        .paint_filter()
        .filter(|f| *f != crate::style::computed::Filter::neutral());
    paint.filter = own_filter.or(parent.paint_filter());
    paint.filter_ops = if own_filter.is_some() {
        c.paint_filter_ops().to_vec()
    } else {
        parent.paint_filter_ops().to_vec()
    };
    paint.foreground = input.foreground;
    if paint_filter && let Some(f) = paint.filter {
        let operations = paint.filter_ops.clone();
        let apply = |color| f.apply_operations(&operations, color);
        if own.background.is_some() {
            paint.background.color = paint.background.color.map(&apply);
        }
        if input.element
            || input.declares_color
            || own.filter.is_some() && input.parent.color.is_none()
        {
            paint.foreground.color = paint.foreground.color.map(&apply);
        }
        for side in &mut paint.border_colors {
            if let crate::style::values::value::StyleColor::Absolute(col) = side {
                *col = apply(*col);
            }
        }
        if own.gradient.is_some()
            && let Some(g) = &mut paint.background.gradient
        {
            g.from = apply(g.from);
            g.to = apply(g.to);
            for stop in &mut g.stops {
                stop.0 = apply(stop.0);
            }
        }
        gradient_filter::apply(&mut paint.background, own, f, &operations);
        if own.bg_image.is_some()
            && let Some(image) = &mut paint.background.top.image
        {
            image.filter_stops(&f, &operations);
        }
        if !own.shadows.is_empty() {
            for sh in &mut paint.shadows {
                if let crate::style::values::value::StyleColor::Absolute(col) = &mut sh.color {
                    *col = apply(*col);
                }
            }
        }
    }
    // У непрозрачной коробки силуэт filter drop-shadow совпадает с box-shadow;
    // размытие коробки равно двум sigma фильтра.
    if let Some(sh) = c.drop_shadow.clone()
        && c.background_paint.paints()
        && paint.background.color.is_some_and(|b| b.a >= 1.0)
    {
        let as_box = crate::style::computed::Shadow {
            blur: sh.blur * 2.0,
            ..sh
        };
        if !paint.shadows.contains(&as_box) {
            paint.shadows.push(as_box);
        }
    }
    let images = vcx.background_images();
    if paint_filter && c.background_paint.paints() {
        crate::style::computed::flat_background::collapse_flat_stack(
            &mut paint.background,
            c.image_orient_none != Some(true),
            &images,
        );
    }
    paint.foreground = super::super::color_inputs::text_clip_colors(
        super::super::color_inputs::TextClipInput {
            parent: input.parent,
            foreground: paint.foreground,
            element: input.element,
            declares_color: input.declares_color,
            out_of_flow: crate::style::flow::absolutely_positioned(own.position),
            background: c.background_paint,
        },
        &paint.background,
        &images,
    );
    paint.preparation = (!paint_filter).then_some(input);
    c.paint = Some(Rc::new(paint));
}
