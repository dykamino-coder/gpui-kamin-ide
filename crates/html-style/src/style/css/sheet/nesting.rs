//! CSS Nesting 1: вложенные правила стиля и условные at-правила внутри правила раскрываются в
//! плоские правила с селектором относительно родителя.
//!
//! css-nesting-1 §2: вложенный селектор без `&` — относительный (`> div`,
//! `.x` ⇒ `& > div`, `& .x`); `&` означает `:is(<список родителя>)` (§3.1).
//! Объявления после вложенного правила образуют свой «nested declarations
//! rule» на месте записи (§3.2, CSSNestedDeclarations), поэтому каждая
//! серия объявлений выпускается отдельным правилом в порядке появления.
//! Blink: `css_parser_impl.cc` (`ConsumeNestedRule`, `CreateNestedDeclarations`)
//! и `css_selector_parser.cc` (`kNesting` ⇒ `:is()` родителя); Servo:
//! `style/stylesheets/rule_parser.rs` (`NestedRuleParser`).

use super::*;
use crate::budget::admission::{Admission, Buffer, Error};

mod items;
use items::{Item, items};

/// Правило верхнего уровня с вложенными блоками раскрыть в `out` (`true`);
/// тело без них (`false`) разбирается прежним путём.
pub(super) fn rule(
    parts: &[String],
    body: &str,
    media: &Media,
    cx: &mut SheetCx,
    out: &mut Vec<Rule>,
    order: &mut usize,
) -> Result<bool, Error> {
    // CSS Nesting 1 sections 3.3 and 5: group declarations retain the
    // parent selector; unknown blocks are skipped without losing later runs.
    let nested = items(body).iter().any(|i| matches!(i, Item::Block(..)));
    if nested {
        expand(parts, body, media, cx, out, order)?;
    }
    Ok(nested)
}

/// Части списка селекторов верхнего уровня: `&` вне правила — `:scope`,
/// то есть корень документа (§3.1, `nesting-basic` test-12).
pub(super) fn top_level_parts(parts: &[&str], cx: &SelectorCx) -> Vec<String> {
    parts
        .iter()
        .map(|one| match has_amp(one) {
            true => substitute(one.trim(), &[":root".to_string()], cx),
            false => one.trim().to_string(),
        })
        .collect()
}

/// Простой составной селектор без псевдоэлемента: его можно подставить
/// на место `&` в начале составной части текстом, не оборачивая в `:is()`.
fn plain_compound(p: &str, cx: &SelectorCx) -> bool {
    let low = p.to_ascii_lowercase();
    !p.contains(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~' | ','))
        && !low.contains("::")
        && ![":before", ":after", ":first-line", ":first-letter"]
            .iter()
            .any(|e| low.contains(e))
        && Selector::parse_in(p, cx).is_some()
}

/// Заменить каждое `&` вне строк на родителя (§3.1).
fn substitute(sel: &str, parents: &[String], cx: &SelectorCx) -> String {
    let direct =
        (parents.len() == 1 && plain_compound(&parents[0], cx)).then(|| parents[0].as_str());
    let wrapped = format!(":is({})", parents.join(", "));
    let mut out = String::with_capacity(sel.len() + wrapped.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in sel.chars() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch == '&' {
            let at_start = out
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace() || matches!(c, '>' | '+' | '~' | '(' | ','));
            match direct {
                Some(p) if at_start => out.push_str(p),
                _ => out.push_str(&wrapped),
            }
            continue;
        }
        out.push(ch);
    }
    out
}

/// Есть ли `&` вне строк.
fn has_amp(sel: &str) -> bool {
    sel.contains('&') && substitute(sel, &["\u{1}".to_string()], &SelectorCx::default()) != sel
}

/// Разрешить список вложенных селекторов относительно родителей.
/// Негодный список — `None`: правило отбрасывается само, без родителя.
fn resolve(parents: &[String], prelude: &str, cx: &SelectorCx) -> Option<Vec<String>> {
    let mut out = vec![];
    for part in split_top_level(prelude, ',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let rel = if has_amp(part) {
            part.to_string()
        } else {
            format!("& {part}")
        };
        let full = substitute(&rel, parents, cx);
        Selector::parse_in(&full, cx)?;
        out.push(full);
    }
    Some(out)
}

/// Раскрыть тело `body` правила с селекторами `parents` в плоские правила.
pub(super) fn expand(
    parents: &[String],
    body: &str,
    media: &Media,
    cx: &mut SheetCx,
    out: &mut Vec<Rule>,
    order: &mut usize,
) -> Result<(), Error> {
    let vcx = cx.doc.values();
    let admission = Admission::new(cx.doc.revision_budget());
    let mut stack = Buffer::default();
    stack.push(
        (parents.to_vec(), items(body).into_iter(), None),
        &admission,
    )?;
    while let Some((parents, siblings, _)) = stack.last_mut() {
        admission.work(1)?;
        let Some(item) = siblings.next() else {
            if let Some((_, _, Some(saved))) = stack.pop() {
                cx.layer_now = saved;
            }
            continue;
        };
        match item {
            Item::Decls(text) => {
                let decls = parse_decls_in(text, vcx);
                if decls.is_empty() {
                    continue;
                }
                for p in parents.iter() {
                    let Some(sel) = Selector::parse_in(p, &cx.selectors) else {
                        continue;
                    };
                    out.push(Rule {
                        sel,
                        decls: decls.clone(),
                        order: *order,
                        origin: crate::style::css::Origin::UserAgent,
                        layer: cx.layer_of_rules(),
                        link: crate::style::css::LinkMatch::Any,
                    });
                    *order += 1;
                }
            }
            Item::Block(prelude, inner) if prelude.starts_with('@') => {
                // Match a complete at-keyword, preserving layer-name case.
                let end = prelude
                    .find(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '@' | '-' | '_'))
                    .unwrap_or(prelude.len());
                let (name, head) = prelude.split_at(end);
                let name = name.to_ascii_lowercase();
                let holds = match name.as_str() {
                    "@media" => media.matches_in(head, cx.custom(), vcx),
                    "@supports" => supports_in(head, &cx.selectors, vcx),
                    "@layer" => true,
                    _ => false,
                };
                if holds {
                    let parents = parents.clone();
                    let saved = (name == "@layer").then(|| cx.enter_layer(head.trim()));
                    stack.push((parents, items(inner).into_iter(), saved), &admission)?;
                }
            }
            Item::Block(prelude, inner) => {
                if let Some(sels) = resolve(parents, prelude, &cx.selectors) {
                    stack.push((sels, items(inner).into_iter(), None), &admission)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "nesting_tests.rs"]
mod tests;
