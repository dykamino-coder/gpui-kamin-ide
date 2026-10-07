//! Style containment isolates counter mutations and quote depth in descendants.

#[path = "quote_language.rs"]
mod quote_language;

use super::{Counters, Entry, covers};
use std::collections::HashMap;

/// Saved outer counters remain readable inside the subtree (counter() is unscoped).
pub(crate) struct Scope {
    stack: HashMap<String, Vec<Entry>>,
    quote_depth: usize,
}

impl Counters {
    /// Page and margin contexts obscure the entire document chain of this name.
    /// CSS Paged Media 3 §page-based-counters; Blink GetCounterValues:356-377.
    pub(crate) fn obscure(&mut self, name: &str, value: i32) {
        self.stack.remove(name);
        self.reset(name, value);
    }

    /// CSS Containment 2 §3.4: enter after the root's own counter directives.
    /// Blink counters_attachment_context.cc:250-255 uses the same boundary.
    pub(crate) fn enter_style_scope(&mut self) -> Scope {
        self.boundaries.push(self.path.clone());
        Scope {
            stack: self.stack.clone(),
            quote_depth: self.quote_depth,
        }
    }

    pub(crate) fn leave_style_scope(&mut self, scope: Scope) {
        self.boundaries.pop();
        self.stack = scope.stack;
        self.quote_depth = scope.quote_depth;
    }

    /// CSS Content 3 §quotes-property resolves auto using the parent's language.
    pub(crate) fn set_quote_language(&mut self, language: &str) {
        let cur = self.path.clone();
        self.quote_languages
            .retain(|(o, _)| covers(o, &cur) && o != &cur);
        self.quote_languages.push((cur, language.to_string()));
    }

    /// Узел задал `quotes`: действует на него и его потомков.
    pub fn set_quotes(&mut self, value: Option<Vec<(String, String)>>) {
        let cur = self.path.clone();
        self.quotes.retain(|(o, _)| covers(o, &cur) && o != &cur);
        self.quotes.push((cur, value));
    }

    /// Кавычка для `open-quote`/`close-quote` с учётом глубины; `own` —
    /// `quotes` самого псевдоэлемента. `auto` выбирает пары языка.
    pub fn quote(
        &mut self,
        open: bool,
        emit: bool,
        own: Option<&Option<Vec<(String, String)>>>,
    ) -> String {
        let cur = self.path.clone();
        self.quotes.retain(|(o, _)| covers(o, &cur));
        self.quote_languages.retain(|(o, _)| covers(o, &cur));
        let depth = if open {
            self.quote_depth += 1;
            self.quote_depth - 1
        } else if self.quote_depth > 0 {
            self.quote_depth -= 1;
            self.quote_depth
        } else {
            // Лишняя закрывающая ничего не печатает и глубину не трогает.
            return String::new();
        };
        if !emit {
            return String::new();
        }
        let pick = |list: &[(String, String)]| -> String {
            list.get(depth.min(list.len().saturating_sub(1)))
                .map(|(o, c)| if open { o.clone() } else { c.clone() })
                .unwrap_or_default()
        };
        match own.or(self.quotes.last().map(|(_, v)| v)) {
            Some(Some(list)) if !list.is_empty() => pick(list),
            Some(None) => String::new(),
            None | Some(Some(_)) => {
                let language = self
                    .quote_languages
                    .last()
                    .map_or("", |(_, lang)| lang.as_str());
                quote_language::marks(language, depth, open).to_string()
            }
        }
    }
}
