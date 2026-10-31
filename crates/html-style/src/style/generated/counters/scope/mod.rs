//! Счётчики: области действия и кавычки. `contain: style` изолирует изменения
//! счётчиков и глубину кавычек в поддереве (CSS Containment 2 §3.4: граница
//! после собственных директив корня; Blink `counters_attachment_context.cc`),
//! внешние счётчики остаются читаемыми. Страничные контексты скрывают всю
//! цепочку имени (CSS Paged Media 3 §page-based-counters). Здесь же `quotes`
//! по узлам, язык для `auto` и выбор кавычки по глубине. Записи `quotes` и
//! языка — стек по глубине коробки: вход кладёт, выход из коробки снимает.

mod quote_language;

use super::{Counters, Entry};
use std::collections::HashMap;

/// Saved outer counters remain readable inside the subtree (counter() is unscoped).
pub struct Scope {
    stack: HashMap<String, Vec<Entry>>,
    quote_depth: usize,
}

impl Counters {
    /// Page and margin contexts obscure the entire document chain of this name.
    /// CSS Paged Media 3 §page-based-counters; Blink GetCounterValues:356-377.
    pub fn obscure(&mut self, name: &str, value: i64) {
        self.stack.remove(name);
        self.reset(name, value);
    }

    /// CSS Containment 2 §3.4: enter after the root's own counter directives.
    /// Blink counters_attachment_context.cc:250-255 uses the same boundary.
    pub fn enter_style_scope(&mut self) -> Scope {
        self.boundaries.push(self.path.clone());
        Scope {
            stack: self.stack.clone(),
            quote_depth: self.quote_depth,
        }
    }

    pub fn leave_style_scope(&mut self, scope: Scope) {
        self.boundaries.pop();
        self.stack = scope.stack;
        self.quote_depth = scope.quote_depth;
    }

    /// CSS Content 3 §quotes-property resolves auto using the parent's language.
    pub fn set_quote_language(&mut self, language: &str) {
        let depth = self.path.len();
        if self
            .quote_languages
            .last()
            .is_some_and(|(d, _)| *d == depth)
        {
            self.quote_languages.pop();
        }
        self.quote_languages.push((depth, language.to_string()));
    }

    /// Узел задал `quotes`: действует на него и его потомков.
    pub fn set_quotes(&mut self, value: Option<Vec<(String, String)>>) {
        let depth = self.path.len();
        if self.quotes.last().is_some_and(|(d, _)| *d == depth) {
            self.quotes.pop();
        }
        self.quotes.push((depth, value));
    }

    /// Выход из коробки на глубине `depth`: её `quotes` и язык кончаются
    /// вместе с её поддеревом (стек синхронен обходу, как в Blink).
    pub(super) fn leave_quote_scope(&mut self, depth: usize) {
        while self.quotes.last().is_some_and(|(d, _)| *d >= depth) {
            self.quotes.pop();
        }
        while self
            .quote_languages
            .last()
            .is_some_and(|(d, _)| *d >= depth)
        {
            self.quote_languages.pop();
        }
    }

    /// Кавычка для `open-quote`/`close-quote` с учётом глубины; `own` —
    /// `quotes` самого псевдоэлемента. `auto` выбирает пары языка.
    pub fn quote(
        &mut self,
        open: bool,
        emit: bool,
        own: Option<&Option<Vec<(String, String)>>>,
    ) -> String {
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
