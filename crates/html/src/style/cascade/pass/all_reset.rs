//! `all: initial | unset` (css-cascade-4 §3.2): сброс стиля к начальному с сохранением направления письма и счётчика объявлений, начальные значения наследуемых свойств.

use super::*;

impl Computed {
    pub(super) fn reset_for_all(&mut self, initial: bool) {
        let keep = (
            self.rtl,
            self.bidi_override,
            self.bidi_isolate,
            self.bidi_plaintext,
            self.bidi_embed,
            self.bidi_inherit,
            self.decl_seq,
        );
        *self = Computed::default();
        (
            self.rtl,
            self.bidi_override,
            self.bidi_isolate,
            self.bidi_plaintext,
            self.bidi_embed,
            self.bidi_inherit,
            self.decl_seq,
        ) = keep;
        self.apply_one("display", "inline");
        if initial {
            // Наследуемые свойства: пустое поле у нас значит «от
            // родителя», поэтому начальное значение ставится явно.
            for key in [
                "color",
                "font-family",
                "font-size",
                "font-style",
                "font-variant",
                "font-weight",
                "letter-spacing",
                "line-height",
                "list-style-position",
                "list-style-type",
                "quotes",
                "text-align",
                "text-indent",
                "text-transform",
                "visibility",
                "white-space",
                "word-spacing",
            ] {
                if let Some(start) = initial_value(key) {
                    self.apply_one(key, start);
                }
            }
        }
    }
}
