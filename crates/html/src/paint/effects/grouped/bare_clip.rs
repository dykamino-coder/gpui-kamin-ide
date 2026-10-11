//! Клип по голой опорной коробке и её радиусам.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn bare_clip(c: &Computed) -> (Option<[Len; 4]>, Option<Len>) {
    let bare_inset = if c.clip_bare_box && c.clip_inset.is_none() {
        let b = c.borders();
        let s = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let [t, r, bo, l] = match c.clip_ref {
            Some(1) => [
                -s(c.margin.top),
                -s(c.margin.right),
                -s(c.margin.bottom),
                -s(c.margin.left),
            ],
            Some(2) => [s(b.top), s(b.right), s(b.bottom), s(b.left)],
            Some(3) => [
                s(b.top) + s(c.padding.top),
                s(b.right) + s(c.padding.right),
                s(b.bottom) + s(c.padding.bottom),
                s(b.left) + s(c.padding.left),
            ],
            _ => [0.0; 4],
        };
        Some([Len::Px(t), Len::Px(r), Len::Px(bo), Len::Px(l)])
    } else {
        None
    };
    // Голая коробка режет ВМЕСТЕ со скруглением углов (css-masking-1
    // §5.1 «<geometry-box>… including any corner shaping (e.g.
    // border-radius)»): радиус коробки — радиус рамки, сдвинутый на ту же
    // толщину (css-backgrounds-3 §5.2 inner/outer curves). Пока — только
    // равные круглые углы и равные стороны сдвига.
    let bare_round = bare_inset.and_then(|inset| {
        if c.radius_ell.is_some() {
            return None;
        }
        let r = match (c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl) {
            (Some(Len::Px(a)), Some(Len::Px(b)), Some(Len::Px(d)), Some(Len::Px(e)))
                if a == b && b == d && d == e && a > 0.0 =>
            {
                a
            }
            _ => return None,
        };
        let d = match inset {
            [Len::Px(t), Len::Px(rr), Len::Px(bo), Len::Px(l)]
                if t == rr && rr == bo && bo == l =>
            {
                t
            }
            _ => return None,
        };
        // margin-box наружу: радиус растёт на поле, когда он не меньше поля
        // (css-shapes-1 §6.1 — при r < m нужна поправка, её здесь нет).
        if d < 0.0 && r < -d {
            return None;
        }
        let r = (r - d).max(0.0);
        (r > 0.0).then_some(Len::Px(r))
    });
    (bare_inset, bare_round)
}
