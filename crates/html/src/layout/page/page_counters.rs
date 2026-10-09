//! Paged counters persist across pages and are obscured by margin-local counters.

use crate::style::computed::Computed;
use crate::style::generated::counters::Counters;
use std::collections::HashMap;

type Values = HashMap<String, i32>;

#[derive(Default)]
pub(crate) struct PageCounters {
    document: Counters,
    values: Values,
    current: Values,
    pages: usize,
}

fn pairs(text: &Option<String>, default: i32) -> Vec<(String, i32)> {
    let Some(text) = text else { return vec![] };
    let mut words = text.split_whitespace().peekable();
    let mut result = Vec::new();
    while let Some(name) = words.next() {
        if name == "none" {
            continue;
        }
        let value = words.peek().and_then(|word| word.parse().ok());
        if value.is_some() {
            words.next();
        }
        result.push((name.to_string(), value.unwrap_or(default)));
    }
    result
}

fn apply(values: &mut Values, style: &Computed, document: &mut Counters) {
    // CSS Paged Media 3 §page-based-counters: pages cannot be manipulated.
    for (name, value) in pairs(&style.counter_reset, 0) {
        if name != "pages" {
            values.insert(name, value);
        }
    }
    for (name, value) in pairs(&style.counter_increment, 1) {
        if name != "pages" {
            let current = values
                .entry(name.clone())
                .or_insert_with(|| document.value_of(&name));
            *current = current.saturating_add(value);
        }
    }
    for (name, value) in pairs(&style.counter_set, 0) {
        if name != "pages" {
            values.insert(name, value);
        }
    }
}

impl PageCounters {
    pub(crate) fn from_document(mut nodes: &[crate::dom::Node]) -> Self {
        let mut state = Self::default();
        loop {
            let mut live = nodes.iter().filter(|node| !crate::render::is_blank(node));
            let Some(crate::dom::Node::Element(element)) = live.next() else {
                break;
            };
            if live.next().is_some() || !matches!(element.tag.as_str(), "html" | "body") {
                break;
            }
            state.document.enter();
            crate::dom::apply_counter_decls(
                &element.style,
                &mut state.document,
                &element.tag,
                &element.attrs,
                &mut false,
                &|_, _| 0,
            );
            nodes = &element.children;
        }
        state
    }

    pub(super) fn begin(&mut self, index: usize, pages: usize, page: &Computed) {
        // PageStack visits all pages in order on each prepaint. Restart at zero
        // so repeated frames cannot increment persistent counters twice.
        if index == 0 {
            self.values.clear();
            self.values
                .insert("page".into(), self.document.value_of("page"));
        }
        self.pages = pages;
        self.current = self.values.clone();
        apply(&mut self.current, page, &mut self.document);
        if !pairs(&page.counter_increment, 1)
            .iter()
            .any(|(name, _)| name == "page")
        {
            let value = self.current.entry("page".into()).or_default();
            *value = value.saturating_add(1);
        }
        // Page resets shadow the incoming counter only on that page. Blink's
        // ObscurePageCounterIfNeeded/UnobscurePageCounterIfNeeded restore the
        // outer value on exit; increments without a reset persist across pages.
        let resets = pairs(&page.counter_reset, 0);
        for (name, value) in &self.current {
            if !resets.iter().any(|(reset, _)| reset == name) {
                self.values.insert(name.clone(), *value);
            }
        }
    }

    pub(super) fn for_margin(&self, style: &Computed) -> Counters {
        // Margin directives start from the page's values but cannot affect
        // another margin box or the next page (CSS Page 3 §page-based-counters).
        let mut values = self.current.clone();
        let mut counters = self.document.clone();
        apply(&mut values, style, &mut counters);
        values.insert("pages".into(), self.pages as i32);
        for (name, value) in values {
            counters.obscure(&name, value);
        }
        counters
    }
}

pub(super) fn resolve(style: &mut Computed, parent: &Computed) {
    for (value, inherited) in [
        (&mut style.counter_reset, &parent.counter_reset),
        (&mut style.counter_increment, &parent.counter_increment),
        (&mut style.counter_set, &parent.counter_set),
    ] {
        if value.as_deref() == Some("inherit") {
            *value = inherited.clone();
        }
    }
}
