//! Разбор свойств по группам: типизированное значение применяется только после успеха.

// None отвергает декларацию; допустимый сброс представляется как Some(None).
// A CSS-wide keyword is valid for every property (css-cascade-5 §7.3): when the
// value grammar does not resolve it itself, the slot takes its empty
// representation, which is the property's initial value.
macro_rules! parsed_value {
    ($parser:expr, $value:expr) => {{
        match ($value) {
            Some(parsed) => {
                $parser.accept_declaration();
                parsed
            }
            None if $parser.css_wide_value => {
                $parser.accept_declaration();
                Default::default()
            }
            None => return,
        }
    }};
}

// A handler that checks its whole grammar before writing resets its slots for
// a CSS-wide keyword instead (the multi-slot form of `parsed_value!`).
macro_rules! css_wide_reset {
    ($parser:ident, $($field:ident),+ $(,)?) => {
        if $parser.css_wide_value {
            $parser.accept_declaration();
            $($parser.$field = Default::default();)+
            return;
        }
    };
}

macro_rules! apply_parsed {
    ($parser:expr, $value:expr) => {{
        match ($value) {
            Some(parsed) => {
                $parser.accept_declaration();
                Some(parsed)
            }
            None if $parser.css_wide_value => {
                $parser.accept_declaration();
                None
            }
            None => return,
        }
    }};
}

pub(super) mod background;
pub(super) mod border;
pub(super) mod box_model;
mod calc_size;
mod display_flex;
pub(super) mod effects;
mod font;
mod grid;
pub(super) mod mask_clip;
mod multicol;
mod position;
mod text;
mod text_decor;
mod transform;

#[cfg(test)]
mod css_wide_tests;
#[cfg(test)]
mod invalid_tests;
#[cfg(test)]
mod valid_tests;
