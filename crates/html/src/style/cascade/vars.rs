//! Каскад: подстановки в значении до разбора — `var()` (с запасным значением и
//! раскрытием цепочек переменных), типизированный `attr()` (css-values-5 §7.7)
//! и `sibling-index()`/`sibling-count()` (css-values-5 §tree-counting).
//! Атрибуты и место среди братьев хозяина ставит `dom::walk` на время каскада
//! элемента; негодная подстановка делает объявление `unset`.

use crate::document::state::ValueCx;

use crate::style::css::VarMap;
use crate::style::values::value::{Color, Len};

/// Подстановка `var(--x)` и `var(--x, запасное)`.
/// Сколько раз раскрывать переменные внутри переменных.
///
/// Тема обычно ссылается на тему: `--btn: var(--accent)`. Один проход такую
/// цепочку не раскрывал, и объявление уходило в разбор строкой `var(--accent)`.
/// Потолок нужен от кольцевых ссылок.
const VAR_DEPTH: usize = 8;

/// The element whose style is being cascaded, as far as substitution
/// functions need it: its attributes for typed `attr()` (css-values-5 §7.7)
/// and its place among siblings for `sibling-index()`/`sibling-count()`
/// (css-values-5 §tree-counting). The style walk passes it for the element's
/// own cascade; styles without an owner element (hover layers, animation
/// frames, pseudo-elements) use [`CascadeCx::NONE`] and take the fallback.
#[derive(Clone, Copy)]
pub struct CascadeCx<'a> {
    pub html_attrs: bool,
    pub attrs: &'a [(String, String)],
    /// `(index, count)`, index from 1.
    pub sibling: Option<(usize, usize)>,
    /// Корневой элемент документа: его фаза шрифта задаёт базу `rem`/`rlh`
    /// (css-values-4 §6.1.4, `compute_cx`).
    pub root: bool,
}

impl CascadeCx<'_> {
    pub const NONE: CascadeCx<'static> = CascadeCx {
        attrs: &[],
        html_attrs: false,
        sibling: None,
        root: false,
    };
}

mod attr_cast;
mod substitute;
pub(super) use substitute::{resolve_attrs, resolve_sibling};

pub(super) fn resolve_vars(value: &str, vars: &VarMap) -> String {
    let mut out = value.to_string();
    for _ in 0..VAR_DEPTH {
        let Some(next) = crate::style::css::variable_values::substitute(&out, &mut |name| {
            vars.get(name).cloned()
        }) else {
            return "unset".into();
        };
        if next == out {
            break;
        }
        out = next;
        if !crate::style::css::variable_values::has_var(&out) {
            break;
        }
    }
    if out.trim().is_empty() && crate::style::css::variable_values::has_var(value) {
        "unset".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests;
