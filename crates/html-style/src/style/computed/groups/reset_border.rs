//! CSS-операнды группы border; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::{Len, StyleColor};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetBorder {
    /// `box-sizing`. По умолчанию в CSS — `content-box`: заданная ширина
    /// НЕ включает отступы и рамку. Движок раскладки под нами всегда считает
    /// по `border-box`, поэтому разницу приходится компенсировать вручную.
    pub border_box: Option<bool>,
    pub border_width: Sides,
    pub radius: Corners,
    /// Прежний признак `border: inherit` целиком: сокращение теперь
    /// раскрывается разбором в длинные свойства, и признак не ставится
    /// (его ещё читает табличный код).
    pub border_inherit: bool,
    /// Есть ли у рамки ВИДИМЫЙ рисунок. Толщина без рисунка не считается:
    /// начальное значение `border-style` — `none`, и по CSS такая рамка
    /// вычисляется в ноль (`descendant-static-position-001`: коробка выходила
    /// шире на заданную, но не нарисованную рамку).
    pub border_visible: [Option<bool>; 4],
    /// Цвета рамки по сторонам. У GPUI цвет рамки один на элемент, поэтому
    /// разные цвета сторон дорисовываются полосами.
    pub border_colors: [StyleColor; 4],
    /// Ранги стилей кромок по сторонам [верх, право, низ, лево] для
    /// разбора конфликтов сросшихся рамок (CSS 2.1 §17.6.2.1):
    /// 0 none, 1 hidden, 3 inset, 4 groove, 5 outset, 6 ridge, 7 dotted,
    /// 8 dashed, 9 solid, 10 double. `None` — стиль не задавался.
    pub border_side_styles: [Option<u8>; 4],
    pub border_dashed: Option<bool>,
    /// `border-style: dotted` — точечный узор.
    pub border_dotted: Option<bool>,
    pub outline: Option<Outline>,
    /// `border-image`: картинка вместо рамки.
    pub border_image: Option<BorderImage>,
    /// Эллиптические радиусы углов (`border-radius: H / V`), tl/tr/br/bl:
    /// растеризатор круглит только окружностью — такой угол уходит
    /// альфа-маской буфера группы (`shape:rrect(...)`).
    pub radius_ell: Option<[Option<(Len, Len)>; 4]>,
    /// Форма углов `corner-shape` (css-borders-4 §corner-shaping): параметр
    /// суперэллипса K по углам tl/tr/br/bl — `round`=1, `squircle`=2,
    /// `square`=+∞, `bevel`=0, `scoop`=−1, `notch`=−∞, `superellipse(K)`.
    /// `None` — все углы круглые (начальное значение). Угол с K≠1 при
    /// ненулевом радиусе рисуется растровой маской (`Computed::corner_shaped`).
    pub corner_shape: Option<[f32; 4]>,
    /// `border-shape` (css-borders-4 §border-shape); `None` — начальное `none`.
    /// Контур режет буфер группы (`render::grouped`), рамку красит слой
    /// (`render::decorations`), `border-radius` при этом игнорируется.
    pub border_shape: Option<BorderShape>,
}
