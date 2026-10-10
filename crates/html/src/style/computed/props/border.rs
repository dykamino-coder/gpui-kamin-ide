//! Computed::apply_one: border*, corner*, radius, outline*, border-spacing/collapse, border-image, border-shape.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod corners;
mod image;
mod shorthand;
mod sides_outline;

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
            _ => self.apply_border_corners(key, val, v, hit),
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
