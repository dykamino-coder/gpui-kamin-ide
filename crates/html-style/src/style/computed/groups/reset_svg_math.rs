//! CSS-операнды группы svg_math; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetSvgMath {
    /// Внутренний SVG использует SVG layout, без главной CSS-коробки.
    /// Только внешний svg может быть size query container (css-contain-2 §2).
    pub svg_no_css_box: bool,
    /// `fill` для SVG-фигур (CSS-презентация, SVG 2).
    pub svg_fill: Option<String>,
    /// `stroke` и `stroke-width` фигуры (SVG 2 §presentation attributes):
    /// правила из `<style>` с селекторами до растеризатора иначе не доедут —
    /// он видит только сериализованную разметку.
    pub svg_stroke: Option<String>,
    pub svg_stroke_width: Option<String>,
    /// CSS-геометрия фигуры (SVG 2 §Geometry properties): `x` и `y`.
    /// Ширина и высота уже живут в `width`/`height`.
    pub svg_x: Option<Len>,
    pub svg_y: Option<Len>,
    /// `vector-effect: non-scaling-stroke` (SVG 2 §vector-effect): толщина
    /// обводки задана в точках экрана.
    pub svg_non_scaling: Option<bool>,
    /// Namespace identity and the winning MathML inner display type.
    pub mathml: bool,
    pub math_layout: Option<Display>,
    pub math_depth: Option<super::math_style::Depth>,
    pub math_font_size: bool,
    /// SVG 2 geometry properties `cx`, `cy`, `r`, `rx`, `ry`, `d` from the
    /// cascade, already in attribute syntax (`svg::serialize::geometry`);
    /// not inherited.
    pub svg_geometry: Vec<(&'static str, String)>,
    /// Experimental CSS path-length; Auto represents none. Not inherited.
    pub svg_path_length: Option<Len>,
}
