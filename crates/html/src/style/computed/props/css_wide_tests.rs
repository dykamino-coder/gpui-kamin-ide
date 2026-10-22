//! CSS-wide keywords (css-cascade-5 §7.3) are valid for every property: every
//! mapped property of the coverage table accepts them, and a resetting keyword
//! gives a result that does not depend on a value applied before it.

use crate::coverage::PROPERTIES;
use crate::style::computed::Computed;
use crate::style::css::parse_decls;

const KEYWORDS: [&str; 5] = ["initial", "inherit", "unset", "revert", "revert-layer"];

/// Keywords whose result is a reset. `inherit` (and `unset` of an inherited
/// property) is recorded as an inheritance flag next to the old slot value and
/// resolved against the parent, so its own fields are not comparable here.
const RESETS: [&str; 3] = ["initial", "revert", "revert-layer"];

/// Handlers that validate their grammar without a CSS-wide branch: a resetting
/// keyword still leaves the earlier value. Known gaps; the list may only shrink.
const KNOWN_GAPS: &[&str] = &[
    "align-items",
    "align-self",
    "appearance",
    "background",
    "background-color",
    "background-image",
    "border",
    "border-bottom",
    "border-color",
    "border-left",
    "border-right",
    "border-style",
    "border-top",
    "border-width",
    "box-shadow",
    "clip-path",
    "container-type",
    "content",
    "display",
    "filter",
    "flex-basis",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "font",
    "font-feature-settings",
    "font-stretch",
    "grid-area",
    "list-style",
    "mask",
    "outline",
    "outline-width",
    "place-items",
    "place-self",
    "text-decoration",
    "text-decoration-line",
    "text-wrap",
    "zoom",
];

/// Top-level `Computed` fields as `name: value` text, without declaration
/// bookkeeping (sequence numbers and the capability-query result).
fn fields(style: &Computed) -> Vec<String> {
    let mut style = style.clone();
    style.decl_seq = 0;
    style.side_seq = Default::default();
    style.declaration_parse = Default::default();
    if let Some(logical) = style.logical.as_mut() {
        logical.padding.seq = [0; 4];
        logical.margin.seq = [0; 4];
        logical.inset.seq = [0; 4];
    }
    let text = format!("{style:?}");
    let inner = &text[text.find('{').unwrap_or(0) + 1..text.rfind('}').unwrap_or(text.len())];
    let (mut out, mut depth, mut start, mut quoted, mut escaped) =
        (Vec::new(), 0i32, 0, false, false);
    for (i, ch) in inner.char_indices() {
        if quoted {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(inner[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(inner[start..].trim().to_string());
    out
}

fn cascade(blocks: &[String]) -> Computed {
    let state = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut style = Computed::default();
    for block in blocks {
        style.apply_decls_with_vars(&parse_decls(block), &Default::default(), state.values());
    }
    style
}

/// Fields that differ between `sample; keyword` and `keyword` alone.
fn dependence(name: &str, sample: &str, keyword: &str) -> Vec<String> {
    let reset = format!("{name}: {keyword}");
    let after_value = fields(&cascade(&[format!("{name}: {sample}"), reset.clone()]));
    let alone = fields(&cascade(&[reset]));
    after_value
        .iter()
        .zip(&alone)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("{a} / {b}"))
        .collect()
}

#[test]
fn resetting_css_wide_keywords_do_not_depend_on_the_previous_value() {
    let mut failures = Vec::new();
    for property in PROPERTIES.iter().filter(|p| !p.sample.is_empty()) {
        let gap = KNOWN_GAPS.contains(&property.name);
        let differing: Vec<_> = RESETS
            .iter()
            .map(|keyword| {
                let fields = dependence(property.name, property.sample, keyword);
                (keyword, fields)
            })
            .filter(|(_, fields)| !fields.is_empty())
            .collect();
        if gap && differing.is_empty() {
            failures.push(format!(
                "{}: fixed, remove it from KNOWN_GAPS",
                property.name
            ));
        } else if !gap {
            for (keyword, fields) in differing {
                failures.push(format!("{}: {keyword}: {fields:?}", property.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn css_wide_keywords_are_accepted_by_every_mapped_property() {
    let state = crate::document::state::DocState::new(crate::style::css::Media::default());
    for property in PROPERTIES.iter().filter(|p| !p.sample.is_empty()) {
        for keyword in KEYWORDS {
            assert_eq!(
                Computed::declaration_result(property.name, keyword, state.values()),
                crate::style::computed::ParseResult::Accepted,
                "{}: {keyword}",
                property.name
            );
        }
    }
}
