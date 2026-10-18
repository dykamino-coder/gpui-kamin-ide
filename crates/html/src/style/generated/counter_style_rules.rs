//! Правила `@counter-style` документа (css-counter-styles-3 §3).
//!
//! Разбор стилевых листов кладёт дескрипторы каждого действительного
//! правила (`register`) в реестр текущего документа (`document::state`):
//! у рамки свой документ и свои правила. Представление значения строится по алгоритму спеки
//! §counter-style-generate: диапазон → резерв, начальное представление по
//! системе, дополнение `pad`, знак `negative`. Предопределённые стили
//! описаны в `counter_style::builtin`; без правил документа путь
//! предопределённых стилей не меняется вовсе.

use crate::style::generated::counter_style::{
    Context, builtin, builtin_initial, builtin_repr_in, normalize_name,
};
use std::collections::HashMap;

mod generate;
mod math;
mod parse;
mod resolve;
mod tokens;
use generate::anonymous;
pub use generate::{custom_marker, custom_repr, valid_symbols_fn};
use math::{Math, is_math_fn};
pub use parse::register;
use resolve::{Algo, in_range, resolve, uses_negative};
use tokens::{Tok, tokenize};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum System {
    Cyclic,
    Numeric,
    Alphabetic,
    Symbolic,
    Additive,
    Fixed(i64),
    Extends(String),
}

/// Дескрипторы одного правила, как они заданы (незаданное — `None`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Raw {
    system: Option<System>,
    symbols: Option<Vec<String>>,
    additive: Option<Vec<(u64, String)>>,
    negative: Option<(String, String)>,
    prefix: Option<String>,
    suffix: Option<String>,
    /// `Some(None)` — `auto`.
    range: Option<Option<Vec<(i64, i64)>>>,
    pad: Option<(usize, String)>,
    fallback: Option<String>,
}

/// Реестр правил документа: имя (с исходным регистром) → дескрипторы.
pub(crate) type CounterStyles = HashMap<String, Raw>;

#[cfg(test)]
mod tests;
