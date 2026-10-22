//! `all: initial | unset` (css-cascade-4 §3.2): сброс стиля к начальному с сохранением направления
//! письма и места каскада, начальные значения наследуемых свойств.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn reset_for_all(&mut self, initial: bool, vcx: ValueCx<'_>) {
        let keep = (
            self.rtl,
            self.bidi_override,
            self.bidi_isolate,
            self.bidi_plaintext,
            self.bidi_embed,
            self.bidi_inherit,
            self.decl_order,
        );
        *self = Computed::default();
        (
            self.rtl,
            self.bidi_override,
            self.bidi_isolate,
            self.bidi_plaintext,
            self.bidi_embed,
            self.bidi_inherit,
            self.decl_order,
        ) = keep;
        self.apply_one("display", "inline", vcx);
        if initial {
            // Наследуемые свойства: пустое поле у нас значит «от
            // родителя», поэтому начальное значение ставится явно.
            // Наследуемые длинные свойства реестра — их начальным значением.
            for id in LonghandId::ALL.iter().filter(|id| id.inherited()) {
                self.apply_one(id.name(), "initial", vcx);
            }
            for key in [
                "color",
                "letter-spacing",
                "list-style-position",
                "list-style-type",
                "quotes",
                "text-align",
                "text-indent",
                "text-transform",
                "visibility",
                "word-spacing",
            ] {
                if let Some(start) = initial_value(key) {
                    self.apply_one(key, start, vcx);
                }
            }
        }
    }
}
