//! CSS-операнды группы shapes; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetShapes {
    pub object_fit: Option<String>,
    /// `aspect-ratio` — отношение ширины к высоте.
    pub aspect_ratio: Option<f32>,
    /// Отношение из записи `auto <ratio>` (css-sizing-4 §5.1): у замещаемого
    /// оно ЗАПАСНОЕ — природное сильнее. Отдельным полем, потому что
    /// НЕзамещаемой коробке отношение из этой записи наша раскладка пока не
    /// выражает (в общем поле ломаются `block-aspect-ratio-002/015/016/018/
    /// 043/047`, `grid-aspect-ratio-005/008`).
    pub aspect_ratio_auto: Option<f32>,
    /// `shape-outside`: сырая запись формы обтекания плавающего блока.
    pub shape_outside: Option<String>,
    /// `shape-margin`: поле вокруг формы обтекания; доля — от ширины
    /// содержащего блока.
    pub shape_margin: Option<Len>,
    /// `shape-image-threshold`: порог альфы для формы из картинки.
    pub shape_threshold: Option<f32>,
    /// Вырезы обтекания для абзацев ПОД этим элементом: формы слева и
    /// справа от верха первого абзаца (заполняет сборка shape-flow).
    pub flow_shapes: Option<
        std::sync::Arc<(
            Vec<crate::style::values::float_shape::FloatShape>,
            Vec<crate::style::values::float_shape::FloatShape>,
        )>,
    >,
    /// `object-position` замещаемого содержимого (css-images-3 §5.2).
    pub object_position: Option<BgPos>,
    /// `object-view-box` (css-images-4 §object-view-box): видимая область
    /// природного объекта как вырез `inset(top right bottom left)` в точках
    /// или долях природного размера — `rect()` и `xywh()` сводятся к нему
    /// при отрисовке (`render::view_box_rect`). Флаг — вид записи:
    /// 0 `inset`, 1 `rect`, 2 `xywh`.
    pub object_view_box: Option<(u8, [Len; 4])>,
    /// Computed image alpha source for shape-outside; basic shapes use shape_outside.
    pub shape_image: Option<super::ImageSource>,
}
