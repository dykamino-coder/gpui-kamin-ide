//! `all: initial | unset` (css-cascade-4 §3.2): сброс стиля к начальному с сохранением направления
//! письма и места каскада, начальные значения наследуемых свойств.

use super::*;
use crate::values::ValueCx;

impl Computed {
    pub(super) fn reset_for_all(&mut self, initial: bool, vcx: ValueCx<'_>) {
        let keep = (
            self.inh.writing.rtl,
            self.reset_cascade.bidi_override,
            self.reset_cascade.bidi_isolate,
            self.reset_cascade.bidi_plaintext,
            self.reset_cascade.bidi_embed,
            self.reset_cascade.bidi_inherit,
            self.reset_cascade.decl_order,
        );
        *self = vcx.doc.initial_style();
        (
            self.inh.writing.rtl,
            self.reset_cascade.bidi_override,
            self.reset_cascade.bidi_isolate,
            self.reset_cascade.bidi_plaintext,
            self.reset_cascade.bidi_embed,
            self.reset_cascade.bidi_inherit,
            self.reset_cascade.decl_order,
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
