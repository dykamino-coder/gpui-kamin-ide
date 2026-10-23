//! Грамматика подсетки css-grid-2 §subgrid-listing: только имена линий,
//! без размеров дорожек; всё объявление проверяется до записи полей стиля.

use crate::document::state::ValueCx;
use crate::style::css::{block_end, selector_tokens, split_top_level};
use crate::style::values::value::number;

pub(super) fn valid(v: &str, vcx: ValueCx<'_>) -> bool {
    parse(v, vcx).is_some()
}

pub(super) fn parse(v: &str, vcx: ValueCx<'_>) -> Option<gpui::GridAxisLineNames> {
    let v = v.trim_matches(space);
    if !v.get(..7)?.eq_ignore_ascii_case("subgrid") {
        return None;
    }
    let mut rest = &v[7..];
    if !rest.is_empty() && !rest.starts_with(['[', ' ', '\t', '\n', '\r', '\u{c}']) {
        return None;
    }
    let mut out = gpui::GridAxisLineNames::default();
    let mut lines = Vec::new();
    loop {
        rest = rest.trim_start_matches(space);
        if rest.is_empty() {
            break;
        }
        if rest.starts_with('[') {
            let (names, tail) = names(rest)?;
            lines.push(names);
            rest = tail;
            continue;
        }
        let open = rest.find('(')?;
        if !rest[..open].eq_ignore_ascii_case("repeat") {
            return None;
        }
        let end = block_end(rest, open, '(')?;
        let args = split_top_level(rest.get(open + 1..end)?, ',');
        let [count, body] = args.as_slice() else {
            return None;
        };
        let mut body = body.trim_matches(space);
        let mut repeated = Vec::new();
        while !body.is_empty() {
            let (names, tail) = names(body)?;
            repeated.push(names);
            body = tail.trim_start_matches(space);
        }
        if repeated.is_empty() {
            return None;
        }
        let count = count.trim_matches(space);
        if count.eq_ignore_ascii_case("auto-fill") {
            if out.repeat.is_some() {
                return None;
            }
            out.before = std::mem::take(&mut lines);
            out.repeat = Some(repeated);
        } else {
            let n = repeat_count(count, vcx)?;
            // Тот же предел раскрытия счётного repeat, что у списка дорожек.
            for _ in 0..n.min(64) {
                lines.extend(repeated.iter().cloned());
            }
        }
        rest = rest.get(end + 1..)?;
    }
    if out.repeat.is_some() {
        out.after = lines;
    } else {
        out.before = lines;
    }
    Some(out)
}

fn repeat_count(v: &str, vcx: ValueCx<'_>) -> Option<usize> {
    let digits = v.strip_prefix('+').unwrap_or(v);
    if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
        return digits.bytes().any(|c| c != b'0').then(|| {
            digits.bytes().fold(0usize, |n, c| {
                n.saturating_mul(10).saturating_add((c - b'0') as usize)
            })
        });
    }
    // CSS Values 4 §10: числовая математика заменяет integer, округляется
    // к ближайшему целому (половина вверх) и ограничивается диапазоном типа.
    if !v.contains('(') || v.chars().any(|c| c.is_whitespace() && !space(c)) {
        return None;
    }
    let n = number(&v.to_ascii_lowercase(), vcx)?;
    Some((n + 0.5).floor().max(1.0) as usize)
}

fn space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

fn names(v: &str) -> Option<(Vec<String>, &str)> {
    if !v.starts_with('[') {
        return None;
    }
    let end = block_end(v, 0, '[')?;
    let names = selector_tokens::ident_list(v.get(1..end)?)?;
    if names.iter().any(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "span"
                | "auto"
                | "initial"
                | "inherit"
                | "unset"
                | "revert"
                | "revert-layer"
                | "default"
        )
    }) {
        return None;
    }
    Some((names, v.get(end + 1..)?))
}

#[cfg(test)]
mod tests {
    use super::{parse, valid};
    use crate::document::state::DocState;
    use crate::style::computed::Computed;
    use crate::style::css::{Media, parse_decls};

    #[test]
    fn subgrid_line_name_grammar() {
        let state = DocState::new(Media::default());
        let vcx = state.values();
        for value in [
            "subgrid",
            "subgrid [] [a b]",
            "subgrid[a]",
            "subgrid repeat(auto-fill, [])",
            "subgrid [a] repeat(+2, [b] [c]) [d]",
            "subgrid repeat(2, [a]) repeat(auto-fill, [b])",
            "SUBGRID REPEAT(AUTO-FILL, [])",
            "subgrid repeat(calc(1 + 1), [a])",
            "subgrid repeat(min(3, 2), [a])",
            "subgrid repeat(calc(-1), [a])",
        ] {
            assert!(valid(value, vcx), "{value}");
        }
        for value in [
            "subgridx",
            "subgrid subgrid",
            "subgrid 1px",
            "subgrid [a] 1px",
            "subgrid repeat(auto-fill, 1px)",
            "subgrid repeat(auto-fill, line)",
            "subgrid repeat(auto-fit, [a])",
            "subgrid repeat(0, [a])",
            "subgrid repeat(2.0, [a])",
            "subgrid repeat(calc(2px), [a])",
            "subgrid repeat(2,)",
            "subgrid repeat(2, repeat(2, [a]))",
            "subgrid [1a]",
            "subgrid [auto]",
            "subgrid [a]\u{a0}[b]",
            "subgrid repeat(2,\u{a0}[b])",
            "subgrid [span]",
            "subgrid [inherit]",
            "subgrid [[a]]",
            "subgrid [a",
            "subgrid repeat(auto-fill, [a]) repeat(auto-fill, [b])",
        ] {
            assert!(!valid(value, vcx), "{value}");
        }
    }

    #[test]
    fn subgrid_names_use_the_validated_parse() {
        let state = DocState::new(Media::default());
        let names = parse(
            "SUBGRID [a] REPEAT(calc(1.5), [b]) REPEAT(AUTO-FILL, []) [c]",
            state.values(),
        )
        .unwrap();
        assert_eq!(names.before, vec![vec!["a"], vec!["b"], vec!["b"]]);
        assert_eq!(names.repeat, Some(vec![Vec::<String>::new()]));
        assert_eq!(names.after, vec![vec!["c"]]);
    }

    #[test]
    fn invalid_subgrid_keeps_both_axes_and_names() {
        let state = DocState::new(Media::default());
        let mut style = Computed::default();
        let vcx = state.values();
        style.apply_decls_with_vars(
            &parse_decls("grid-template: subgrid [row] / subgrid [col]"),
            &Default::default(),
            vcx,
        );
        for key in [
            "grid-template-columns",
            "grid-template-rows",
            "grid-template",
            "grid",
        ] {
            for value in [
                "subgrid 1px",
                "subgrid repeat(auto-fill, [bad] 10px)",
                "subgrid repeat(auto-fit, [bad])",
                "subgrid repeat(2, [bad] 10px)",
            ] {
                let value = if matches!(key, "grid" | "grid-template") {
                    format!("20px / {value}")
                } else {
                    value.to_string()
                };
                style.apply_decls_with_vars(
                    &parse_decls(&format!("{key}: {value}")),
                    &Default::default(),
                    vcx,
                );
                assert!(style.subgrid_cols && style.subgrid_rows, "{key}: {value}");
                assert_eq!(
                    style.grid_col_line_names.as_ref().unwrap().before,
                    vec![vec!["col".to_string()]]
                );
                assert_eq!(
                    style.grid_row_line_names.as_ref().unwrap().before,
                    vec![vec!["row".to_string()]]
                );
            }
        }
    }
}
