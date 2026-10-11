//! Сбор SVG-геометрии для ссылок offset-path.

use crate::dom::Node;

/// Собрать фигуры с `id`. По `getElementById` побеждает ПЕРВАЯ.
pub(super) fn collect_shapes(nodes: &[Node], out: &mut std::collections::HashMap<String, String>) {
    for n in nodes {
        let Node::Element(e) = n else { continue };
        if let Some(d) = shape_d(e)
            && let Some(id) = e.attr("id")
        {
            out.entry(id.to_string()).or_insert(d);
        }
        collect_shapes(&e.children, out);
    }
}

/// Эквивалентный путь SVG-фигуры в её пользовательских единицах (SVG 2
/// §9.x «equivalent path»): прямоугольник — от левого верхнего угла по
/// часовой, круг и эллипс — от самой правой точки по часовой, линия и
/// ломаная — от первой точки. `None` — не фигура (тогда `url()` ведёт себя
/// как `path("m 0 0")`, motion-1 §offset-path: `offset-path-url-011`).
pub(super) fn shape_d(e: &crate::dom::Element) -> Option<String> {
    let n = |k: &str| {
        e.attr(k)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let arcs = |cx: f32, cy: f32, rx: f32, ry: f32| {
        format!(
            "M{} {cy} A{rx} {ry} 0 0 1 {cx} {} A{rx} {ry} 0 0 1 {} {cy} A{rx} {ry} 0 0 1 {cx} {} A{rx} {ry} 0 0 1 {} {cy} Z",
            cx + rx,
            cy + ry,
            cx - rx,
            cy - ry,
            cx + rx
        )
    };
    let pts = |closed: bool| -> Option<String> {
        let nums: Vec<f32> = e
            .attr("points")?
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter_map(|t| t.parse().ok())
            .collect();
        let mut d = String::new();
        for (i, p) in nums.chunks_exact(2).enumerate() {
            d.push_str(&format!(
                "{}{} {} ",
                if i == 0 { 'M' } else { 'L' },
                p[0],
                p[1]
            ));
        }
        if closed && !d.is_empty() {
            d.push('Z');
        }
        (!d.is_empty()).then_some(d)
    };
    Some(match e.tag.as_str() {
        "path" => e.attr("d")?.to_string(),
        "rect" => {
            let (x, y) = (n("x"), n("y"));
            format!("M{x} {y} H{} V{} H{x} Z", x + n("width"), y + n("height"))
        }
        "circle" => {
            let r = n("r");
            arcs(n("cx"), n("cy"), r, r)
        }
        "ellipse" => arcs(n("cx"), n("cy"), n("rx"), n("ry")),
        "line" => format!("M{} {} L{} {}", n("x1"), n("y1"), n("x2"), n("y2")),
        "polyline" => pts(false)?,
        "polygon" => pts(true)?,
        _ => return None,
    })
}
