//! Производные стили: без краски (paint_off), только текст (text_only), смешение для переходов
//! (blend).

use super::*;

impl Computed {
    /// Тот же стиль без коробки — только то, что относится к тексту.
    ///
    /// Нужен там, где абзац разбит на куски: фон, отступы, рамка и размеры
    /// принадлежат абзацу целиком, и повторять их на каждом слове нельзя —
    /// иначе у каждого слова появляется своя подложка и своё поле.
    pub fn text_only(&self) -> Computed {
        {
            let mut with_inherited = Computed {
                text_shadow_ink: self.text_shadow_ink,
                font_weight_step: self.font_weight_step,
                underline: self.underline,
                line_through: self.line_through,
                bidi_override: self.bidi_override,
                bidi_isolate: self.bidi_isolate,
                bidi_embed: self.bidi_embed,
                ellipsis: self.ellipsis,
                overflow_marker: self.overflow_marker.clone(),
                line_clamp: self.line_clamp,
                clamp_auto: self.clamp_auto,
                svg_fill: self.svg_fill.clone(),
                // `stroke` и `stroke-width` в SVG НАСЛЕДУЮТСЯ (SVG 2 §Painting),
                // как и `fill`. Геометрия (`x`, `y`) — нет, её здесь нет намеренно.
                svg_stroke: self.svg_stroke.clone(),
                svg_stroke_width: self.svg_stroke_width.clone(),
                webkit_box: self.webkit_box,
                // Сдвиг от базовой линии — свойство ТЕКСТА: без него строчный
                // кусок в общем прогоне остаётся на базовой линии.
                vertical_shift: self.vertical_shift,
                vertical_shift_pct: self.vertical_shift_pct,
                vertical_shift_px: self.vertical_shift_px,
                vertical_shift_len: self.vertical_shift_len,
                vertical_align_text: self.vertical_align_text,
                vertical_align_base: self.vertical_align_base,
                rel_shift: self.rel_shift,
                box_shift: self.box_shift,
                ..Computed::default()
            };
            with_inherited.color = self.color;
            // Видимость — свойство ТЕКСТА тоже: скрытый кусок держит место, но
            // не красится, а запасная ветка «строка из слов» флаг теряла.
            with_inherited.hidden = self.hidden;
            with_inherited.font_size = self.font_size;
            // Гарнитура — свойство ТЕКСТА: без неё кусок в строчном ряду
            // набирался подменным системным шрифтом, и `@font-face` (в том
            // числе Ahem у стенда) не доезжал никуда, где рядом стоит
            // картинка или иной атом.
            with_inherited.font_family = self.font_family.clone();
            with_inherited.font_families = self.font_families.clone();
            with_inherited.font_weight = self.font_weight;
            with_inherited.italic = self.italic;
            with_inherited.oblique = self.oblique;
            with_inherited.font_oblique_angle = self.font_oblique_angle;
            with_inherited.font_oblique_only = self.font_oblique_only;
            with_inherited.line_height = self.line_height;
            with_inherited.text_align = self.text_align;
            with_inherited.text_align_last = self.text_align_last;
            with_inherited.break_word = self.break_word;
            with_inherited.word_break_word = self.word_break_word;
            with_inherited.balance_lines = self.balance_lines;
            with_inherited.hanging = self.hanging;
            with_inherited.nowrap = self.nowrap;
            with_inherited.monospace = self.monospace;
            with_inherited.letter_spacing = self.letter_spacing;
            with_inherited.font_features = self.font_features.clone();
            with_inherited.font_kerning = self.font_kerning;
            with_inherited.font_settings = self.font_settings.clone();
            with_inherited.font_alternates = self.font_alternates.clone();
            with_inherited.ruby_merge = self.ruby_merge;
            with_inherited.text_transform = self.text_transform;
            with_inherited.clamp_mark = self.clamp_mark.clone();
            with_inherited.clamp_legacy = self.clamp_legacy;
            with_inherited.webkit_box_vertical = self.webkit_box_vertical;
            with_inherited.text_fit = self.text_fit;
            with_inherited.hyphen_char = self.hyphen_char.clone();
            with_inherited.hyphen_limit_chars = self.hyphen_limit_chars;
            with_inherited.hyphen_limit_lines = self.hyphen_limit_lines;
            with_inherited.hyphenate = self.hyphenate;
            with_inherited.hyphens_auto = self.hyphens_auto;
            with_inherited.lang = self.lang.clone();
            // Кусок, собранный из `text_only`, бывает родителем: без базы он
            // отдал бы детям ПОДОГНАННЫЙ кегль, и подгонка накопилась бы.
            with_inherited.font_size_adjust = self.font_size_adjust;
            with_inherited.font_adjust_base = self.font_adjust_base;
            with_inherited
        }
    }

    /// Смесь этого стиля с наведённым по доле перехода.
    ///
    /// Смешиваются свойства, которые в наведении и меняют: цвета, заливка,
    /// прозрачность, толщина рамки. Остальное берётся у наведённого стиля,
    /// как только доля переваливает половину — ступенькой, потому что
    /// промежуточного значения у них нет.
    pub fn blend(&self, hover: &Computed, k: f32) -> Computed {
        let k = k.clamp(0.0, 1.0);
        if k <= 0.0 {
            return self.clone();
        }
        let mut out = if k >= 0.5 {
            hover.clone()
        } else {
            self.clone()
        };
        let mix = |a: Option<Color>, b: Option<Color>| -> Option<Color> {
            match (a, b) {
                (Some(a), Some(b)) => Some(Color {
                    r: a.r + (b.r - a.r) * k,
                    g: a.g + (b.g - a.g) * k,
                    b: a.b + (b.b - a.b) * k,
                    a: a.a + (b.a - a.a) * k,
                }),
                (a, b) => b.or(a),
            }
        };
        out.background = mix(self.background, hover.background);
        out.color = mix(self.color, hover.color);
        if self.border_colors != hover.border_colors {
            out.border_colors = std::array::from_fn(|i| {
                let from = self.used_border_color(i);
                mix(Some(from), Some(hover.used_border_color(i)))
                    .unwrap_or(from)
                    .into()
            });
        }
        out.opacity = match (self.opacity, hover.opacity) {
            (Some(a), Some(b)) => Some(a + (b - a) * k),
            (a, b) => b.or(a),
        };
        out
    }
}

impl Computed {
    /// Стиль без СВОЕЙ краски: `visibility: hidden` прячет коробку, но не
    /// поддерево — потомок с `visibility: visible` обязан рисоваться (§11.2).
    /// Гасить целиком нельзя: раскладка обязана остаться прежней, поэтому
    /// снимается только краска, а размеры и рамки по толщине не трогаются.
    pub fn paint_off(&self) -> Computed {
        let mut c = self.clone();
        c.hidden = None;
        c.background = None;
        c.bg_image = None;
        c.gradient = None;
        c.gradient_raw = None;
        c.border_colors = std::array::from_fn(|_| {
            crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
            .into()
        });
        c.outline = None;
        c.shadows.clear();
        c.text_shadow = None;
        c.text_shadow_rest.clear();
        c.underline = None;
        c.line_through = None;
        c
    }
}
