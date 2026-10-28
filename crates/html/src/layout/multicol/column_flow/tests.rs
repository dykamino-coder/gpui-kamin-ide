//! Текстовый путь колонок берёт только схлопываемый текст с мягкими переносами.

use super::column_flow;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};

fn styled(css: &str, children: Vec<Node>) -> (Element, Computed) {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut style = Computed::default();
    style.apply_decls(&crate::style::css::parse_decls(css), doc.values());
    (
        Element::synthetic("div", style.clone(), Default::default(), children),
        style,
    )
}

/// `line-clamp-fragmentation-001-ref`: a `pre` child of a `height: 4lh`
/// multicol keeps its twelve lines; the text path would merge them into one
/// wrapped paragraph and cut 5/5/2 instead of 4/4/4.
#[test]
fn preserved_line_breaks_nowrap_and_first_letter_leave_the_text_path() {
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let lines = (1..=12).map(|i| format!("Line {i}")).collect::<Vec<_>>();
    for (child, text_path) in [
        ("", true),
        ("::first-letter", false),
        ("white-space:pre", false),
        ("white-space:pre-line", false),
        ("white-space:nowrap", false),
    ] {
        let doc = crate::document::state::DocState::new(Default::default());
        let (mut only, _) = styled(
            child.trim_start_matches(':'),
            vec![Node::Text(lines.join("\n"))],
        );
        // `::first-letter` styles part of the first line: the text path paints raw text.
        if child.starts_with(':') {
            only.first_letter = Some(Computed::default().into());
        }
        let (multicol, style) = styled("columns:3 10em;height:4lh", vec![Node::Element(only)]);
        let merged =
            crate::style::cascade::inherit::inherit(&BoxStyle::default(), &style, doc.values());
        let flow = column_flow(&multicol, &merged, &opts, Some(3), Some(160.), None);
        assert_eq!(flow.is_some(), text_path, "{child}");
    }
}

/// css-multicol-1 §7.1, §8.2: a balanced multicol never makes a column taller
/// than its definite height; lines that do not fit `count` columns go to
/// overflow columns (`discard-multicol-001`: 7 lines, 3 columns, `2lh`).
#[test]
fn balanced_columns_stop_at_the_box_height_and_overflow_inline() {
    use super::measure::column_ends;
    assert_eq!(column_ends(7, 3, 10., None, None, (1, 1)), (vec![3, 6], 3));
    assert_eq!(
        column_ends(7, 3, 10., None, Some(20.), (1, 1)),
        (vec![2, 4, 6], 2)
    );
    // Content that fits keeps its balanced split.
    assert_eq!(
        column_ends(5, 3, 10., None, Some(20.), (1, 1)),
        (vec![2, 4], 2)
    );
}

/// css-overflow-4 §continue: `discard` parses; the other keywords clear it.
#[test]
fn continue_discard_is_its_own_computed_value() {
    for (css, discard) in [
        ("continue:discard", Some(true)),
        ("continue:collapse", Some(false)),
        ("continue:normal", Some(false)),
        ("", None),
    ] {
        let (_, style) = styled(css, vec![]);
        assert_eq!(style.clamp_discard, discard, "{css}");
    }
}
