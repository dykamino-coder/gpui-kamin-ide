//! Computed::apply_one: border*, corner*, radius, outline*, border-spacing/collapse, border-image, border-shape.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

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

impl Computed {
    /// Толщина рамки, как её видит модель коробки. Начальный `border-style`
    /// — `none`, а рамка без рисунка вычисляется в ноль (css-backgrounds-3
    /// §4.3), поэтому заданная, но не нарисованная толщина не занимает места.
    /// Разбор всех пяти свойств рамки-картинки в одну запись.
    ///
    /// Сокращение несёт до трёх частей через косую: `<источник> <срез> /
    /// <ширина> / <вылет>` и укладку в конце. Отдельные свойства дополняют ту
    /// же запись, поэтому она заводится по первому же из них.
    pub(crate) fn set_border_image(&mut self, name: &str, v: &str) {
        let mut image = self.border_image.clone().unwrap_or(BorderImage {
            src: String::new(),
            slice: [BorderImageSlice::Pct(1.0); 4],
            fill: false,
            width: [BorderImageWidth::Times(1.0); 4],
            outset: [0.0; 4],
            repeat: (Tiling::None, Tiling::None),
        });
        let mut set = |part: &str, value: &str| match part {
            "border-image-source" => {
                let value = value.trim();
                if value == "none" {
                    image.src.clear();
                } else if value.starts_with("linear-gradient(")
                    || value.starts_with("radial-gradient(")
                    || value.starts_with("conic-gradient(")
                {
                    // Источником может быть любой `<image>`, включая градиент
                    // (css-backgrounds-3 §6.1). Запись хранится как есть:
                    // растрирует её загрузчик картинок.
                    image.src = value.to_string();
                } else if let Some(url) = parse_url(value) {
                    image.src = url;
                }
            }
            "border-image-slice" => {
                image.fill = value.split_whitespace().any(|w| w == "fill");
                let nums: Vec<&str> = value.split_whitespace().filter(|w| *w != "fill").collect();
                let one = |t: &str| match t.strip_suffix('%') {
                    Some(n) => n
                        .parse()
                        .ok()
                        .map(|k: f32| BorderImageSlice::Pct(k / 100.0)),
                    None => t.parse().ok().map(BorderImageSlice::Px),
                };
                if let Some(sides) = four(&nums, one) {
                    image.slice = sides;
                }
            }
            "border-image-width" => {
                let one = |t: &str| {
                    if t == "auto" {
                        return Some(BorderImageWidth::Auto);
                    }
                    if let Some(n) = t.strip_suffix('%') {
                        return n
                            .parse()
                            .ok()
                            .map(|k: f32| BorderImageWidth::Pct(k / 100.0));
                    }
                    // Голое число — множитель толщины рамки, и проверяется ДО
                    // Len: та принимает числа без единиц как точки, и
                    // `border-image-width: 1` превращался в рамку 1px.
                    if let Ok(k) = t.parse::<f32>() {
                        return Some(BorderImageWidth::Times(k));
                    }
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(BorderImageWidth::Px(v)),
                        _ => None,
                    }
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.width = sides;
                }
            }
            "border-image-outset" => {
                let one = |t: &str| match Len::parse(t) {
                    Some(Len::Px(v)) => Some(v),
                    // Голое число — тоже множитель толщины рамки, но её здесь
                    // ещё нет; берём как точки, это ближе всего к правде.
                    _ => t.parse().ok(),
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.outset = sides;
                }
            }
            "border-image-repeat" => {
                let one = |t: &str| match t {
                    "stretch" => Some(Tiling::None),
                    "repeat" => Some(Tiling::Repeat),
                    "round" => Some(Tiling::Round),
                    "space" => Some(Tiling::Space),
                    _ => None,
                };
                let mut it = value.split_whitespace();
                if let (Some(x), y) = (it.next().and_then(one), it.next().and_then(one)) {
                    image.repeat = (x, y.unwrap_or(x));
                }
            }
            _ => {}
        };
        if name == "border-image" {
            // Части сокращения разделены косой: источник со срезом, ширина,
            // вылет. Укладка и `fill` живут в своих частях и находятся по
            // ключевым словам.
            // Косая режет части ТОЛЬКО вне скобок: в адресе она разделяет
            // каталоги, и запись рвалась по первому же пути (`url(C:/tmp/…)`).
            let mut parts: Vec<&str> = vec![];
            let mut depth = 0i32;
            let mut from = 0usize;
            for (at, ch) in v.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    '/' if depth == 0 => {
                        parts.push(&v[from..at]);
                        from = at + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&v[from..]);
            let head = parts.first().copied().unwrap_or("");
            // Слова головы режутся ВНЕ скобок: у градиента-источника пробелы
            // внутри (`linear-gradient(green, green)`), и он должен остаться
            // одним словом.
            let words = split_outside_parens(head);
            let (urls, rest): (Vec<&String>, Vec<&String>) = words.iter().partition(|w| {
                w.starts_with("url(")
                    || w.starts_with("linear-gradient(")
                    || w.starts_with("radial-gradient(")
                    || w.starts_with("conic-gradient(")
                    || w.as_str() == "none"
            });
            if let Some(src) = urls.first() {
                set("border-image-source", src);
            }
            let (repeat, slice): (Vec<&&String>, Vec<&&String>) = rest
                .iter()
                .partition(|w| matches!(w.as_str(), "stretch" | "repeat" | "round" | "space"));
            if !slice.is_empty() {
                let joined: Vec<&str> = slice.iter().map(|w| w.as_str()).collect();
                set("border-image-slice", &joined.join(" "));
            }
            if !repeat.is_empty() {
                let joined: Vec<&str> = repeat.iter().map(|w| w.as_str()).collect();
                set("border-image-repeat", &joined.join(" "));
            }
            // Укладка (`stretch|repeat|round|space`) по грамматике `||` может
            // стоять и ПОСЛЕ ширины без своей косой: `/ 0px space round`.
            // Слова укладки вынимаются из хвостовых частей, остаток — ширина
            // и вылет.
            let mut tail_repeat: Vec<String> = vec![];
            let mut strip = |part: &str, reps: &mut Vec<String>| -> String {
                let (found, rest): (Vec<&str>, Vec<&str>) = part
                    .split_whitespace()
                    .partition(|w| matches!(*w, "stretch" | "repeat" | "round" | "space"));
                reps.extend(found.into_iter().map(str::to_string));
                rest.join(" ")
            };
            let width = parts.get(1).map(|p| strip(p, &mut tail_repeat));
            let outset = parts.get(2).map(|p| strip(p, &mut tail_repeat));
            drop(strip);
            if let Some(width) = width.filter(|w| !w.is_empty()) {
                set("border-image-width", &width);
            }
            if let Some(outset) = outset.filter(|o| !o.is_empty()) {
                set("border-image-outset", &outset);
            }
            if !tail_repeat.is_empty() {
                set("border-image-repeat", &tail_repeat.join(" "));
            }
        } else {
            set(name, v);
        }
        drop(set);
        // Запись хранится и БЕЗ источника: лонгхенды приходят в любом порядке,
        // и `border-image-slice` до `border-image-source` иначе выбрасывался —
        // источник, пришедший следом, получал срезы по умолчанию (вся
        // картинка), и рамка рисовалась четырьмя сжатыми копиями образа.
        // Не рисовать и не подавлять обычную рамку при пустом источнике —
        // забота потребителей.
        self.border_image = Some(image);
    }

    /// `border-left-style: solid` — рисунок одной стороны. Без рисунка её
    /// толщина не считается, поэтому видимость помним по сторонам.
    pub(crate) fn set_border_style(&mut self, v: &str, side: Option<usize>) {
        let on = border_style(v);
        if let Some(r) = border_style_rank(v) {
            match side {
                None => self.border_side_styles = [Some(r); 4],
                Some(i) => self.border_side_styles[i] = Some(r),
            }
        }
        self.set_visible(side, on);
        if on {
            let w = match side {
                Some(0) => &mut self.border_width.top,
                Some(1) => &mut self.border_width.right,
                Some(2) => &mut self.border_width.bottom,
                _ => &mut self.border_width.left,
            };
            if w.is_none() {
                *w = Some(Len::Px(3.0));
            }
        }
    }

    pub(crate) fn set_visible(&mut self, side: Option<usize>, on: bool) {
        match side {
            None => self.border_visible = [Some(on); 4],
            Some(i) => self.border_visible[i] = Some(on),
        }
    }

    /// `border: 1px solid #333` — ширина и цвет; стиль линии GPUI различает
    /// только solid/dashed на весь элемент, поэтому его не разбираем.
    pub(crate) fn apply_border_shorthand(&mut self, v: &str, side: Option<usize>) {
        // Негодная часть роняет ВСЁ объявление (§4.2), а не пропускается:
        // `border: -1px solid red` не даёт ни рамки `medium`, ни красного
        // цвета. Проверка отдельным проходом — применение ниже правит поля по
        // ходу разбора, и откатить его на середине уже нельзя.
        let known = |token: &str| {
            token == "none"
                || token == "hidden"
                || border_style(token)
                || line_width(token).is_some()
                || Color::parse(token).is_some()
        };
        if !split_outside_parens(v)
            .iter()
            .all(|t| known(t.as_str().trim()))
        {
            return;
        }
        // Каждая часть встречается не больше ОДНОГО раза (§8.5.4: сокращение
        // это `<border-width> || <border-style> || <border-color>`).
        // `border: 1px solid red green` негодно целиком, а прежде вторая
        // краска просто побеждала первую.
        {
            let (mut w, mut st, mut c) = (0usize, 0usize, 0usize);
            for token in split_outside_parens(v) {
                let t = token.as_str().trim();
                if t == "none" || t == "hidden" || border_style(t) {
                    st += 1;
                } else if line_width(t).is_some() {
                    w += 1;
                } else if Color::parse(t).is_some() {
                    c += 1;
                }
            }
            if w > 1 || st > 1 || c > 1 {
                return;
            }
        }
        let mut width = None;
        let mut color = None;
        let mut visible_style = false;
        // Скобки не разрываем: `border: 1px solid rgba(0, 0, 0, .2)`.
        for token in split_outside_parens(v) {
            let token = token.as_str();
            if token == "none" || token == "hidden" {
                width = Some(Len::Px(0.0));
                self.set_visible(side, false);
                let r = border_style_rank(token);
                match side {
                    None => self.border_side_styles = [r; 4],
                    Some(i) => self.border_side_styles[i] = r,
                }
            } else if border_style(token) {
                visible_style = true;
                self.set_visible(side, true);
                if let Some(r) = border_style_rank(token) {
                    match side {
                        None => self.border_side_styles = [Some(r); 4],
                        Some(i) => self.border_side_styles[i] = Some(r),
                    }
                }
                self.border_dashed = Some(token == "dashed");
                self.border_dotted = Some(token == "dotted");
            } else if let Some(l) = line_width(token) {
                width = Some(l);
            } else if let Some(c) = Color::parse(token) {
                color = Some(c);
            }
        }
        // Толщина в сокращении необязательна: `border: orange solid` — это
        // рамка `medium`, то есть 3px (css-backgrounds-3 §4.2). Раньше такая
        // запись оставляла коробку БЕЗ рамки, и её содержимое считалось по
        // другой ширине (`word-break-break-all-062`).
        if width.is_none() && visible_style {
            width = Some(Len::Px(3.0));
        }
        // Цвет из БОКОВОГО сокращения принадлежит своей стороне: раньше он
        // писался в общий цвет, и `border-bottom: 2px solid red` красил все
        // четыре стороны.
        // Опущенная часть сокращения возвращается к НАЧАЛЬНОМУ значению
        // (§1.4.2, §8.5.4): у цвета это `currentColor`, то есть пусто — цвет
        // решает отрисовка. Прежде запись `border: solid 1em` оставляла цвет
        // от менее специфичного правила.
        match (color, side) {
            (Some(c), None) => {
                self.border_color = Some(c);
                self.border_colors = [Some(c); 4];
            }
            (Some(c), Some(i)) => self.border_colors[i] = Some(c),
            (None, None) => {
                self.border_color = None;
                self.border_colors = [None; 4];
            }
            // Боковое сокращение без цвета даёт стороне `currentColor`
            // (§8.5.4), и он обязан перебить общий цвет менее специфичного
            // правила: `div { border-color: red }` + `.test { border-top: solid
            // 1em }` — верх цвета ТЕКСТА (`border-shorthands-003`). Пустой слот
            // стороны значит «взять общий», поэтому сторона помечается.
            (None, Some(i)) => {
                self.border_colors[i] = None;
                self.border_side_current[i] = true;
            }
        }
        let Some(w) = width else { return };
        match side {
            None => {
                self.border_width = Sides {
                    top: Some(w),
                    right: Some(w),
                    bottom: Some(w),
                    left: Some(w),
                }
            }
            Some(0) => self.border_width.top = Some(w),
            Some(1) => self.border_width.right = Some(w),
            Some(2) => self.border_width.bottom = Some(w),
            Some(3) => self.border_width.left = Some(w),
            _ => {}
        }
    }
}

/// Рисунок рамки, при котором она ВИДНА. `none` и `hidden` сюда не входят:
/// они рамку убирают.
/// Ранг стиля кромки для разбора конфликтов (см. `border_side_styles`).
fn border_style_rank(v: &str) -> Option<u8> {
    Some(match v.to_ascii_lowercase().as_str() {
        "none" => 0,
        "hidden" => 1,
        "inset" => 3,
        "groove" => 4,
        "outset" => 5,
        "ridge" => 6,
        "dotted" => 7,
        "dashed" => 8,
        "solid" => 9,
        "double" => 10,
        _ => return None,
    })
}

fn border_style(v: &str) -> bool {
    // Значения CSS нечувствительны к регистру: `border: 1px SOLID red` — та же
    // рамка. К нижнему регистру приводится только ИМЯ свойства.
    matches!(
        v.to_ascii_lowercase().as_str(),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset"
    )
}

/// Толщина рамки словом: `thin`, `medium`, `thick` (css-backgrounds-3 §4.1).
/// Сторона по имени свойства: верх, право, низ, лево.
fn side_index(key: &str) -> usize {
    match key.split('-').nth(1) {
        Some("right") => 1,
        Some("bottom") => 2,
        Some("left") => 3,
        _ => 0,
    }
}

fn line_width(v: &str) -> Option<Len> {
    // Отрицательная толщина недействительна (§8.5.1) и делает объявление
    // НЕВАЛИДНЫМ целиком (§4.2): `border-width: -1px` доживало до отрисовки
    // вместо отката к прежнему значению.
    let non_negative = |l: Len| {
        (!matches!(
            l,
            Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v) if v < 0.0
        ))
        .then_some(l)
    };
    match v.to_ascii_lowercase().as_str() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v).and_then(non_negative),
    }
}

/// Пара значений «вдоль оси»: одно значение — обе грани, два — по порядку.
pub(crate) fn axis_pair(v: &str) -> (Option<Len>, Option<Len>) {
    let mut it = v.split_whitespace();
    let a = it.next().and_then(Len::parse);
    let b = it.next().and_then(Len::parse).or(a);
    (a, b)
}

/// Цвет стороны рамки; `currentColor` даёт цвет текста этого же узла.
fn side_color(v: &str, current: Option<Color>) -> Option<Color> {
    if v.eq_ignore_ascii_case("currentcolor") {
        return current;
    }
    Color::parse(v)
}
