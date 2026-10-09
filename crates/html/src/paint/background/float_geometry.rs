//! Preserve continuous geometry for rounded shape-outside boxes.

use super::ShapeBox;

pub fn rounded_float(raw: &str, b: &super::ShapeBox, side: i32) -> Option<crate::flow::RoundedBox> {
    let (rect, radii) = rrect_of(raw, b)?;
    Some(crate::flow::RoundedBox::new(
        rect,
        radii,
        (b.mw, b.mh),
        side,
    ))
}

/// inset/rect/xywh/слово-коробка → прямоугольник (x,y,w,h) + радиусы.
pub(super) fn rrect_of(raw: &str, b: &ShapeBox) -> Option<((f32, f32, f32, f32), [(f32, f32); 4])> {
    let len_px = |t: &str, base: f32| -> f32 {
        match crate::value::Len::parse(t) {
            Some(crate::value::Len::Px(v)) => v,
            Some(crate::value::Len::Pct(k)) => k * base,
            _ => 0.0,
        }
    };
    let parse_round = |tail: &str| -> [(f32, f32); 4] {
        // `round r1 r2 r3 r4 / v1 v2 v3 v4` — как border-radius.
        let (hs, vs) = match tail.split_once('/') {
            Some((a, c)) => (a, c),
            None => (tail, tail),
        };
        let four = |src: &str, base: f32| -> [f32; 4] {
            let v: Vec<f32> = src.split_whitespace().map(|t| len_px(t, base)).collect();
            match v.len() {
                0 => [0.0; 4],
                1 => [v[0]; 4],
                2 => [v[0], v[1], v[0], v[1]],
                3 => [v[0], v[1], v[2], v[1]],
                _ => [v[0], v[1], v[2], v[3]],
            }
        };
        let h = four(hs, b.rw);
        let v = four(vs, b.rh);
        [(h[0], v[0]), (h[1], v[1]), (h[2], v[2]), (h[3], v[3])]
    };
    if let Some(at) = raw.find("inset(") {
        let inner = raw[at + 6..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 6..]);
        {
            let (sides_s, round_s) = match inner.split_once("round") {
                Some((a, r)) => (a, Some(r)),
                None => (inner, None),
            };
            let v: Vec<&str> = sides_s.split_whitespace().collect();
            let side = |i: usize| v.get(i).copied().unwrap_or("0");
            let (t, r, bo, l) = match v.len() {
                1 => (side(0), side(0), side(0), side(0)),
                2 => (side(0), side(1), side(0), side(1)),
                3 => (side(0), side(1), side(2), side(1)),
                _ => (side(0), side(1), side(2), side(3)),
            };
            let (t, r2, bo, l) = (
                len_px(t, b.rh),
                len_px(r, b.rw),
                len_px(bo, b.rh),
                len_px(l, b.rw),
            );
            let rect = (
                b.rx + l,
                b.ry + t,
                (b.rw - l - r2).max(0.0),
                (b.rh - t - bo).max(0.0),
            );
            let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
            return Some((rect, radii));
        }
    }
    // `rect(t r b l)` — края от сторон опорной коробки, `auto` значит край
    // (css-shapes-1 §3.1). Отличается от `inset` тем, что правый и нижний
    // отсчитываются от ЛЕВОГО и ВЕРХНЕГО края, а не внутрь от своих.
    if let Some(at) = raw.find("rect(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        let (sides_s, round_s) = match inner.split_once("round") {
            Some((a, r)) => (a, Some(r)),
            None => (inner, None),
        };
        let v: Vec<&str> = sides_s.split_whitespace().collect();
        let edge = |i: usize, base: f32, dflt: f32| -> f32 {
            match v.get(i).copied() {
                None | Some("auto") => dflt,
                Some(t) => len_px(t, base),
            }
        };
        let (t, r2, bo, l) = (
            edge(0, b.rh, 0.0),
            edge(1, b.rw, b.rw),
            edge(2, b.rh, b.rh),
            edge(3, b.rw, 0.0),
        );
        let rect = (b.rx + l, b.ry + t, (r2 - l).max(0.0), (bo - t).max(0.0));
        let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
        return Some((rect, radii));
    }
    // `xywh(x y w h)` — угол и размер прямо (css-shapes-1 §3.1).
    if let Some(at) = raw.find("xywh(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        let (sides_s, round_s) = match inner.split_once("round") {
            Some((a, r)) => (a, Some(r)),
            None => (inner, None),
        };
        let v: Vec<&str> = sides_s.split_whitespace().collect();
        let at_i = |i: usize, base: f32| -> f32 { v.get(i).map_or(0.0, |t| len_px(t, base)) };
        let rect = (
            b.rx + at_i(0, b.rw),
            b.ry + at_i(1, b.rh),
            at_i(2, b.rw).max(0.0),
            at_i(3, b.rh).max(0.0),
        );
        let radii = round_s.map(parse_round).unwrap_or([(0.0, 0.0); 4]);
        return Some((rect, radii));
    }
    // Слово-коробка (или пустая/непонятная запись формы НЕ здесь — сюда
    // приходят только распознанные): margin/border/padding/content-box без
    // функции — прямоугольник опорной коробки с её радиусами.
    let word_only = raw.split_whitespace().all(|w| w.ends_with("-box"));
    if word_only && !raw.is_empty() {
        return Some(((b.rx, b.ry, b.rw, b.rh), b.radius));
    }
    None
}
