//! Computed::apply_border, хвост цепочки: ширины сторон, логические кромки, цвета и стили сторон, border-style, border-spacing, outline*, border-image*. Ветви в исходном порядке после ветвей углов.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_border_sides_outline(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
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
