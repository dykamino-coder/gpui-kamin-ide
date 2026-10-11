//! Тесты каскада и разбора свойств Computed: var(), !important, порядок источников, сокращения, дорожки сетки.

use super::*;
use crate::style::css::Decls;
use crate::style::css::parse_decls;

mod cascade;

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
