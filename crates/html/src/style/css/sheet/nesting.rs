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
    out: &mut Vec<Rule>,
    order: &mut usize,
) -> bool {
    // Раскрытие включает только вложенное ПРАВИЛО СТИЛЯ. Тело, где блоки —
    // одни at-правила, разбирается прежним путём (CSS 2.1 §4.1.8: at-правило
    // в блоке объявлений — негодное объявление): иначе хвостовые объявления
    // `#c { color: green; @media {…} color: red }` (declarations-009).
    let nested = items(body)
        .iter()
        .any(|i| matches!(i, Item::Block(p, _) if !p.starts_with('@')));
    if nested {
        expand(parts, body, media, out, order);
    }
    nested
}

/// Части списка селекторов верхнего уровня: `&` вне правила — `:scope`,
/// то есть корень документа (§3.1, `nesting-basic` test-12).
pub(super) fn top_level_parts(parts: &[&str]) -> Vec<String> {
    parts
        .iter()
        .map(|one| match has_amp(one) {
            true => substitute(one.trim(), &[":root".to_string()]),
            false => one.trim().to_string(),
        })
        .collect()
}

/// Простой составной селектор без псевдоэлемента: его можно подставить
/// на место `&` в начале составной части текстом, не оборачивая в `:is()`.
fn plain_compound(p: &str) -> bool {
    let low = p.to_ascii_lowercase();
    !p.contains(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~' | ','))
        && !low.contains("::")
        && ![":before", ":after", ":first-line", ":first-letter"]
            .iter()
            .any(|e| low.contains(e))
        && Selector::parse(p).is_some()
}

/// Заменить каждое `&` вне строк на родителя (§3.1).
fn substitute(sel: &str, parents: &[String]) -> String {
    let direct = (parents.len() == 1 && plain_compound(&parents[0])).then(|| parents[0].as_str());
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
    sel.contains('&') && substitute(sel, &["\u{1}".to_string()]) != sel
}

/// Разрешить список вложенных селекторов относительно родителей.
/// Негодный список — `None`: правило отбрасывается само, без родителя.
fn resolve(parents: &[String], prelude: &str) -> Option<Vec<String>> {
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
        let full = substitute(&rel, parents);
        Selector::parse(&full)?;
        out.push(full);
    }
    Some(out)
}

/// Раскрыть тело `body` правила с селекторами `parents` в плоские правила.
pub(super) fn expand(
    parents: &[String],
    body: &str,
    media: &Media,
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
                    let Some(sel) = Selector::parse(p) else {
                        continue;
                    };
                    out.push(Rule {
                        sel,
                        decls: decls.clone(),
                        order: *order,
                        origin: 0,
                        layer: layer_of_rules(),
                    });
                    *order += 1;
                }
            }
            Item::Block(prelude, inner) if prelude.starts_with('@') => {
                // Условные группы внутри правила (§2.3): их тело — то же
                // тело правила стиля с тем же родителем.
                let name = prelude.to_ascii_lowercase();
                let holds = if name.starts_with("@media") {
                    media.matches(&name)
                } else if name.starts_with("@supports") {
                    supports(&prelude[9..])
                } else {
                    name.starts_with("@layer")
                };
                if holds {
                    expand(parents, inner, media, out, order);
                }
            }
            Item::Block(prelude, inner) => {
                if let Some(sels) = resolve(parents, prelude) {
                    expand(&sels, inner, media, out, order);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "nesting_tests.rs"]
mod tests;
