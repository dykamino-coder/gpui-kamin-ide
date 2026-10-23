//! Грамматика подсетки css-grid-2 §subgrid-listing: только имена линий,
//! без размеров дорожек; всё объявление проверяется до записи полей стиля.

use crate::style::css::{block_end, selector_tokens};

pub(super) fn valid(v: &str) -> bool {
    let v = v.trim();
    let Some(keyword) = v.get(..7) else {
        return false;
    };
    if !keyword.eq_ignore_ascii_case("subgrid") {
        return false;
    }
    let mut rest = &v[7..];
    if !rest.is_empty() && !rest.starts_with(['[', ' ', '\t', '\n', '\r', '\u{c}']) {
        return false;
    }
    let mut auto_fill = false;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return true;
        }
        if rest.starts_with('[') {
            let Some(tail) = names(rest) else { return false };
            rest = tail;
            continue;
        }
        let Some(open) = rest.find('(') else { return false };
        if !rest[..open].eq_ignore_ascii_case("repeat") {
            return false;
        }
        let Some(end) = block_end(rest, open, '(') else { return false };
        let Some(body) = rest.get(open + 1..end) else { return false };
        let Some((count, mut body)) = body.split_once(',') else { return false };
        let count = count.trim();
        if count.eq_ignore_ascii_case("auto-fill") {
            if auto_fill { return false; }
            auto_fill = true;
        } else {
            let digits = count.strip_prefix('+').unwrap_or(count);
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit())
                || digits.bytes().all(|c| c == b'0')
            {
                return false;
            }
        }
        body = body.trim_start();
        if body.is_empty() { return false; }
        while !body.is_empty() {
            let Some(tail) = names(body) else { return false };
            body = tail.trim_start();
        }
        let Some(tail) = rest.get(end + 1..) else { return false };
        rest = tail;
    }
}

fn names(v: &str) -> Option<&str> {
    if !v.starts_with('[') { return None; }
    let end = block_end(v, 0, '[')?;
    let names = selector_tokens::ident_list(v.get(1..end)?)?;
    if names.iter().any(|name| matches!(name.to_ascii_lowercase().as_str(),
        "span" | "auto" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default")) {
        return None;
    }
    v.get(end + 1..)
}

#[cfg(test)]
mod tests {
    use super::valid;
    use crate::document::state::DocState;
    use crate::style::computed::Computed;
    use crate::style::css::{Media, parse_decls};

    #[test]
    fn subgrid_line_name_grammar() {
        for value in ["subgrid", "subgrid [] [a b]", "subgrid[a]",
            "subgrid repeat(auto-fill, [])", "subgrid [a] repeat(+2, [b] [c]) [d]",
            "subgrid repeat(2, [a]) repeat(auto-fill, [b])"] {
            assert!(valid(value), "{value}");
        }
        for value in ["subgridx", "subgrid subgrid", "subgrid 1px", "subgrid [a] 1px",
            "subgrid repeat(auto-fill, 1px)", "subgrid repeat(auto-fill, line)",
            "subgrid repeat(auto-fit, [a])", "subgrid repeat(0, [a])",
            "subgrid repeat(2.0, [a])", "subgrid repeat(2,)",
            "subgrid repeat(2, repeat(2, [a]))", "subgrid [1a]", "subgrid [auto]",
            "subgrid [span]", "subgrid [inherit]", "subgrid [[a]]", "subgrid [a",
            "subgrid repeat(auto-fill, [a]) repeat(auto-fill, [b])"] {
            assert!(!valid(value), "{value}");
        }
    }

    #[test]
    fn invalid_subgrid_keeps_both_axes_and_names() {
        let state = DocState::new(Media::default());
        let mut style = Computed::default();
        let vcx = state.values();
        style.apply_decls_with_vars(&parse_decls(
            "grid-template: subgrid [row] / subgrid [col]"), &Default::default(), vcx);
        for key in ["grid-template-columns", "grid-template-rows", "grid-template", "grid"] {
            for value in ["subgrid 1px", "subgrid repeat(auto-fill, [bad] 10px)",
                "subgrid repeat(auto-fit, [bad])", "subgrid repeat(2, [bad] 10px)"] {
                let value = if matches!(key, "grid" | "grid-template") {
                    format!("20px / {value}")
                } else { value.to_string() };
                style.apply_decls_with_vars(&parse_decls(&format!("{key}: {value}")),
                    &Default::default(), vcx);
                assert!(style.subgrid_cols && style.subgrid_rows, "{key}: {value}");
                assert_eq!(style.grid_col_line_names.as_ref().unwrap().before,
                    vec![vec!["col".to_string()]]);
                assert_eq!(style.grid_row_line_names.as_ref().unwrap().before,
                    vec![vec!["row".to_string()]]);
            }
        }
    }
}
