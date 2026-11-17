//! Stable facts of a normalized subtree, owned by the copy-on-write children snapshot.

/// Text of the list: direct children apart from the text of in-flow descendants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoxTextFacts {
    pub elements: bool,
    pub nonempty: bool,
    pub significant: bool,
    pub in_flow: bool,
}

impl BoxTextFacts {
    pub fn text(text: &str) -> Self {
        let significant = text
            .bytes()
            .any(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r'));
        Self {
            elements: false,
            nonempty: !text.is_empty(),
            significant,
            in_flow: significant,
        }
    }
}

impl std::ops::BitOr for BoxTextFacts {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            elements: self.elements || rhs.elements,
            nonempty: self.nonempty || rhs.nonempty,
            significant: self.significant || rhs.significant,
            in_flow: self.in_flow || rhs.in_flow,
        }
    }
}

/// Evidence for flow preparation. `block` is seen through inline/contents,
/// the other flags cover the whole subtree, independent blocks included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoxTreeSummary {
    pub breaks: bool,
    pub floats: bool,
    pub margins: bool,
    pub block: bool,
    pub clearable: bool,
    pub split_inline: bool,
    pub text: BoxTextFacts,
}

impl std::ops::BitOr for BoxTreeSummary {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            breaks: self.breaks || rhs.breaks,
            floats: self.floats || rhs.floats,
            margins: self.margins || rhs.margins,
            block: self.block || rhs.block,
            clearable: self.clearable || rhs.clearable,
            split_inline: self.split_inline || rhs.split_inline,
            text: self.text | rhs.text,
        }
    }
}
