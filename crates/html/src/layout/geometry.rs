//! Снимок CSSOM View: прямоугольники фрагментов, не области краски.

mod collect;
pub(crate) use collect::Collector;

/// Неподвижный снимок DOMRect в CSS px относительно области просмотра.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub(crate) fn from_bounds(b: gpui::Bounds<gpui::Pixels>) -> Self {
        Self {
            x: f32::from(b.origin.x).into(),
            y: f32::from(b.origin.y).into(),
            width: f32::from(b.size.width).into(),
            height: f32::from(b.size.height).into(),
        }
    }

    /// CSSOM View §6: пустой список, первый вырожденный либо объединение непустых.
    pub fn bounding(rects: &[Self]) -> Self {
        let Some(first) = rects.first() else {
            return Self::default();
        };
        let mut nonempty = rects.iter().filter(|r| r.width != 0.0 && r.height != 0.0);
        let Some(initial) = nonempty.next().copied() else {
            return *first;
        };
        nonempty.fold(initial, |out, r| {
            let right = (out.x + out.width).max(r.x + r.width);
            let bottom = (out.y + out.height).max(r.y + r.height);
            let x = out.x.min(r.x);
            let y = out.y.min(r.y);
            Self {
                x,
                y,
                width: right - x,
                height: bottom - y,
            }
        })
    }
}

/// Сохраняет вычисленное `normal`, а не подменяет его использованным интерлиньяжем.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    Pixels(f64),
}

/// Отказ раскладки либо неоднозначная привязка документа к состоянию окна.
#[derive(Debug)]
pub enum GeometryError {
    Document(crate::dom::outcome::DocumentFailure),
    /// Несколько экземпляров DOM имеют одинаковые локальные идентификаторы.
    /// Передайте scope конкретного экземпляра в `element_geometry_in_scope`.
    AmbiguousScrollScope,
}

impl From<crate::dom::outcome::DocumentFailure> for GeometryError {
    fn from(error: crate::dom::outcome::DocumentFailure) -> Self {
        Self::Document(error)
    }
}

impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document(error) => error.fmt(f),
            Self::AmbiguousScrollScope => {
                f.write_str("Для запроса прокрутки нужен scope экземпляра документа")
            }
        }
    }
}

impl std::error::Error for GeometryError {}

pub(crate) fn font(
    style: &crate::style::computed::Computed,
    base: f32,
    vcx: crate::values::ValueCx<'_>,
) -> (f64, LineHeight) {
    let size = style.used_font(base, vcx).computed_px;
    let line = if style.inh.font.line_height.is_none() {
        LineHeight::Normal
    } else {
        LineHeight::Pixels(style.numeric_line_px(base).unwrap_or(size).into())
    };
    (size.into(), line)
}

/// Геометрия одного исходного элемента; порядок списков — порядок фрагментов.
#[derive(Clone, Debug)]
pub struct ElementGeometry {
    /// Индексы childNodes от Document; текст и комментарии занимают свои места.
    pub path: Vec<usize>,
    pub tag: String,
    pub id: String,
    pub class: String,
    pub border_box: Rect,
    pub client_rects: Vec<Rect>,
    pub padding_boxes: Vec<Rect>,
    pub content_boxes: Vec<Rect>,
    pub font_size: f64,
    pub line_height: LineHeight,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounding_ignores_degenerate_rectangles_unless_all_are_degenerate() {
        let empty = Rect {
            x: 900.,
            y: 900.,
            width: 0.,
            height: 10.,
        };
        let full = Rect {
            x: -5.,
            y: 3.,
            width: 8.,
            height: 4.,
        };
        assert_eq!(Rect::bounding(&[]), Rect::default());
        assert_eq!(Rect::bounding(&[empty]), empty);
        assert_eq!(Rect::bounding(&[empty, full]), full);
    }
}
