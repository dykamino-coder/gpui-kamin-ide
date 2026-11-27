//! CSS Text normalization before shaping, shared by retained IFC and the adapter.

use super::{bidi_format, collect::text_nodes::normalize_text, text_case};
use crate::style::computed::{Computed, TextTransform};
use std::string::String;

/// Inputs read by node-local normalization; paint, fonts and available size are absent.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Inputs {
    preserve_newlines: Option<bool>,
    keep_spaces: Option<bool>,
    text_transform: Option<TextTransform>,
    text_transform_flags: u8,
    lang: Option<String>,
    hyphenate: Option<bool>,
    break_after_spaces: Option<bool>,
    keep_all: Option<bool>,
    break_anywhere: Option<bool>,
    break_anywhere_strict: Option<bool>,
    ruby_suppress_breaks: bool,
    owned_breaks: bool,
}

impl Inputs {
    #[cfg(feature = "heap-report")]
    pub(crate) fn flow_heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        kamin_heap::HeapSize::heap_size(&self.lang, ops)
    }

    /// `suppress_breaks`: forced breaks become spaces (`TreeFacts::ruby_suppress_breaks`).
    pub(crate) fn matches(&self, style: &Computed, suppress_breaks: bool) -> bool {
        let flow = &style.inh.text_flow;
        self.preserve_newlines == flow.preserve_newlines
            && self.keep_spaces == flow.keep_spaces
            && self.text_transform == flow.text_transform
            && self.text_transform_flags == flow.text_transform_flags
            && self.lang == flow.lang
            && self.hyphenate == flow.hyphenate
            && self.break_after_spaces == flow.break_after_spaces
            && self.keep_all == flow.keep_all
            && self.break_anywhere == flow.break_anywhere
            && self.break_anywhere_strict == flow.break_anywhere_strict
            && self.ruby_suppress_breaks == suppress_breaks
            && self.owned_breaks == crate::text::paragraph::rules(style).is_some()
    }

    pub(crate) fn of(style: &Computed, suppress_breaks: bool) -> Self {
        let flow = &style.inh.text_flow;
        Self {
            preserve_newlines: flow.preserve_newlines,
            keep_spaces: flow.keep_spaces,
            text_transform: flow.text_transform,
            text_transform_flags: flow.text_transform_flags,
            lang: flow.lang.clone(),
            hyphenate: flow.hyphenate,
            break_after_spaces: flow.break_after_spaces,
            keep_all: flow.keep_all,
            break_anywhere: flow.break_anywhere,
            break_anywhere_strict: flow.break_anywhere_strict,
            ruby_suppress_breaks: suppress_breaks,
            owned_breaks: crate::text::paragraph::rules(style).is_some(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Normalizer {
    case: text_case::Context,
    previous_zwsp: bool,
}

impl Normalizer {
    pub(crate) fn text(&mut self, raw: &str, style: &Computed, suppress_breaks: bool) -> String {
        let text = normalize_text(
            raw,
            style,
            suppress_breaks,
            &mut self.case,
            self.previous_zwsp,
        );
        if !text.is_empty() && text != super::SPACER && !text.chars().all(bidi_format) {
            self.previous_zwsp = text.ends_with('\u{200b}');
        }
        text
    }
}
