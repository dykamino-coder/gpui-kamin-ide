//! Каскад: подстановки в значении до разбора — `var()` (с запасным значением и
//! раскрытием цепочек переменных), типизированный `attr()` (css-values-5 §7.7)
//! и `sibling-index()`/`sibling-count()` (css-values-5 §tree-counting).
//! Атрибуты и место среди братьев хозяина ставит `dom::walk` на время каскада
//! элемента; негодная подстановка делает объявление `unset`.

use crate::values::ValueCx;

use crate::style::css::VarMap;
use crate::style::values::value::{Color, Len};

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
mod attr_expansion;
mod substitute;
pub(super) use substitute::{resolve_attrs, resolve_sibling};

/// Значение без `var()` возвращается как есть; подстановка одного значения делит один
/// бюджет работы на все проходы и списывает его с ревизии документа. Предел делает
/// объявление `unset` (invalid at computed-value time) и попадает в журнал документа.
pub(super) fn resolve_vars(value: &str, vars: &VarMap, vcx: ValueCx<'_>) -> String {
    use crate::style::css::variable_values::{Work, has_var};
    if !has_var(value) {
        return value.to_string();
    }
    let revision = vcx.doc.revision_budget();
    let work = Work::new(&revision);
    let resolved = substitute_passes(value, vars, &work);
    work.finish(vcx.doc);
    resolved
}

fn substitute_passes(
    value: &str,
    vars: &VarMap,
    work: &crate::style::css::variable_values::Work<'_>,
) -> String {
    let Some(out) = crate::style::css::variable_values::resolve(value, vars, work) else {
        return "unset".into();
    };
    if out.trim().is_empty() {
        "unset".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod attr_graph_tests;
