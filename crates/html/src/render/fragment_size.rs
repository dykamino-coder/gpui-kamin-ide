//! Used block size and overflow extent are separate inputs to fragmentation.
//!
//! A max-height changes the box's used size even with visible overflow. The
//! overflowing descendants remain a parallel flow, rather than increasing the
//! distance to the following sibling. Min-height wins when min exceeds max.

use super::{Shape, ShapeCx, shape_contents, table_box};
use crate::{
    computed::{Computed, Overflow},
    dom::Element,
    value::Len,
};

pub(crate) fn shape_full(element: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    let mut shape = shape_contents(element, depth, cx)?;
    // Table height is a minimum of the row grid (CSS Tables 3), and its
    // specialised measurement already owns that sizing/distribution pass.
    if table_box(element) {
        return Some(shape);
    }
    let resolve = |value: Option<Len>, padding: bool| match value {
        Some(Len::Px(1.0)) if padding && cx.cell_pad.is_some() => cx.cell_pad.unwrap(),
        Some(Len::Px(v)) => v,
        Some(Len::Vw(k)) => cx.viewport.map_or(0.0, |v| k * v.0),
        Some(Len::Vh(k)) => cx.viewport.map_or(0.0, |v| k * v.1),
        _ => 0.0,
    };
    let border = element.style.borders();
    let decoration = resolve(element.style.padding.top, true)
        + resolve(element.style.padding.bottom, true)
        + resolve(border.top, false)
        + resolve(border.bottom, false);
    let height = constrain(
        shape.0,
        &element.style,
        decoration,
        cx.viewport,
        cx.unclamped,
    );
    if height < shape.0 {
        shape
            .3
            .retain(|&(need, _)| need > 0.01 && need < height - 0.01);
        shape
            .4
            .retain(|&forced| forced > 0.01 && forced < height - 0.01);
        shape.5.retain(|&(start, _)| start < height - 0.01);
        for (_, end) in &mut shape.5 {
            *end = end.min(height);
        }
    }
    shape.0 = height;
    Some(shape)
}

pub(crate) fn border_size(value: f32, style: &Computed, decoration: f32) -> f32 {
    if style.border_box == Some(true) {
        value.max(decoration)
    } else {
        value.max(0.0) + decoration
    }
}

pub(crate) fn constrain(
    natural_border_size: f32,
    style: &Computed,
    decoration: f32,
    viewport: Option<(f32, f32)>,
    overflow_extent: bool,
) -> f32 {
    let resolve = |length: Option<Len>| match length {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Vw(k)) => viewport.map(|v| k * v.0),
        Some(Len::Vh(k)) => viewport.map(|v| k * v.1),
        _ => None,
    };
    let border_size = |v: f32| border_size(v, style, decoration);
    let minimum = resolve(style.min_height).map(border_size);
    let maximum = resolve(style.max_height).map(border_size);
    let clips = matches!(style.overflow_y, Some(Overflow::Hidden | Overflow::Clip));
    if overflow_extent && !clips {
        return natural_border_size.max(minimum.unwrap_or(0.0));
    }
    // Explicit height has already been resolved by the content measurement.
    // Do not apply it again after that pass has accounted for positioned
    // descendants: their extent also contributes to column balancing.
    let mut used = natural_border_size;
    if let Some(maximum) = maximum {
        used = used.min(maximum);
    }
    used.max(minimum.unwrap_or(0.0)).max(decoration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;

    fn element(style: Computed, children: Vec<Node>) -> Element {
        Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: "div".into(),
            style,
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: Vec::new(),
            inline: false,
        }
    }

    #[test]
    fn positioned_descendants_keep_their_balancing_extent() {
        use crate::computed::Position;
        let child = element(
            Computed {
                height: Some(Len::Px(160.0)),
                position: Some(Position::Absolute),
                ..Default::default()
            },
            Vec::new(),
        );
        let parent = element(
            Computed {
                height: Some(Len::Px(100.0)),
                position: Some(Position::Relative),
                ..Default::default()
            },
            vec![Node::Element(child)],
        );
        assert_eq!(shape_full(&parent, 4, ShapeCx::COLUMNS).unwrap().0, 160.0);
    }

    #[test]
    fn measured_box_and_parallel_flow_keep_distinct_sizes_and_break_ranges() {
        let child = element(
            Computed {
                height: Some(Len::Px(206.0)),
                ..Default::default()
            },
            Vec::new(),
        );
        let parent = element(
            Computed {
                min_height: Some(Len::Px(120.0)),
                max_height: Some(Len::Px(160.0)),
                ..Default::default()
            },
            vec![Node::Element(child)],
        );
        let used = shape_full(&parent, 4, ShapeCx::COLUMNS).unwrap();
        let overflow = shape_full(
            &parent,
            4,
            ShapeCx {
                unclamped: true,
                ..ShapeCx::COLUMNS
            },
        )
        .unwrap();
        assert_eq!(used.0, 160.0);
        assert_eq!(overflow.0, 206.0);
        assert!(used.5.iter().all(|(_, end)| *end <= used.0));
    }

    #[test]
    fn visible_overflow_does_not_remove_the_box_maximum() {
        let style = Computed {
            min_height: Some(Len::Px(120.0)),
            max_height: Some(Len::Px(160.0)),
            ..Default::default()
        };
        assert_eq!(constrain(206.0, &style, 0.0, None, false), 160.0);
        assert_eq!(constrain(206.0, &style, 0.0, None, true), 206.0);
    }

    #[test]
    fn minimum_wins_over_maximum_in_both_flow_modes() {
        let style = Computed {
            min_height: Some(Len::Px(200.0)),
            max_height: Some(Len::Px(100.0)),
            overflow_y: Some(Overflow::Clip),
            ..Default::default()
        };
        assert_eq!(constrain(300.0, &style, 0.0, None, false), 200.0);
        assert_eq!(constrain(300.0, &style, 0.0, None, true), 200.0);
    }

    #[test]
    fn border_box_constraints_include_decoration_once() {
        let measure = |style: Computed| {
            shape_full(&element(style, Vec::new()), 4, ShapeCx::COLUMNS)
                .unwrap()
                .0
        };
        let mut style = Computed {
            height: Some(Len::Px(40.0)),
            min_height: Some(Len::Px(30.0)),
            max_height: Some(Len::Px(50.0)),
            border_box: Some(true),
            padding: crate::computed::Sides {
                top: Some(Len::Px(10.0)),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(measure(style.clone()), 40.0);
        style.border_box = Some(false);
        assert_eq!(measure(style.clone()), 50.0);
        style.height = Some(Len::Px(2.0));
        style.min_height = None;
        style.border_box = Some(true);
        assert_eq!(measure(style), 10.0);
    }

    #[test]
    fn viewport_constraints_use_the_actual_page_area() {
        let style = Computed {
            height: Some(Len::Vh(0.5)),
            max_height: Some(Len::Vw(0.25)),
            ..Default::default()
        };
        assert_eq!(
            constrain(400.0, &style, 0.0, Some((800.0, 600.0)), false),
            200.0
        );
        assert_eq!(constrain(400.0, &style, 0.0, None, false), 400.0);
    }
}
