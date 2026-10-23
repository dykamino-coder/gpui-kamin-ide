//! Кандидат fallback origin: compute и zoom отдельно от состояния кадра.

use super::{apply_style, tactics};
use crate::document::state::ValueCx;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

pub(super) fn build(
    base: &BoxStyle,
    inherited: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Vec<(usize, BoxStyle)> {
    let mut styles = vec![(0, base.clone())];
    for (index, fb) in base.position_try_fallbacks.iter().enumerate() {
        let mut resolved = base.clone();
        if let Some(name) = &fb.name {
            let Some(rule) = vcx.doc.try_rule(name) else {
                continue;
            };
            super::super::resolve::try_rule_onto(&mut resolved, inherited, &rule, vcx);
        }
        let mut c = base.clone();
        apply_style(&mut c, &resolved);
        if let Some(area) = fb.area {
            c.position_area = Some(area);
        }
        tactics::apply(&mut c, inherited, fb.tactics, vcx);
        // Потомки наследуют computed-вариант элемента, не used-геометрию пробы.
        // Между кадрами сохраняется только Owned из принятых свойств.
        if let Some(element) = &mut c.element_style {
            let mut raw = (*element.computed).clone();
            let source = resolved
                .element_style
                .as_ref()
                .map(|e| &*e.computed)
                .unwrap_or(&resolved);
            apply_style(&mut raw, source);
            if let Some(area) = fb.area {
                raw.position_area = Some(area);
            }
            tactics::apply(&mut raw, inherited, fb.tactics, vcx);
            element.computed = Rc::new(raw);
        }
        styles.push((index + 1, c));
    }
    styles
}

#[cfg(test)]
#[path = "candidate_tests.rs"]
mod tests;
