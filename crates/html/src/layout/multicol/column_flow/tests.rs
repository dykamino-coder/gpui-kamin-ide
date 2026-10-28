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
fn preserved_line_breaks_and_nowrap_leave_the_text_path() {
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
        ("white-space:pre", false),
        ("white-space:pre-line", false),
        ("white-space:nowrap", false),
    ] {
        let doc = crate::document::state::DocState::new(Default::default());
        let (only, _) = styled(child, vec![Node::Text(lines.join("\n"))]);
        let (multicol, style) = styled("columns:3 10em;height:4lh", vec![Node::Element(only)]);
        let merged =
            crate::style::cascade::inherit::inherit(&BoxStyle::default(), &style, doc.values());
        let flow = column_flow(&multicol, &merged, &opts, Some(3), Some(160.), None);
        assert_eq!(flow.is_some(), text_path, "{child}");
    }
}
