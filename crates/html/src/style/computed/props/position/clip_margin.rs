//! Разбор коробки отсчёта и длины overflow-clip-margin.
use super::*;
impl Computed {
    pub(super) fn apply_clip_margin(&mut self, v: &str, vcx: ValueCx<'_>) {
        // `inherit` у НЕнаследуемого свойства — вычисленное значение
        // родителя целиком (css-cascade-4 §inherit). Ветка длины ниже
        // слово не понимала, и объявление пропадало
        // (`overflow-clip-margin-009`: поле 20 родителя не доезжало).
        if v.trim() == "inherit" {
            self.inherit_bits |= inh::CLIP_MARGIN;
            return;
        }
        // `<visual-box> || <length>`: коробка отсчёта и поле, в любом
        // порядке, любая часть может отсутствовать (умолчание —
        // padding-box, поле 0).
        let mut margin = None;
        let mut bx = None;
        let words = split_outside_parens(v);
        if words.is_empty() {
            return;
        }
        for w in &words {
            match w.as_str() {
                "border-box" if bx.is_none() => bx = Some(2u8),
                "padding-box" if bx.is_none() => bx = Some(1),
                "content-box" if bx.is_none() => bx = Some(0),
                t => {
                    // Отрицательная длина — ВТЯЖКА внутрь коробки
                    // (css-overflow-4 §overflow-clip-margin:
                    // «Negative values indicate insets»), а не
                    // негодное объявление.
                    if let Some(Len::Px(px)) = Len::parse(t, vcx)
                        && margin.is_none()
                    {
                        margin = Some(px);
                    } else {
                        return;
                    }
                }
            }
        }
        if margin.is_some() || bx.is_some() {
            self.clip_margin = accept_parsed!(self, Some(margin.unwrap_or(0.0)));
            self.clip_margin_box = bx;
        }
    }
}
