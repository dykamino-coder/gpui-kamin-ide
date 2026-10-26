//! Рамка листа печатной пары из длинных свойств `border-*`.
//!
//! Каскад раскрывает `border` и его стороны в длинные свойства уже при
//! разборе (css-backgrounds-3 §4.4: опущенная часть — начальное значение,
//! `medium none currentcolor`), поэтому лист читает только их. Логические
//! стороны (`border-block-start-*` и родня, css-logical-1 §4.4) — по письму
//! корня, как поля листа (css-page-3 §page-properties: к коробке страницы
//! применимы свойства рамки).

use kamin_html::value::{Color, ValueCx};

/// Сторона рамки листа: заданная толщина, есть ли стиль, цвет.
#[derive(Clone, Copy)]
struct Side {
    width: f32,
    styled: bool,
    color: Color,
}

/// Четыре физические стороны рамки листа: верх/право/низ/лево.
pub(super) struct PageBorder([Side; 4]);

/// `currentColor` листа — чёрный (цвет текста по умолчанию).
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

impl PageBorder {
    /// Начальные значения: `medium none currentcolor`.
    pub(super) fn new() -> Self {
        Self(
            [Side {
                width: 3.0,
                styled: false,
                color: BLACK,
            }; 4],
        )
    }

    /// Применяет длинное свойство рамки; `false` — `name` не толщина, стиль
    /// или цвет стороны рамки. `logical(block, start)` — физическая сторона.
    pub(super) fn apply(
        &mut self,
        name: &str,
        value: &str,
        logical: impl Fn(bool, bool) -> usize,
        px_of: impl Fn(&str, bool) -> Option<f32>,
        vcx: ValueCx<'_>,
    ) -> bool {
        let Some(rest) = name.strip_prefix("border-") else {
            return false;
        };
        let Some((side, part)) = rest.rsplit_once('-') else {
            return false;
        };
        let i = match side {
            "top" => 0,
            "right" => 1,
            "bottom" => 2,
            "left" => 3,
            "block-start" => logical(true, true),
            "block-end" => logical(true, false),
            "inline-start" => logical(false, true),
            "inline-end" => logical(false, false),
            _ => return false,
        };
        let v = value.trim();
        let s = &mut self.0[i];
        match part {
            "width" => {
                s.width = match v.to_ascii_lowercase().as_str() {
                    "thin" => 1.0,
                    "medium" => 3.0,
                    "thick" => 5.0,
                    _ => px_of(v, false).unwrap_or(s.width),
                }
            }
            "style" => s.styled = !matches!(v.to_ascii_lowercase().as_str(), "none" | "hidden"),
            "color" => s.color = Color::parse(v, vcx).unwrap_or(BLACK),
            _ => return false,
        }
        true
    }

    /// Рамка листа для `PageGeom`: толщина (0 у стороны без стиля,
    /// css-backgrounds-3 §4.3) и цвет. Лист движка рисует одинаковую рамку
    /// со всех сторон, поэтому берётся верхняя сторона.
    pub(super) fn used(&self) -> (f32, Color) {
        let top = self.0[0];
        (if top.styled { top.width } else { 0.0 }, top.color)
    }
}

#[cfg(test)]
mod tests {
    use super::PageBorder;

    fn longhands(b: &mut PageBorder, decls: &[(&str, &str)]) {
        let doc = kamin_html::Document::builder("", "").build().unwrap();
        for (k, v) in decls {
            assert!(b.apply(
                k,
                v,
                |_, _| 0,
                |t, _| t.strip_suffix("px")?.parse().ok(),
                doc.values()
            ));
        }
    }

    #[test]
    fn expanded_shorthand_without_width_is_medium() {
        // `@page { border: solid }` (margin-boxes/auto-margins-001).
        let mut b = PageBorder::new();
        let sides = ["top", "right", "bottom", "left"];
        let decls: Vec<(String, &str)> = sides
            .iter()
            .flat_map(|s| {
                [
                    (format!("border-{s}-width"), "medium"),
                    (format!("border-{s}-style"), "solid"),
                    (format!("border-{s}-color"), "currentcolor"),
                ]
            })
            .collect();
        let refs: Vec<(&str, &str)> = decls.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        longhands(&mut b, &refs);
        let (w, c) = b.used();
        assert_eq!(w, 3.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn width_without_style_paints_nothing() {
        let mut b = PageBorder::new();
        longhands(
            &mut b,
            &[("border-top-width", "10px"), ("border-top-style", "none")],
        );
        assert_eq!(b.used().0, 0.0);
        longhands(&mut b, &[("border-block-start-style", "dotted")]);
        assert_eq!(b.used().0, 10.0);
    }

    #[test]
    fn radius_and_image_are_not_sides() {
        let mut b = PageBorder::new();
        let doc = kamin_html::Document::builder("", "").build().unwrap();
        assert!(!b.apply(
            "border-top-left-radius",
            "4px",
            |_, _| 0,
            |_, _| None,
            doc.values()
        ));
        assert!(!b.apply(
            "border-image-width",
            "1",
            |_, _| 0,
            |_, _| None,
            doc.values()
        ));
    }
}
