//! Правило повтора секций по разложенной таблице: четверть фрагментаинера, `avoid`, края
//! коробки при сросшихся рамках.

use super::*;
use gpui::IntoElement;

/// Шапка 0..20, тело, подвал 280..300; зазор `spacing`, края `edges`.
fn sections(head: f32, avoid: bool, spacing: f32, edges: (f32, f32)) -> TableSections {
    TableSections {
        head: Some((edges.0 + spacing, head, avoid)),
        foot: Some((280.0, 20.0, avoid)),
        spacing,
        edge_top: edges.0,
        edge_bottom: edges.1,
        row_ends: vec![edges.0 + spacing + head, 150.0, 300.0],
        box_top: 0.0,
        box_end: 300.0 + edges.1,
    }
}

#[test]
fn a_section_up_to_a_quarter_with_avoid_repeats() {
    let spec = |h: f32, avoid: bool| repeat_spec(&sections(h, avoid, 0.0, (0.0, 0.0)), 100.0);
    let (head, _, g) = spec(25.0, true).expect("exactly a quarter repeats");
    assert_eq!(head, Some((0.0, 25.0)));
    assert_eq!((g.head_need, g.foot_need), (100.0, 80.0));
    let (head, foot, _) = spec(25.5, true).expect("footer still repeats");
    assert_eq!((head, foot), (None, Some((280.0, 20.0))));
    assert!(
        spec(10.0, false).is_none(),
        "break-inside: auto never repeats"
    );
}

#[test]
fn separated_bands_carry_the_spacing_and_collapsed_bands_the_table_edges() {
    let (head, foot, g) = repeat_spec(&sections(20.0, true, 4.0, (0.0, 0.0)), 200.0).unwrap();
    assert_eq!(head, Some((4.0, 24.0)));
    assert_eq!(foot, Some((276.0, 24.0)));
    assert_eq!((g.head_end, g.foot_at, g.foot_end), (28.0, 276.0, 300.0));
    // Сросшиеся рамки: повторённая шапка рисует верхнюю рамку таблицы, подвал
    // — нижнюю (css-tables-3 §repeated-headers).
    let (head, foot, g) = repeat_spec(&sections(20.0, true, 0.0, (5.0, 3.0)), 200.0).unwrap();
    assert_eq!(head, Some((0.0, 25.0)));
    assert_eq!(foot, Some((280.0, 23.0)));
    assert_eq!((g.head_end, g.foot_end, g.box_end), (25.0, 303.0, 303.0));
    // Четверть меряется по секции, без краёв и зазора.
    assert_eq!(g.head_need, 80.0);
}

#[test]
fn refresh_takes_the_row_ends_or_drops_the_repeat() {
    let mut r = Repeat {
        head: Some((0.0, 0.0)),
        foot: None,
        geom: Default::default(),
        row_ends: Vec::new(),
        head_els: vec![gpui::div().into_any_element()],
        foot_els: Vec::new(),
    };
    let sec = sections(20.0, true, 0.0, (0.0, 0.0));
    assert!(r.refresh(&sec, 100.0));
    assert_eq!(r.row_ends, sec.row_ends);
    assert_eq!(r.head, Some((0.0, 20.0)));
    assert!(!r.refresh(&sections(20.0, false, 0.0, (0.0, 0.0)), 100.0));
}
