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
                reset_text_box: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetTextBox {
                        text_shadow_ink: self.reset_text_box.text_shadow_ink,
                        underline: self.reset_text_box.underline,
                        line_through: self.reset_text_box.line_through,
                        ellipsis: self.reset_text_box.ellipsis,
                        overflow_marker: self.reset_text_box.overflow_marker.clone(),
                        vertical_shift: self.reset_text_box.vertical_shift,
                        vertical_shift_pct: self.reset_text_box.vertical_shift_pct,
                        vertical_shift_px: self.reset_text_box.vertical_shift_px,
                        vertical_shift_len: self.reset_text_box.vertical_shift_len,
                        vertical_align_text: self.reset_text_box.vertical_align_text,
                        vertical_align_base: self.reset_text_box.vertical_align_base,
                        ..Default::default()
                    },
                ),
                reset_cascade: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetCascade {
                        font_weight_step: self.reset_cascade.font_weight_step,
                        bidi_override: self.reset_cascade.bidi_override,
                        bidi_isolate: self.reset_cascade.bidi_isolate,
                        bidi_embed: self.reset_cascade.bidi_embed,
                        ..Default::default()
                    },
                ),
                reset_fragmentation: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetFragmentation {
                        line_clamp: self.reset_fragmentation.line_clamp,
                        clamp_auto: self.reset_fragmentation.clamp_auto,
                        ..Default::default()
                    },
                ),
                reset_svg_math: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetSvgMath {
                        svg_fill: self.reset_svg_math.svg_fill.clone(),
                        svg_stroke: self.reset_svg_math.svg_stroke.clone(),
                        svg_stroke_width: self.reset_svg_math.svg_stroke_width.clone(),
                        ..Default::default()
                    },
                ),
                reset_flex: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetFlex {
                        webkit_box: self.reset_flex.webkit_box,
                        ..Default::default()
                    },
                ),
                reset_positioned: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetPositioned {
                        rel_shift: self.reset_positioned.rel_shift,
                        box_shift: self.reset_positioned.box_shift,
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.ui.color = self.inh.ui.color;
            // Видимость — свойство ТЕКСТА тоже: скрытый кусок держит место, но
            // не красится, а запасная ветка «строка из слов» флаг теряла.
            with_inherited.inh.ui.hidden = self.inh.ui.hidden;
            with_inherited.inh.font.font_size = self.inh.font.font_size;
            // Гарнитура — свойство ТЕКСТА: без неё кусок в строчном ряду
            // набирался подменным системным шрифтом, и `@font-face` (в том
            // числе Ahem у стенда) не доезжал никуда, где рядом стоит
            // картинка или иной атом.
            with_inherited.inh.font.font_family = self.inh.font.font_family.clone();
            with_inherited.inh.font.font_families = self.inh.font.font_families.clone();
            with_inherited.inh.font.font_weight = self.inh.font.font_weight;
            with_inherited.inh.font.italic = self.inh.font.italic;
            with_inherited.inh.font.oblique = self.inh.font.oblique;
            with_inherited.inh.font.font_oblique_angle = self.inh.font.font_oblique_angle;
            with_inherited.inh.font.font_oblique_only = self.inh.font.font_oblique_only;
            with_inherited.inh.font.line_height = self.inh.font.line_height;
            with_inherited.inh.text_flow.text_align = self.inh.text_flow.text_align;
            with_inherited.inh.text_flow.text_align_last = self.inh.text_flow.text_align_last;
            with_inherited.inh.text_flow.break_word = self.inh.text_flow.break_word;
            with_inherited.inh.text_flow.word_break_word = self.inh.text_flow.word_break_word;
            with_inherited.inh.text_flow.balance_lines = self.inh.text_flow.balance_lines;
            with_inherited.inh.text_flow.hanging = self.inh.text_flow.hanging;
            with_inherited.inh.text_flow.nowrap = self.inh.text_flow.nowrap;
            with_inherited.inh.font.monospace = self.inh.font.monospace;
            with_inherited.inh.text_flow.letter_spacing = self.inh.text_flow.letter_spacing;
            with_inherited.inh.font.font_features = self.inh.font.font_features.clone();
            with_inherited.inh.font.font_kerning = self.inh.font.font_kerning;
            with_inherited.inh.font.font_settings = self.inh.font.font_settings.clone();
            with_inherited.inh.font.font_alternates = self.inh.font.font_alternates.clone();
            with_inherited.inh.text_flow.ruby_merge = self.inh.text_flow.ruby_merge;
            with_inherited.inh.text_flow.text_transform = self.inh.text_flow.text_transform;
            with_inherited.inh.text_flow.clamp_mark = self.inh.text_flow.clamp_mark.clone();
            with_inherited.inh.text_flow.clamp_legacy = self.inh.text_flow.clamp_legacy;
            with_inherited.inh.text_flow.webkit_box_vertical =
                self.inh.text_flow.webkit_box_vertical;
            with_inherited.inh.text_flow.text_fit = self.inh.text_flow.text_fit;
            with_inherited.inh.text_flow.hyphen_char = self.inh.text_flow.hyphen_char.clone();
            with_inherited.inh.text_flow.hyphen_limit_chars = self.inh.text_flow.hyphen_limit_chars;
            with_inherited.inh.text_flow.hyphen_limit_lines = self.inh.text_flow.hyphen_limit_lines;
            with_inherited.inh.text_flow.hyphenate = self.inh.text_flow.hyphenate;
            with_inherited.inh.text_flow.hyphens_auto = self.inh.text_flow.hyphens_auto;
            with_inherited.inh.text_flow.lang = self.inh.text_flow.lang.clone();
            // Кусок, собранный из `text_only`, бывает родителем: без базы он
            // отдал бы детям ПОДОГНАННЫЙ кегль, и подгонка накопилась бы.
            with_inherited.inh.font.font_size_adjust = self.inh.font.font_size_adjust;
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
        out.reset_background.background = mix(
            self.reset_background.background,
            hover.reset_background.background,
        );
        out.inh.ui.color = mix(self.inh.ui.color, hover.inh.ui.color);
        if self.reset_border.border_colors != hover.reset_border.border_colors {
            out.reset_border.border_colors = std::array::from_fn(|i| {
                let from = self.used_border_color(i);
                mix(Some(from), Some(hover.used_border_color(i)))
                    .unwrap_or(from)
                    .into()
            });
        }
        out.reset_effects.opacity = match (self.reset_effects.opacity, hover.reset_effects.opacity)
        {
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
        c.inh.ui.hidden = None;
        c.reset_background.background = None;
        c.reset_background.bg_image = None;
        c.reset_background.gradient = None;
        c.reset_background.gradient_raw = None;
        c.reset_border.border_colors = std::array::from_fn(|_| {
            crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
            .into()
        });
        c.reset_border.outline = None;
        c.reset_effects.shadows.clear();
        c.inh.text_ink.text_shadow = None;
        c.inh.text_ink.text_shadow_rest.clear();
        c.reset_text_box.underline = None;
        c.reset_text_box.line_through = None;
        c
    }
}
