//! Box trees that paint alike although their `color` differs: no box of them
//! paints with it (Blink paint-invalidates on a colour change only the
//! objects that read `currentcolor`, layout_object.cc:2889-2905; the rest keep
//! their cached display items, paint_controller.cc:228). Text glyphs, list
//! markers, form controls, editable boxes and replaced content paint with the
//! colour of their box whatever its other values; the remaining boxes read it
//! where their style says so (`Computed::paint_reads_color`).

use super::{BoxStyles, Children, Element, Node, SharedStyle, layers, same_shape, walk};

/// Whether the subtree of `b` paints like the recorded one of `same_tree_as`:
/// the same values throughout but for `color`, which no box of either reads
/// (`SharedStyle::same_unread_color`, `inkless`). Attributes and texts
/// compare exactly.
pub fn same_unread_color_tree_as(
    shell: &Element,
    styles: &BoxStyles,
    children: &Children,
    b: &Element,
) -> bool {
    let root = || {
        shell.marks == b.marks
            && inkless(shell)
            && inkless_texts(children)
            && styles.same_unread_color(b)
    };
    // The walks stop at a child list shared with the recorded tree: its
    // boxes are the same, yet they inherit the changed colour, so the whole
    // subtree is asked whether it paints in it.
    walk(same_shape(shell, b), children, b, same_shape)
        && walk(root(), children, b, same_unread_styles)
        && paints_without_color(b)
}

fn same_unread_styles(a: &Element, b: &Element) -> bool {
    a.marks == b.marks
        && inkless(a)
        && inkless_texts(&a.children)
        && BoxStyles::same_unread_color_as(&a.style, layers(a), b)
}

/// Whether no box of the subtree of `e` paints in its colour: a recolouring
/// restyle leaves its paint as it was (`same_unread_color_tree_as`).
pub fn paints_without_color(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        let mut styles = std::iter::once(&e.style).chain(layers(e).into_iter().flatten());
        if !inkless(e) || !inkless_texts(&e.children) || styles.any(SharedStyle::paint_reads_color)
        {
            return false;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            Node::Text(_) => None,
        }));
    }
    true
}

/// Element kinds that paint only through CSS boxes: no replaced content, no
/// native control and no SVG or MathML painting of their own.
const BOX_TAGS: &[&str] = &[
    "a",
    "address",
    "article",
    "aside",
    "b",
    "blockquote",
    "body",
    "br",
    "caption",
    "center",
    "code",
    "col",
    "colgroup",
    "dd",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "html",
    "i",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "search",
    "section",
    "small",
    "span",
    "strong",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
    "wbr",
];

/// Whether the element itself paints nothing in its colour apart from its
/// style: not a list item (its marker), not animated, a box kind
/// (`BOX_TAGS`) and not editable (its caret).
fn inkless(e: &Element) -> bool {
    let Element {
        list_item,
        anim,
        tag,
        attrs,
        node_id: _,
        style: _,
        hover: _,
        first_letter: _,
        first_line: _,
        children: _,
        inline: _,
        marks: _,
    } = e;
    list_item.is_none()
        && anim.is_none()
        && BOX_TAGS.contains(&tag.as_str())
        && !attrs
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("contenteditable"))
}

/// Whether the texts among `children` have no glyphs: white space only.
fn inkless_texts(children: &Children) -> bool {
    children.iter().all(|n| match n {
        Node::Text(t) => t
            .as_str()
            .chars()
            .all(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')),
        Node::Element(_) => true,
    })
}

impl BoxStyles {
    pub(super) fn same_unread_color(&self, b: &Element) -> bool {
        let layers = self
            .layers
            .as_deref()
            .map(|l| l.each_ref().map(Option::as_ref));
        Self::same_unread_color_as(&self.style, layers.unwrap_or_default(), b)
    }

    fn same_unread_color_as(
        style: &SharedStyle,
        own: [Option<&SharedStyle>; 3],
        b: &Element,
    ) -> bool {
        let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => x.same_unread_color(y),
            _ => false,
        };
        style.same_unread_color(&b.style)
            && own.into_iter().zip(layers(b)).all(|(x, y)| layer(x, y))
    }
}
