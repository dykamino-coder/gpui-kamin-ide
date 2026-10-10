//! Computed::apply_border, продолжение цепочки: border-shape, corner-shape*, corner-*-shape, радиусы углов. Ветви в исходном порядке; не совпавший ключ уходит в apply_border_sides_outline.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_border_corners(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `border-shape` (css-borders-4 §border-shape): `none` либо одна-две
            // базовые фигуры, каждая с необязательной опорной коробкой ПОСЛЕ
            // неё. Дефолты — как в Blink `ConvertBorderShape`: одна фигура →
            // half-border-box, две → border-box и padding-box. Неразобранное
            // значение — объявление отбрасывается (прежнее остаётся).
            "border-shape" => {
                if v.eq_ignore_ascii_case("none") {
                    self.border_shape = None;
                } else if let Some(bs) = parse_border_shape(v) {
                    self.border_shape = Some(bs);
                }
            }
            // `corner-shape` (css-borders-4 §corner-shaping-shorthand): 1–4
            // значения раскладываются по углам как `border-radius`.
            // Сокращение `corner` (css-borders-4 §corner-shorthand): до
            // четырёх углов через `/` по часовой от верхнего левого (схема
            // 1–4 значений — как у `border-radius`), в каждом — радиус
            // `<length-percentage>{1,2}` и/или форма `<corner-shape-value>`
            // в любом порядке; опущенное — начальное (`0`, `round`).
            // Раскладывается в те же лонгхенды, что пишет эталон
            // (`corner-shorthand-rendering`).
            "corner" => {
                let mut corners: Vec<(Vec<&str>, Option<&str>)> = Vec::new();
                for part in v.split('/').map(str::trim) {
                    let mut lens = Vec::new();
                    let mut shape = None;
                    for tok in part.split_whitespace() {
                        if corner_shape_param(tok).is_some() {
                            shape = Some(tok);
                        } else if Len::parse(tok).is_some() {
                            lens.push(tok);
                        } else {
                            return;
                        }
                    }
                    corners.push((lens, shape));
                }
                let pick: [usize; 4] = match corners.len() {
                    1 => [0, 0, 0, 0],
                    2 => [0, 1, 0, 1],
                    3 => [0, 1, 2, 1],
                    4 => [0, 1, 2, 3],
                    _ => return,
                };
                const NAMES: [&str; 4] = ["top-left", "top-right", "bottom-right", "bottom-left"];
                for (slot, &i) in pick.iter().enumerate() {
                    let (lens, shape) = &corners[i];
                    let radius = if lens.is_empty() {
                        "0".to_string()
                    } else {
                        lens.join(" ")
                    };
                    self.apply_one(&format!("border-{}-radius", NAMES[slot]), &radius);
                    self.apply_one(
                        &format!("corner-{}-shape", NAMES[slot]),
                        shape.unwrap_or("round"),
                    );
                }
            }
            "corner-shape" => {
                if let Some(k) = corner_shape_shorthand(v) {
                    self.corner_shape = Some(k);
                }
            }
            // Боковые шортхенды (§corner-shaping-side-shorthands): 1–2 значения
            // на два угла стороны.
            "corner-top-shape"
            | "corner-bottom-shape"
            | "corner-left-shape"
            | "corner-right-shape" => {
                let vals: Vec<f32> = v
                    .split_whitespace()
                    .filter_map(corner_shape_param)
                    .collect();
                if let Some(first) = vals.first().copied() {
                    let second = vals.get(1).copied().unwrap_or(first);
                    let mut k = self.corner_shape.unwrap_or([1.0; 4]);
                    // Порядок пар — по часовой от первого угла стороны.
                    let (a, b) = match key {
                        "corner-top-shape" => (0, 1),
                        "corner-right-shape" => (1, 2),
                        "corner-bottom-shape" => (3, 2),
                        _ => (0, 3),
                    };
                    k[a] = first;
                    k[b] = second;
                    self.corner_shape = Some(k);
                }
            }
            "corner-top-left-shape"
            | "corner-top-right-shape"
            | "corner-bottom-right-shape"
            | "corner-bottom-left-shape" => {
                if let Some(val) = corner_shape_param(v) {
                    let mut k = self.corner_shape.unwrap_or([1.0; 4]);
                    k[match key {
                        "corner-top-left-shape" => 0,
                        "corner-top-right-shape" => 1,
                        "corner-bottom-right-shape" => 2,
                        _ => 3,
                    }] = val;
                    self.corner_shape = Some(k);
                }
            }
            // Сокращения стороны (css-borders-4 §corner-sizing, tentative):
            // два угла одной стороны. До `/` — первый угол (`rx [ry]`), после —
            // второй; без `/` оба одинаковы. Углы стороны идут слева направо
            // (верх/низ) и сверху вниз (лево/право): `border-bottom-radius:
            // 3em 2em / 4em 1em` ≡ `bottom-left: 3em 2em; bottom-right: 4em
            // 1em` (`border-radius-side-shorthands-001`). Логические — по
            // письму на момент объявления; вертикальное письмо пока как
            // горизонтальное (`-002` горизонтальный).
            "border-top-radius"
            | "border-right-radius"
            | "border-bottom-radius"
            | "border-left-radius"
            | "border-block-start-radius"
            | "border-block-end-radius"
            | "border-inline-start-radius"
            | "border-inline-end-radius" => {
                let rtl = self.rtl == Some(true);
                let (a, b) = match (key, rtl) {
                    ("border-top-radius", _) | ("border-block-start-radius", false) => (0, 1),
                    ("border-block-start-radius", true) => (1, 0),
                    ("border-bottom-radius", _) | ("border-block-end-radius", false) => (3, 2),
                    ("border-block-end-radius", true) => (2, 3),
                    ("border-left-radius", _)
                    | ("border-inline-start-radius", false)
                    | ("border-inline-end-radius", true) => (0, 3),
                    _ => (1, 2),
                };
                let (first, second) = match v.split_once('/') {
                    Some((x, y)) => (x.trim(), y.trim()),
                    None => (v, v),
                };
                const LONG: [&str; 4] = [
                    "border-top-left-radius",
                    "border-top-right-radius",
                    "border-bottom-right-radius",
                    "border-bottom-left-radius",
                ];
                self.apply_one(LONG[a], first);
                self.apply_one(LONG[b], second);
            }
            "border-radius" => self.apply_radius_shorthand(v),
            "border-top-left-radius"
            | "border-top-right-radius"
            | "border-bottom-right-radius"
            | "border-bottom-left-radius" => self.apply_radius_corner(key, v),

            _ => self.apply_border_sides_outline(key, val, v, hit),
        }
    }
}
