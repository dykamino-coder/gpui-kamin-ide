//! Computed::apply_one: border*, corner*, radius, outline*, border-spacing/collapse, border-image, border-shape.

use crate::style::computed::*;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_border(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {

            "border" => {
                // `border: inherit` — рамка родителя целиком: слово копирует
                // вычисленное значение, самим разбором его не выразить.
                if v == "inherit" {
                    self.border_inherit = true;
                    self.border_inherit_w = [true; 4];
                    self.border_inherit_s = [true; 4];
                    self.border_inherit_c = [true; 4];
                    return;
                }
                self.apply_border_shorthand(v, None)
            }
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                let i = match key {
                    "border-top" => 0,
                    "border-right" => 1,
                    "border-bottom" => 2,
                    _ => 3,
                };
                // `border-bottom: inherit` — все три части ОДНОЙ стороны.
                if v == "inherit" {
                    self.border_inherit_w[i] = true;
                    self.border_inherit_s[i] = true;
                    self.border_inherit_c[i] = true;
                    return;
                }
                self.apply_border_shorthand(v, Some(i))
            }
            "border-width" if v == "inherit" => self.border_inherit_w = [true; 4],
            "border-style" if v == "inherit" => self.border_inherit_s = [true; 4],
            "border-color" if v == "inherit" => self.border_inherit_c = [true; 4],
            "border-top-width"
            | "border-right-width"
            | "border-bottom-width"
            | "border-left-width"
                if v == "inherit" =>
            {
                self.border_inherit_w[side_index(key)] = true;
            }
            "border-top-style"
            | "border-right-style"
            | "border-bottom-style"
            | "border-left-style"
                if v == "inherit" =>
            {
                self.border_inherit_s[side_index(key)] = true;
            }
            "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
                if v == "inherit" =>
            {
                self.border_inherit_c[side_index(key)] = true;
            }
            "border-width" => {
                // Толщина словом (`thin`/`medium`/`thick`, §8.5.1) до сюда не
                // доезжала: общее сокращение по сторонам знает только длины, и
                // запись `border-width: thin medium medium medium` стирала
                // толщину на всех сторонах — рамка пропадала целиком.
                let list: Vec<Option<Len>> = v.split_whitespace().map(line_width).collect();
                // Недействительное значение делает НЕВАЛИДНЫМ всё объявление
                // (§4.2), а не одну сторону: иначе опечатка гасила рамку.
                if list.is_empty() || list.len() > 4 || list.iter().any(Option::is_none) {
                    return;
                }
                let at = |i: usize| -> Option<Len> {
                    let pick = match (list.len(), i) {
                        (1, _) => 0,
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i,
                    };
                    list[pick]
                };
                self.border_width = Sides {
                    top: at(0),
                    right: at(1),
                    bottom: at(2),
                    left: at(3),
                };
            }
            "border-collapse" => self.border_collapse = Some(v == "collapse"),
            "empty-cells" => self.empty_cells_hide = Some(v.trim() == "hide"),
            "border-color" => {
                // От одного до четырёх значений, как у всякого сокращения по
                // сторонам (§8.5.2). Прежде строка разбиралась целиком, и
                // запись `border-color: red orange red yellow` не давала
                // НИЧЕГО: цвет пропадал на всех сторонах разом.
                let list: Vec<&str> = v.split_whitespace().collect();
                if list.is_empty() || list.len() > 4 {
                    return;
                }
                if list.len() == 1 {
                    self.apply_single_border_color(v);
                    return;
                }
                let colors: Vec<Option<Color>> =
                    list.iter().map(|t| side_color(t, self.color)).collect();
                // Недействительное значение делает НЕВАЛИДНЫМ всё объявление
                // (§4.2), а не одну сторону.
                if colors.iter().any(Option::is_none) {
                    return;
                }
                let at = |i: usize| -> Option<Color> {
                    let pick = match (colors.len(), i) {
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i,
                    };
                    colors[pick]
                };
                for i in 0..4 {
                    self.border_colors[i] = at(i);
                }
                // Общий цвет остаётся у верхней стороны: его читают пути, не
                // знающие о сторонах.
                self.border_color = at(0);
            }
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
                    let radius = if lens.is_empty() { "0".to_string() } else { lens.join(" ") };
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
            "corner-top-shape" | "corner-bottom-shape" | "corner-left-shape" | "corner-right-shape" => {
                let vals: Vec<f32> = v.split_whitespace().filter_map(corner_shape_param).collect();
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

            // --- Рамки и обводка --------------------------------------------
            // Толщина словом (`thin`/`medium`/`thick`) — законное значение;
            // неразобранное значение НЕ стирает уже заданную толщину.
            "border-top-width" => self.border_width.top = line_width(v).or(self.border_width.top),
            "border-right-width" => {
                self.border_width.right = line_width(v).or(self.border_width.right)
            }
            "border-bottom-width" => {
                self.border_width.bottom = line_width(v).or(self.border_width.bottom)
            }
            "border-left-width" => {
                self.border_width.left = line_width(v).or(self.border_width.left)
            }
            // Логические кромки уходят в логический слой: физическая сторона
            // известна только после наследования письма (css-logical-1 §4.2;
            // у `vertical-rl` inline-start — ВЕРХ, разбор же писал влево:
            // logical-props-001).
            "border-inline" => {
                let l = self.logical();
                l.border[1] = Some(v.to_string());
                l.border[3] = Some(v.to_string());
            }
            "border-block" => {
                let l = self.logical();
                l.border[0] = Some(v.to_string());
                l.border[2] = Some(v.to_string());
            }
            "border-block-start" => self.logical().border[0] = Some(v.to_string()),
            "border-block-end" => self.logical().border[2] = Some(v.to_string()),
            "border-inline-start" => self.logical().border[3] = Some(v.to_string()),
            "border-inline-end" => self.logical().border[1] = Some(v.to_string()),
            "border-block-start-color" => self.border_colors[0] = side_color(v, self.color),
            "border-block-end-color" => self.border_colors[2] = side_color(v, self.color),
            "border-inline-start-color" => self.border_colors[3] = side_color(v, self.color),
            "border-inline-end-color" => self.border_colors[1] = side_color(v, self.color),
            "border-block-start-width" => {
                self.border_width.top = line_width(v).or(self.border_width.top)
            }
            "border-block-end-width" => {
                self.border_width.bottom = line_width(v).or(self.border_width.bottom)
            }
            "border-inline-start-width" => {
                self.border_width.left = line_width(v).or(self.border_width.left)
            }
            "border-inline-end-width" => {
                self.border_width.right = line_width(v).or(self.border_width.right)
            }
            "border-block-start-style" => self.set_border_style(v, Some(0)),
            "border-block-end-style" => self.set_border_style(v, Some(2)),
            "border-inline-start-style" => self.set_border_style(v, Some(3)),
            "border-inline-end-style" => self.set_border_style(v, Some(1)),
            "border-top-color" => self.border_colors[0] = side_color(v, self.color),
            "border-right-color" => self.border_colors[1] = side_color(v, self.color),
            "border-bottom-color" => self.border_colors[2] = side_color(v, self.color),
            "border-left-color" => self.border_colors[3] = side_color(v, self.color),
            "border-top-style" => self.set_border_style(v, Some(0)),
            "border-right-style" => self.set_border_style(v, Some(1)),
            "border-bottom-style" => self.set_border_style(v, Some(2)),
            "border-left-style" => self.set_border_style(v, Some(3)),
            "border-style" => {
                // От одного до четырёх значений, как у любого сокращения по
                // сторонам: `border-style: solid none` — рамка сверху и снизу
                // (css-backgrounds-3 §4.2). Прежде строка сравнивалась целиком,
                // и любая многозначная запись гасила рамку на всех сторонах.
                let side = |list: &[&str], i: usize| -> String {
                    let pick = match (list.len(), i) {
                        (1, _) => 0,
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i.min(list.len().saturating_sub(1)),
                    };
                    list[pick].to_ascii_lowercase()
                };
                let list: Vec<&str> = v.split_whitespace().collect();
                if list.is_empty() {
                    return;
                }
                // `dashed` и `dotted` — разные узоры; `double`, `groove` и
                // прочие рельефные сводятся к сплошной: рельефа в конвейере нет.
                self.border_dashed = Some(side(&list, 0) == "dashed");
                self.border_dotted = Some(side(&list, 0) == "dotted");
                let widths = [
                    &mut self.border_width.top,
                    &mut self.border_width.right,
                    &mut self.border_width.bottom,
                    &mut self.border_width.left,
                ];
                for (i, w) in widths.into_iter().enumerate() {
                    let one = side(&list, i);
                    let on = border_style(&one);
                    self.border_visible[i] = Some(on);
                    // Ранг рисунка — участник разбора сросшихся кромок
                    // (§17.6.2.1), и `hidden` там гасит соседей. Сокращение
                    // его не писало вовсе, поэтому `border-style: hidden` на
                    // ряде или группе до разбора не доезжал.
                    if let Some(rank) = border_style_rank(&one) {
                        self.border_side_styles[i] = Some(rank);
                    }
                    // Свой рисунок рамки делает её видимой: начальная толщина
                    // `medium` — это 3px, и задавать её отдельно не требуется.
                    if on && w.is_none() {
                        *w = Some(Len::Px(3.0));
                    }
                    if one == "none" || one == "hidden" {
                        *w = Some(Len::Px(0.0));
                    }
                }
            }
            "border-spacing" => {
                let (a, b) = axis_pair(v);
                self.border_spacing = Some((a, b));
            }
            "outline" => {
                // `outline: inherit` — вычисленное значение родителя целиком
                // (§6.2.1): само свойство не наследуется, поэтому копируются
                // все его части (`outline-002`).
                if v == "inherit" {
                    self.inherit_bits |= inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S;
                    return;
                }
                // Умолчание CSS — `medium`, три точки: без него запись
                // `outline: solid red` не рисовала ничего.
                let mut o = self.outline.unwrap_or(Outline {
                    width: Some(Len::Px(3.0)),
                    color: None,
                    offset: None,
                    inset: false,
                    style: None,
                });
                for token in split_ws_top(v) {
                    if let Some(st) = outline_style_of(token) {
                        o.style = Some(st);
                    } else if let Some(l) = outline_width_of(token) {
                        o.width = Some(l);
                    } else if let Some(c) = Color::parse(token) {
                        o.color = Some(c);
                    }
                }
                self.outline = Some(o);
            }
            "outline-width" | "outline-color" | "outline-offset" | "outline-style" => {
                // `outline-width: inherit` берёт у родителя ТОЛЬКО толщину:
                // остальные части обводки остаются своими. Разбор слова здесь
                // не выражается — `outline_width_of("inherit")` даёт `None`, и
                // толщина пропадала.
                if v == "inherit" {
                    self.inherit_bits |= match key {
                        "outline-width" => inh::OUTLINE_W,
                        "outline-color" => inh::OUTLINE_C,
                        "outline-style" => inh::OUTLINE_S,
                        _ => inh::OUTLINE_O,
                    };
                    return;
                }
                let mut o = self.outline.unwrap_or_default();
                match key {
                    // `initial`/`unset` ненаследуемой ширины — `medium`, 3px
                    // (css-ui-4 §outline-width); слова разбор не знал, ширина
                    // оставалась пустой, и обводка не рисовалась вовсе
                    // (`zoom/outline-width-keywords`, коробка `.initial`).
                    "outline-width" if matches!(v, "initial" | "unset") => {
                        o.width = Some(Len::Px(3.0))
                    }
                    "outline-width" => o.width = outline_width_of(v).or(o.width),
                    "outline-color" => o.color = Color::parse(v),
                    "outline-style" => o.style = outline_style_of(v),
                    // `outline-offset: inset` — «внутрь на толщину»: слово
                    // уходило в разбор длины, давало `None`, и контур ложился
                    // СНАРУЖИ коробки (`outline-offset-inset-002` 0.93,
                    // `-004` 3.31).
                    _ => {
                        o.inset = v == "inset";
                        o.offset = if o.inset { None } else { Len::parse(v) };
                    }
                }
                self.outline = Some(o);
            }
            // Картинка вместо рамки. Свойств пять, и каждое дополняет одну и
            // ту же запись — поэтому разбор общий.
            "border-image"
            | "border-image-source"
            | "border-image-slice"
            | "border-image-width"
            | "border-image-outset"
            | "border-image-repeat" => self.set_border_image(key, v),
            _ => *hit = false,
        }
    }
}
