//! Переходный реэкспорт: разбор `position-area`, `position-try-fallbacks`/`-order` и
//! `position-visibility` живёт в `style::values::position_area`.

pub(crate) use crate::style::values::position_area::{
    AreaAxis, AreaKw, AreaSide, FLIP_BLOCK, FLIP_INLINE, FLIP_START, PositionArea, VIS_NO_OVERFLOW,
    VIS_VALID, VIS_VISIBLE,
};
