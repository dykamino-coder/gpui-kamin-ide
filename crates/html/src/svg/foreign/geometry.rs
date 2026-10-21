//! Геометрия слоя `foreignObject`: увеличение по трансформам (наибольшее сингулярное число
//! матрицы), область снимка с запасом под `overflow: visible` отдельно от вьюпорта HTML,
//! раскрытие `<image>` во вложенный `<svg>` с `overflow="visible"` для вылезшей краски и выбор
//! единственного вклада `mix-blend-mode`, который может смешиваться с подложкой хоста.
use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Overflow};
use gpui::{Pixels, Point, Size, point, px, size};

// Largest singular value bounds the magnification, including skew/rotation.
pub(super) fn scale(e: &Element) -> f32 {
    let matrix = e
        .style
        .transform
        .map(|t| {
            t.after_individual(e.style.rotate_prop, e.style.scale_prop)
                .lin
        })
        .or_else(|| {
            e.attr("transform")
                .and_then(|s| s.parse::<svgtypes::Transform>().ok())
                .map(|t| [[t.a as f32, t.c as f32], [t.b as f32, t.d as f32]])
        });
    let Some([[a, b], [c, d]]) = matrix else {
        return 1.0;
    };
    let trace = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    ((trace + (trace * trace - 4.0 * det * det).max(0.0).sqrt()) * 0.5).sqrt()
}

pub(super) fn capture(
    style: &Computed,
    (w, h): (f32, f32),
    viewport: (f32, f32),
) -> (Point<Pixels>, Size<Pixels>) {
    let x = if style.overflow_x == Some(Overflow::Visible) {
        viewport.0
    } else {
        0.0
    };
    let y = if style.overflow_y == Some(Overflow::Visible) {
        viewport.1
    } else {
        0.0
    };
    (point(px(x), px(y)), size(px(w + 2.0 * x), px(h + 2.0 * y)))
}

pub(super) fn expand(e: &mut Element, origin: Point<Pixels>, size: Size<Pixels>) {
    let mut image = e.clone();
    image.style = Computed::default().into();
    image.attrs = [
        ("x", -f32::from(origin.x)),
        ("y", -f32::from(origin.y)),
        ("width", f32::from(size.width)),
        ("height", f32::from(size.height)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.to_string()))
    .collect();
    image
        .attrs
        .push(("preserveAspectRatio".into(), "none".into()));
    e.tag = "svg".into();
    e.attrs.push(("overflow".into(), "visible".into()));
    e.children.push(Node::Element(image));
}

// A sole SVG paint contribution can blend with the host backdrop after SVG
// transforms/clips have been applied. Multiple contributions retain SVG order.
pub(super) fn sole_blend(e: &Element) -> Option<u8> {
    if e.style.isolate == Some(true) || e.style.opacity.is_some_and(|v| v < 1.0) {
        return None;
    }
    let mut children = e.children.iter().filter_map(|n| match n {
        Node::Element(c) if !matches!(c.tag.as_str(), "defs" | "style" | "title" | "desc") => {
            Some(c)
        }
        _ => None,
    });
    let child = children.next()?;
    if children.next().is_some() {
        return None;
    }
    if child.tag.eq_ignore_ascii_case("foreignObject") {
        child.style.blend.filter(|v| *v != 0)
    } else if matches!(child.tag.as_str(), "g" | "svg") {
        sole_blend(child)
    } else {
        None
    }
}
