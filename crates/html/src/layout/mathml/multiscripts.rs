//! Multiscripts MathML Core: общие сдвиги и парные колонки пре-/постскриптов.
use super::{
    geometry::Metrics,
    parameters::ScriptKind,
    scripts::{self, Parameters},
};
pub(super) fn layout(
    children: &[Metrics],
    p: Parameters,
    kind: ScriptKind,
    rtl: bool,
) -> (Metrics, Vec<(f32, f32)>) {
    let base = children[0];
    if !matches!(kind, ScriptKind::Multi(_)) {
        let sub = matches!(kind, ScriptKind::Sub | ScriptKind::Both).then(|| children[1]);
        let sup = match kind {
            ScriptKind::Sup => Some(children[1]),
            ScriptKind::Both => Some(children[2]),
            _ => None,
        };
        return scripts::scripts(base, sub, sup, p, rtl);
    }
    let ScriptKind::Multi(marker) = kind else {
        // Все остальные ScriptKind завершили функцию в предыдущем if.
        unreachable!()
    };
    let marker = marker.unwrap_or(children.len());
    let mut up = 0.0f32;
    let mut down = 0.0f32;
    let pairs = children[1..]
        .chunks_exact(2)
        .map(|v| (v[0], v[1]))
        .collect::<Vec<_>>();
    for &(sub, sup) in &pairs {
        let (m, o) = scripts::scripts(base, Some(sub), Some(sup), p, false);
        up = up.max(m.ascent - o[2].1 - sup.ascent);
        down = down.max(o[1].1 + sub.ascent - m.ascent);
    }
    let ascent = pairs
        .iter()
        .fold(base.ascent, |a, (_, s)| a.max(up + s.ascent));
    let descent = pairs
        .iter()
        .fold(base.descent, |d, (s, _)| d.max(down + s.descent));
    let pre_width = pairs
        .iter()
        .enumerate()
        .filter(|(i, _)| 1 + 2 * i >= marker)
        .map(|(_, (a, b))| a.width.max(b.width) + p.space)
        .sum::<f32>();
    let width = base.width
        + pairs
            .iter()
            .map(|(a, b)| a.width.max(b.width) + p.space)
            .sum::<f32>();
    let mut positions = vec![(pre_width, ascent - base.ascent)];
    let mut post = pre_width + base.width;
    let mut pre = 0.0;
    for (i, &(sub, sup)) in pairs.iter().enumerate() {
        let column = sub.width.max(sup.width);
        let before = 1 + 2 * i >= marker;
        let start = if before {
            let x = pre + p.space;
            pre += column + p.space;
            x
        } else {
            let x = post;
            post += column + p.space;
            x
        };
        positions.push((
            start + if before { column - sub.width } else { 0.0 },
            ascent + down - sub.ascent,
        ));
        positions.push((
            start + if before { column - sup.width } else { 0.0 },
            ascent - up - sup.ascent,
        ));
    }
    if rtl {
        for (o, c) in positions.iter_mut().zip(children) {
            o.0 = width - o.0 - c.width;
        }
    }
    (
        Metrics {
            width,
            ascent,
            descent,
        },
        positions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rtl_prescripts_leave_space_on_the_outer_side_of_each_column() {
        let p = Parameters {
            sub_shift: 0.0,
            sub_drop: 0.0,
            sub_top: 0.0,
            sup_shift: 0.0,
            sup_drop: 0.0,
            sup_bottom: 0.0,
            sup_bottom_with_sub: 0.0,
            gap: 0.0,
            space: 30.0,
        };
        let child = Metrics {
            width: 40.0,
            ascent: 10.0,
            descent: 0.0,
        };
        let (_, positions) = layout(&[child; 9], p, ScriptKind::Multi(Some(5)), true);
        assert_eq!(positions[0].0, 140.0);
        assert_eq!(positions[5].0, 250.0);
        assert_eq!(positions[7].0, 180.0);
    }
}
