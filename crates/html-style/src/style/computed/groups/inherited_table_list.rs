//! CSS-операнды группы table_list; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedTableList {
    /// `border-collapse: collapse` — зазор между ячейками пропадает.
    pub border_collapse: Option<bool>,
    /// `empty-cells: hide` — у ПУСТОЙ ячейки не рисуются ни фон, ни рамка
    /// (CSS 2.1 §17.6.1.1). Свойство наследуемое.
    pub empty_cells_hide: Option<bool>,
    /// Наследуемый вид маркера целиком: счётчик, строка или `none`.
    pub list_style_type: Option<MarkerType>,
    /// `list-style-position: inside` — маркер идёт первым куском содержимого
    /// пункта, а не отдельной колонкой снаружи (css-lists-3 §4).
    pub list_style_inside: Option<bool>,
    /// `caption-side: bottom` — заголовок таблицы под сеткой.
    pub caption_bottom: Option<bool>,
    /// `border-spacing` таблицы: горизонтальный и вертикальный зазор.
    pub border_spacing: Option<(Option<Len>, Option<Len>)>,
    /// `list-style-image` (css-lists-3 §3.3): `Some(None)` — `none`; `None` —
    /// от родителя. Маркер-картинку раскладка пока не рисует.
    pub list_style_image: Option<Option<ImageSource>>,
}
