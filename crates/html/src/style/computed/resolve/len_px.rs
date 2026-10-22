//! Перевод `Len` в точки: явная база долей и контекст шрифта — одна точка
//! для всех потребителей, один запасной контракт.
//!
//! Шрифтовые единицы (`em`/`ex`/`ch`/`ic`/`lh`) меряются по `FontCx`; доли и
//! смесь `calc(px + %)` — по `basis`; единицы окна без окна не решаются (с
//! окном — `LenCx`, `len_cx.rs`), а `auto`, размеры по содержимому и
//! `anchor()` — не длины, у них `None`.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, calc_get};

/// Контекст шрифта для единиц, зависящих от кегля и глифов.
#[derive(Clone, Copy, Debug)]
pub struct FontCx<'a> {
    pub family: &'a str,
    pub font_px: f32,
    /// Использованная высота строки для `lh`; `None` — запасные 1.2 кегля.
    pub line_px: Option<f32>,
}

impl FontCx<'static> {
    /// ЕДИНСТВЕННЫЙ запасной контракт: пустое семейство, кегль 16px, `lh` =
    /// 1.2·кегль. Берут его только узлы вне наследования (элементы форм,
    /// корень) и места, где стиль недоступен; везде, где `Computed` под
    /// рукой, — `Computed::font_cx()`.
    pub const FALLBACK: FontCx<'static> = FontCx {
        family: "",
        font_px: 16.0,
        line_px: None,
    };

    /// Запасной контекст с заданным кеглем: таблица меряет `border-spacing`
    /// своим кеглем, но без семейства.
    pub const fn with_font_px(font_px: f32) -> FontCx<'static> {
        FontCx {
            family: "",
            font_px,
            line_px: None,
        }
    }
}

impl<'a> FontCx<'a> {
    pub const fn new(family: &'a str, font_px: f32) -> Self {
        FontCx {
            family,
            font_px,
            line_px: None,
        }
    }
}

impl Computed {
    /// Контекст шрифта вычисленного стиля: кегль в точках (до `resolve_em` —
    /// запасные 16), семейство (моноширинное — по языку), высота строки,
    /// если она уже в точках или кратна кеглю.
    pub fn font_cx(&self, vcx: ValueCx<'_>) -> FontCx<'_> {
        let font_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = match self.font_family.as_deref() {
            Some(f) => f,
            None if self.monospace == Some(true) => {
                crate::text::metrics::mono_family_for(self.lang.as_deref(), vcx)
            }
            None => "",
        };
        let line_px = match self.line_height {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) | Some(Len::Em(k)) => Some(k * font_px),
            _ => None,
        };
        FontCx {
            family,
            font_px,
            line_px,
        }
    }
}

impl Len {
    /// Длина в точках без окна: `LenCx` с базой `basis` и шрифтом `font`.
    /// Единицы окна, `calc` с иной природой, `auto`, размеры по содержимому и
    /// `anchor()` — `None`.
    pub fn to_px(self, basis: Option<f32>, font: FontCx<'_>, vcx: ValueCx<'_>) -> Option<f32> {
        super::len_cx::LenCx {
            basis,
            font: Some(font),
            viewport: None,
        }
        .px(self, vcx)
    }

    pub(crate) fn needs_line_height(self, vcx: ValueCx<'_>) -> bool {
        match self {
            Len::Lh(_) | Len::LhPx(..) => true,
            Len::Calc(id) => calc_get(id, vcx).lh != 0.0,
            _ => false,
        }
    }

    /// Fold a computed line-height basis without resolving percentages.
    pub(crate) fn resolve_line_height(&mut self, line: f32, vcx: ValueCx<'_>) {
        match *self {
            Len::Lh(_) | Len::LhPx(..) => {
                *self = self.font_units_to_px(
                    FontCx {
                        line_px: Some(line),
                        ..FontCx::FALLBACK
                    },
                    vcx,
                );
            }
            Len::Calc(id) => {
                let mut sum = calc_get(id, vcx);
                if sum.lh != 0.0 {
                    sum.px += sum.lh * line;
                    sum.lh = 0.0;
                    if let Some(length) = sum.collapse_mixed(vcx) {
                        *self = length;
                    }
                }
            }
            _ => {}
        }
    }

    /// Шрифтовые единицы в точки по `font`, остальное — как было: шаг
    /// «решить em» без базы долей (прежний `fallback_len_px(...).map(Len::Px)`).
    pub fn font_units_to_px(self, font: FontCx<'_>, vcx: ValueCx<'_>) -> Len {
        match self {
            Len::Em(_)
            | Len::EmPx(..)
            | Len::Ch(_)
            | Len::Ex(_)
            | Len::Ic(_)
            | Len::Lh(_)
            | Len::LhPx(..) => self.to_px(None, font, vcx).map_or(self, Len::Px),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_needs_a_basis() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(Len::Pct(0.5).to_px(None, FontCx::FALLBACK, vcx), None);
        assert_eq!(
            Len::Pct(0.5).to_px(Some(200.0), FontCx::FALLBACK, vcx),
            Some(100.0)
        );
        assert_eq!(Len::Px(7.0).to_px(None, FontCx::FALLBACK, vcx), Some(7.0));
    }

    #[test]
    fn font_units_follow_the_context() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let font = FontCx::new("", 20.0);
        assert_eq!(Len::Em(2.0).to_px(None, font, vcx), Some(40.0));
        assert_eq!(Len::EmPx(1.0, 3.0).to_px(None, font, vcx), Some(23.0));
        assert_eq!(Len::Lh(1.0).to_px(None, font, vcx), Some(24.0));
        let with_line = FontCx {
            line_px: Some(30.0),
            ..font
        };
        assert_eq!(Len::LhPx(2.0, 1.0).to_px(None, with_line, vcx), Some(61.0));
    }

    #[test]
    fn keywords_and_viewport_units_are_not_lengths() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for l in [
            Len::Auto,
            Len::MinContent,
            Len::FitContent,
            Len::Vw(1.0),
            Len::Vh(1.0),
        ] {
            assert_eq!(l.to_px(Some(100.0), FontCx::FALLBACK, vcx), None);
        }
    }

    #[test]
    fn computed_font_context_reads_size_family_and_line() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        let c = Computed {
            font_size: Some(Len::Px(10.0)),
            font_family: Some("Ahem".into()),
            line_height: Some(Len::Pct(2.0)),
            ..Default::default()
        };
        let f = c.font_cx(vcx);
        assert_eq!((f.family, f.font_px, f.line_px), ("Ahem", 10.0, Some(20.0)));
        assert_eq!(Computed::default().font_cx(vcx).font_px, 16.0);
    }
}
