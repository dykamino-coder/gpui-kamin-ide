//! CSS Nesting 1: вложенные правила стиля и условные at-правила внутри правила раскрываются в плоские правила с селектором относительно родителя.
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

/// Часть тела правила: серия объявлений или вложенный блок `(прелюдия, тело)`.
enum Item<'a> {
    Decls(&'a str),
    Block(&'a str, &'a str),
}

/// Индекс закрывающей `}` для `{` на позиции `open` (или длина при обрыве).
fn matching_brace(b: &[u8], open: usize) -> usize {
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1 + usize::from(b[i] == b'\\');
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Разрезать тело правила на серии объявлений и вложенные блоки.
/// Блок в значении пользовательского свойства (`--x: {a}`) — часть объявления.
fn items(body: &str) -> Vec<Item<'_>> {
    let b = body.as_bytes();
    let mut out = vec![];
    let (mut run, mut stmt, mut depth, mut i) = (0usize, 0usize, 0i32, 0usize);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1 + usize::from(b[i] == b'\\');
                }
            }
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b';' if depth <= 0 => stmt = i + 1,
            b'{' if depth <= 0 => {
                let close = matching_brace(b, i);
                let prelude = &body[stmt..i];
                if !(prelude.trim_start().starts_with("--") && prelude.contains(':')) {
                    if stmt > run {
                        out.push(Item::Decls(&body[run..stmt]));
                    }
                    out.push(Item::Block(prelude.trim(), &body[i + 1..close]));
                    run = (close + 1).min(b.len());
                    stmt = run;
                }
                i = close + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if run < b.len() {
        out.push(Item::Decls(&body[run..]));
    }
    out
}

/// Правило верхнего уровня с вложенными блоками раскрыть в `out` (`true`);
/// тело без них (`false`) разбирается прежним путём.
pub(super) fn rule(
    parts: &[String],
    body: &str,
    media: &Media,
    cx: &SheetCx,
    out: &mut Vec<Rule>,
    order: &mut usize,
) -> bool {
    // Nested @supports keeps the parent selector even without nested style rules
    // (CSS Nesting 1 §2.3). Preserve declaration recovery for other at-rules,
    // including malformed @media in CSS2 declarations-009.
    let nested = items(body)
        .iter()
        .any(|i| matches!(i, Item::Block(p, _) if !p.starts_with('@') || supports_group(p)));
    if nested {
        expand(parts, body, media, cx, out, order);
    }
    nested
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
    cx: &SheetCx,
    out: &mut Vec<Rule>,
    order: &mut usize,
) {
    for item in items(body) {
        match item {
            Item::Decls(text) => {
                let decls = parse_decls(text);
                if decls.is_empty() {
                    continue;
                }
                for p in parents {
                    let Some(sel) = Selector::parse_in(p, &cx.selectors) else {
                        continue;
                    };
                    out.push(Rule {
                        sel,
                        decls: decls.clone(),
                        order: *order,
                        origin: 0,
                        layer: cx.layer_of_rules(),
                    });
                    *order += 1;
                }
            }
            Item::Block(prelude, inner) if prelude.starts_with('@') => {
                // Условные группы внутри правила (§2.3): их тело — то же
                // тело правила стиля с тем же родителем.
                let name = prelude.to_ascii_lowercase();
                let holds = if name.starts_with("@media") {
                    media.matches_in(&name, cx.custom())
                } else if name.starts_with("@supports") {
                    supports_in(&prelude[9..], &cx.selectors)
                } else {
                    name.starts_with("@layer")
                };
                if holds {
                    expand(parents, inner, media, cx, out, order);
                }
            }
            Item::Block(prelude, inner) => {
                if let Some(sels) = resolve(parents, prelude, &cx.selectors) {
                    expand(&sels, inner, media, cx, out, order);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "nesting_tests.rs"]
mod tests;
