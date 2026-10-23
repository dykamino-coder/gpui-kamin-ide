//! Группа свойств `Computed::apply_one`: оформление текста вне реестра —
//! `text-decoration-inset`. Линии, стиль, цвет и толщина оформления — длинные
//! свойства реестра (`props::text::longhands`); наследуемые `text-shadow`,
//! `text-underline-*`, `-skip-*`, `text-emphasis-position`, `ruby-*` — в
//! `props::inherited`.

use crate::document::state::ValueCx;
use crate::style::computed::*;

impl Computed {
    pub(crate) fn apply_text_decor(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "text-decoration-inset" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "auto" {
                    self.accept_declaration();
                    self.td_inset = Some(None);
                    return;
                }
                let words = crate::style::css::value_tokens::split_top(&t);
                let lens: Vec<DecorLen> = words
                    .iter()
                    .filter_map(|w| parse_decor_inset(w, vcx))
                    .collect();
                if lens.len() != words.len() || lens.is_empty() || lens.len() > 2 {
                    return;
                }
                self.td_inset = accept_parsed!(
                    self,
                    Some(Some([lens[0], *lens.get(1).unwrap_or(&lens[0])]))
                );
            }
            _ => *hit = false,
        }
    }
}
