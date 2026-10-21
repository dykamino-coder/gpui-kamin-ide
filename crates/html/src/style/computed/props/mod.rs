//! Разбор свойств по группам: типизированное значение применяется только после успеха.

// None отвергает декларацию; допустимый сброс представляется как Some(None).
macro_rules! parsed_value {
    ($parser:expr, $value:expr) => {{
        let Some(parsed) = ($value) else { return };
        $parser.accept_declaration();
        parsed
    }};
}

macro_rules! apply_parsed {
    ($parser:expr, $value:expr) => {{ Some(parsed_value!($parser, $value)) }};
}

mod background;
pub(super) mod border;
mod box_model;
mod calc_size;
mod display_flex;
mod effects;
mod font;
mod grid;
mod mask_clip;
mod multicol;
mod position;
mod text;
mod text_decor;
mod transform;

#[cfg(test)]
mod invalid_tests;
