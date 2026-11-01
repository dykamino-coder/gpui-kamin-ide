//! CSS-операнды группы font; значение за общим COW-указателем.
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedFont {
    pub font_size: Option<Len>,
    pub font_weight: Option<u16>,
    pub italic: Option<bool>,
    /// `font-style: oblique` отдельно от `italic`: набору наклон один
    /// (`italic` держит оба), а подбору лица это РАЗНЫЕ запросы (css-fonts-4
    /// §font-style-matching) — от выбранного лица зависит `size-adjust`
    /// (`fonts::size_adjust`, `italic-oblique-fallback`).
    pub oblique: Option<bool>,
    /// Requested oblique angle in degrees, inherited with font-style.
    pub font_oblique_angle: Option<f32>,
    /// Synthesis is allowed for oblique requests, but not italic fallback.
    pub font_oblique_only: Option<bool>,
    pub line_height: Option<Len>,
    pub monospace: Option<bool>,
    /// First available family supplies font-relative metrics.
    pub font_family: Option<String>,
    pub font_families: Option<Vec<String>>,
    /// `font-stretch` — ширина начертания в процентах от обычной.
    ///
    /// Это НЕ возможность OpenType: узкое начертание — отдельный шрифт
    /// семейства, и выбирается он при подборе.
    pub font_stretch: Option<f32>,
    /// `font-size-adjust` (css-fonts-5): метрика (0 ex-height, 1 cap-height,
    /// 2 ch-width, 3 ic-width, 4 ic-height; `u8::MAX` — `none`) и желаемая
    /// доля кегля (`NaN` — каскадный `from-font`, вычисляется числом). Наследуется.
    pub font_size_adjust: Option<(u8, f32)>,
    /// Действующий зум («effective zoom», §599): произведение по цепочке
    /// предков вместе со своим. `None` ≡ 1 — выведенный `Default`
    /// тождество, и страница без `zoom` не несёт ни множителя, ни ветки.
    /// Ставит стадия вычисленных значений; коробка, которую сделала
    /// раскладка, несёт зум своего родителя.
    pub zoom_eff: Option<f32>,
    /// Возможности шрифта (`font-feature-settings`, `font-variant`).
    pub font_features: Vec<(String, u32)>,
    /// Значение СВОЙСТВА `font-feature-settings` целиком (`normal` — пустой
    /// список). Отдельно от `font_features`: по css-fonts-4 §7.2 оно старше
    /// `font-variant-*` при ЛЮБОМ порядке объявлений и наследуется своим
    /// значением, а не пропадает, стоит ребёнку задать `font-variant`
    /// (`font-variant-04`).
    pub font_settings: Option<Vec<(String, u32)>>,
    /// Font-specific names stay unresolved until the used family is known
    /// (CSS Fonts 4 §font-variant-alternates-prop).
    pub font_alternates: Option<crate::style::computed::font_alternates::Alternates>,
    /// `font-synthesis-weight|style|small-caps: none` — подмена начертания
    /// запрещена (css-fonts-4 §6.5). Ложь = `none`, пусто = `auto`.
    pub font_synth: (Option<bool>, Option<bool>, Option<bool>),
    /// `font-kerning`: 0 `none`, 1 `normal`, 2 `auto`; inherits independently
    /// of font-variant and resolves before font-feature-settings.
    pub font_kerning: Option<u8>,
    /// `font-variant-emoji` (css-fonts-4 §6.13): 0 normal, 1 text, 2 emoji,
    /// 3 unicode; `None` — от родителя. Выбор представления эмодзи шрифтовой
    /// системой не управляется.
    pub font_variant_emoji: Option<u8>,
    /// `font-synthesis-position` (css-fonts-4 §font-synthesis-position):
    /// разрешён ли синтез надстрочных/подстрочных; `None` — от родителя.
    pub font_synth_position: Option<bool>,
}
