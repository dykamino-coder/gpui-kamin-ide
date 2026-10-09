//! Вычисленный стиль узла: что получилось после каскада, до применения к GPUI.
//!
//! Промежуточная структура нужна по двум причинам. Во-первых, её видно в
//! тестах без окна и рендера — а `gpui::Style` собрать в тесте нельзя.
//! Во-вторых, ровно она задаёт границу охвата: поле есть — свойство
//! поддержано, поля нет — свойство игнорируется осознанно, а не потеряно.

mod gradient_paint;
mod font_kerning;
pub(super) mod font_members;
pub(crate) mod font_family;
mod white_space;
mod font_shorthand;
pub(super) mod font_weight;
mod text_indent;
mod bidi_properties;
mod image_color;
mod radius_mask;
mod border_color;
mod radius_parse;
pub(crate) use image_color::parse as parse_image_color;
mod mask_size;
mod mask_shorthand;
pub(crate) mod orthogonal;
mod tab_size;
mod quotes;
mod counters;
mod list_style_string;
mod list_style;
mod size_range;
mod content_functions;
pub(crate) use content_functions::parse_content;
mod outline_style;
use crate::style::computed::outline_style::parse as outline_style_of;
pub(crate) use outline_style::DOUBLE as OUTLINE_DOUBLE;
pub(super) mod props;
mod resolve;
mod queries;
#[cfg(test)]
pub(crate) mod snapshot_tests;

use crate::style::values::value::Len;
mod fields;
pub use crate::style::computed::fields::*;
pub(super) mod transform;
pub use crate::style::computed::transform::*;
mod filter;
pub use crate::style::computed::filter::*;
pub(super) mod gradient;
pub(crate) use crate::style::computed::gradient::*;
pub(super) mod grid_tracks;
pub use crate::style::computed::grid_tracks::*;
pub(crate) use crate::style::computed::props::border::*;
pub(super) mod types;
pub use crate::style::computed::types::*;
pub(super) mod parse_util;
pub use crate::style::computed::parse_util::*;

/// Четыре стороны: `top right bottom left`, как в CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides {
    pub top: Option<Len>,
    pub right: Option<Len>,
    pub bottom: Option<Len>,
    pub left: Option<Len>,
}

impl Sides {
    /// Раскрытие сокращённой записи: 1 значение — все стороны, 2 — верт/гориз,
    /// 3 — верх/гориз/низ, 4 — по часовой.
    pub(crate) fn shorthand(raw: &str) -> Sides {
        // Разрез — по пробелам ВНЕ скобок: `calc(10px + 1%) 0 0 0` — четыре
        // значения, а не шесть обрывков (`calc-margin-block-1`). Смесь с долей
        // доживает индексом (`parse_mixed`) — раскладка складывает её сама.
        let v: Vec<Option<Len>> = split_outside_parens(raw)
            .iter()
            .map(|t| Len::parse_mixed(t))
            .collect();
        match v.len() {
            1 => Sides {
                top: v[0],
                right: v[0],
                bottom: v[0],
                left: v[0],
            },
            2 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[0],
                left: v[1],
            },
            3 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[1],
            },
            4 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[3],
            },
            _ => Sides::default(),
        }
    }
}

/// Четыре угла скругления.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: Option<Len>,
    pub tr: Option<Len>,
    pub br: Option<Len>,
    pub bl: Option<Len>,
}

/// Разряды `inherit_bits`: ненаследуемые свойства, у которых слово `inherit`
/// обязано скопировать вычисленное значение родителя (§6.2.1).
pub(super) mod ainh {
    pub(crate) const ALIGN_ITEMS: u8 = 1 << 0;
    pub(crate) const JUSTIFY_ITEMS: u8 = 1 << 1;
    pub(crate) const ALIGN_CONTENT: u8 = 1 << 2;
    pub(crate) const JUSTIFY_CONTENT: u8 = 1 << 3;
    pub(crate) const JUSTIFY_SELF: u8 = 1 << 4;
}

pub(crate) mod inh {
    pub(crate) const BG_REPEAT: u32 = 1 << 0;
    pub(crate) const Z_INDEX: u32 = 1 << 1;
    pub(crate) const OUTLINE_W: u32 = 1 << 2;
    pub(crate) const DISPLAY: u32 = 1 << 3;
    pub(crate) const BG_IMAGE: u32 = 1 << 4;
    pub(crate) const BG_POS: u32 = 1 << 5;
    pub(crate) const CLIP: u32 = 1 << 6;
    pub(crate) const BG_ORIGIN: u32 = 1 << 7;
    pub(crate) const BG_CLIP: u32 = 1 << 8;
    pub(crate) const BG_SIZE: u32 = 1 << 9;
    pub(crate) const TRANSFORM: u32 = 1 << 10;
    pub(crate) const TRANSFORM_ORIGIN: u32 = 1 << 11;
    pub(crate) const OUTLINE_C: u32 = 1 << 12;
    pub(crate) const OUTLINE_S: u32 = 1 << 13;
    pub(crate) const OUTLINE_O: u32 = 1 << 14;
    /// `overflow-clip-margin: inherit` — коробка отсчёта и поле родителя.
    pub(crate) const CLIP_MARGIN: u32 = 1 << 15;
    /// `column-rule-color: inherit` — скалярный цвет и список линеек родителя.
    pub(crate) const COLUMN_RULE_C: u32 = 1 << 16;
    /// `row-rule-color: inherit`.
    pub(crate) const ROW_RULE_C: u32 = 1 << 17;
}

/// Разряды `will_change` (css-will-change-1 §2.1): чего ждать от коробки,
/// которая свойство только ОБЕЩАЕТ. «If any non-initial value of a property
/// would create a stacking context on the element, specifying that property
/// in will-change must create a stacking context on the element» — и то же
/// дословно про содержащий блок для `absolute` и для `fixed`.
pub(crate) mod wc {
    /// Содержащий блок для `position: absolute`.
    pub(crate) const CB_ABS: u8 = 1 << 0;
    /// Содержащий блок для `position: fixed`.
    pub(crate) const CB_FIXED: u8 = 1 << 1;
    /// Контекст наложения.
    pub(crate) const STACK: u8 = 1 << 2;
    /// `z-index`: контекст только там, где `z-index` действует
    /// (позиционированная коробка, элемент flex/grid) — решает
    /// `inline::inherit`, где известен вид родителя.
    pub(crate) const STACK_Z: u8 = 1 << 3;
    /// Обещано свойство семьи `transform` или `contain`: к строчной
    /// НЕатомарной коробке они не применяются (css-transforms-1
    /// «transformable element»), поэтому три разряда выше ставит `dom::walk`,
    /// когда вид коробки уже известен (`will-change-transform-inline`).
    pub(crate) const BOX: u8 = 1 << 4;
}


#[cfg(test)]
mod tests {
    #[test]
    fn var_fallback_with_nested_parens_survives_a_defined_variable() {
        // Конец записи — парная скобка, а запятая ищется на верхнем уровне:
        // иначе при ЗАДАННОЙ переменной оставалась лишняя скобка и значение
        // умирало, а при незаданной выходило случайно верно — из-за чего
        // дефект и не был виден.
        let mut vars = super::super::css::Decls::new();
        vars.insert("--c".into(), "red".into());
        let mut c = super::Computed::default();
        c.apply_decls_with_vars(
            &super::super::css::parse_decls("color: var(--c, rgba(0,0,0,.5))"),
            &vars,
        );
        assert_eq!(c.color, crate::style::values::value::Color::parse("red"));
        // Незаданная переменная берёт запасное значение ЦЕЛИКОМ.
        let mut c = super::Computed::default();
        c.apply_decls_with_vars(
            &super::super::css::parse_decls("color: var(--none, rgba(0,0,0,1))"),
            &super::super::css::Decls::new(),
        );
        assert_eq!(c.color, crate::style::values::value::Color::parse("rgba(0,0,0,1)"));
    }

    #[test]
    fn important_survives_a_later_ordinary_rule() {
        // Важность — самый старший ключ сравнения (CSS Cascade §6.1): важное
        // объявление раннего правила переживает обычное объявление позднего,
        // даже если то и специфичнее. Пока проходы шли внутри правила,
        // `!important` действовал только против соседей по своему блоку.
        let early = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p").expect("селектор тега"),
            decls: super::super::css::parse_decls("color: red !important"),
            order: 0,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let late = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p.x").expect("селектор класса"),
            decls: super::super::css::parse_decls("color: green"),
            order: 1,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let mut matched = vec![&early, &late];
        let c = super::Computed::resolve(&mut matched, &super::super::css::Decls::new());
        assert_eq!(c.color, crate::style::values::value::Color::parse("red"));
    }

    #[test]
    fn author_sheet_beats_user_agent_regardless_of_specificity() {
        // Происхождение старше специфичности (CSS Cascade §6.4.4). Пока обе
        // таблицы сравнивались только специфичностью, `* { margin: 0 }` со
        // специфичностью (0,0,0) проигрывал умолчанию `p { margin: 6px 0 }`
        // — то есть не работал ни один reset.
        let ua = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p").expect("селектор тега"),
            decls: super::super::css::parse_decls("margin-top: 6px"),
            order: 0,
            origin: 0,
            layer: vec![u32::MAX],
        };
        let author = super::super::css::Rule {
            sel: super::super::css::Selector::parse("*").expect("универсальный селектор"),
            decls: super::super::css::parse_decls("margin-top: 0"),
            order: 1,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let mut matched = vec![&ua, &author];
        let c = super::Computed::resolve(&mut matched, &super::super::css::Decls::new());
        assert_eq!(c.margin.top, Some(crate::style::values::value::Len::Px(0.0)));
    }

    #[test]
    fn font_shorthand_takes_size_with_line_height_and_family() {
        // Пробелы вокруг косой черты допустимы, и семейство начинается ПОСЛЕ
        // высоты строки: раньше «/ 1 Ahem» уезжало в семейство целиком, и
        // страница набиралась чужим шрифтом.
        let mut c = super::Computed::default();
        c.apply_one("font", "50px / 1 Ahem");
        assert_eq!(c.font_size, Some(crate::style::values::value::Len::Px(50.0)));
        assert_eq!(c.line_height, Some(crate::style::values::value::Len::Pct(1.0)));
        assert_eq!(c.font_family.as_deref(), Some("Ahem"));
    }

    use super::*;
    use crate::style::css::Decls;
    use crate::style::css::parse_decls;

    fn computed(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn flex_shorthand_zeroes_the_omitted_basis() {
        // Опущенная основа — 0%, а не ширина элемента.
        let one = computed("flex: 1");
        assert_eq!(one.flex_grow, Some(1.0));
        assert_eq!(one.flex_shrink, Some(1.0));
        assert_eq!(one.flex_basis, Some(Len::Pct(0.0)));
        let two = computed("flex: 0 1");
        assert_eq!(two.flex_grow, Some(0.0));
        assert_eq!(two.flex_shrink, Some(1.0));
        assert_eq!(two.flex_basis, Some(Len::Pct(0.0)));
        // Длина в сокращении — это основа, а рост и сжатие становятся 1.
        let len = computed("flex: 30px");
        assert_eq!(len.flex_grow, Some(1.0));
        assert_eq!(len.flex_shrink, Some(1.0));
        assert_eq!(len.flex_basis, Some(Len::Px(30.0)));
        let pair = computed("flex: 2 30px");
        assert_eq!(pair.flex_grow, Some(2.0));
        assert_eq!(pair.flex_shrink, Some(1.0));
        assert_eq!(pair.flex_basis, Some(Len::Px(30.0)));
        // Ключевые слова оставляют основу `auto`.
        assert_eq!(computed("flex: none").flex_basis, Some(Len::Auto));
        assert_eq!(computed("flex: auto").flex_basis, Some(Len::Auto));
        assert_eq!(computed("flex: initial").flex_basis, Some(Len::Auto));
    }

    #[test]
    fn shorthand_sides_expand_like_css() {
        let c = computed("padding: 4px 8px");
        assert_eq!(c.padding.top, Some(Len::Px(4.0)));
        assert_eq!(c.padding.right, Some(Len::Px(8.0)));
        assert_eq!(c.padding.bottom, Some(Len::Px(4.0)));
        assert_eq!(c.padding.left, Some(Len::Px(8.0)));

        let c = computed("margin: 1px 2px 3px 4px");
        assert_eq!(c.margin.bottom, Some(Len::Px(3.0)));
        assert_eq!(c.margin.left, Some(Len::Px(4.0)));
    }

    #[test]
    fn border_shorthand_takes_width_and_color() {
        let c = computed("border: 2px solid #ff0000");
        assert_eq!(c.border_width.top, Some(Len::Px(2.0)));
        assert_eq!(c.border_color.map(|c| c.r), Some(1.0));

        let c = computed("border-left: 3px solid teal");
        assert_eq!(c.border_width.left, Some(Len::Px(3.0)));
        assert_eq!(
            c.border_width.top, None,
            "боковая запись не трогает другие стороны"
        );
    }

    #[test]
    fn line_height_number_is_a_multiplier() {
        assert_eq!(
            computed("line-height: 1.5").line_height,
            Some(Len::Pct(1.5))
        );
        assert_eq!(
            computed("line-height: 20px").line_height,
            Some(Len::Px(20.0))
        );
    }

    #[test]
    fn gradient_direction_words_become_angles() {
        let g = computed("background: linear-gradient(to right, #000, #fff)")
            .gradient
            .unwrap();
        assert_eq!(g.angle_deg, 90.0);
        assert_eq!(g.from.r, 0.0);
        assert_eq!(g.to.r, 1.0);

        let g = computed("background: linear-gradient(45deg, red 10%, blue 90%)")
            .gradient
            .unwrap();
        assert_eq!(g.angle_deg, 45.0);
    }

    #[test]
    fn shadows_split_and_skip_inset() {
        let s = computed("box-shadow: 0 2px 8px rgba(0, 0, 0, .4), inset 0 0 2px red").shadows;
        assert_eq!(s.len(), 1, "inset-тень отбрасывается, её нечем рисовать");
        assert_eq!(s[0].y, 2.0);
        assert_eq!(s[0].blur, 8.0);
        assert!((s[0].color.a - 0.4).abs() < 0.01);
    }

    #[test]
    fn grid_tracks_are_counted_both_ways() {
        assert_eq!(
            computed("grid-template-columns: repeat(3, 1fr)").grid_cols,
            Some(3)
        );
        assert_eq!(
            computed("grid-template-columns: 1fr 1fr").grid_cols,
            Some(2)
        );
    }

    #[test]
    fn track_list_keeps_the_kind_of_each_track() {
        let t = computed("grid-template-columns: 120px auto 1fr")
            .grid_tracks
            .unwrap();
        assert_eq!(
            t,
            vec![
                TrackSize::Single(Track::Px(120.0)),
                TrackSize::Single(Track::Auto),
                TrackSize::Single(Track::Fr(1.0)),
            ]
        );
    }

    #[test]
    fn repeat_expands_into_equal_tracks() {
        let t = computed("grid-template-columns: repeat(3, 1fr)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![TrackSize::Single(Track::Fr(1.0)); 3]);
    }

    #[test]
    fn minmax_keeps_both_bounds() {
        // Обе грани доходят до раскладки: сведение к одной меняло ширину
        // колонки и расходилось с браузером.
        let t = computed("grid-template-columns: minmax(120px, 1fr)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![TrackSize::MinMax(Track::Px(120.0), Track::Fr(1.0))]);
    }

    #[test]
    fn content_sized_tracks_are_recognised() {
        let t = computed("grid-template-columns: min-content max-content")
            .grid_tracks
            .unwrap();
        assert_eq!(
            t,
            vec![
                TrackSize::Single(Track::MinContent),
                TrackSize::Single(Track::MaxContent),
            ]
        );
    }

    #[test]
    fn cascade_order_specificity_then_inline() {
        let rules = crate::style::css::parse_stylesheet(".a { color: red } div.a { color: blue }");
        let mut matched: Vec<&crate::style::css::Rule> = rules.iter().collect();
        let c = Computed::resolve(&mut matched, &parse_decls("color: green"));
        assert_eq!(c.color.map(|c| c.g), Some(0.5019608), "инлайн бьёт таблицу");

        let mut matched: Vec<&crate::style::css::Rule> = rules.iter().collect();
        let c = Computed::resolve(&mut matched, &Decls::new());
        assert_eq!(
            c.color.map(|c| c.b),
            Some(1.0),
            "выше специфичность — тот и выигрывает"
        );
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;
    use crate::style::css::parse_decls;

    fn c(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn radial_gradient_is_recognised_with_its_shape() {
        let g = c("background: radial-gradient(circle at center, #fff, #000)")
            .gradient
            .unwrap();
        assert!(g.radial && g.circle, "форма окружности обязана дойти");
        let e = c("background: radial-gradient(#fff, #000)")
            .gradient
            .unwrap();
        assert!(e.radial && !e.circle, "без ключевого слова — эллипс");
    }

    #[test]
    fn every_stop_survives_with_its_position() {
        let g = c("background: linear-gradient(180deg, #e03131 0%, #fcc419 50%, #2f9e44 100%)")
            .gradient
            .unwrap();
        assert_eq!(
            g.stops.len(),
            3,
            "средний стоп терялся — градиент был двух-цветным"
        );
        assert_eq!(g.stops[1].1, 0.5);
    }

    #[test]
    fn stops_without_positions_spread_evenly() {
        let g = c("background: linear-gradient(90deg, #000, #888, #fff)")
            .gradient
            .unwrap();
        assert_eq!(g.stops[1].1, 0.5, "равномерная раскладка, как в CSS");
    }
}

#[cfg(test)]
mod border_image_tests {
    use super::*;

    /// Сокращение несёт источник, срез и укладку разом.
    #[test]
    fn shorthand_carries_source_slice_and_repeat() {
        let mut c = Computed::default();
        c.apply_one("border-image", "url(C:/tmp/border.png) 27 round");
        let bi = c.border_image.expect("рамка-картинка разобрана");
        assert_eq!(bi.src, "C:/tmp/border.png");
        assert_eq!(bi.slice[0], BorderImageSlice::Px(27.0));
        assert_eq!(bi.repeat, (Tiling::Round, Tiling::Round));
    }

    /// Отдельные свойства дополняют ту же запись.
    #[test]
    fn longhands_add_up() {
        let mut c = Computed::default();
        c.apply_one("border-image-source", "url(C:/tmp/b.png)");
        c.apply_one("border-image-slice", "30% fill");
        c.apply_one("border-image-repeat", "round space");
        let bi = c.border_image.expect("рамка-картинка разобрана");
        assert!(bi.fill);
        assert_eq!(bi.slice[1], BorderImageSlice::Pct(0.3));
        assert_eq!(bi.repeat, (Tiling::Round, Tiling::Space));
    }
}
