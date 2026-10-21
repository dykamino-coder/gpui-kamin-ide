//! Группа свойств `Computed::apply_one`: `width`/`height`, `min-*`/`max-*`,
//! `box-sizing`; стороны (`margin-*`, `padding-*`, вставки) и логические
//! стороны и размеры — в `sides`, `aspect-ratio`/`frame-sizing` — в `aspect`.
//! Сокращения сторон сюда не доходят: они раскрыты разбором (реестр
//! `style::css::property`). Значения размеров и сторон приходят
//! проверенными (`values`, тот же разбор `ParsedSize`/`side_len`, что у
//! применения); общее слово у них решает `apply_wide`.

use crate::document::state::ValueCx;
use crate::style::computed::*;

mod aspect;
mod sides;
mod values;
pub(in crate::style::computed) use sides::side_slot;

impl Computed {
    pub(crate) fn apply_box_model(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(i) = [
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
        ]
        .iter()
        .position(|k| *k == key)
        {
            // Место физического размера в каскаде: спор с логическим
            // размером решает `resolve_logical` (css-logical-1 §4).
            self.side_order.size[i] = self.decl_order;
            self.apply_physical_size(i, v, vcx);
            return;
        }
        match key {
            "box-sizing" => {
                self.border_box = apply_parsed!(
                    self,
                    match v.to_ascii_lowercase().as_str() {
                        "border-box" => Some(true),
                        "content-box" => Some(false),
                        _ => None,
                    }
                );
            }
            _ => self.apply_box_model_sides(key, v, hit, vcx),
        }
    }
}
