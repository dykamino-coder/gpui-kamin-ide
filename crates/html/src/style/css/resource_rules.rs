//! At-правила ресурсов используют токены таблицы стилей и те же условные правила, что каскад.

use super::{Media, Piece, next_piece, strip_comments, supports, unescape};
use crate::document::state::ValueCx;

pub(crate) struct ResourceRule {
    pub name: String,
    pub prelude: String,
    pub body: String,
}

/// Resource descriptors share declaration tokenization, but do not accept priority.
pub(crate) fn descriptor(block: &str, name: &str) -> Option<String> {
    let declarations = super::parse_decls(block);
    declarations
        .get(name)?
        .split(super::DECL_SEP)
        .rev()
        .find(|value| super::top_level_bang(value).is_none())
        .map(str::to_string)
}

/// Consume a CSS at-keyword, including a hex escape's optional whitespace.
fn keyword(head: &str) -> Option<(String, &str)> {
    let raw = head.trim().strip_prefix('@')?;
    let mut chars = raw.char_indices().peekable();
    let mut end = 0;
    while let Some((at, ch)) = chars.next() {
        if ch == '\\' {
            let (_, escaped) = chars.next()?;
            if matches!(escaped, '\n' | '\r' | '\u{c}') {
                return None;
            }
            end = at + 1 + escaped.len_utf8();
            if escaped.is_ascii_hexdigit() {
                for _ in 1..6 {
                    if let Some(&(i, digit)) = chars.peek()
                        && digit.is_ascii_hexdigit()
                    {
                        chars.next();
                        end = i + digit.len_utf8();
                    } else {
                        break;
                    }
                }
                if let Some(&(i, space)) = chars.peek()
                    && space.is_ascii_whitespace()
                {
                    chars.next();
                    end = i + space.len_utf8();
                    if space == '\r' && chars.peek().is_some_and(|(_, ch)| *ch == '\n') {
                        chars.next();
                        end += 1;
                    }
                }
            }
        } else if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch >= '\u{80}' {
            end = at + ch.len_utf8();
        } else {
            break;
        }
    }
    (end > 0).then(|| {
        (
            unescape(&raw[..end]).to_ascii_lowercase(),
            raw[end..].trim(),
        )
    })
}

pub(crate) fn resources(css: &str, media: Media, vcx: ValueCx<'_>) -> Vec<ResourceRule> {
    let css = strip_comments(css);
    let mut pending = vec![super::stylesheet_tokens::start(&css)];
    let mut out = Vec::new();
    while let Some(mut remaining) = pending.pop() {
        while let Some((piece, tail)) = next_piece(remaining.trim_start()) {
            remaining = tail;
            let Piece::Block { head, body } = piece else {
                continue;
            };
            let Some((name, prelude)) = keyword(head) else {
                continue;
            };
            match name.as_str() {
                "font-face" if prelude.is_empty() => out.push(ResourceRule {
                    name,
                    prelude: String::new(),
                    body: body.to_string(),
                }),
                "color-profile"
                    if super::selector_tokens::ident(prelude)
                        && unescape(prelude).starts_with("--") =>
                {
                    out.push(ResourceRule {
                        name,
                        prelude: unescape(prelude),
                        body: body.to_string(),
                    })
                }
                "media" if media.matches(&format!("@media {prelude}"), vcx) => {
                    pending.push(remaining);
                    remaining = body;
                }
                "supports" if supports(prelude, vcx) => {
                    pending.push(remaining);
                    remaining = body;
                }
                "layer" | "scope" => {
                    pending.push(remaining);
                    remaining = body;
                }
                _ => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_are_rules_rather_than_substrings() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let css = r#"
            /* @font-face { src: url(comment.ttf) } */
            p { content: "@font-face { src: url(string.ttf) }" }
            @font-face-extra { src: url(prefix.ttf) }
            @unknown { @font-face { src: url(unknown.ttf) } }
            @media not all { @font-face { src: url(media.ttf) } }
            @supports (not-a-property: 1) { @font-face { src: url(supports.ttf) } }
            @layer fonts { @font-face { src: url("quoted}file.ttf") } }
            @\66 ont-face { src: url(escaped.ttf) }
            @color-profile --test { src: url("profile}.icc") }
        "#;
        let rules = resources(css, Media::default(), vcx);
        assert_eq!(rules.len(), 3);
        assert!(rules[0].body.contains("quoted}file.ttf"));
        assert!(rules[1].body.contains("escaped.ttf"));
        assert_eq!(rules[2].prelude, "--test");
    }

    #[test]
    fn descriptors_use_tokens_and_last_non_important_declaration() {
        assert_eq!(
            descriptor(r#"src: url("a;b).ttf"); SRC: url("c}d.ttf")"#, "src").as_deref(),
            Some(r#"url("c}d.ttf")"#)
        );
        assert_eq!(
            descriptor(
                "src: url(first.ttf); src: url(ignored.ttf) !important",
                "src"
            )
            .as_deref(),
            Some("url(first.ttf)")
        );
        assert!(descriptor("src: url(ignored.ttf) !important", "src").is_none());
    }
}
