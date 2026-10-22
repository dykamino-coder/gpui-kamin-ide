//! `mask-border-*` (css-masking-1 §8): части рамки-маски разбираются той же
//! грамматикой, что части `border-image` (§8.2–8.6 повторяют
//! css-backgrounds-3 §6), но с начальными значениями маски (`slice: 0`,
//! `width: auto`); `mask-border-mode` — `alpha | luminance`. Запись из одних
//! начальных значений хранится отсутствием (`None`). Рисование рамки-маски —
//! отдельная задача; здесь — вычисленное значение.

use crate::document::state::ValueCx;
use crate::style::computed::{BorderImage, Computed};
use crate::style::css::LonghandId;

/// Имя части `border-image` с той же грамматикой.
fn border_image_part(id: LonghandId) -> Option<&'static str> {
    Some(match id {
        LonghandId::MaskBorderSource => "border-image-source",
        LonghandId::MaskBorderSlice => "border-image-slice",
        LonghandId::MaskBorderWidth => "border-image-width",
        LonghandId::MaskBorderOutset => "border-image-outset",
        LonghandId::MaskBorderRepeat => "border-image-repeat",
        _ => return None,
    })
}

/// Начальная рамка-маска (css-masking-1 §8.1–8.6).
pub(crate) fn initial_mask_border(vcx: ValueCx<'_>) -> BorderImage {
    let mut s = Computed::default();
    for id in [
        LonghandId::MaskBorderSource,
        LonghandId::MaskBorderSlice,
        LonghandId::MaskBorderWidth,
        LonghandId::MaskBorderOutset,
        LonghandId::MaskBorderRepeat,
    ] {
        if let Some(part) = border_image_part(id) {
            s.set_border_image(part, id.initial(), vcx);
        }
    }
    s.border_image.unwrap_or_else(|| BorderImage {
        src: None,
        slice: [crate::style::computed::BorderImageSlice::Px(0.0); 4],
        fill: false,
        width: [crate::style::computed::BorderImageWidth::Auto; 4],
        outset: [crate::style::computed::BorderImageWidth::Px(0.0); 4],
        repeat: (
            crate::style::computed::Tiling::None,
            crate::style::computed::Tiling::None,
        ),
    })
}

impl Computed {
    /// Часть `mask-border` из значения `v`; негодное значение ничего не
    /// меняет.
    pub(crate) fn set_mask_border(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        if id == LonghandId::MaskBorderMode {
            match v.trim() {
                "luminance" | "alpha" => {
                    self.accept_declaration();
                    self.mask_border_luminance = v.trim() == "luminance";
                }
                _ => {}
            }
            return;
        }
        let Some(part) = border_image_part(id) else {
            return;
        };
        if self.mask_border.is_none() && v.trim() == id.initial() {
            self.accept_declaration();
            return;
        }
        let initial = initial_mask_border(vcx);
        let mut scratch = Computed {
            border_image: Some(self.mask_border.clone().unwrap_or_else(|| initial.clone())),
            declaration_parse: self.declaration_parse,
            ..Computed::default()
        };
        scratch.set_border_image(part, v, vcx);
        self.declaration_parse = scratch.declaration_parse;
        if let Some(image) = scratch.border_image {
            self.mask_border = (image != initial).then_some(image);
        }
    }
}
