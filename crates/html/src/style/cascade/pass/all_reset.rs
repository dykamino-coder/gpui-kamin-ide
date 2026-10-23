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
            // родителя», поэтому начальное значение ставится явно — всем
            // наследуемым длинным свойствам реестра, кроме `direction`
            // (`all` его не задаёт, §3.2).
            for id in LonghandId::ALL
                .iter()
                .filter(|id| id.inherited() && **id != LonghandId::Direction)
            {
                self.apply_one(id.name(), "initial", vcx);
            }
        }
    }
}
