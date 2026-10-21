//! Наложение объявлений правила `@position-try` на стиль кандидата.

use crate::document::state::ValueCx;
use crate::style::cascade::vars::CascadeCx;
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, VarMap};

/// «Accepted @position-try properties» (css-anchor-position-1
/// §fallback-rule): вставки, поля, размеры, самовыравнивание,
/// `position-anchor`, `position-area`. Тот же набор — Blink
/// `valid_for_position_try` в `css_properties.json5`.
const ACCEPTED: &[&str] = &[
    "top",
    "right",
    "bottom",
    "left",
    "inset",
    "inset-block",
    "inset-inline",
    "inset-block-start",
    "inset-block-end",
    "inset-inline-start",
    "inset-inline-end",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "margin-block",
    "margin-inline",
    "margin-block-start",
    "margin-block-end",
    "margin-inline-start",
    "margin-inline-end",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "inline-size",
    "block-size",
    "min-inline-size",
    "min-block-size",
    "max-inline-size",
    "max-block-size",
    "align-self",
    "justify-self",
    "place-self",
    "position-anchor",
    "position-area",
];

/// Объявления правила накладываются тем же проходом каскада, что и обычные
/// правила (`apply_pass`: порядок «сокращение, затем длинные свойства»,
/// детерминированный). Неподходящие свойства отбрасываются; `!important`
/// внутри правила делает объявление недействительным (§fallback-rule), и
/// проход обычных объявлений его не берёт.
pub(crate) fn apply_try_rule(c: &mut Computed, decls: &DeclBlock, vcx: ValueCx<'_>) {
    let accepted: DeclBlock = decls
        .iter()
        .filter(|d| ACCEPTED.contains(&d.name.as_str()))
        .cloned()
        .collect();
    c.apply_pass(&accepted, &VarMap::default(), false, CascadeCx::NONE, vcx);
}

#[cfg(test)]
mod tests {
    use super::apply_try_rule;
    use crate::style::computed::Computed;
    use crate::style::css::parse_decls;
    use crate::style::values::value::Len;

    fn applied(css: &str) -> Computed {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let mut c = Computed::default();
        apply_try_rule(&mut c, &parse_decls(css), doc.values());
        c
    }

    #[test]
    fn try_rule_is_deterministic() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..64 {
            seen.insert(format!("{:?}", applied("inset: 1px; top: 5px").inset.top));
        }
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(
            seen.contains(&format!("{:?}", Some(Len::Px(5.0)))),
            "{seen:?}"
        );
    }

    #[test]
    fn try_rule_drops_unaccepted_and_important() {
        let c = applied("inset: 1px; left: 9px !important; color: red; width: 7px");
        assert_eq!(c.inset.left, Some(Len::Px(1.0)));
        assert_eq!(c.color, Computed::default().color);
        assert_eq!(c.width, Some(Len::Px(7.0)));
    }
}
