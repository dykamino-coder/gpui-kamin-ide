//! `<gap-rule-list>` по спискам длинных свойств (css-gaps-1 §rule-shorthands).

use crate::style::computed::split_outside_parens;

fn is_style(w: &str) -> bool {
    matches!(
        w,
        "none"
            | "hidden"
            | "solid"
            | "dashed"
            | "dotted"
            | "double"
            | "groove"
            | "ridge"
            | "inset"
            | "outset"
    )
}

fn is_width(w: &str) -> bool {
    matches!(w, "thin" | "medium" | "thick")
        || w.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
        || ["calc(", "min(", "max(", "clamp("]
            .iter()
            .any(|f| w.starts_with(f))
}

/// `<gap-rule> = <line-width> || <line-style> || <color>`: части по местам
/// (ширина, стиль, цвет), опущенная — начальное значение.
fn gap_rule(entry: &str) -> Option<[String; 3]> {
    let mut parts: [Option<String>; 3] = [None, None, None];
    let toks = split_outside_parens(entry.trim());
    if toks.is_empty() {
        return None;
    }
    for t in toks {
        let lower = t.to_ascii_lowercase();
        let i = if is_style(&lower) {
            1
        } else if is_width(&lower) {
            0
        } else {
            2
        };
        if parts[i].replace(t).is_some() {
            return None;
        }
    }
    let [w, s, c] = parts;
    Some([
        w.unwrap_or_else(|| "medium".into()),
        s.unwrap_or_else(|| "none".into()),
        c.unwrap_or_else(|| "currentcolor".into()),
    ])
}

/// `<gap-rule-list> | <gap-auto-rule-list>` по спискам ширин, стилей и
/// цветов: каждая запись (и тело `repeat()`) раскладывается на части.
pub(super) fn gap_rules(v: &str) -> Option<Vec<String>> {
    let mut lists: [Vec<String>; 3] = Default::default();
    for entry in crate::style::css::split_args(v) {
        let entry = entry.trim();
        let lower = entry.to_ascii_lowercase();
        if let Some(inner) = lower
            .starts_with("repeat(")
            .then(|| entry["repeat(".len()..].strip_suffix(')'))
            .flatten()
        {
            let args = crate::style::css::split_args(inner);
            let (count, body) = args.split_first()?;
            if body.is_empty() {
                return None;
            }
            let rules: Vec<[String; 3]> =
                body.iter().map(|b| gap_rule(b)).collect::<Option<_>>()?;
            for (k, list) in lists.iter_mut().enumerate() {
                let items: Vec<&str> = rules.iter().map(|r| r[k].as_str()).collect();
                list.push(format!("repeat({}, {})", count.trim(), items.join(", ")));
            }
        } else {
            let rule = gap_rule(entry)?;
            for (list, part) in lists.iter_mut().zip(rule) {
                list.push(part);
            }
        }
    }
    if lists[0].is_empty() {
        return None;
    }
    Some(lists.iter().map(|l| l.join(", ")).collect())
}
