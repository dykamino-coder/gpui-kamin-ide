//! Победитель длинного свойства реестра (`style::css::property`): значение
//! проверено разбором, общее слово решает реестр, значение с подстановкой
//! разбирается после неё.

use super::super::map::Candidate;
use super::*;

impl Computed {
    /// Победитель длинного свойства реестра: значение `all`, общее слово,
    /// проверенное значение или значение с подстановкой. Негодное после
    /// подстановки — `unset` (css-variables-1 §3, «invalid at computed-value
    /// time»); у длинного свойства сокращения разбирается сокращение целиком
    /// и берётся его часть (Blink `ResolvePendingSubstitution`).
    pub(super) fn apply_longhand(
        &mut self,
        id: LonghandId,
        cand: Candidate<'_>,
        value: &str,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        let shorthand = match cand.decl.specified {
            // Значение `all` (у другого имени) — общее слово.
            _ if cand.decl.id != Some(id) => return self.apply_one(id.name(), value, vcx),
            Specified::Pending(shorthand) => shorthand,
            Specified::Value | Specified::Wide(_) => return self.apply_one(id.name(), value, vcx),
        };
        let name = shorthand.map_or(id.name(), |s| s.name());
        let resolved =
            resolve_sibling(resolve_attrs(name, &resolve_vars(value, vars, vcx), cx, vcx), cx);
        let text = match CssWide::parse(&resolved) {
            Some(_) => Some(resolved),
            None => match shorthand {
                Some(s) => s.component_of(id, &resolved, vcx),
                None => id.accepts(&resolved, vcx).then_some(resolved),
            },
        };
        self.apply_one(id.name(), text.as_deref().unwrap_or("unset"), vcx);
    }
}
